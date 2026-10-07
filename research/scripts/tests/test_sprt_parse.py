import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import sprt_parse  # noqa: E402


def fixture(name):
    return (HERE / "fixtures" / name).read_text()


class ParseTest(unittest.TestCase):
    def test_h1(self):
        r = sprt_parse.parse(fixture("h1.txt"))
        self.assertEqual(r["verdict"], "H1")
        self.assertEqual((r["wins"], r["losses"], r["draws"]), (580, 460, 600))
        self.assertEqual(r["games"], 1640)
        self.assertAlmostEqual(r["elo"], 25.9)
        self.assertAlmostEqual(r["elo_err"], 11.2)
        self.assertAlmostEqual(r["llr"], 2.97)

    def test_h0(self):
        r = sprt_parse.parse(fixture("h0.txt"))
        self.assertEqual(r["verdict"], "H0")
        self.assertAlmostEqual(r["elo"], -34.0)

    def test_inconclusive_when_no_accept_line(self):
        r = sprt_parse.parse(fixture("inconclusive.txt"))
        self.assertEqual(r["verdict"], "inconclusive")
        self.assertEqual(r["games"], 4000)

    def test_last_progress_line_wins(self):
        text = (
            "Score of new vs old: 1 - 0 - 0  [1.0] 1\n"
            "SPRT: llr 0.10 (3.0%), lbound -2.94, ubound 2.94\n"
            "Score of new vs old: 5 - 1 - 2  [0.7] 8\n"
            "SPRT: llr 0.20 (6.0%), lbound -2.94, ubound 2.94\n"
        )
        r = sprt_parse.parse(text)
        self.assertEqual(r["games"], 8)
        self.assertAlmostEqual(r["llr"], 0.20)

    def test_non_finite_elo_becomes_none(self):
        text = (
            "Score of new vs old: 10 - 0 - 0  [1.0] 10\n"
            "Elo difference: inf +/- nan, LOS: 100 %, DrawRatio: 0 %\n"
        )
        r = sprt_parse.parse(text)
        self.assertIsNone(r["elo"])
        self.assertIsNone(r["elo_err"])

    def test_no_score_line_raises(self):
        with self.assertRaises(ValueError):
            sprt_parse.parse("cutechess-cli: error: engine failed to start\n")

    def test_candidate_must_be_listed_first(self):
        with self.assertRaises(ValueError):
            sprt_parse.parse("Score of old vs new: 1 - 2 - 3  [0.5] 6\n")


if __name__ == "__main__":
    unittest.main()
