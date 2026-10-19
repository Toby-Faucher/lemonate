# Lemonate

A UCI chess engine written in Rust: bitboard move generation with magic bitboards, an
alpha-beta search with the usual modern pruning, a tapered hand-written evaluation, and Lazy SMP.
It speaks plain UCI, so any UCI GUI or `cutechess-cli` can drive it.

**New to the code?** [`docs/GUIDE.md`](docs/GUIDE.md) is a guided tour in learning order, with
exercises: bitboards, move generation, alpha-beta, the pruning techniques, evaluation, UCI, and
how changes are measured.

## Build and run

```bash
RUSTFLAGS="-C target-cpu=native -C target-feature=+bmi2,+popcnt" cargo build --release
./target/release/lemonate        # then type UCI commands, or point a GUI at the binary
```

The release profile uses fat LTO and `panic = "abort"`. The CPU flags are for speed: with `bmi1` and
`popcnt` enabled the bit scans and piece counts compile to single instructions, and the engine still
builds and runs without them, only slower. Build on the machine that will run it.

**ARM (for example a Raspberry Pi 5):** leave out the x86 flags, which do not exist there:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

The engine has no architecture-specific code beyond an x86 shortcut that falls back to portable
code. The perft tests were run for `aarch64` under emulation and pass.

```text
uci
position startpos moves e2e4 e7e5
go movetime 1000
```

**UCI options:** `Hash` (MB, 1-4096, default 64) and `Threads` (1-256, default 1; more than one
thread uses Lazy SMP with a shared table). `go` understands `depth`, `nodes`, `movetime`, `mate`,
`wtime/btime/winc/binc/movestogo`, `infinite` and `searchmoves`.

## What is in it

| Area | Where | Notes |
|---|---|---|
| Board, move generation | `src/board`, `src/magic`, `src/bitboard` | Magic bitboards, Zobrist hashing, FEN, make/unmake |
| Search | `src/search` | Iterative deepening, aspiration windows, null move, late move reductions, futility and reverse futility, internal iterative reduction, killers, history and counter moves, quiescence with SEE, draw detection (repetition, 50-move, dead material), Lazy SMP |
| Evaluation | `src/eval` | Tapered material and piece-square tables, pawn structure, king safety, mobility; drawn material scores 0 |
| UCI | `src/uci` | Search runs on a worker thread so `stop` works at any time |
| Opening book | `src/book` | Polyglot reader and hashing (not yet used by the UCI layer) |
| Tuner | `src/bin/tuner` | Texel tuning of the evaluation weights from a labelled EPD file |

## Tests

```bash
cargo test --release            # unit tests, perft, end-to-end UCI tests
cargo bench --bench perft       # move generation, evaluation and fixed-depth search timings
```

- `tests/perft.rs` pins node counts for standard perft positions.
- `tests/uci.rs` starts the real binary and checks protocol behaviour: bad input, tiny clocks,
  negative times, illegal moves in `position`.
- Some book tests look for `bins/Perfect2021.bin`, which is not in the repository; they skip
  when it is missing.

## Measuring changes

Three tools live beside the engine; each answers a different question.

| Question | Tool | How |
|---|---|---|
| Did a change make the engine stronger? | `research/` | An SPRT match against the baseline on a test machine decides; see `research/README.md` |
| How fast is each version? | `bench/` | Replays the git history in containers and plots a record progression; see `bench/README.md` |
| Is a change faster, with the same search? | `bench/bench.py compare A B` | Interleaved runs with a confidence interval; reports whether the node counts match |

`research/CATALOG.md` lists every experiment that has been tried, including the ones that failed.

## Repository layout

```text
src/            engine library, the `lemonate` binary and the `tuner` binary
tests/          perft and end-to-end UCI tests
benches/        criterion benchmarks
bench/          speed history, A/B compare, SPRT queue (Python, stdlib only)
research/       experiment gate, catalog and backlog
docs/           design specs and plans
examples/       small debugging programs
```
