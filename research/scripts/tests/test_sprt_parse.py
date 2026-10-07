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

    def test_real_cutechess_1_5_1_output(self):
        # Captured from cutechess-cli 1.5.1: the accept line says "H1 was accepted" and the
        # Elo line is "inf +/- nan" (one finished game).
        r = sprt_parse.parse(fixture("real_cutechess_h1_was.txt"))
        self.assertEqual(r["verdict"], "H1")
        self.assertEqual(r["games"], 1)
        self.assertIsNone(r["elo"])
        self.assertIsNone(r["elo_err"])
        self.assertAlmostEqual(r["llr"], 3.5)

    def test_h0(self):
        r = sprt_parse.parse(fixture("h0.txt"))
        self.assertEqual(r["verdict"], "H0")
        self.assertAlmostEqual(r["elo"], -34.0)

    def test_inconclusive_when_no_accept_line(self):
        r = sprt_parse.parse(fixture("inconclusive.txt"))
        self.assertEqual(r["verdict"], "inconclusive")
        self.assertEqual(r["games"], 4000)

    def test_was_accepted_wording_h1(self):
        text = (
            "Score of new vs old: 10 - 2 - 3  [0.7] 15\n"
            "SPRT: llr 2.99 (101.0%), lbound -2.94, ubound 2.94 - H1 was accepted\n"
        )
        self.assertEqual(sprt_parse.parse(text)["verdict"], "H1")

    def test_was_accepted_wording_h0(self):
        text = (
            "Score of new vs old: 2 - 10 - 3  [0.3] 15\n"
            "SPRT: llr -2.99 (101.0%), lbound -2.94, ubound 2.94 - H0 was accepted\n"
        )
        self.assertEqual(sprt_parse.parse(text)["verdict"], "H0")

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
