import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench  # noqa: E402


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


if __name__ == "__main__":
    unittest.main()
