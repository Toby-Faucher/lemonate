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

## Local prerequisites

Python >= 3.11 (`tomllib`), `jq`, `git`, `bash` and `ssh`.

## What the gate checks

1. **preflight**: the recorded baseline must be a full commit sha that exists, is an ancestor of the candidate and is an ancestor of the baseline branch (`repo.baseline_branch`); then the diff must not touch `research/scripts/`, `research/config.toml`, `tests/` or `lean/`. The protected-path check is fail-closed (if git cannot compute the diff the result is `broken`) and NUL-safe (non-ASCII paths are not missed).
2. **build**: `gate.build_cmd` in the candidate worktree.
3. **test** and **perft**: `cargo test` and `tests/perft.rs`.
4. **ship**: the committed candidate and the baseline are sent to the container with `git archive` and built there. The candidate and baseline are built under the same remote `flock` as the match (so builds never overlap a running SPRT), re-check the cache inside the lock, and the binary is smoke-tested with a `uci`/`uciok` exchange and moved into place atomically (tmp then mv). Binaries are cached by commit prefix (12 hex) + config hash and a hash of the container's `rustc -vV` output, named `base-<b12>-<cfgh>-<tc>` and `cand-<c12>-<cfgh>-<tc>`, so a toolchain update never mixes old and new binaries.
5. **sprt**: cutechess-cli, settings from `config.toml`, serialized by `flock`. The match's exit status does not decide the outcome; the parsed cutechess output does (a parse failure is `broken`, a nonzero exit with a parseable verdict is recorded in `reason`).
6. **record**: `result.json`, `patch.diff`, `games.pgn`, then `CATALOG.md` is regenerated.

## Statuses

| status | meaning |
|---|---|
| `accepted` | Preflight, build, test, perft all passed AND SPRT accepted H1: a gain of at least `elo1` Elo. Merge is up to you. |
| `rejected` | Build, test, or perft failed, OR SPRT accepted H0. |
| `inconclusive` | Hit the game cap with no verdict. Never treated as accepted. |
| `broken` | Protected paths touched, or an infrastructure failure (see `reason`). |
| `pending` | In the catalog only: an attempt with no result yet. |

After merging an accepted change the baseline moves with `main`. Each result keeps
the commit it was measured against.

## Container prerequisites

`config.toml` `[remote]` points at an SSH alias (default `chess-test`) for the
test container. The container must have: a Rust toolchain (`~/.cargo/env` set up
and cargo/rustup installed), bash, `cutechess-cli`, `flock`, `timeout`, and `tar`
on `PATH`, and the opening book at `match.book`. The container setup is described
in an untracked `mds/elo-testing.md` file kept locally (not in the repo). On the
first real run, check the output against `sprt.log`: the parser accepts both `H1
accepted` and `H1 was accepted`, but the exact cutechess output format and exit
status are unverified until the self-tests in the plan's Task 9 are run.

## Known limitations

- The gate measures the committed HEAD of the worktree, but local build/test/perft run in
  the live worktree; editing the worktree while a gate is running can make the local
  stages and the match see different trees.
- The protected-path list does not cover everything that can influence the verifier:
  an attempt could change `Cargo.toml` (for example redirect the `perft` test target)
  or `.cargo/config.toml` (a `[target.*] runner` override can make `cargo test` pass without
  running any tests), or add a `build.rs`, which runs on the container. Consider protecting
  `.cargo/`, `Cargo.toml`, `Cargo.lock`, `build.rs` and `rust-toolchain*`, and running container
  builds as an unprivileged user.
- The games PGN pull is best effort: a failed pull leaves an empty `games.pgn` without
  a recorded reason; it does not affect the verdict.
- `flock` waits have no timeout.
- The catalog displays any `result.json` present in an experiment directory; only `gate.sh` should write them. Concurrent gates on the same id are not guarded.

## Tests

```bash
python3 -m unittest discover -s research/scripts/tests -p 'test_*.py'
cargo test --release --test perft
bash research/scripts/tests/test_attempt.sh
bash research/scripts/tests/test_gate.sh
```
