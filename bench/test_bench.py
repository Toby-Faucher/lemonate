import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench  # noqa: E402
import elo  # noqa: E402


def pts(*vals):
    return [{"commit": f"c{i}", "short": f"c{i}", "ts": 1000 + i * 86400, "author": "a", "subject": f"s{i}", "value": v}
            for i, v in enumerate(vals)]


class Records(unittest.TestCase):
    def test_lower_is_better_tracks_running_minimum(self):
        recs = bench.records(pts(10, 12, 8, 9, 5), True)
        self.assertEqual([r["value"] for r in recs], [10, 8, 5])
        self.assertAlmostEqual(recs[1]["improvement_pct"], 20.0)
        self.assertIsNone(recs[0]["improvement_pct"])

    def test_higher_is_better(self):
        recs = bench.records(pts(100, 90, 150), False)
        self.assertEqual([r["value"] for r in recs], [100, 150])
        self.assertAlmostEqual(recs[1]["improvement_pct"], 50.0)

    def test_held_days_and_current(self):
        recs = bench.records(pts(10, 9, 5), True)
        self.assertEqual(recs[0]["held_days"], 1.0)
        self.assertTrue(recs[-1]["current"])

    def test_equal_value_is_not_a_new_record(self):
        self.assertEqual(len(bench.records(pts(5, 5, 5), True)), 1)

    def test_noise_floor_ignores_tiny_improvements(self):
        # 100 -> 99.5 (-0.5%) is noise at a 1.5% floor; 100 -> 97 (-3%) is a record.
        old = dict(bench.RECORD_MIN_PCT)
        bench.RECORD_MIN_PCT["x"] = 1.5
        try:
            recs = bench.records(pts(100, 99.5, 97), True, "x")
        finally:
            bench.RECORD_MIN_PCT.clear()
            bench.RECORD_MIN_PCT.update(old)
        self.assertEqual([r["value"] for r in recs], [100, 97])
        self.assertAlmostEqual(recs[1]["improvement_pct"], 3.0)

    def test_elo_record_needs_a_gain_beyond_the_combined_error(self):
        def e(v, err, ts):
            return {"value": v, "err": err, "ts": ts}
        # +33 with +/-40 on both is inside the combined +/-57; +72 is outside it.
        recs = bench.records([e(1644, 46, 1), e(1716, 41, 2), e(1749, 39, 3), e(1757, 45, 4)], False, "elo")
        self.assertEqual([r["value"] for r in recs], [1644, 1716])

    def test_empty(self):
        self.assertEqual(bench.records([], True), [])


class Points(unittest.TestCase):
    def row(self, i, status="ok", dup=None, ttd=10.0):
        r = {"commit": f"c{i}", "short": f"c{i}", "ts": 100 + i, "author": "a", "subject": "s", "status": status,
             "ttd_norm_ms": ttd, "ttd_ms": ttd}
        if dup:
            r["dup_of"] = dup
        return r

    def test_skips_duplicates_and_failures(self):
        rows = [self.row(0), self.row(1, dup="c0"), self.row(2, status="build-failed"), self.row(3, ttd=None)]
        self.assertEqual([p["short"] for p in bench.points(rows, "ttd_ms")], ["c0"])

    def test_sorted_by_time(self):
        rows = [self.row(2), self.row(0), self.row(1)]
        self.assertEqual([p["short"] for p in bench.points(rows, "ttd_ms")], ["c0", "c1", "c2"])


class Parsing(unittest.TestCase):
    def test_last_info_with_nodes_wins(self):
        lines = ["info depth 3 nodes 10 time 1", "info string hi", "info depth 4 seldepth 6 nodes 99 nps 5", "bestmove e2e4"]
        self.assertEqual(bench.parse_info(lines), {"depth": 4, "nodes": 99})

    def test_no_info(self):
        self.assertEqual(bench.parse_info(["bestmove e2e4"]), {"nodes": 0, "depth": 0})


class Stats(unittest.TestCase):
    def test_bootstrap_ci_brackets_true_ratio(self):
        a = [200 + (i % 5) for i in range(12)]
        b = [100 + (i % 5) for i in range(12)]
        lo, hi = bench.bootstrap_ratio(a, b, n=500)
        self.assertLess(lo, 2.0)
        self.assertGreater(hi, 1.9)
        self.assertGreater(lo, 1.0)


class EloFit(unittest.TestCase):
    def test_half_the_points_rates_at_the_anchor(self):
        rating, err, games = elo.fit_rating({2000: (40, 40, 20)})
        self.assertAlmostEqual(rating, 2000, places=1)
        self.assertEqual(games, 100)
        self.assertGreater(err, 0)

    def test_three_quarters_of_the_points_is_400_log10_3_above(self):
        rating, _, _ = elo.fit_rating({2000: (60, 10, 30)})
        self.assertAlmostEqual(rating, 2000 + 400 * 1.0 * __import__("math").log10(3), places=1)

    def test_recovers_a_known_rating_from_two_anchors(self):
        results = {}
        for anchor in (1800, 2100):
            wins = round(elo.expected(1900 - anchor) * 100000)
            results[anchor] = (wins, 100000 - wins, 0)
        self.assertAlmostEqual(elo.fit_rating(results)[0], 1900, delta=1)

    def test_more_games_shrink_the_error(self):
        small = elo.fit_rating({2000: (20, 20, 10)})[1]
        large = elo.fit_rating({2000: (200, 200, 100)})[1]
        self.assertLess(large, small)

    def test_a_clean_sweep_is_flagged_as_outside_the_anchors(self):
        self.assertTrue(elo.saturated(elo.fit_rating({1500: (0, 100, 0)})[0]))
        self.assertTrue(elo.saturated(elo.fit_rating({1500: (100, 0, 0)})[0]))
        self.assertFalse(elo.saturated(elo.fit_rating({1500: (60, 20, 20)})[0]))

    def test_no_games_gives_no_rating(self):
        self.assertIsNone(elo.fit_rating({}))

    def test_log_parsing_counts_games_from_either_colour(self):
        log = (
            "Finished game 1 (v vs sf1500): 1-0 {White wins by adjudication}\n"
            "Finished game 2 (sf1500 vs v): 0-1 {Black wins by adjudication}\n"   # v wins both
            "Finished game 3 (v vs sf1800): 1/2-1/2 {Draw by 3-fold repetition}\n"
            "Finished game 4 (sf1800 vs v): 1-0 {White wins by adjudication}\n"   # v loses
            "Finished game 5 (sf2100 vs v): 1/2-1/2 {Draw by adjudication}\n"
            "Rank Name Elo +/- Games Score Draw\nnoise\n"
        )
        self.assertEqual(elo.parse_log(log), {1500: (2, 0, 0), 1800: (0, 1, 1), 2100: (0, 0, 1)})

    def test_games_between_two_anchors_are_ignored(self):
        self.assertEqual(elo.parse_log("Finished game 1 (sf1500 vs sf1800): 1-0 {x}\n"), {})


if __name__ == "__main__":
    unittest.main()
