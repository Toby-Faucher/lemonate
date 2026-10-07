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

    def test_h1_with_build_error_is_broken(self):
        gate = dict(PASS_ALL, build="error")
        self.assertEqual(result.derive_status(gate, "H1", False), "broken")

    def test_h1_with_test_skipped_is_broken(self):
        gate = dict(PASS_ALL, test="skipped")
        self.assertEqual(result.derive_status(gate, "H1", False), "broken")

    def test_preflight_and_build_fail_is_broken(self):
        gate = dict(PASS_ALL, preflight="fail", build="fail")
        self.assertEqual(result.derive_status(gate, None, False), "broken")


class BuildTest(unittest.TestCase):
    def test_missing_stages_are_skipped(self):
        r = result.build_result("b" * 40, "c" * 40, "abc123", {"preflight": "fail"}, reason="x")
        self.assertEqual(r["gate"]["build"], "skipped")
        self.assertEqual(r["status"], "broken")
        self.assertEqual(r["reason"], "x")
        self.assertIsNone(r["sprt"])

    def test_sprt_without_verdict_is_broken(self):
        r = result.build_result("b" * 40, "c" * 40, "abc123", PASS_ALL, sprt={"elo": 5.0, "games": 100})
        self.assertEqual(r["status"], "broken")


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

    def test_cli_rejects_misspelled_stage(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "result.json"
            r = subprocess.run(
                [
                    sys.executable, str(HERE.parent / "result.py"),
                    "--out", str(out), "--baseline", "b1", "--candidate", "c1",
                    "--config-hash", "h1",
                    "--gate", "preflight=pass", "--gate", "badstage=pass",
                ],
                capture_output=True, text=True,
            )
            self.assertNotEqual(r.returncode, 0)
            self.assertNotIn("Traceback", r.stderr)
            self.assertFalse(out.exists())

    def test_cli_rejects_bad_value(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "result.json"
            r = subprocess.run(
                [
                    sys.executable, str(HERE.parent / "result.py"),
                    "--out", str(out), "--baseline", "b1", "--candidate", "c1",
                    "--config-hash", "h1",
                    "--gate", "preflight=ok",
                ],
                capture_output=True, text=True,
            )
            self.assertNotEqual(r.returncode, 0)
            self.assertNotIn("Traceback", r.stderr)
            self.assertFalse(out.exists())

    def test_cli_rejects_gate_without_equals(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "result.json"
            r = subprocess.run(
                [
                    sys.executable, str(HERE.parent / "result.py"),
                    "--out", str(out), "--baseline", "b1", "--candidate", "c1",
                    "--config-hash", "h1",
                    "--gate", "foo",
                ],
                capture_output=True, text=True,
            )
            self.assertNotEqual(r.returncode, 0)
            self.assertNotIn("Traceback", r.stderr)
            self.assertFalse(out.exists())

    def test_cli_rejects_missing_sprt_file(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "result.json"
            r = subprocess.run(
                [
                    sys.executable, str(HERE.parent / "result.py"),
                    "--out", str(out), "--baseline", "b1", "--candidate", "c1",
                    "--config-hash", "h1",
                    "--gate", "preflight=pass",
                    "--sprt-file", str(Path(d) / "nonexistent.json"),
                ],
                capture_output=True, text=True,
            )
            self.assertNotEqual(r.returncode, 0)
            self.assertNotIn("Traceback", r.stderr)
            self.assertFalse(out.exists())

    def test_cli_infra_error_with_clean_gates(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "result.json"
            subprocess.run(
                [
                    sys.executable, str(HERE.parent / "result.py"),
                    "--out", str(out), "--baseline", "b1", "--candidate", "c1",
                    "--config-hash", "h1",
                    "--gate", "preflight=pass", "--gate", "build=pass",
                    "--gate", "test=pass", "--gate", "perft=pass", "--gate", "sprt=pass",
                    "--infra-error",
                ],
                check=True,
            )
            data = json.loads(out.read_text())
            self.assertEqual(data["status"], "broken")


if __name__ == "__main__":
    unittest.main()
