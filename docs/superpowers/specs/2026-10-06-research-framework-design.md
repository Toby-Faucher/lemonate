# Research framework: verified engine-improvement loop

Date: 2026-10-06
Status: design, awaiting review

## Goal

Apply the shape of OpenAI's `openai/math` pipeline to lemonate: run many attempts
at many candidate problems, let an objective verifier decide what counts, keep only
results that pass a significance bar, and publish a catalog of results (including
negative ones) with per-attempt reasoning summaries.

| OpenAI pipeline | Lemonate |
|---|---|
| Thousands of open problems | `research/backlog.md`: candidate engine changes |
| Hours of model compute per attempt | One agent attempt per hypothesis, in its own git worktree |
| Lean as objective verifier | The gate: build, tests, perft, then SPRT self-play vs a pinned baseline |
| Significance filtering | Only SPRT-accepted changes are recommended for merge |
| `CONTENTS.md`, manuscripts, reasoning traces | `CATALOG.md`, `hypothesis.md` + `patch.diff`, `trace.md` |

## Decisions

- **Verifier is strength, not proofs.** An SPRT match against the baseline decides.
  Lean is a separate, later track (see "Out of scope").
- **Matches run remotely** in the existing Proxmox `chess-test` container from
  `mds/elo-testing.md` (cutechess-cli + ordo, 10 usable threads). Attempts and builds
  run on the dev machine, so agent work and matches never compete for CPU.
- **File-based pipeline with thin scripts.** No orchestrator daemon. Claude Code
  subagents run attempts in parallel worktrees. The on-disk record format is the
  interface a future orchestrator would drive.
- **The gate recommends, a human merges.** Nothing is merged automatically.

## Layout

```
research/
  README.md              how the loop works, plus the agent prompt template
  CATALOG.md             generated index; never hand-edited
  config.toml            fixed gate parameters
  backlog.md             candidate hypotheses, one line each
  experiments/
    NNNN-slug/
      hypothesis.md      written before any code
      patch.diff         the change, against the recorded baseline
      result.json        gate output
      trace.md           agent's abridged reasoning and dropped ideas
  scripts/
    attempt.sh <id>
    gate.sh <id>
    catalog.sh
```

### `config.toml`

Fixed gate settings, hashed into every result so results from different settings are
never compared:

- time control (`10+0.1`), opening book (`UHO_Lichess_4852_v1.epd`), concurrency (10)
- SPRT: `elo0=0`, `elo1=5`, `alpha=0.05`, `beta=0.05`, game cap 4000
  (`elo1=5` is a starting value to tune; `elo-testing.md` uses 10)
- container host and paths
- optional fast-screen stage (`tc=2+0.02`), disabled by default

### `result.json`

- `baseline_commit`, `candidate_commit`, `config_hash`
- `gate`: per-stage pass/fail for preflight, build, test, perft, sprt
- `sprt`: verdict (`H1` | `H0` | `inconclusive`), games, W/D/L, Elo and error bar, LLR
- `status`: `accepted` | `rejected` | `inconclusive` | `broken`

Rejected and inconclusive experiments are kept with their traces as the negative-results
record, so ideas are not retried blindly. After a human merges an accepted change the
baseline moves to the new `main`; each result keeps the commit it was measured against.

## The gate (`gate.sh <id>`)

Stages run in order and stop at the first failure.

1. **Preflight (local).** Branch exists; its diff touches none of the protected paths
   (`research/scripts/`, `research/config.toml`, `tests/`, and `lean/` once it exists).
   A violation records `broken` and stops, so an attempt cannot weaken its own verifier.
2. **Build (local).** `cargo build --release` with the flags from `CLAUDE.md`
   (`target-cpu=native`, BMI2, POPCNT). Build errors fail; warnings do not.
3. **Correctness (local).** `cargo test` plus a perft check against known node counts
   (startpos, Kiwipete) at fixed depths. This stops a speedup that breaks movegen from
   looking like a gain.
4. **Ship (remote).** rsync the candidate worktree to the container. The baseline binary
   is built from the recorded baseline commit and cached by hash.
5. **SPRT (remote).** cutechess-cli with settings from `config.toml`; PGN pulled back
   into the experiment directory. One SPRT runs at a time on the container, serialized
   by a lock file, so timing stays clean.
6. **Record.** Parse cutechess/ordo output, write `result.json`, run `catalog.sh`.

Failure handling:

- SPRT reaches the game cap without H0 or H1 -> `inconclusive`, never accepted.
- SSH or container failure -> `broken` with the reason; the gate never guesses.

## Attempts and agents

**`attempt.sh <id>`** creates a worktree at `../lemonate-wt/<id>` on branch `exp/<id>`
from the current baseline commit, creates `experiments/<id>/` with a `hypothesis.md`
stub (the change, the expected mechanism, the expected Elo), and prints the worktree path
and the agent prompt.

**Agent prompt template** (in `research/README.md`):

- read `CLAUDE.md`, the hypothesis, and `CATALOG.md` (so rejected ideas are not retried)
- edit under `src/` only; never the protected paths
- run `cargo test` while iterating; write `trace.md` before finishing
- never run `gate.sh`. Only a human or the orchestrating session runs it, so an agent
  cannot tune a patch against SPRT results.

**Batch flow.** A human picks N items from `backlog.md`. Per item: `attempt.sh`, then a
subagent in that worktree, in parallel. When an agent finishes, its diff is saved to
`patch.diff` and `gate.sh <id>` runs; gates queue on the container lock.

**`catalog.sh`** generates `CATALOG.md` from `experiments/*/result.json`: a table grouped
by status (accepted first), with id, one-line hypothesis, Elo +/- error, games, baseline
commit, and a short reason from `trace.md` for rejected items.

## Seeding the backlog

Start `backlog.md` with 10 to 15 real candidates drawn from existing code: LMR formula,
aspiration window size, null-move reduction, history gravity, SEE thresholds, futility
margins, tuner mobility weights. Real items make the first batch an end-to-end test.

## Testing the framework

- `0000-noop`: an empty patch must come out `rejected` or `inconclusive` with Elo near 0.
  A reported gain means the gate is broken.
- `0000-sabotage`: a deliberately weakened engine (for example, null-move pruning
  disabled) must come out `rejected`.

Both must behave as stated before any real result is trusted.

## Out of scope

- **Lean track (next spec).** A `lean/` directory following the oarfish pattern: a
  hand-written Lean model of the Rust, each definition naming the Rust it mirrors and
  stating departures. Candidate theorems: alpha-beta equals negamax within the window;
  move-order independence of the returned value; make/unmake round trip with Zobrist
  restoration; tapered-eval bounds. Pruning heuristics (null move, LMR, futility) are
  unsound by design and would be stated, not proved. Requires installing the Lean
  toolchain (not present on the dev machine; oarfish pins v4.34.0). This framework only
  reserves `lean/` as a protected path and leaves room in `CATALOG.md` for a proofs section.
- Orchestrator daemon / unattended batches.
- Multi-machine matches.
- Automatic merging.
- Slow-TC confirmation runs (re-run an accepted experiment manually at `60+0.6`).
