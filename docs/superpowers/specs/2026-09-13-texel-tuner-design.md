# Texel Tuning Infrastructure — Design Spec

Date: 2026-09-13

## Problem

Lemonate's evaluation constants (piece-square tables, pawn structure,
king safety, and mobility bonuses) are either copied verbatim from a
different engine's tuning (PeSTO/Rofchade PST) or hand-picked round
numbers (pawn/king/mobility bonuses). None have been tuned against
Lemonate's own search behavior. An engine-strength audit
(2026-09-13, subagent-dispatched review of `src/eval/` and
`src/search/`) identified automated eval tuning via Texel's method as
the single highest-Elo lever available (+50 to +100 Elo estimated),
larger than any individual search change, but requiring new
infrastructure that doesn't exist yet.

## Goal

Build a standalone tuner binary that:
1. Loads a labeled dataset of quiet chess positions (FEN + game
   result).
2. Fits the sigmoid scaling constant `K`.
3. Runs coordinate descent over the tunable eval constants to
   minimize prediction error against game results.
4. Prints the tuned constants as ready-to-paste Rust `const` blocks.

Material values (`MG_VALUE`/`EG_VALUE` in `pst.rs`) are explicitly
**out of scope** — they stay fixed as an anchor. Everything else in
`pst.rs`, `pawn_structure.rs`, `king_safety.rs`, and `mobility.rs` is
in scope.

## Non-goals

- No self-play data generation pipeline (use a public quiet-labeled
  dataset instead — e.g. the zurichess-style `quiet-labeled.epd`
  corpus; user downloads/sources it manually, same workflow already
  used for the cutechess-cli opening book).
- No automatic write-back into source files. Output is printed
  const blocks; a human pastes them in and the change gets SPRT-
  validated like any other engine change.
- No feature-vector/dot-product fast-path architecture. Each
  optimizer trial re-runs the real evaluation code. Slower per
  trial, but zero risk of the tuner's scoring logic silently
  diverging from production's, and simple enough to implement
  correctly on the first pass. (Noted as a future upgrade path if a
  tuning run proves too slow.)
- No workspace/separate-crate restructuring. The tuner is a new
  binary target (`src/bin/tuner.rs`) in the existing crate, following
  the same pattern as `src/main.rs`.

## Architecture

### 1. `EvalWeights` (new file: `src/eval/weights.rs`)

A single struct holding every currently-hardcoded tunable constant,
grouped by the module it comes from:

- **PST**: `pst_mg: [[i32; 64]; 6]`, `pst_eg: [[i32; 64]; 6]`, indexed
  by `PieceType as usize`. (Values migrated from `MG_*_TABLE` /
  `EG_*_TABLE` in `pst.rs`; `MG_VALUE`/`EG_VALUE` material stay as
  free-standing consts, untouched.)
- **Pawn structure**: `doubled_mg/eg`, `isolated_mg/eg`,
  `backward_mg/eg` (scalars), `passed_mg/eg: [i32; 8]`,
  `connected_mg/eg` (scalars). (From `pawn_structure.rs`.)
- **King safety**: `shield_close_mg/eg`, `shield_far_mg/eg`,
  `open_file_near_king_mg/eg`, `semi_open_file_near_king_mg/eg`.
  (From `king_safety.rs`.)
- **Mobility**: `knight_mg/eg: [i32; 9]`, `bishop_mg/eg: [i32; 14]`,
  `rook_mg/eg: [i32; 15]`, `queen_mg/eg: [i32; 28]`. (From
  `mobility.rs`.)

`impl EvalWeights { pub const DEFAULT: EvalWeights = EvalWeights { ... }; }`
populated with today's exact values, so production behavior is
unchanged by construction.

### 2. Refactor of the 4 eval modules

`PieceSquareTableEval`, `PawnStructureEval`, `KingSafetyEval`, and
`MobilityEval` each gain a `evaluate_with_phase_and_weights(&self,
board: &Board, phase: i32, weights: &EvalWeights) -> i32` method
(and, where the module's `evaluate`/`evaluate_with_phase` compute
sub-scores via a private per-color helper, that helper gains a
`weights: &EvalWeights` parameter too). The existing
parameterless `evaluate`/`evaluate_with_phase` methods become thin
wrappers that pass `&EvalWeights::DEFAULT` — **no call-site changes
needed anywhere else in the engine** (`Evaluator::new()`, search,
UCI, all existing tests are unaffected), and no stored field/lifetime
parameter is needed on the structs themselves. The tuner calls the
`_and_weights` variants directly with its candidate `EvalWeights`,
which lives only as long as one trial (an ordinary stack value, not
`'static`) — avoiding the memory-leak trap of a `&'static` field that
can't represent a transient candidate. Internal bodies swap direct
references to the old top-level per-module consts (`DOUBLED_PAWN_MG`,
`MG_PAWN_TABLE`, etc.) for `weights.<field>` lookups; those old
top-level consts are kept as `pub const NAME: T =
EvalWeights::DEFAULT.field;` aliases so the existing test suites
(50+ pawn structure tests, king safety tests, mobility tests) that
reference them by name keep compiling unchanged, with `EvalWeights`
as the single source of truth they now alias.

**Verification**: a test asserting `Evaluator::new().evaluate(&board)`
produces exact known values (captured from the current, pre-refactor
build) on a batch of representative FENs. This runs *before* the
tuner is built, as a standalone checkpoint.

### 3. Dataset loader

Parses the quiet-labeled EPD/PGN-derived format into
`Vec<(Board, f64)>` — board position plus result normalized to
White's perspective (1.0 = White win, 0.5 = draw, 0.0 = Black win).
Exact line format is confirmed once the user has the actual dataset
file in hand (typical zurichess-style format: FEN followed by a
result annotation comment); loader is written against that format
once confirmed.

### 4. Tuner binary (`src/bin/tuner.rs`)

- **Static eval for training**: uses
  `Evaluator::evaluate_detailed(&board).total()` (already exists,
  sums pst+pawn+king+mobility from White's perspective, no
  side-to-move flip — exactly what's needed) built from an
  `Evaluator` constructed with a candidate `EvalWeights`.
- **K-fitting**: coarse-to-fine grid search over `K`, minimizing MSE
  between `sigmoid(K * eval / 400)` and the dataset's results, using
  `EvalWeights::DEFAULT`.
- **Coordinate descent**: for each tunable scalar (flattening all
  `EvalWeights` fields into one parameter list, ~800 values), try
  `+1`/`-1`; keep the change if total dataset MSE drops; otherwise
  leave it. Repeat full sweeps until a sweep makes zero improvements,
  or a max-sweep cap is hit.
- **Parallelism**: plain `std::thread` (no new dependency) — split
  the dataset into N chunks (N = available cores), each thread
  computes partial squared error for a given candidate weight set,
  main thread sums.
- **Progress output**: print current total error after each sweep so
  a long run is observable.
- **Final output**: print every tuned field as a Rust `pub const`
  block, same names and shapes as the originals in `pst.rs` /
  `pawn_structure.rs` / `king_safety.rs` / `mobility.rs`, so pasting
  over the old blocks is a direct swap.

### 5. Validation workflow (unchanged from existing process)

Paste the tuned consts in, `cargo build --release` on the container,
run the same cutechess-cli + ordo SPRT pipeline already set up
(`/root/engines/`, `10.0.0.63`, `UHO_Lichess_4852_v1.epd`,
`elo0=0 elo1=10`) comparing tuned-eval build vs. the pre-tuning
build.

## Risks

- **Refactor correctness**: the `EvalWeights` migration touches 4
  files' worth of scoring logic. Mitigated by the bit-identical parity
  test before any tuner code is written.
- **Runtime**: full-eval-per-trial coordinate descent is slower than
  a feature-vector architecture. Mitigated by parallelizing across
  the container's 12 cores and treating this as an offline/overnight
  batch job; dataset size is reducible if a run is too slow.
- **Overfitting / bad local optimum**: standard risk of any Texel
  tuning pass. Mitigated by the mandatory before/after SPRT gate —
  a tuning run that regresses strength is never adopted regardless of
  what the training error says.
