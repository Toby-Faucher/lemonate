import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import result  # noqa: E402

PASS_ALL = {"preflight": "pass", "build": "pass", "test": "pass", "perft": "pass", "sprt": "pass"}


class StatusTest(unittest.TestCase):
    def test_h1_is_accepted(self):
        self.assertEqual(result.derive_status(PASS_ALL, "H1", False), "accepted")

    def test_h0_is_rejected(self):
        self.assertEqual(result.derive_status(PASS_ALL, "H0", False), "rejected")

    def test_inconclusive_is_never_accepted(self):
        self.assertEqual(result.derive_status(PASS_ALL, "inconclusive", False), "inconclusive")

    def test_correctness_failure_is_rejected_even_with_h1(self):
        for stage in ("build", "test", "perft"):
            gate = dict(PASS_ALL, **{stage: "fail"})
            self.assertEqual(result.derive_status(gate, "H1", False), "rejected", stage)

    def test_preflight_failure_is_broken(self):
        gate = dict(PASS_ALL, preflight="fail")
        self.assertEqual(result.derive_status(gate, None, False), "broken")

    def test_infra_error_is_broken(self):
        self.assertEqual(result.derive_status(PASS_ALL, None, True), "broken")

    def test_missing_verdict_without_failures_is_broken(self):
        self.assertEqual(result.derive_status(PASS_ALL, None, False), "broken")


class BuildTest(unittest.TestCase):
    def test_missing_stages_are_skipped(self):
        r = result.build_result("b" * 40, "c" * 40, "abc123", {"preflight": "fail"}, reason="x")
        self.assertEqual(r["gate"]["build"], "skipped")
        self.assertEqual(r["status"], "broken")
        self.assertEqual(r["reason"], "x")
        self.assertIsNone(r["sprt"])


class CliTest(unittest.TestCase):
    def test_cli_writes_json(self):
        with tempfile.TemporaryDirectory() as d:
            sprt = Path(d) / "sprt.json"
            sprt.write_text(json.dumps({"verdict": "H1", "elo": 5.0, "elo_err": 3.0, "games": 100}))
            out = Path(d) / "result.json"
            subprocess.run(
                [
                    sys.executable, str(HERE.parent / "result.py"),
                    "--out", str(out), "--baseline", "b1", "--candidate", "c1",
                    "--config-hash", "h1", "--sprt-file", str(sprt),
                    "--gate", "preflight=pass", "--gate", "build=pass",
                    "--gate", "test=pass", "--gate", "perft=pass", "--gate", "sprt=pass",
                ],
                check=True,
            )
            data = json.loads(out.read_text())
            self.assertEqual(data["status"], "accepted")
            self.assertEqual(data["baseline_commit"], "b1")
            self.assertEqual(data["sprt"]["games"], 100)


if __name__ == "__main__":
    unittest.main()
