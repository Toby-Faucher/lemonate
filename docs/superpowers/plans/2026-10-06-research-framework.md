# Research Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a file-based pipeline in which agents attempt engine-improvement hypotheses in isolated worktrees and an SPRT self-play gate decides, with a generated catalog of every result.

**Architecture:** `research/` holds config, backlog, one directory per experiment, and thin scripts. `attempt.sh` creates a worktree and experiment record; `gate.sh` runs preflight, local build/test/perft, then ships the candidate and a cached baseline to the Proxmox container over SSH and runs a cutechess SPRT under a lock; small Python helpers parse output, write `result.json`, and render `CATALOG.md`. A new `tests/perft.rs` is the correctness verifier.

**Tech Stack:** Rust (cargo test), bash, Python 3.11+ stdlib only (`tomllib`, `unittest`, `argparse`, `json`), `jq`, `git worktree`, cutechess-cli on the remote container.

**Spec:** `docs/superpowers/specs/2026-10-06-research-framework-design.md`

## Global Constraints

Copied from the spec. Every task's requirements include these.

- Time control `10+0.1`; opening book `UHO_Lichess_4852_v1.epd`; concurrency `10`; game cap `4000`.
- SPRT: `elo0=0`, `elo1=5`, `alpha=0.05`, `beta=0.05`. All of these live in `research/config.toml` and its hash is written into every result.
- Protected paths an attempt may not touch: `research/scripts/`, `research/config.toml`, `tests/`, `lean/`.
- `status` is one of `accepted`, `rejected`, `inconclusive`, `broken`.
- `inconclusive` (game cap hit with no H0/H1) is never treated as accepted.
- Matches run only on the remote container, one SPRT at a time, serialized by a lock file.
- The gate recommends; a human merges. Nothing is merged automatically.
- Attempt agents never run `gate.sh`.
- Out of scope: Lean track, orchestrator daemon, multi-machine matches, automatic merging, slow-TC confirmation runs.

## Deviations from the spec (decided while planning, from reading the code)

1. Attempt worktrees live in `.worktrees/<id>` (the repo already gitignores `/.worktrees`), not `../lemonate-wt/<id>`.
2. The candidate is shipped with `git archive <candidate_commit>` piped over SSH instead of rsync, so exactly the recorded commit is what gets built and measured.
3. The baseline commit is stored in `experiments/<id>/baseline_commit` at attempt time. `gate.sh`, not the agent, generates `patch.diff`.
4. `build`, `test` and `perft` failures give `rejected` (the candidate is bad). `broken` is reserved for protected-path violations and infrastructure failures (SSH, remote build, unparseable output). `result.json` gets a `reason` string.
5. Elo and error bar come from cutechess's own output; ordo is not used.
6. The cutechess command adds `proto=uci` and `-repeat` (each opening played with colours swapped) to the recipe in `mds/elo-testing.md`.
7. A new `tests/perft.rs` is added, because the repo has no `tests/` directory and no perft in the library (only a private copy in `benches/perft.rs`).

## Preconditions

- The working tree currently has uncommitted edits to `src/eval/{king_safety,mobility,mod,pawn_structure,pst}.rs`. Attempts branch from the **committed** `main`, so commit or stash those first if you want them in the baseline. Every `git add` in this plan names files explicitly and never touches them.
- For Task 9 only: a reachable container with `cutechess-cli` and the UHO book, and an SSH alias for it.

## File Structure

| File | Responsibility |
|---|---|
| `tests/perft.rs` | Correctness verifier: known perft node counts for 5 positions |
| `research/config.toml` | Fixed gate parameters (hashed into results) |
| `research/scripts/cfg.py` | Read config values and compute the config hash |
| `research/scripts/sprt_parse.py` | cutechess-cli output to the `sprt` object |
| `research/scripts/result.py` | Derive status and write `result.json` |
| `research/scripts/catalog.py`, `catalog.sh` | Render `CATALOG.md` |
| `research/scripts/lib.sh` | Shared shell helpers: paths, `cfg`, `die`, protected-path check |
| `research/scripts/attempt.sh` | Create worktree and experiment record, print agent prompt |
| `research/scripts/gate.sh` | The six-stage gate |
| `research/scripts/tests/` | Python unit tests, bash flow tests, fixtures |
| `research/agent-prompt.md` | Agent prompt template |
| `research/README.md`, `research/backlog.md` | Docs and seed hypotheses |

---

### Task 1: Perft integration test

**Files:**
- Create: `tests/perft.rs`

**Interfaces:**
- Consumes: `lemonate::Board::{from_fen, generate_legal_moves_into, do_move, undo_move}` (same calls as `benches/perft.rs`).
- Produces: integration test target `perft`, run as `cargo test --release --test perft`. `gate.sh` uses this exact command (Task 2 config key `gate.perft_cmd`).

- [ ] **Step 1: Write the test**

```rust
use lemonate::Board;

fn perft(board: &mut Board, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut buf = Vec::new();
    board.generate_legal_moves_into(&mut buf);
    if depth == 1 {
        return buf.len() as u64;
    }
    let mut nodes = 0;
    for i in 0..buf.len() {
        let mv = buf[i];
        let undo = board.do_move(mv);
        nodes += perft(board, depth - 1);
        board.undo_move(mv, undo);
    }
    nodes
}

/// `expected[i]` is the node count at depth `i + 1`.
fn check(fen: &str, expected: &[u64]) {
    let mut board = Board::from_fen(fen).expect("valid FEN");
    for (i, &want) in expected.iter().enumerate() {
        let depth = (i + 1) as u8;
        assert_eq!(perft(&mut board, depth), want, "perft({depth}) of {fen}");
    }
}

#[test]
fn startpos() {
    check(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        &[20, 400, 8_902, 197_281, 4_865_609],
    );
}

#[test]
fn kiwipete() {
    check(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        &[48, 2_039, 97_862, 4_085_603],
    );
}

#[test]
fn position3_en_passant_pins() {
    check(
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        &[14, 191, 2_812, 43_238, 674_624],
    );
}

#[test]
fn position4_promotions_and_castling() {
    check(
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        &[6, 264, 9_467, 422_333],
    );
}

#[test]
fn position5() {
    check(
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        &[44, 1_486, 62_379, 2_103_487],
    );
}
```

- [ ] **Step 2: Run it**

Run: `cargo test --release --test perft`
Expected: 5 passed. These test existing, presumably correct, behavior, so PASS is the expected first result. If any fail, stop: that is a real movegen bug. Report it and do not adjust the numbers; they are the standard published counts.

- [ ] **Step 3: Prove the test can fail**

Temporarily change `20` to `21` in `startpos`, run `cargo test --release --test perft startpos`.
Expected: FAIL with `perft(1) of rnbqkbnr/...`. Then revert `21` back to `20`.

- [ ] **Step 4: Commit**

```bash
git add tests/perft.rs
git commit -m "test: add perft integration test with known node counts"
```

---

### Task 2: Scaffold, config and `cfg.py`

**Files:**
- Create: `research/config.toml`
- Create: `research/scripts/cfg.py`
- Create: `research/scripts/tests/test_cfg.py`
- Create: `research/experiments/.gitkeep`
- Modify: `.gitignore` (append)

**Interfaces:**
- Produces: `cfg.py get <section.key>` prints a value; `cfg.py hash` prints a 12-hex-char digest. `RESEARCH_CONFIG` env var overrides the config path. Python API: `load(path=None) -> dict`, `get(cfg, dotted) -> Any`, `digest(cfg) -> str`.
- Config keys used by later tasks: `match.tc`, `match.book`, `match.concurrency`, `match.game_cap`, `sprt.elo0`, `sprt.elo1`, `sprt.alpha`, `sprt.beta`, `remote.host`, `remote.workdir`, `remote.engines_dir`, `remote.lock`, `gate.build_cmd`, `gate.test_cmd`, `gate.perft_cmd`, `repo.baseline_branch`.

- [ ] **Step 1: Write the failing test**

`research/scripts/tests/test_cfg.py`:

```python
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
```

- [ ] **Step 2: Run it to verify it fails**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_cfg.py' -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'cfg'`.

- [ ] **Step 3: Implement**

`research/scripts/cfg.py`:

```python
#!/usr/bin/env python3
"""Read research/config.toml.

  cfg.py get <section.key>   print one value
  cfg.py hash                print a short digest of the whole config

RESEARCH_CONFIG overrides the config path (used by the flow tests).
"""
import hashlib
import json
import os
import sys
import tomllib
from pathlib import Path

DEFAULT = Path(__file__).resolve().parent.parent / "config.toml"


def load(path=None):
    path = Path(path or os.environ.get("RESEARCH_CONFIG") or DEFAULT)
    with open(path, "rb") as f:
        return tomllib.load(f)


def get(cfg, dotted):
    node = cfg
    for part in dotted.split("."):
        node = node[part]
    return node


def digest(cfg):
    blob = json.dumps(cfg, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(blob).hexdigest()[:12]


def main(argv):
    if len(argv) == 3 and argv[1] == "get":
        print(get(load(), argv[2]))
    elif len(argv) == 2 and argv[1] == "hash":
        print(digest(load()))
    else:
        sys.exit("usage: cfg.py get <section.key> | hash")


if __name__ == "__main__":
    main(sys.argv)
```

`research/config.toml` (set `remote.host` to an SSH alias for your container; Task 9 covers creating it):

```toml
[repo]
baseline_branch = "main"

[match]
tc = "10+0.1"
book = "/root/UHO_Lichess_4852_v1.epd"
concurrency = 10
game_cap = 4000

[sprt]
elo0 = 0
elo1 = 5
alpha = 0.05
beta = 0.05

[remote]
host = "chess-test"
workdir = "/root/research"
engines_dir = "/root/engines"
lock = "/root/research.lock"

[gate]
build_cmd = "RUSTFLAGS='-C target-cpu=native -C target-feature=+bmi2,+popcnt' cargo build --release"
test_cmd = "cargo test --release --lib --bins"
perft_cmd = "cargo test --release --test perft"
```

Append to `.gitignore`:

```
/research/experiments/*/*.pgn
/research/experiments/*/*.log
/research/experiments/*/*.err
```

Create the empty `research/experiments/.gitkeep`.

- [ ] **Step 4: Run tests and the CLI**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_cfg.py' -v`
Expected: 4 tests OK.

Run: `python3 research/scripts/cfg.py get sprt.elo1 && python3 research/scripts/cfg.py hash`
Expected: `5`, then a 12-character hex string.

- [ ] **Step 5: Commit**

```bash
git add research/config.toml research/scripts/cfg.py research/scripts/tests/test_cfg.py research/experiments/.gitkeep .gitignore
git commit -m "feat(research): add gate config and cfg.py reader"
```

---

### Task 3: cutechess output parser

**Files:**
- Create: `research/scripts/sprt_parse.py`
- Create: `research/scripts/tests/fixtures/h1.txt`, `h0.txt`, `inconclusive.txt`
- Create: `research/scripts/tests/test_sprt_parse.py`

**Interfaces:**
- Produces: `parse(text, candidate="new") -> dict` with keys `games, wins, losses, draws, elo, elo_err, llr, verdict`. `verdict` is `"H1"`, `"H0"` or `"inconclusive"`. `elo` and `elo_err` are `None` when cutechess prints a non-finite value. Raises `ValueError` if there is no score line or the candidate is not listed first. CLI: reads stdin, prints JSON, exits 1 on error. `gate.sh` (Task 7) pipes `sprt.log` through it.
- Contract with `gate.sh`: the candidate engine is named `new` and listed first; the baseline is `old`.

- [ ] **Step 1: Write the fixtures**

`research/scripts/tests/fixtures/h1.txt`:

```
Started game 1 of 2000 (new vs old)
Finished game 1 (new vs old): 1-0 {White mates}
Score of new vs old: 580 - 460 - 600  [0.537] 1640
...      new playing White: 300 - 220 - 300  [0.549] 820
...      new playing Black: 280 - 240 - 300  [0.524] 820
...      White vs Black: 540 - 500 - 600  [0.512] 1640
Elo difference: 25.9 +/- 11.2, LOS: 100.0 %, DrawRatio: 36.6 %
SPRT: llr 2.97 (101.0%), lbound -2.94, ubound 2.94 - H1 accepted
Finished match
```

`research/scripts/tests/fixtures/h0.txt`:

```
Score of new vs old: 410 - 560 - 530  [0.451] 1500
Elo difference: -34.0 +/- 12.0, LOS: 0.0 %, DrawRatio: 35.3 %
SPRT: llr -2.96 (-100.7%), lbound -2.94, ubound 2.94 - H0 accepted
Finished match
```

`research/scripts/tests/fixtures/inconclusive.txt`:

```
Score of new vs old: 900 - 880 - 2220  [0.505] 4000
Elo difference: 1.7 +/- 6.5, LOS: 69.3 %, DrawRatio: 55.5 %
SPRT: llr 0.31 (10.5%), lbound -2.94, ubound 2.94
Finished match
```

(Note the H1 fixture is internally consistent: 580 + 460 + 600 = 1640.)

- [ ] **Step 2: Write the failing test**

`research/scripts/tests/test_sprt_parse.py`:

```python
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
```

- [ ] **Step 3: Run it to verify it fails**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_sprt_parse.py' -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'sprt_parse'`.

- [ ] **Step 4: Implement**

`research/scripts/sprt_parse.py`:

```python
#!/usr/bin/env python3
"""Parse cutechess-cli output on stdin into the `sprt` object of result.json."""
import json
import math
import re
import sys

SCORE = re.compile(r"Score of (\S+) vs (\S+): (\d+) - (\d+) - (\d+)")
ELO = re.compile(r"Elo difference: (\S+) \+/- (\S+?),")
SPRT = re.compile(r"^SPRT: llr (-?\d+(?:\.\d+)?)(?:.*? - (H[01]) accepted)?", re.M)


def _num(text):
    try:
        value = float(text)
    except ValueError:
        return None
    return value if math.isfinite(value) else None


def parse(text, candidate="new"):
    scores = SCORE.findall(text)
    if not scores:
        raise ValueError("no 'Score of' line: the match did not run")
    first, _second, wins, losses, draws = scores[-1]
    if first != candidate:
        raise ValueError(f"expected '{candidate}' listed first, got '{first}'")
    wins, losses, draws = int(wins), int(losses), int(draws)

    elo = ELO.findall(text)
    sprt = SPRT.findall(text)
    return {
        "games": wins + losses + draws,
        "wins": wins,
        "losses": losses,
        "draws": draws,
        "elo": _num(elo[-1][0]) if elo else None,
        "elo_err": _num(elo[-1][1]) if elo else None,
        "llr": float(sprt[-1][0]) if sprt else None,
        "verdict": sprt[-1][1] if sprt and sprt[-1][1] else "inconclusive",
    }


if __name__ == "__main__":
    try:
        print(json.dumps(parse(sys.stdin.read())))
    except ValueError as e:
        sys.exit(f"sprt_parse: {e}")
```

- [ ] **Step 5: Run tests**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_sprt_parse.py' -v`
Expected: 7 tests OK.

- [ ] **Step 6: Commit**

```bash
git add research/scripts/sprt_parse.py research/scripts/tests/test_sprt_parse.py research/scripts/tests/fixtures
git commit -m "feat(research): parse cutechess SPRT output"
```

---

### Task 4: Result writer

**Files:**
- Create: `research/scripts/result.py`
- Create: `research/scripts/tests/test_result.py`

**Interfaces:**
- Consumes: the `sprt` dict produced by Task 3 (read from a JSON file).
- Produces: `derive_status(gate, verdict, infra_error) -> str`, `build_result(baseline, candidate, config_hash, gate, sprt=None, reason="", infra_error=False) -> dict`. CLI: `result.py --out PATH --baseline SHA --candidate SHA --config-hash H [--gate stage=value ...] [--sprt-file F] [--reason TEXT] [--infra-error]`. Gate values: `pass`, `fail`, `error`, `skipped`. Stages: `preflight, build, test, perft, sprt`. `gate.sh` (Task 7) calls the CLI.
- `result.json` keys: `baseline_commit, candidate_commit, config_hash, gate, sprt, status, reason`.

- [ ] **Step 1: Write the failing test**

`research/scripts/tests/test_result.py`:

```python
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
```

- [ ] **Step 2: Run it to verify it fails**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_result.py' -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'result'`.

- [ ] **Step 3: Implement**

`research/scripts/result.py`:

```python
#!/usr/bin/env python3
"""Assemble experiments/<id>/result.json from gate outcomes."""
import argparse
import json
from pathlib import Path

STAGES = ("preflight", "build", "test", "perft", "sprt")
CORRECTNESS = ("build", "test", "perft")


def derive_status(gate, verdict, infra_error):
    """Preflight violations and infrastructure failures are `broken`; a candidate that
    fails build/test/perft is `rejected`; otherwise the SPRT verdict decides."""
    if infra_error or gate.get("preflight") == "fail":
        return "broken"
    if any(gate.get(stage) == "fail" for stage in CORRECTNESS):
        return "rejected"
    return {"H1": "accepted", "H0": "rejected", "inconclusive": "inconclusive"}.get(verdict, "broken")


def build_result(baseline, candidate, config_hash, gate, sprt=None, reason="", infra_error=False):
    full = {stage: gate.get(stage, "skipped") for stage in STAGES}
    verdict = sprt["verdict"] if sprt else None
    return {
        "baseline_commit": baseline,
        "candidate_commit": candidate,
        "config_hash": config_hash,
        "gate": full,
        "sprt": sprt,
        "status": derive_status(full, verdict, infra_error),
        "reason": reason,
    }


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--out", required=True)
    p.add_argument("--baseline", required=True)
    p.add_argument("--candidate", required=True)
    p.add_argument("--config-hash", required=True)
    p.add_argument("--gate", action="append", default=[], metavar="STAGE=VALUE")
    p.add_argument("--sprt-file")
    p.add_argument("--reason", default="")
    p.add_argument("--infra-error", action="store_true")
    a = p.parse_args()

    gate = dict(item.split("=", 1) for item in a.gate)
    sprt = json.loads(Path(a.sprt_file).read_text()) if a.sprt_file else None
    data = build_result(a.baseline, a.candidate, a.config_hash, gate, sprt, a.reason, a.infra_error)
    Path(a.out).write_text(json.dumps(data, indent=2) + "\n")


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run tests**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_result.py' -v`
Expected: 9 tests OK.

- [ ] **Step 5: Commit**

```bash
git add research/scripts/result.py research/scripts/tests/test_result.py
git commit -m "feat(research): derive status and write result.json"
```

---

### Task 5: Catalog generator

**Files:**
- Create: `research/scripts/catalog.py`
- Create: `research/scripts/catalog.sh`
- Create: `research/scripts/tests/test_catalog.py`

**Interfaces:**
- Consumes: `experiments/<id>/result.json` (Task 4 shape), `hypothesis.md` (first `# ` line is the one-line hypothesis), `trace.md` (first non-empty line under `## Summary`).
- Produces: `render(root: Path) -> str` and `catalog.py [--root DIR]` which writes `DIR/CATALOG.md` (default root: the `research/` directory). `catalog.sh` takes no arguments. An experiment with no `result.json` is listed as `pending`.

- [ ] **Step 1: Write the failing test**

`research/scripts/tests/test_catalog.py`:

```python
import json
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

    def test_main_writes_file(self):
        make(self.root, "0001-a", "first", "accepted", 1.0, 1.0, 10)
        catalog.main(["--root", str(self.root)])
        self.assertTrue((self.root / "CATALOG.md").read_text().startswith("# Experiment catalog"))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run it to verify it fails**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_catalog.py' -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'catalog'`.

- [ ] **Step 3: Implement**

`research/scripts/catalog.py`:

```python
#!/usr/bin/env python3
"""Generate research/CATALOG.md from experiments/*/result.json."""
import argparse
import json
from pathlib import Path

ORDER = ["accepted", "inconclusive", "rejected", "broken", "pending"]


def _title(path):
    if path.exists():
        for line in path.read_text().splitlines():
            if line.startswith("# "):
                return line[2:].strip()
    return ""


def _first_line_under(path, heading):
    if not path.exists():
        return ""
    capturing = False
    for line in path.read_text().splitlines():
        if line.startswith("## "):
            capturing = line[3:].strip().lower() == heading.lower()
        elif capturing and line.strip():
            return line.strip()
    return ""


def _elo(sprt):
    if not sprt or sprt.get("elo") is None:
        return "n/a"
    if sprt.get("elo_err") is None:
        return f"{sprt['elo']:+.1f}"
    return f"{sprt['elo']:+.1f} ± {sprt['elo_err']:.1f}"


def _cell(text):
    return str(text).replace("|", "\\|")


def _load(exp):
    path = exp / "result.json"
    result = json.loads(path.read_text()) if path.exists() else {}
    sprt = result.get("sprt") or {}
    return {
        "id": exp.name,
        "status": result.get("status", "pending"),
        "title": _title(exp / "hypothesis.md"),
        "elo": _elo(sprt),
        "games": sprt.get("games", 0),
        "baseline": (result.get("baseline_commit") or "")[:12],
        "note": result.get("reason") or _first_line_under(exp / "trace.md", "Summary"),
    }


def render(root):
    rows = [_load(p) for p in sorted((root / "experiments").iterdir()) if p.is_dir()]
    out = [
        "# Experiment catalog",
        "",
        "Generated by `research/scripts/catalog.sh`. Do not edit.",
        "",
    ]
    for status in ORDER:
        group = [r for r in rows if r["status"] == status]
        if not group:
            continue
        out += [
            f"## {status.capitalize()} ({len(group)})",
            "",
            "| id | hypothesis | Elo | games | baseline | note |",
            "|---|---|---|---|---|---|",
        ]
        for r in group:
            cells = [r["id"], r["title"], r["elo"], r["games"], r["baseline"], r["note"]]
            out.append("| " + " | ".join(_cell(c) for c in cells) + " |")
        out.append("")
    out += ["## Proofs", "", "No Lean proofs yet.", ""]
    return "\n".join(out)


def main(argv=None):
    p = argparse.ArgumentParser()
    p.add_argument("--root", default=str(Path(__file__).resolve().parent.parent))
    root = Path(p.parse_args(argv).root)
    (root / "CATALOG.md").write_text(render(root))


if __name__ == "__main__":
    main()
```

`research/scripts/catalog.sh`:

```bash
#!/usr/bin/env bash
# catalog.sh: regenerate research/CATALOG.md.
exec python3 "$(dirname "${BASH_SOURCE[0]}")/catalog.py" "$@"
```

Run `chmod +x research/scripts/catalog.sh`.

- [ ] **Step 4: Run tests**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_catalog.py' -v`
Expected: 5 tests OK.

- [ ] **Step 5: Commit**

```bash
git add research/scripts/catalog.py research/scripts/catalog.sh research/scripts/tests/test_catalog.py
git commit -m "feat(research): generate CATALOG.md from experiment records"
```

---

### Task 6: Shell helpers, agent prompt and `attempt.sh`

**Files:**
- Create: `research/scripts/lib.sh`
- Create: `research/scripts/attempt.sh`
- Create: `research/agent-prompt.md`
- Create: `research/scripts/tests/flow_common.sh`
- Create: `research/scripts/tests/test_attempt.sh`

**Interfaces:**
- Produces from `lib.sh` (sourced, not executed): variables `RESEARCH_DIR, REPO_ROOT, SCRIPTS, EXP_DIR, WORKTREES`; functions `die MSG`, `cfg KEY`, `touched_protected BASE HEAD` (prints the changed paths that fall under a protected prefix, one per line, empty if none). `PROTECTED_PATHS=(research/scripts/ research/config.toml tests/ lean/)`.
- `attempt.sh NNNN-slug`: creates branch `exp/<id>`, worktree `.worktrees/<id>` at `git rev-parse <repo.baseline_branch>`, `experiments/<id>/baseline_commit`, `experiments/<id>/hypothesis.md`; prints the filled agent prompt on stdout. Exits non-zero on a bad id or an existing experiment.
- `flow_common.sh` (sourced by both flow tests): creates a throwaway repo at `$repo` with `$R=$repo/research`, a copy of the scripts and config, commit `main`; exports `RESEARCH_CONFIG`; defines `fail`, `assert_eq`, `write_config PATH BUILD_CMD`, and `$tmp`.

- [ ] **Step 1: Write the failing test**

`research/scripts/tests/flow_common.sh`:

```bash
# Sourced by test_attempt.sh and test_gate.sh: builds a throwaway repo holding a copy of research/.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAL_RESEARCH="$(cd "$HERE/../.." && pwd)"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
repo="$tmp/repo"
R="$repo/research"

fail() { echo "FAIL: $*" >&2; exit 1; }
assert_eq() { [[ "$1" == "$2" ]] || fail "$3: expected '$2', got '$1'"; }

write_config() { # path build_cmd
  cat > "$1" <<EOF
[repo]
baseline_branch = "main"
[match]
tc = "10+0.1"
book = "/book.epd"
concurrency = 2
game_cap = 100
[sprt]
elo0 = 0
elo1 = 5
alpha = 0.05
beta = 0.05
[remote]
host = "fakehost"
workdir = "/work"
engines_dir = "/engines"
lock = "/lock"
[gate]
build_cmd = "$2"
test_cmd = "true"
perft_cmd = "true"
EOF
}

git init -q -b main "$repo"
git -C "$repo" config user.email t@example.com
git -C "$repo" config user.name t
mkdir -p "$repo/src" "$repo/tests" "$R/experiments"
echo 'fn main() {}' > "$repo/src/main.rs"
echo '// t' > "$repo/tests/t.rs"
touch "$R/experiments/.gitkeep"
cp -r "$REAL_RESEARCH/scripts" "$REAL_RESEARCH/agent-prompt.md" "$R/"
printf '/.worktrees\n' > "$repo/.gitignore"
write_config "$R/config.toml" "true"
git -C "$repo" add -A
git -C "$repo" commit -q -m base

export RESEARCH_CONFIG="$R/config.toml"
```

`research/scripts/tests/test_attempt.sh`:

```bash
#!/usr/bin/env bash
source "$(dirname "${BASH_SOURCE[0]}")/flow_common.sh"

base=$(git -C "$repo" rev-parse main)

out=$("$R/scripts/attempt.sh" 0001-demo)
assert_eq "$(<"$R/experiments/0001-demo/baseline_commit")" "$base" "baseline_commit"
[[ -d "$repo/.worktrees/0001-demo/src" ]] || fail "worktree missing"
assert_eq "$(git -C "$repo/.worktrees/0001-demo" branch --show-current)" "exp/0001-demo" "branch"
assert_eq "$(git -C "$repo/.worktrees/0001-demo" rev-parse HEAD)" "$base" "worktree at baseline"
grep -q '^# <one-line hypothesis>' "$R/experiments/0001-demo/hypothesis.md" || fail "hypothesis stub"
grep -qF "$repo/.worktrees/0001-demo" <<<"$out" || fail "prompt should name the worktree path"
grep -qF "$R/experiments/0001-demo/trace.md" <<<"$out" || fail "prompt should name the trace path"
grep -q '{{' <<<"$out" && fail "unfilled placeholder in prompt"

if "$R/scripts/attempt.sh" 0001-demo >/dev/null 2>&1; then fail "duplicate id should fail"; fi
if "$R/scripts/attempt.sh" bad_id >/dev/null 2>&1; then fail "bad id should fail"; fi
if "$R/scripts/attempt.sh" 12-short >/dev/null 2>&1; then fail "short number should fail"; fi

echo "test_attempt: OK"
```

Run `chmod +x research/scripts/tests/test_attempt.sh`.

- [ ] **Step 2: Run it to verify it fails**

Run: `bash research/scripts/tests/test_attempt.sh`
Expected: FAIL. `attempt.sh` does not exist yet (`No such file or directory`), and `agent-prompt.md` is missing so the `cp` in `flow_common.sh` also errors.

- [ ] **Step 3: Implement**

`research/agent-prompt.md`:

```markdown
You are attempting one engine-improvement hypothesis for the lemonate chess engine.

- Experiment: {{ID}}
- Work only in this git worktree: {{WORKTREE}} (branch `exp/{{ID}}`)
- Hypothesis file: {{EXPERIMENT}}/hypothesis.md. Fill in its three sections BEFORE editing any code.
- Read first: {{REPO}}/CLAUDE.md (engine architecture) and {{RESEARCH}}/CATALOG.md (results so far, so you do not retry rejected ideas).

Rules:

1. Edit files under `src/` only. Do not touch `tests/`, `research/` or `lean/`; changes there make the attempt `broken`.
2. While iterating, run `cargo test --release --lib --bins`. That is the only verification you run.
3. Do not run `research/scripts/gate.sh`, cutechess, or any engine-vs-engine match. The gate is run by someone else, so you cannot tune the patch against its results.
4. Commit your work on branch `exp/{{ID}}`. The gate measures the committed HEAD and refuses uncommitted changes.
5. Before finishing, write {{EXPERIMENT}}/trace.md with these sections:
   - `## Summary`: one line saying what you changed and why you expect it to help
   - `## Tried`: what you tried, in order
   - `## Dropped`: ideas you discarded and the reason
   - `## Surprises`: anything unexpected in the code or behaviour
```

`research/scripts/lib.sh`:

```bash
#!/usr/bin/env bash
# Shared helpers for attempt.sh and gate.sh. Source it; do not execute it.
set -euo pipefail

RESEARCH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(git -C "$RESEARCH_DIR" rev-parse --show-toplevel)"
SCRIPTS="$RESEARCH_DIR/scripts"
EXP_DIR="$RESEARCH_DIR/experiments"
WORKTREES="$REPO_ROOT/.worktrees"

PROTECTED_PATHS=("research/scripts/" "research/config.toml" "tests/" "lean/")

die() { echo "error: $*" >&2; exit 1; }

cfg() { python3 "$SCRIPTS/cfg.py" get "$1"; }

# touched_protected BASE HEAD: print changed paths under a protected prefix, one per line.
touched_protected() {
  local changed p
  changed=$(git -C "$REPO_ROOT" diff --name-only "$1" "$2")
  for p in "${PROTECTED_PATHS[@]}"; do
    awk -v p="$p" 'index($0, p) == 1' <<<"$changed"
  done
}
```

`research/scripts/attempt.sh`:

```bash
#!/usr/bin/env bash
# attempt.sh NNNN-slug: create a worktree and experiment record, print the agent prompt.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

id=${1:-}
[[ $id =~ ^[0-9]{4}-[a-z0-9]+(-[a-z0-9]+)*$ ]] || die "usage: attempt.sh NNNN-slug (e.g. 0001-aspiration-windows)"
exp="$EXP_DIR/$id"
wt="$WORKTREES/$id"
[[ ! -e $exp ]] || die "experiment $id already exists"

baseline=$(git -C "$REPO_ROOT" rev-parse "$(cfg repo.baseline_branch)")
mkdir -p "$exp" "$WORKTREES"
git -C "$REPO_ROOT" worktree add -q -b "exp/$id" "$wt" "$baseline"
echo "$baseline" > "$exp/baseline_commit"
cat > "$exp/hypothesis.md" <<'EOF'
# <one-line hypothesis>

## Change

## Mechanism

## Expected Elo
EOF

sed -e "s|{{ID}}|$id|g" \
    -e "s|{{WORKTREE}}|$wt|g" \
    -e "s|{{EXPERIMENT}}|$exp|g" \
    -e "s|{{REPO}}|$REPO_ROOT|g" \
    -e "s|{{RESEARCH}}|$RESEARCH_DIR|g" \
    "$RESEARCH_DIR/agent-prompt.md"
```

Run `chmod +x research/scripts/attempt.sh`.

- [ ] **Step 4: Run the flow test**

Run: `bash research/scripts/tests/test_attempt.sh`
Expected: `test_attempt: OK`.

- [ ] **Step 5: Commit**

```bash
git add research/scripts/lib.sh research/scripts/attempt.sh research/agent-prompt.md research/scripts/tests/flow_common.sh research/scripts/tests/test_attempt.sh
git commit -m "feat(research): add attempt.sh, shell helpers and agent prompt"
```

---

### Task 7: `gate.sh`

**Files:**
- Create: `research/scripts/gate.sh`
- Create: `research/scripts/tests/test_gate.sh`

**Interfaces:**
- Consumes: `lib.sh` (Task 6); `cfg.py` keys (Task 2); `sprt_parse.py` CLI (Task 3, stdin to JSON); `result.py` CLI (Task 4); `catalog.sh` (Task 5); `touched_protected`.
- Produces: `gate.sh <id>` writes `patch.diff`, `result.json`, stage logs, `sprt.log`/`sprt.json`/`games.pgn` into `experiments/<id>/`, regenerates `CATALOG.md`, prints `<status>  <reason>`. Exit code 0 whenever a result was recorded; non-zero only for usage errors (unknown experiment, uncommitted worktree).
- The `RESEARCH_SSH` env var replaces `ssh` (the flow test uses a fake). All remote interaction goes through that one command.

- [ ] **Step 1: Write the failing test**

`research/scripts/tests/test_gate.sh`:

```bash
#!/usr/bin/env bash
source "$(dirname "${BASH_SOURCE[0]}")/flow_common.sh"

# Fake ssh: ignores the host, answers by recognising the remote command.
cat > "$tmp/fakessh" <<'EOF'
#!/usr/bin/env bash
shift
cmd="$*"
case "$cmd" in
  *cutechess-cli*) cat "$FAKE_CUTECHESS_OUTPUT" ;;
  "cat "*)         echo '[Event "fake"]' ;;
  "test -x"*)      exit 1 ;;
  *"tar -x"*)      cat > /dev/null ;;
  *)               ;;
esac
EOF
chmod +x "$tmp/fakessh"
export RESEARCH_SSH="$tmp/fakessh"

status_of() { jq -r .status "$R/experiments/$1/result.json"; }

run_case() { # id fixture
  "$R/scripts/attempt.sh" "$1" >/dev/null
  FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/$2" "$R/scripts/gate.sh" "$1" >/dev/null
}

run_case 0001-h1 h1.txt
assert_eq "$(status_of 0001-h1)" "accepted" "H1 -> accepted"
assert_eq "$(jq -r .sprt.verdict "$R/experiments/0001-h1/result.json")" "H1" "verdict recorded"
assert_eq "$(jq -r .config_hash "$R/experiments/0001-h1/result.json")" "$(python3 "$R/scripts/cfg.py" hash)" "config hash recorded"
[[ -f "$R/experiments/0001-h1/patch.diff" ]] || fail "patch.diff missing"
[[ -f "$R/experiments/0001-h1/games.pgn" ]] || fail "games.pgn missing"
grep -q '0001-h1' "$R/CATALOG.md" || fail "catalog not regenerated"

run_case 0002-h0 h0.txt
assert_eq "$(status_of 0002-h0)" "rejected" "H0 -> rejected"

run_case 0003-inc inconclusive.txt
assert_eq "$(status_of 0003-inc)" "inconclusive" "no verdict -> inconclusive"

# A candidate that edits a protected path is broken at preflight and never reaches the match.
"$R/scripts/attempt.sh" 0004-protected >/dev/null
wt="$repo/.worktrees/0004-protected"
echo '// weakened' >> "$wt/tests/t.rs"
git -C "$wt" commit -qam "weaken tests"
FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" "$R/scripts/gate.sh" 0004-protected >/dev/null
assert_eq "$(status_of 0004-protected)" "broken" "protected path -> broken"
assert_eq "$(jq -r .gate.preflight "$R/experiments/0004-protected/result.json")" "fail" "preflight failed"
assert_eq "$(jq -r .gate.sprt "$R/experiments/0004-protected/result.json")" "skipped" "sprt skipped"

# A failing local build is rejected without a match.
write_config "$tmp/config-badbuild.toml" "false"
"$R/scripts/attempt.sh" 0005-badbuild >/dev/null
RESEARCH_CONFIG="$tmp/config-badbuild.toml" FAKE_CUTECHESS_OUTPUT="$HERE/fixtures/h1.txt" \
  "$R/scripts/gate.sh" 0005-badbuild >/dev/null
assert_eq "$(status_of 0005-badbuild)" "rejected" "build failure -> rejected"
assert_eq "$(jq -r .gate.build "$R/experiments/0005-badbuild/result.json")" "fail" "build failed"

# Unparseable match output is an infrastructure failure, not a verdict.
echo "cutechess-cli: error: could not start engine" > "$tmp/garbage.txt"
"$R/scripts/attempt.sh" 0006-garbage >/dev/null
FAKE_CUTECHESS_OUTPUT="$tmp/garbage.txt" "$R/scripts/gate.sh" 0006-garbage >/dev/null
assert_eq "$(status_of 0006-garbage)" "broken" "unparseable output -> broken"

# Uncommitted work in the worktree is refused rather than silently measured.
"$R/scripts/attempt.sh" 0007-dirty >/dev/null
echo '// wip' >> "$repo/.worktrees/0007-dirty/src/main.rs"
if "$R/scripts/gate.sh" 0007-dirty >/dev/null 2>&1; then fail "dirty worktree should be refused"; fi

# Unknown experiment.
if "$R/scripts/gate.sh" 9999-none >/dev/null 2>&1; then fail "unknown id should fail"; fi

echo "test_gate: OK"
```

Run `chmod +x research/scripts/tests/test_gate.sh`.

- [ ] **Step 2: Run it to verify it fails**

Run: `bash research/scripts/tests/test_gate.sh`
Expected: FAIL. `gate.sh` does not exist (`No such file or directory`).

- [ ] **Step 3: Implement**

`research/scripts/gate.sh`:

```bash
#!/usr/bin/env bash
# gate.sh <id>: preflight, local build/test/perft, remote SPRT, record result.json.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

id=${1:-}
exp="$EXP_DIR/$id"
wt="$WORKTREES/$id"
[[ -n $id && -f $exp/baseline_commit ]] || die "usage: gate.sh <id> (no such experiment: '$id')"
[[ -d $wt ]] || die "worktree $wt not found"
[[ -z $(git -C "$wt" status --porcelain) ]] || die "worktree has uncommitted changes; commit them first"

baseline=$(<"$exp/baseline_commit")
candidate=$(git -C "$wt" rev-parse HEAD)
b12=${baseline:0:12}
c12=${candidate:0:12}

declare -A gate=([preflight]=skipped [build]=skipped [test]=skipped [perft]=skipped [sprt]=skipped)
reason=""
infra=0
sprt_file=""

# record: write patch.diff and result.json, refresh the catalog, print the outcome.
record() {
  git -C "$wt" diff "$baseline" "$candidate" > "$exp/patch.diff"
  local args=(--out "$exp/result.json" --baseline "$baseline" --candidate "$candidate"
              --config-hash "$(python3 "$SCRIPTS/cfg.py" hash)")
  local s
  for s in "${!gate[@]}"; do args+=(--gate "$s=${gate[$s]}"); done
  [[ -n $sprt_file ]] && args+=(--sprt-file "$sprt_file")
  [[ -n $reason ]] && args+=(--reason "$reason")
  (( infra )) && args+=(--infra-error)
  python3 "$SCRIPTS/result.py" "${args[@]}"
  "$SCRIPTS/catalog.sh"
  jq -r '"\(.status)  \(.reason)"' "$exp/result.json"
}

# --- 1. Preflight: an attempt must not touch the verifier. ---
bad=$(touched_protected "$baseline" "$candidate")
if [[ -n $bad ]]; then
  gate[preflight]=fail
  reason="touches protected paths: $(tr '\n' ' ' <<<"$bad")"
  record; exit 0
fi
gate[preflight]=pass

# --- 2-3. Local build, tests, perft (inside the candidate worktree). ---
run_stage() { # name command
  if (cd "$wt" && bash -c "$2") >"$exp/$1.log" 2>&1; then
    gate[$1]=pass
  else
    gate[$1]=fail
    reason="$1 failed (see $1.log)"
    record; exit 0
  fi
}
build_cmd=$(cfg gate.build_cmd)
run_stage build "$build_cmd"
run_stage test "$(cfg gate.test_cmd)"
run_stage perft "$(cfg gate.perft_cmd)"

# --- 4. Ship the committed candidate and a cached baseline to the container. ---
host=$(cfg remote.host)
work=$(cfg remote.workdir)
engines=$(cfg remote.engines_dir)
lock=$(cfg remote.lock)
ssh_() { ${RESEARCH_SSH:-ssh} "$@"; }

infra_fail() {
  gate[sprt]=error
  infra=1
  reason="$1"
  record; exit 0
}

ship_and_build() { # commit name
  local dir="$work/$2"
  git -C "$REPO_ROOT" archive "$1" \
    | ssh_ "$host" "rm -rf '$dir' && mkdir -p '$dir' && tar -x -C '$dir'" \
    || infra_fail "could not ship $2 to $host"
  ssh_ "$host" ". ~/.cargo/env; cd '$dir' && $build_cmd && mkdir -p '$engines' && cp target/release/lemonate '$engines/$2'" \
    >"$exp/remote-build-$2.log" 2>&1 \
    || infra_fail "remote build of $2 failed (see remote-build-$2.log)"
}

ssh_ "$host" "test -x '$engines/base-$b12'" || ship_and_build "$baseline" "base-$b12"
ship_and_build "$candidate" "cand-$c12"

# --- 5. SPRT on the container, one match at a time (flock). ---
pgn="$work/$id.pgn"
match="rm -f '$pgn'; flock '$lock' cutechess-cli \
  -engine name=new cmd='$engines/cand-$c12' proto=uci \
  -engine name=old cmd='$engines/base-$b12' proto=uci \
  -each tc=$(cfg match.tc) -games 2 -repeat -rounds $(( $(cfg match.game_cap) / 2 )) \
  -concurrency $(cfg match.concurrency) \
  -openings file='$(cfg match.book)' format=epd order=random \
  -resign movecount=3 score=400 -draw movenumber=40 movecount=8 score=10 \
  -sprt elo0=$(cfg sprt.elo0) elo1=$(cfg sprt.elo1) alpha=$(cfg sprt.alpha) beta=$(cfg sprt.beta) \
  -pgnout '$pgn'"
ssh_ "$host" ". ~/.cargo/env; $match" >"$exp/sprt.log" 2>"$exp/sprt.err" \
  || infra_fail "remote match failed (see sprt.err)"
python3 "$SCRIPTS/sprt_parse.py" <"$exp/sprt.log" >"$exp/sprt.json" 2>"$exp/sprt.err" \
  || infra_fail "could not parse cutechess output (see sprt.log, sprt.err)"
ssh_ "$host" "cat '$pgn'" >"$exp/games.pgn" 2>/dev/null || true

# --- 6. Record. ---
verdict=$(jq -r .verdict "$exp/sprt.json")
case $verdict in
  H1) gate[sprt]=pass ;;
  *)  gate[sprt]=fail; reason="SPRT verdict: $verdict" ;;
esac
sprt_file="$exp/sprt.json"
record
```

Run `chmod +x research/scripts/gate.sh`.

- [ ] **Step 4: Run the flow tests**

Run: `bash research/scripts/tests/test_gate.sh`
Expected: `test_gate: OK`.
If an assertion fails, read the message, which names the case. The most likely trouble spots are the `case` pattern order in the fake ssh and `local` inside `record` under `set -e`.

- [ ] **Step 5: Run the whole suite**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_*.py' && bash research/scripts/tests/test_attempt.sh && bash research/scripts/tests/test_gate.sh`
Expected: Python tests OK (25 tests), then `test_attempt: OK`, then `test_gate: OK`.

- [ ] **Step 6: Commit**

```bash
git add research/scripts/gate.sh research/scripts/tests/test_gate.sh
git commit -m "feat(research): add the six-stage SPRT gate"
```

---

### Task 8: README, backlog and spec notes

**Files:**
- Create: `research/README.md`
- Create: `research/backlog.md`
- Modify: `docs/superpowers/specs/2026-10-06-research-framework-design.md` (append a section, set status)

**Interfaces:**
- Consumes: everything above. Produces documentation only.

- [ ] **Step 1: Write `research/README.md`**

````markdown
# research/

A verified engine-improvement loop. Agents attempt hypotheses in isolated git
worktrees; a gate decides what counts; `CATALOG.md` records every result,
including the failures. Design: `docs/superpowers/specs/2026-10-06-research-framework-design.md`.

## The loop

```bash
# 1. Pick a line from backlog.md and start an attempt. This prints the agent prompt.
research/scripts/attempt.sh 0001-aspiration-windows

# 2. Give the printed prompt to an agent. It works in .worktrees/<id>, commits on
#    branch exp/<id>, and writes experiments/<id>/trace.md. Agents never run the gate.

# 3. When it finishes, run the gate yourself.
research/scripts/gate.sh 0001-aspiration-windows

# 4. Read CATALOG.md. If an experiment is `accepted`, merge exp/<id> into main yourself.
```

Run several attempts in parallel (one agent per worktree). Gates can also be started
in parallel: matches queue on the container's lock file, one SPRT at a time.

## What the gate checks

1. **preflight**: the diff must not touch `research/scripts/`, `research/config.toml`, `tests/` or `lean/`.
2. **build**: `gate.build_cmd` in the candidate worktree.
3. **test** and **perft**: `cargo test` and `tests/perft.rs`.
4. **ship**: the committed candidate and the baseline are sent to the container with `git archive` and built there; the baseline binary is cached by commit.
5. **sprt**: cutechess-cli, settings from `config.toml`, serialized by `flock`.
6. **record**: `result.json`, `patch.diff`, `games.pgn`, then `CATALOG.md` is regenerated.

## Statuses

| status | meaning |
|---|---|
| `accepted` | SPRT accepted H1: a gain of at least `elo1` Elo. Merge is up to you. |
| `rejected` | SPRT accepted H0, or build/test/perft failed. |
| `inconclusive` | Hit the game cap with no verdict. Never treated as accepted. |
| `broken` | Protected paths touched, or an infrastructure failure (see `reason`). |
| `pending` | In the catalog only: an attempt with no result yet. |

After merging an accepted change the baseline moves with `main`. Each result keeps
the commit it was measured against.

## Container prerequisites

`config.toml` `[remote]` points at an SSH alias (default `chess-test`) for the
container described in `mds/elo-testing.md`. It needs a Rust toolchain
(`~/.cargo/env`), `cutechess-cli` on `PATH`, `flock`, and the book at `match.book`.

## Tests

```bash
python3 -m unittest discover -s research/scripts/tests -p 'test_*.py'
bash research/scripts/tests/test_attempt.sh
bash research/scripts/tests/test_gate.sh
```
````

- [ ] **Step 2: Write `research/backlog.md`**

```markdown
# Backlog

One line per candidate hypothesis. Pick one, run `scripts/attempt.sh <id>`, and mark it
`[x]` once its experiment has a result. Line numbers refer to `src/search/mod.rs` at the
time of writing.

- [ ] 0001-aspiration-windows: re-enable aspiration windows (disabled near line 408 as "TEMPORARILY DISABLED ... to debug queen blunder"); the blunder may have been fixed elsewhere
- [ ] 0002-aspiration-narrow: shrink `ASPIRATION_WINDOW` from 50 to 25 (only meaningful once 0001 is settled)
- [ ] 0003-null-move-reduction: lower `NULL_MOVE_REDUCTION` from 3 to 2, or change the `depth / 6` term in the null-move reduction
- [ ] 0004-null-move-min-depth: lower `NULL_MOVE_MIN_DEPTH` from 3 to 2
- [ ] 0005-lmr-full-depth: reduce `LMR_FULL_DEPTH_MOVES` from 4 to 3 so reductions start earlier
- [ ] 0006-lmr-divisor: change the LMR table divisor (`ln(depth) * ln(move_count) / 2.0`) to 2.25
- [ ] 0007-futility-margin: change `FUTILITY_MARGIN_BASE` from 150 to 120, and separately to 200
- [ ] 0008-reverse-futility: add static-eval beta pruning (reverse futility) at shallow depth in non-PV nodes, a technique the search does not have yet
- [ ] 0009-history-gravity: replace the history update in `src/search/history.rs` with a gravity-style update that decays old values in proportion to the bonus
- [ ] 0010-check-extension: extend the search by one ply when the side to move is in check
- [ ] 0011-tt-replacement: change the transposition table replacement scheme in `src/search/transposition.rs` to age-aware replacement
- [ ] 0012-iir: internal iterative reduction: reduce depth at nodes with no hash move
- [ ] 0013-retune-mobility: re-run `src/bin/tuner` on a larger quiet-position dataset and adopt the new mobility weights
```

- [ ] **Step 3: Update the spec**

In `docs/superpowers/specs/2026-10-06-research-framework-design.md`, change `Status: design, awaiting review` to `Status: approved; implementation plan at docs/superpowers/plans/2026-10-06-research-framework.md`, and append this section at the end of the file:

```markdown
## Implementation notes

Decisions made while writing the plan, from reading the code:

- Attempt worktrees live in `.worktrees/<id>`, the directory the repo already gitignores.
- The candidate is shipped with `git archive` of the committed candidate instead of rsync,
  so exactly the recorded commit is built and measured.
- The baseline commit is stored in `experiments/<id>/baseline_commit`; `gate.sh` generates
  `patch.diff`.
- `build`, `test` and `perft` failures are `rejected`; `broken` is reserved for
  protected-path violations and infrastructure failures. `result.json` carries a `reason`.
- Elo and its error bar come from cutechess's own output; ordo is not used.
- The cutechess command adds `proto=uci` and `-repeat` to the `elo-testing.md` recipe.
- `tests/perft.rs` is new: the repo had no `tests/` directory and no perft in the library.
```

- [ ] **Step 4: Verify the docs**

Run: `python3 -m unittest discover -s research/scripts/tests -p 'test_*.py' && bash research/scripts/tests/test_attempt.sh && bash research/scripts/tests/test_gate.sh`
Expected: all OK (nothing here changes behavior, so this confirms nothing was disturbed).

- [ ] **Step 5: Commit**

```bash
git add research/README.md research/backlog.md docs/superpowers/specs/2026-10-06-research-framework-design.md
git commit -m "docs(research): add README, seed backlog and spec implementation notes"
```

---

### Task 9: Live self-tests on the container (manual, needs your container)

**Files:**
- Create: `research/experiments/0000-noop/` and `research/experiments/0000-sabotage/` (generated)
- Modify: `research/config.toml` only if the container address or book path differs

**Interfaces:**
- Consumes: the finished pipeline and a reachable container.
- Produces: the two self-test results the spec requires before any real result is trusted.

This task cannot be automated: it needs your container. Stop and ask the user for any missing prerequisite below rather than guessing.

- [ ] **Step 1: Prepare the container**

Add an SSH alias to `~/.ssh/config` (replace the address with the container's):

```
Host chess-test
    HostName 192.168.1.50
    User root
```

Verify:

Run: `ssh chess-test 'command -v cutechess-cli flock; . ~/.cargo/env && cargo --version; ls -l /root/UHO_Lichess_4852_v1.epd'`
Expected: paths for `cutechess-cli` and `flock`, a cargo version, and the book file. If `cutechess-cli` is missing, install it (the install line in `mds/elo-testing.md` needs checking, since it may not work as written). If the book path differs, edit `match.book` in `research/config.toml` and commit that change before continuing.

- [ ] **Step 2: Run the no-op self-test**

```bash
research/scripts/attempt.sh 0000-noop >/dev/null
cat > research/experiments/0000-noop/hypothesis.md <<'EOF'
# Self-test: an empty patch must not show a gain

## Change
None.

## Mechanism
The candidate is identical to the baseline.

## Expected Elo
0
EOF
research/scripts/gate.sh 0000-noop
```

Expected: the printed status is `rejected` or `inconclusive`, and `jq .sprt.elo research/experiments/0000-noop/result.json` is within a few Elo of 0. A single `accepted` has about a 5% chance by design (alpha = 0.05): re-run once with `rm -rf research/experiments/0000-noop .worktrees/0000-noop && git branch -D exp/0000-noop` and repeat Step 2. A repeated `accepted` means the gate has a bug: stop and debug it (check that the baseline and candidate binaries differ only in name, and that the engines are listed in the right order in the cutechess command).

- [ ] **Step 3: Run the sabotage self-test**

```bash
research/scripts/attempt.sh 0000-sabotage >/dev/null
cat > research/experiments/0000-sabotage/hypothesis.md <<'EOF'
# Self-test: disabling null-move pruning must lose

## Change
Set NULL_MOVE_MIN_DEPTH to 99 so null-move pruning never fires.

## Mechanism
Removing a major pruning technique makes the search shallower at equal time.

## Expected Elo
Strongly negative.
EOF
wt=.worktrees/0000-sabotage
sed -i 's/pub const NULL_MOVE_MIN_DEPTH: i32 = 3;/pub const NULL_MOVE_MIN_DEPTH: i32 = 99;/' "$wt/src/search/mod.rs"
git -C "$wt" diff --stat
git -C "$wt" commit -qam "sabotage: disable null-move pruning"
research/scripts/gate.sh 0000-sabotage
```

Expected: `git diff --stat` shows one line changed in `src/search/mod.rs`; the gate prints `rejected  SPRT verdict: H0` and `jq .sprt.elo research/experiments/0000-sabotage/result.json` is clearly negative. If it comes out `accepted`, the gate is broken: stop and debug.

- [ ] **Step 4: Commit the self-test records**

```bash
git add research/experiments/0000-noop research/experiments/0000-sabotage research/CATALOG.md
git commit -m "test(research): record gate self-tests (noop, sabotage)"
```

(The `.pgn`, `.log` and `.err` files are gitignored, so only the records are added.) The framework is now trusted to run real backlog items.

---

## Self-review

**Spec coverage**
- Layout (`README`, `CATALOG`, `config`, `backlog`, `experiments/`, three scripts): Tasks 2, 5, 6, 7, 8.
- `config.toml` contents and hashing: Task 2 (values), Task 4 (`config_hash` recorded), Task 7 test checks it.
- `result.json` schema and four statuses: Task 4; `inconclusive` never accepted: Task 4 tests.
- Gate stages 1-6 and protected paths: Task 7 (preflight, build, test/perft, ship, SPRT, record) with the lock via `flock`; perft verifier: Task 1.
- Failure handling (game cap, SSH/container failure): `inconclusive` from the parser (Task 3); `broken` via `infra_fail` (Task 7, tested with unparseable output).
- Attempts, agent prompt, protected edits, agents never run the gate: Task 6.
- Catalog grouping, accepted first, reasons from `trace.md`, proofs section: Task 5.
- Backlog seeding (10 to 15 real candidates): Task 8 (13 items).
- Self-tests `0000-noop` and `0000-sabotage`: Task 9.
- Out-of-scope items: not built; `lean/` reserved in `PROTECTED_PATHS` and the catalog has a Proofs section.
- Not covered: the optional fast-screen stage (spec marks it "disabled by default"; it is deferred, not built, and `config.toml` has no key for it). Add it only if the first batch shows it is needed.

**Placeholder scan:** no TBD or "similar to" steps. The one user-specific value is the container address in Task 9 Step 1, which only the user knows. The two `<...>` strings in `attempt.sh` and the README are template text for users to fill in, not plan gaps.

**Type consistency:** `parse()` keys (`games, wins, losses, draws, elo, elo_err, llr, verdict`) match what `result.py` reads (`verdict`) and what `catalog.py` reads (`elo, elo_err, games`). `gate` dict stages and values (`pass/fail/error/skipped`) match between `gate.sh` and `result.py`. `touched_protected` is defined in `lib.sh` (Task 6) and called in `gate.sh` (Task 7). Config keys listed in Task 2 match every `cfg` call in Task 7. Engine names `new`/`old` match between the cutechess command and the parser default.
