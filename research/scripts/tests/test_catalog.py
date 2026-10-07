import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import catalog  # noqa: E402


def make(root, id_, title, status=None, elo=None, err=None, games=0, summary=None):
    d = root / "experiments" / id_
    d.mkdir(parents=True)
    (d / "hypothesis.md").write_text(f"# {title}\n\n## Change\n")
    if status:
        (d / "result.json").write_text(json.dumps({
            "baseline_commit": "abcdef1234567890",
            "status": status,
            "reason": "",
            "sprt": {"elo": elo, "elo_err": err, "games": games, "verdict": "x"},
        }))
    if summary:
        (d / "trace.md").write_text(f"## Summary\n\n{summary}\n\n## Tried\n")


class CatalogTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_groups_and_orders_by_status(self):
        make(self.root, "0001-a", "first", "rejected", -3.2, 4.0, 900, summary="made it worse")
        make(self.root, "0002-b", "second", "accepted", 12.5, 6.1, 1500)
        make(self.root, "0003-c", "third")
        out = catalog.render(self.root)
        self.assertLess(out.index("## Accepted"), out.index("## Rejected"))
        self.assertLess(out.index("## Rejected"), out.index("## Pending"))
        self.assertIn("| 0002-b | second | +12.5 ± 6.1 | 1500 | abcdef123456 |", out)
        self.assertIn("made it worse", out)

    def test_pending_without_result(self):
        make(self.root, "0003-c", "third")
        out = catalog.render(self.root)
        self.assertIn("## Pending (1)", out)
        self.assertIn("| 0003-c | third | n/a | 0 |", out)

    def test_pipes_in_cells_are_escaped(self):
        make(self.root, "0004-d", "a | b", "rejected", 0.0, 1.0, 10)
        self.assertIn("a \\| b", catalog.render(self.root))

    def test_empty_sections_omitted_and_proofs_section_present(self):
        make(self.root, "0001-a", "first", "accepted", 1.0, 1.0, 10)
        out = catalog.render(self.root)
        self.assertNotIn("## Rejected", out)
        self.assertIn("## Proofs", out)

    def test_malformed_result_is_rendered_broken(self):
        make(self.root, "0001-a", "first", "accepted", 1.0, 1.0, 10)
        (self.root / "experiments" / "0001-a" / "result.json").write_text("{not json")
        out = catalog.render(self.root)
        self.assertIn("## Broken (1)", out)
        self.assertIn("unreadable result.json", out)

    def test_reason_and_summary_both_shown(self):
        make(self.root, "0001-a", "first", "rejected", -1.0, 1.0, 10, summary="tried X")
        p = self.root / "experiments" / "0001-a" / "result.json"
        data = json.loads(p.read_text())
        data["reason"] = "SPRT verdict: H0"
        p.write_text(json.dumps(data))
        self.assertIn("SPRT verdict: H0 \u2014 tried X", catalog.render(self.root))

    def test_atomic_write_leaves_no_temp_file(self):
        make(self.root, "0001-a", "first", "accepted", 1.0, 1.0, 10)
        catalog.main(["--root", str(self.root)])
        self.assertEqual([p.name for p in self.root.iterdir() if p.is_file()], ["CATALOG.md"])

    def test_utf8_output_under_c_locale(self):
        make(self.root, "0001-a", "first", "accepted", 1.0, 1.0, 10)
        script = Path(__file__).resolve().parent.parent / "catalog.py"
        env = {"PATH": os.environ["PATH"], "LC_ALL": "C", "PYTHONUTF8": "0", "PYTHONCOERCECLOCALE": "0"}
        subprocess.run([sys.executable, str(script), "--root", str(self.root)], check=True, env=env)
        self.assertIn("\u00b1", (self.root / "CATALOG.md").read_bytes().decode("utf-8"))

    def test_main_writes_file(self):
        make(self.root, "0001-a", "first", "accepted", 1.0, 1.0, 10)
        catalog.main(["--root", str(self.root)])
        self.assertTrue((self.root / "CATALOG.md").read_text().startswith("# Experiment catalog"))


if __name__ == "__main__":
    unittest.main()
