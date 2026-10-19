# bench/

Track engine speed across the whole git history, like a world-record progression:
every commit that changes the compiled binary is built, timed, and plotted against the
best result so far.

```bash
bench/bench.py doctor             # podman, image, CPU pinning, noise
bench/bench.py history            # build + measure every unmeasured commit on main (resumable)
bench/bench.py run HEAD WORKTREE  # measure specific revisions; WORKTREE = your uncommitted checkout
bench/bench.py compare main WORKTREE   # interleaved A/B with a 95% confidence interval
bench/bench.py list               # record progression in the terminal
bench/bench.py report             # writes bench/report.html (open it, hit Replay)
python3 bench/test_bench.py       # unit tests for the record logic
```

Python 3 stdlib only. Needs `podman` and `taskset`.

## Strength: SPRT matches on chess-test

`bench/sprt.py` queues cutechess matches with the same settings as the research gate (they are
read from `research/config.toml`), but lets several run back to back and be watched:

```bash
bench/sprt.py start 0015-pvs main exp/0015-pvs   # build both here, ship, queue (bounds from the config)
bench/sprt.py start 0019-x main feat/x --elo0 -4 --elo1 1   # non-regression bounds
bench/sprt.py status                             # one line per match: games, score, Elo, LLR, verdict
bench/sprt.py wait NAME                          # block until it finishes
bench/sprt.py cancel NAME
bench/sprt.py record NAME ID                     # write research/experiments/ID/{result,sprt}.json + patch.diff
```

Two machines can play matches, one match at a time each: `chess-test` (the container the gate uses)
and the local machine, where cutechess runs in a small Debian container on threads 0-9 at low
priority (the benchmark pins to cpu 14, so the two do not collide). `--runner auto`, the default,
sends a match to whichever has fewer matches waiting or running; `--runner bawn|chess-test` forces
one. The first local match fetches the cutechess binary and the opening book from `chess-test` and
builds the runner image. Each result is fair on its own because both engines play on the same
machine, but the two machines are different CPUs, so compare Elo between matches, not nps.

Engines are built in the pinned container with a portable target (`x86-64-v3`) and, for
`chess-test`, copied over, so nothing compiles on the match machine while a game is running.
Remote matches queue on the same lock the gate uses. `record` leaves `hypothesis.md` and `trace.md` to be written by
hand; then `research/scripts/catalog.sh` regenerates `CATALOG.md`. Unlike `gate.sh`, this does not
check the diff against the protected paths or run the local test stages, so run the tests yourself.

## What is measured

| Metric | How | Why |
|---|---|---|
| **Time to depth 12** (headline) | 12 fixed positions searched to depth 12, one thread, min of 5 passes | The "speedrun": total wall time. Combines raw speed and search efficiency. |
| **Time to depth 7** | same, depth 7 | Lets engines that cannot finish depth 12 appear on the chart. |
| **NPS** | `go movetime 400` per position, median of 3 passes | Raw speed independent of what the search prunes. |
| **Nodes to depth 12** | node count from the depth-12 pass | Search efficiency. Deterministic: if it is identical between two commits the search tree is identical and any time change is pure speed. |

`nodes` is the engine's reported interior-node count and **excludes quiescence nodes**, so it
and NPS are only comparable between versions with the same quiescence search. A new record has
to beat the standing one by a noise floor (1.5% for depth-12 time, 3% for depth 7, 1.5% for NPS,
0 for nodes) so a 0.1% wobble is not a "record".

A faster time with fewer nodes can just mean "searches less". Strength still needs the SPRT
gate in `research/`; this tool says how fast, not how good.

## How noise is controlled

- Builds run in the pinned image from `config.json` (`rust:1.98.1-slim`), one core each, in
  parallel. Measurements run afterwards, never during builds.
- Timing runs are pinned with `taskset` to one P-core (`measure_cpu`); keep its hyper-thread
  sibling idle. The suite is warmed up, each pass uses a fresh `ucinewgame`, and per-position
  results are the best of N passes.
- An *anchor* binary (the first measured HEAD, `anchor.json`) is re-timed every 8 measurements.
  Results are scaled by the anchor drift so a slow hour on the machine does not look like a
  regression. Raw values are stored too (`ttd_ms`), normalised ones are `*_norm_*`.
- Rows whose source tree is identical (docs-only commits) reuse the measurement (`dup_of`).
- Rootless podman has no `cpuset` cgroup controller here, so builds are pinned with `taskset`
  inside the container instead of `--cpuset-cpus`.

Results are only comparable on the same machine, toolchain and suite: changing `suite.epd`,
the depths, `movetime_ms`, `passes`, `rustflags` or `image` changes the suite hash and starts a
new series (old rows are ignored, not deleted).

## Files

- `bench.py`: CLI. `report.py`: HTML generator. `suite.epd`, `config.json`: the benchmark definition.
- `history.jsonl`: one row per commit (committed). `anchor.json`: drift reference (committed).
- `.work/`: binaries, build logs, sources. Git-ignored.
