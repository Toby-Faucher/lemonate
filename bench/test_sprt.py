#!/usr/bin/env python3
"""Tests for the pieces of sprt.py that decide what a match result means."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sprt

META = {"elo0": 0, "elo1": 5}
LOG = """Score of new vs old: 10 - 5 - 5  [0.625] 20
Elo difference: 88.7 +/- 90.0, LOS: 98.9 %, DrawRatio: 25.0 %
Score of new vs old: 30 - 10 - 20  [0.667] 60
Elo difference: 120.4 +/- 60.0, LOS: 99.9 %, DrawRatio: 33.3 %
SPRT: llr 2.95 (100.3%), lbound -2.94, ubound 2.94 - H1 was accepted
"""


class Parse(unittest.TestCase):
    def test_uses_last_score_and_verdict(self):
        r = sprt.parse(LOG, META)
        self.assertEqual((r["w"], r["l"], r["d"], r["games"]), (30, 10, 20, 60))
        self.assertEqual((r["elo"], r["err"]), (120.4, 60.0))
        self.assertEqual((r["llr"], r["verdict"]), (2.95, "H1"))

    def test_empty_log_has_no_games(self):
        r = sprt.parse("", META)
        self.assertEqual(r["games"], 0)
        self.assertIsNone(r["verdict"])

    def test_running_log_gives_estimate_not_verdict(self):
        r = sprt.parse(LOG.rsplit("SPRT:", 1)[0], META)
        self.assertIsNone(r["verdict"])
        self.assertIn("llr_est", r)


class Names(unittest.TestCase):
    def test_rejects_paths_and_dots(self):
        for bad in ("../x", "a/b", "", "-x", "a b", "a;rm", ".."):
            with self.assertRaises(SystemExit, msg=bad):
                sprt.check_name(bad)

    def test_accepts_normal_names(self):
        for ok in ("0029-lmp-improving-stack", "pipeline-check", "a.b_c"):
            self.assertEqual(sprt.check_name(ok), ok)


class LocalState(unittest.TestCase):
    """status_many reads a local (bawn) match from its directory: finished + rc decide the state."""

    def state(self, files):
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            for name, text in files.items():
                (d / name).write_text(text)
            old = sprt.WORK
            sprt.WORK = d
            try:
                (d / "m.json").write_text(
                    '{"name": "m", "dir": "%s", "runner": "bawn", "elo0": 0, "elo1": 5}' % t)
                return sprt.status_many(["m"])[0]
            finally:
                sprt.WORK = old

    def test_queued_running_finished(self):
        self.assertEqual(self.state({})["state"], "QUEUED")
        self.assertEqual(self.state({"started": "x"})["state"], "RUNNING")
        done = self.state({"started": "x", "finished": "finished=1 rc=0\n", "log": LOG})
        self.assertEqual((done["state"], done["verdict"]), ("FINISHED", "H1"))

    def test_crash_is_failed_not_inconclusive(self):
        r = self.state({"started": "x", "finished": "finished=1 rc=137\n"})
        self.assertEqual(r["state"], "FAILED")
        self.assertEqual(r["verdict"], "match process failed")

    def test_finished_with_no_games_is_not_a_cap_result(self):
        r = self.state({"started": "x", "finished": "finished=1 rc=0\n"})
        self.assertEqual(r["verdict"], "no games played")


if __name__ == "__main__":
    unittest.main()
