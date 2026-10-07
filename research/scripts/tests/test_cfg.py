import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import cfg  # noqa: E402

SAMPLE = '[match]\ntc = "10+0.1"\nconcurrency = 10\n\n[sprt]\nelo1 = 5\nalpha = 0.05\n'


def write(text):
    f = tempfile.NamedTemporaryFile("w", suffix=".toml", delete=False)
    f.write(text)
    f.close()
    return f.name


class CfgTest(unittest.TestCase):
    def test_get_dotted_values(self):
        c = cfg.load(write(SAMPLE))
        self.assertEqual(cfg.get(c, "match.tc"), "10+0.1")
        self.assertEqual(cfg.get(c, "match.concurrency"), 10)
        self.assertEqual(cfg.get(c, "sprt.alpha"), 0.05)

    def test_missing_key_raises(self):
        c = cfg.load(write(SAMPLE))
        with self.assertRaises(KeyError):
            cfg.get(c, "match.nope")

    def test_digest_is_stable_and_sensitive(self):
        a = cfg.digest(cfg.load(write(SAMPLE)))
        b = cfg.digest(cfg.load(write(SAMPLE)))
        c = cfg.digest(cfg.load(write(SAMPLE.replace("elo1 = 5", "elo1 = 10"))))
        self.assertEqual(a, b)
        self.assertNotEqual(a, c)
        self.assertEqual(len(a), 12)

    def test_digest_ignores_formatting(self):
        a = cfg.digest(cfg.load(write(SAMPLE)))
        b = cfg.digest(cfg.load(write("# comment\n" + SAMPLE.replace("tc =", "tc   ="))))
        self.assertEqual(a, b)


if __name__ == "__main__":
    unittest.main()
