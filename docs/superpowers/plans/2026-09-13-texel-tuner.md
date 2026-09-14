# Texel Tuning Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `tuner` binary that automatically tunes Lemonate's PST/pawn/king/mobility evaluation constants against a labeled position dataset using Texel's method, printing paste-ready Rust const blocks.

**Architecture:** Consolidate every tunable eval constant into one `EvalWeights` struct (`src/eval/weights.rs`), whose `DEFAULT` mirrors today's values exactly. Each of the 4 eval modules (PST, pawn structure, king safety, mobility) gains a `_with_weights` variant of its scoring logic that takes an explicit `&EvalWeights` parameter, with the existing parameterless methods becoming thin wrappers around it — zero behavior change and zero existing-test churn. A new `src/bin/tuner` binary loads a labeled EPD dataset, fits the sigmoid scaling constant `K`, then runs coordinate descent over the ~784 tunable scalars using `std::thread::scope` for parallelism, and prints the tuned values as ready-to-paste `pub const` blocks.

**Tech Stack:** Rust (edition 2024), existing `lemonate` crate (no new dependencies — `std::thread` only, no rayon).

**Spec:** `docs/superpowers/specs/2026-09-13-texel-tuner-design.md`

## Global Constraints

- Material values (`MG_VALUE`/`EG_VALUE` in `src/eval/pst.rs`) are out of scope and stay fixed — never added to `EvalWeights`.
- No new crate dependencies. Parallelism uses `std::thread::scope` only.
- Tuner output is printed Rust const blocks for manual paste-back — the tuner never writes to `src/eval/*.rs` itself.
- Every existing eval module's parameterless `evaluate`/`evaluate_with_phase` (and any private helper already called directly by that module's own `#[cfg(test)]` suite) must keep its exact current signature and behavior — refactors add new `_with_weights` siblings, they never change or remove the originals' call sites.
- The tuner is a new binary target at `src/bin/tuner/main.rs` (Cargo auto-discovers it; no `Cargo.toml` edit needed), in the same crate/package as the engine.
- Dataset format: quiet-labeled EPD, one position per line, `<FEN> c9 "<result>";` where `<result>` is `1-0`, `0-1`, or `1/2-1/2` (the standard zurichess-tuner quiet-labeled format).

---

### Task 1: `EvalWeights` struct

**Files:**
- Create: `src/eval/weights.rs`
- Modify: `src/eval/mod.rs:4-9` (add `mod weights;` to the `mod` list) and after `:16` (add `pub use weights::EvalWeights;` to the `pub use` list)
- Test: inline `#[cfg(test)] mod tests` in `src/eval/weights.rs`

**Interfaces:**
- Produces: `pub struct EvalWeights { pst_mg: [[i32; 64]; 6], pst_eg: [[i32; 64]; 6], doubled_pawn_mg/eg: i32, isolated_pawn_mg/eg: i32, backward_pawn_mg/eg: i32, passed_pawn_mg/eg: [i32; 8], connected_pawn_mg/eg: i32, pawn_shield_close_mg/eg: i32, pawn_shield_far_mg/eg: i32, open_file_near_king_mg/eg: i32, semi_open_file_near_king_mg/eg: i32, knight_mobility_mg/eg: [i32; 9], bishop_mobility_mg/eg: [i32; 14], rook_mobility_mg/eg: [i32; 15], queen_mobility_mg/eg: [i32; 28] }` (all fields `pub`), `impl EvalWeights { pub const DEFAULT: EvalWeights; pub fn to_vec(&self) -> Vec<i32>; pub fn from_vec(v: &[i32]) -> EvalWeights; }`, derives `Clone`.
- Consumes: nothing new (reads the existing per-module consts in `pst.rs`/`pawn_structure.rs`/`king_safety.rs`/`mobility.rs` to build `DEFAULT`, which every later task's `_with_weights` code will take as a parameter).

- [ ] **Step 1: Write the struct definition and a stub `DEFAULT` to get a compiling skeleton**

Create `src/eval/weights.rs`:

```rust
use super::king_safety::{
    OPEN_FILE_NEAR_KING_EG, OPEN_FILE_NEAR_KING_MG, PAWN_SHIELD_CLOSE_EG, PAWN_SHIELD_CLOSE_MG,
    PAWN_SHIELD_FAR_EG, PAWN_SHIELD_FAR_MG, SEMI_OPEN_FILE_NEAR_KING_EG,
    SEMI_OPEN_FILE_NEAR_KING_MG,
};
use super::mobility::{
    BISHOP_MOBILITY_EG, BISHOP_MOBILITY_MG, KNIGHT_MOBILITY_EG, KNIGHT_MOBILITY_MG,
    QUEEN_MOBILITY_EG, QUEEN_MOBILITY_MG, ROOK_MOBILITY_EG, ROOK_MOBILITY_MG,
};
use super::pawn_structure::{
    BACKWARD_PAWN_EG, BACKWARD_PAWN_MG, CONNECTED_PAWN_EG, CONNECTED_PAWN_MG, DOUBLED_PAWN_EG,
    DOUBLED_PAWN_MG, ISOLATED_PAWN_EG, ISOLATED_PAWN_MG, PASSED_PAWN_EG, PASSED_PAWN_MG,
};
use super::pst::{
    EG_BISHOP_TABLE, EG_KING_TABLE, EG_KNIGHT_TABLE, EG_PAWN_TABLE, EG_QUEEN_TABLE, EG_ROOK_TABLE,
    MG_BISHOP_TABLE, MG_KING_TABLE, MG_KNIGHT_TABLE, MG_PAWN_TABLE, MG_QUEEN_TABLE, MG_ROOK_TABLE,
};

/// Every tunable evaluation constant, grouped into one struct so the
/// tuner binary can construct alternative candidate weight sets at
/// runtime without touching the engine's hardcoded consts.
///
/// `DEFAULT` mirrors today's values exactly by referencing the same
/// per-module consts the engine has always used, so pasting tuner
/// output into those consts (in `pst.rs`, `pawn_structure.rs`,
/// `king_safety.rs`, `mobility.rs`) updates `DEFAULT` automatically on
/// the next build - there is no separate copy to keep in sync.
///
/// Material values (`MG_VALUE`/`EG_VALUE` in `pst.rs`) are
/// deliberately not included here - they stay fixed.
#[derive(Clone)]
pub struct EvalWeights {
    pub pst_mg: [[i32; 64]; 6],
    pub pst_eg: [[i32; 64]; 6],

    pub doubled_pawn_mg: i32,
    pub doubled_pawn_eg: i32,
    pub isolated_pawn_mg: i32,
    pub isolated_pawn_eg: i32,
    pub backward_pawn_mg: i32,
    pub backward_pawn_eg: i32,
    pub passed_pawn_mg: [i32; 8],
    pub passed_pawn_eg: [i32; 8],
    pub connected_pawn_mg: i32,
    pub connected_pawn_eg: i32,

    pub pawn_shield_close_mg: i32,
    pub pawn_shield_close_eg: i32,
    pub pawn_shield_far_mg: i32,
    pub pawn_shield_far_eg: i32,
    pub open_file_near_king_mg: i32,
    pub open_file_near_king_eg: i32,
    pub semi_open_file_near_king_mg: i32,
    pub semi_open_file_near_king_eg: i32,

    pub knight_mobility_mg: [i32; 9],
    pub knight_mobility_eg: [i32; 9],
    pub bishop_mobility_mg: [i32; 14],
    pub bishop_mobility_eg: [i32; 14],
    pub rook_mobility_mg: [i32; 15],
    pub rook_mobility_eg: [i32; 15],
    pub queen_mobility_mg: [i32; 28],
    pub queen_mobility_eg: [i32; 28],
}
```

Leave `impl EvalWeights` out for now (Step 3 adds it) — this alone won't compile yet (nothing constructs an `EvalWeights`), which is fine; Step 2's test targets the not-yet-written `DEFAULT`.

- [ ] **Step 2: Write the failing tests**

Append to `src/eval/weights.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::piece::PieceType;

    #[test]
    fn default_pst_matches_original_tables() {
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Pawn as usize], MG_PAWN_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Knight as usize], MG_KNIGHT_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Bishop as usize], MG_BISHOP_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Rook as usize], MG_ROOK_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Queen as usize], MG_QUEEN_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::King as usize], MG_KING_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Pawn as usize], EG_PAWN_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Knight as usize], EG_KNIGHT_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Bishop as usize], EG_BISHOP_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Rook as usize], EG_ROOK_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Queen as usize], EG_QUEEN_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::King as usize], EG_KING_TABLE);
    }

    #[test]
    fn default_pawn_structure_matches_originals() {
        assert_eq!(EvalWeights::DEFAULT.doubled_pawn_mg, DOUBLED_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.doubled_pawn_eg, DOUBLED_PAWN_EG);
        assert_eq!(EvalWeights::DEFAULT.isolated_pawn_mg, ISOLATED_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.isolated_pawn_eg, ISOLATED_PAWN_EG);
        assert_eq!(EvalWeights::DEFAULT.backward_pawn_mg, BACKWARD_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.backward_pawn_eg, BACKWARD_PAWN_EG);
        assert_eq!(EvalWeights::DEFAULT.passed_pawn_mg, PASSED_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.passed_pawn_eg, PASSED_PAWN_EG);
        assert_eq!(EvalWeights::DEFAULT.connected_pawn_mg, CONNECTED_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.connected_pawn_eg, CONNECTED_PAWN_EG);
    }

    #[test]
    fn default_king_safety_matches_originals() {
        assert_eq!(EvalWeights::DEFAULT.pawn_shield_close_mg, PAWN_SHIELD_CLOSE_MG);
        assert_eq!(EvalWeights::DEFAULT.pawn_shield_close_eg, PAWN_SHIELD_CLOSE_EG);
        assert_eq!(EvalWeights::DEFAULT.pawn_shield_far_mg, PAWN_SHIELD_FAR_MG);
        assert_eq!(EvalWeights::DEFAULT.pawn_shield_far_eg, PAWN_SHIELD_FAR_EG);
        assert_eq!(EvalWeights::DEFAULT.open_file_near_king_mg, OPEN_FILE_NEAR_KING_MG);
        assert_eq!(EvalWeights::DEFAULT.open_file_near_king_eg, OPEN_FILE_NEAR_KING_EG);
        assert_eq!(
            EvalWeights::DEFAULT.semi_open_file_near_king_mg,
            SEMI_OPEN_FILE_NEAR_KING_MG
        );
        assert_eq!(
            EvalWeights::DEFAULT.semi_open_file_near_king_eg,
            SEMI_OPEN_FILE_NEAR_KING_EG
        );
    }

    #[test]
    fn default_mobility_matches_originals() {
        assert_eq!(EvalWeights::DEFAULT.knight_mobility_mg, KNIGHT_MOBILITY_MG);
        assert_eq!(EvalWeights::DEFAULT.knight_mobility_eg, KNIGHT_MOBILITY_EG);
        assert_eq!(EvalWeights::DEFAULT.bishop_mobility_mg, BISHOP_MOBILITY_MG);
        assert_eq!(EvalWeights::DEFAULT.bishop_mobility_eg, BISHOP_MOBILITY_EG);
        assert_eq!(EvalWeights::DEFAULT.rook_mobility_mg, ROOK_MOBILITY_MG);
        assert_eq!(EvalWeights::DEFAULT.rook_mobility_eg, ROOK_MOBILITY_EG);
        assert_eq!(EvalWeights::DEFAULT.queen_mobility_mg, QUEEN_MOBILITY_MG);
        assert_eq!(EvalWeights::DEFAULT.queen_mobility_eg, QUEEN_MOBILITY_EG);
    }

    #[test]
    fn to_vec_from_vec_round_trips() {
        let original = EvalWeights::DEFAULT.clone();
        let flat = original.to_vec();
        let rebuilt = EvalWeights::from_vec(&flat);

        assert_eq!(rebuilt.pst_mg, original.pst_mg);
        assert_eq!(rebuilt.pst_eg, original.pst_eg);
        assert_eq!(rebuilt.doubled_pawn_mg, original.doubled_pawn_mg);
        assert_eq!(rebuilt.passed_pawn_mg, original.passed_pawn_mg);
        assert_eq!(rebuilt.queen_mobility_eg, original.queen_mobility_eg);
    }

    #[test]
    fn to_vec_has_expected_length() {
        // 6 pieces * 64 squares * 2 (mg/eg) = 768 PST values, plus
        // 6 pawn scalars + 8+8 passed-pawn arrays + 2 connected scalars = 24,
        // plus 8 king-safety scalars,
        // plus (9+9+14+14+15+15+28+28) = 132 mobility values.
        assert_eq!(EvalWeights::DEFAULT.to_vec().len(), 768 + 24 + 8 + 132);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib eval::weights -- --nocapture`
Expected: FAIL to compile (`EvalWeights::DEFAULT`, `to_vec`, `from_vec` don't exist yet).

- [ ] **Step 4: Implement `DEFAULT`, `to_vec`, and `from_vec`**

Append to `src/eval/weights.rs` (after the struct definition, before the test module):

```rust
impl EvalWeights {
    pub const DEFAULT: EvalWeights = EvalWeights {
        pst_mg: [
            MG_PAWN_TABLE,
            MG_KNIGHT_TABLE,
            MG_BISHOP_TABLE,
            MG_ROOK_TABLE,
            MG_QUEEN_TABLE,
            MG_KING_TABLE,
        ],
        pst_eg: [
            EG_PAWN_TABLE,
            EG_KNIGHT_TABLE,
            EG_BISHOP_TABLE,
            EG_ROOK_TABLE,
            EG_QUEEN_TABLE,
            EG_KING_TABLE,
        ],
        doubled_pawn_mg: DOUBLED_PAWN_MG,
        doubled_pawn_eg: DOUBLED_PAWN_EG,
        isolated_pawn_mg: ISOLATED_PAWN_MG,
        isolated_pawn_eg: ISOLATED_PAWN_EG,
        backward_pawn_mg: BACKWARD_PAWN_MG,
        backward_pawn_eg: BACKWARD_PAWN_EG,
        passed_pawn_mg: PASSED_PAWN_MG,
        passed_pawn_eg: PASSED_PAWN_EG,
        connected_pawn_mg: CONNECTED_PAWN_MG,
        connected_pawn_eg: CONNECTED_PAWN_EG,
        pawn_shield_close_mg: PAWN_SHIELD_CLOSE_MG,
        pawn_shield_close_eg: PAWN_SHIELD_CLOSE_EG,
        pawn_shield_far_mg: PAWN_SHIELD_FAR_MG,
        pawn_shield_far_eg: PAWN_SHIELD_FAR_EG,
        open_file_near_king_mg: OPEN_FILE_NEAR_KING_MG,
        open_file_near_king_eg: OPEN_FILE_NEAR_KING_EG,
        semi_open_file_near_king_mg: SEMI_OPEN_FILE_NEAR_KING_MG,
        semi_open_file_near_king_eg: SEMI_OPEN_FILE_NEAR_KING_EG,
        knight_mobility_mg: KNIGHT_MOBILITY_MG,
        knight_mobility_eg: KNIGHT_MOBILITY_EG,
        bishop_mobility_mg: BISHOP_MOBILITY_MG,
        bishop_mobility_eg: BISHOP_MOBILITY_EG,
        rook_mobility_mg: ROOK_MOBILITY_MG,
        rook_mobility_eg: ROOK_MOBILITY_EG,
        queen_mobility_mg: QUEEN_MOBILITY_MG,
        queen_mobility_eg: QUEEN_MOBILITY_EG,
    };

    /// Flattens every tunable scalar into one vector, in a fixed order,
    /// for the coordinate-descent optimizer to iterate over. Paired
    /// with `from_vec`.
    pub fn to_vec(&self) -> Vec<i32> {
        let mut v = Vec::with_capacity(768 + 24 + 8 + 132);
        for table in &self.pst_mg {
            v.extend_from_slice(table);
        }
        for table in &self.pst_eg {
            v.extend_from_slice(table);
        }
        v.push(self.doubled_pawn_mg);
        v.push(self.doubled_pawn_eg);
        v.push(self.isolated_pawn_mg);
        v.push(self.isolated_pawn_eg);
        v.push(self.backward_pawn_mg);
        v.push(self.backward_pawn_eg);
        v.extend_from_slice(&self.passed_pawn_mg);
        v.extend_from_slice(&self.passed_pawn_eg);
        v.push(self.connected_pawn_mg);
        v.push(self.connected_pawn_eg);
        v.push(self.pawn_shield_close_mg);
        v.push(self.pawn_shield_close_eg);
        v.push(self.pawn_shield_far_mg);
        v.push(self.pawn_shield_far_eg);
        v.push(self.open_file_near_king_mg);
        v.push(self.open_file_near_king_eg);
        v.push(self.semi_open_file_near_king_mg);
        v.push(self.semi_open_file_near_king_eg);
        v.extend_from_slice(&self.knight_mobility_mg);
        v.extend_from_slice(&self.knight_mobility_eg);
        v.extend_from_slice(&self.bishop_mobility_mg);
        v.extend_from_slice(&self.bishop_mobility_eg);
        v.extend_from_slice(&self.rook_mobility_mg);
        v.extend_from_slice(&self.rook_mobility_eg);
        v.extend_from_slice(&self.queen_mobility_mg);
        v.extend_from_slice(&self.queen_mobility_eg);
        v
    }

    /// Rebuilds an `EvalWeights` from a vector produced by `to_vec`.
    /// Panics if `v.len()` doesn't match the expected total (768 + 24
    /// + 8 + 132 = 932).
    pub fn from_vec(v: &[i32]) -> Self {
        assert_eq!(v.len(), 768 + 24 + 8 + 132, "unexpected parameter vector length");

        let mut i = 0;
        let mut take = |n: usize| {
            let slice = &v[i..i + n];
            i += n;
            slice
        };

        let mut pst_mg = [[0i32; 64]; 6];
        for table in pst_mg.iter_mut() {
            table.copy_from_slice(take(64));
        }
        let mut pst_eg = [[0i32; 64]; 6];
        for table in pst_eg.iter_mut() {
            table.copy_from_slice(take(64));
        }

        let doubled_pawn_mg = take(1)[0];
        let doubled_pawn_eg = take(1)[0];
        let isolated_pawn_mg = take(1)[0];
        let isolated_pawn_eg = take(1)[0];
        let backward_pawn_mg = take(1)[0];
        let backward_pawn_eg = take(1)[0];

        let mut passed_pawn_mg = [0i32; 8];
        passed_pawn_mg.copy_from_slice(take(8));
        let mut passed_pawn_eg = [0i32; 8];
        passed_pawn_eg.copy_from_slice(take(8));

        let connected_pawn_mg = take(1)[0];
        let connected_pawn_eg = take(1)[0];
        let pawn_shield_close_mg = take(1)[0];
        let pawn_shield_close_eg = take(1)[0];
        let pawn_shield_far_mg = take(1)[0];
        let pawn_shield_far_eg = take(1)[0];
        let open_file_near_king_mg = take(1)[0];
        let open_file_near_king_eg = take(1)[0];
        let semi_open_file_near_king_mg = take(1)[0];
        let semi_open_file_near_king_eg = take(1)[0];

        let mut knight_mobility_mg = [0i32; 9];
        knight_mobility_mg.copy_from_slice(take(9));
        let mut knight_mobility_eg = [0i32; 9];
        knight_mobility_eg.copy_from_slice(take(9));
        let mut bishop_mobility_mg = [0i32; 14];
        bishop_mobility_mg.copy_from_slice(take(14));
        let mut bishop_mobility_eg = [0i32; 14];
        bishop_mobility_eg.copy_from_slice(take(14));
        let mut rook_mobility_mg = [0i32; 15];
        rook_mobility_mg.copy_from_slice(take(15));
        let mut rook_mobility_eg = [0i32; 15];
        rook_mobility_eg.copy_from_slice(take(15));
        let mut queen_mobility_mg = [0i32; 28];
        queen_mobility_mg.copy_from_slice(take(28));
        let mut queen_mobility_eg = [0i32; 28];
        queen_mobility_eg.copy_from_slice(take(28));

        EvalWeights {
            pst_mg,
            pst_eg,
            doubled_pawn_mg,
            doubled_pawn_eg,
            isolated_pawn_mg,
            isolated_pawn_eg,
            backward_pawn_mg,
            backward_pawn_eg,
            passed_pawn_mg,
            passed_pawn_eg,
            connected_pawn_mg,
            connected_pawn_eg,
            pawn_shield_close_mg,
            pawn_shield_close_eg,
            pawn_shield_far_mg,
            pawn_shield_far_eg,
            open_file_near_king_mg,
            open_file_near_king_eg,
            semi_open_file_near_king_mg,
            semi_open_file_near_king_eg,
            knight_mobility_mg,
            knight_mobility_eg,
            bishop_mobility_mg,
            bishop_mobility_eg,
            rook_mobility_mg,
            rook_mobility_eg,
            queen_mobility_mg,
            queen_mobility_eg,
        }
    }
}
```

Then wire the module into `src/eval/mod.rs`: add `mod weights;` next to the other `mod` declarations (near line 9) and `pub use weights::EvalWeights;` next to the other `pub use` lines (near line 16).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib eval::weights -- --nocapture`
Expected: PASS (6 tests: 4 `default_*_matches_originals`, `to_vec_from_vec_round_trips`, `to_vec_has_expected_length`).

- [ ] **Step 6: Run the full existing test suite to confirm nothing else broke**

Run: `cargo test --lib`
Expected: PASS, same test count as before this task plus the 6 new ones.

- [ ] **Step 7: Commit**

```bash
git add src/eval/weights.rs src/eval/mod.rs
git commit -m "feat: add EvalWeights struct consolidating tunable eval constants"
```

---

### Task 2: PST module — weighted evaluation

**Files:**
- Modify: `src/eval/pst.rs` (whole file's `PieceSquareTableEval` impl, roughly lines 91-191)

**Interfaces:**
- Consumes: `EvalWeights` (Task 1) — `weights.pst_mg`, `weights.pst_eg`.
- Produces: `PieceSquareTableEval::evaluate_with_phase_and_weights(&self, board: &Board, phase: i32, weights: &EvalWeights) -> i32`, used by Task 6.

- [ ] **Step 1: Write the failing test**

In `src/eval/pst.rs`, append (there is no existing `#[cfg(test)]` module in this file yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::EvalWeights;

    #[test]
    fn weighted_default_matches_unweighted() {
        let board = Board::from_fen(
            "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
        )
        .unwrap();
        let eval = PieceSquareTableEval::new();
        let phase = eval.phase.calculate(&board);

        let default_score = eval.evaluate_with_phase(&board, phase);
        let weighted_score =
            eval.evaluate_with_phase_and_weights(&board, phase, &EvalWeights::DEFAULT);

        assert_eq!(default_score, weighted_score);
        assert_eq!(default_score, 338); // captured ground truth for this FEN
    }

    #[test]
    fn known_fen_scores_unchanged() {
        let cases = [
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 0),
            ("8/8/8/8/8/4k3/8/4K3 w - - 0 1", -34),
            (
                "r1bqk2r/pp1nbppp/2p2n2/3p4/3P4/2N1PN2/PPP1BPPP/R1BQK2R w KQ - 0 1",
                183,
            ),
        ];
        let eval = PieceSquareTableEval::new();
        for (fen, expected) in cases {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(eval.evaluate(&board), expected, "mismatch for {}", fen);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib eval::pst -- --nocapture`
Expected: FAIL to compile (`evaluate_with_phase_and_weights` doesn't exist).

- [ ] **Step 3: Implement the weighted method and remove the now-redundant fields**

Replace the `PieceSquareTableEval` struct definition and `impl` block (lines 91-191) with:

```rust
pub struct PieceSquareTableEval {
    phase: GamePhase,
}

impl PieceSquareTableEval {
    pub fn new() -> Self {
        Self {
            phase: GamePhase::new(),
        }
    }

    /// Evaluate the position using tapered evaluation
    pub fn evaluate(&self, board: &Board) -> i32 {
        let phase = self.phase.calculate(board);
        self.evaluate_with_phase(board, phase)
    }

    /// Evaluate the position with a pre-computed game phase.
    ///
    /// Skips the internal `phase.calculate` call so callers that already
    /// know the phase (e.g. `Evaluator::evaluate`) can share one
    /// calculation across all eval terms. Scores are bit-identical to
    /// `evaluate` when passed `self.phase.calculate(board)`.
    pub fn evaluate_with_phase(&self, board: &Board, phase: i32) -> i32 {
        self.evaluate_with_phase_and_weights(board, phase, &super::EvalWeights::DEFAULT)
    }

    /// Same as `evaluate_with_phase`, but scores PST terms from an
    /// explicit `weights` set instead of the engine's default. Material
    /// (`MG_VALUE`/`EG_VALUE`) is always the fixed default regardless of
    /// `weights` - only PST tables are tunable here.
    pub fn evaluate_with_phase_and_weights(
        &self,
        board: &Board,
        phase: i32,
        weights: &super::EvalWeights,
    ) -> i32 {
        let mut mg_score = 0;
        let mut eg_score = 0;

        for piece_type in [
            PieceType::Pawn,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
            PieceType::King,
        ] {
            let piece_idx = piece_type as usize;
            let mg_table = &weights.pst_mg[piece_idx];
            let eg_table = &weights.pst_eg[piece_idx];

            // White pieces
            let mut white_bb = board.piece_bitboard(Color::White, piece_type);
            while white_bb.0 != 0 {
                let square = white_bb.pop_lsb().unwrap().index() as u8;
                mg_score += MG_VALUE[piece_idx];
                eg_score += EG_VALUE[piece_idx];
                mg_score += mg_table[square as usize];
                eg_score += eg_table[square as usize];
            }

            // Black pieces (flip square vertically for Black's perspective)
            let mut black_bb = board.piece_bitboard(Color::Black, piece_type);
            while black_bb.0 != 0 {
                let square = black_bb.pop_lsb().unwrap().index() as u8;
                let flipped_square = square ^ 56; // Flip rank
                mg_score -= MG_VALUE[piece_idx];
                eg_score -= EG_VALUE[piece_idx];
                mg_score -= mg_table[flipped_square as usize];
                eg_score -= eg_table[flipped_square as usize];
            }
        }

        // Tapered evaluation using the phase module
        self.phase.taper(mg_score, eg_score, phase)
    }
}

impl Default for PieceSquareTableEval {
    fn default() -> Self {
        Self::new()
    }
}
```

This removes the `mg_table`/`eg_table` fields, the dead `get_piece_value` method (it referenced the removed fields and was already marked `#[allow(dead_code)]` / unused), and the `use super::phase::GamePhase;` import stays as-is. The `phase` field on `PieceSquareTableEval` needs to be reachable from the new test module — since `PieceSquareTableEval::phase` is a private field and the test module is a child (`mod tests { use super::*; }`), it can access it directly as `eval.phase` (private fields are visible to descendant modules in the same file).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib eval::pst -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 5: Run the full test suite**

Run: `cargo test --lib`
Expected: PASS, no regressions elsewhere (nothing outside `pst.rs` referenced the removed fields/dead method — confirmed via `grep -rn "mg_table\|eg_table\|get_piece_value" src/`).

- [ ] **Step 6: Commit**

```bash
git add src/eval/pst.rs
git commit -m "feat: add weighted PST evaluation for the upcoming tuner"
```

---

### Task 3: Pawn structure module — weighted evaluation

**Files:**
- Modify: `src/eval/pawn_structure.rs:94-207` (the `PawnStructureEval` impl block, `evaluate_with_phase` and `evaluate_pawns`)

**Interfaces:**
- Consumes: `EvalWeights` (Task 1) — pawn-structure fields.
- Produces: `PawnStructureEval::evaluate_with_phase_and_weights(&self, board: &Board, phase: i32, weights: &EvalWeights) -> i32`, used by Task 6.

- [ ] **Step 1: Write the failing test**

Append inside the existing `#[cfg(test)] mod tests` block in `src/eval/pawn_structure.rs` (after the last test, before the closing `}`):

```rust
    #[test]
    fn weighted_default_matches_unweighted() {
        use crate::eval::EvalWeights;

        let board = Board::from_fen("8/4p3/8/8/4P3/8/4P3/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let white_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);
        let black_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        let default = eval.evaluate_pawns(white_pawns, black_pawns, Color::White);
        let weighted = eval.evaluate_pawns_with_weights(
            white_pawns,
            black_pawns,
            Color::White,
            &EvalWeights::DEFAULT,
        );

        assert_eq!(default, weighted);
    }

    #[test]
    fn known_fen_scores_unchanged() {
        let cases = [
            (
                "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
                -5,
            ),
            (
                "r1bqk2r/pp1nbppp/2p2n2/3p4/3P4/2N1PN2/PPP1BPPP/R1BQK2R w KQ - 0 1",
                5,
            ),
        ];
        let eval = PawnStructureEval::new();
        for (fen, expected) in cases {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(eval.evaluate(&board), expected, "mismatch for {}", fen);
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib eval::pawn_structure -- --nocapture`
Expected: FAIL to compile (`evaluate_pawns_with_weights` doesn't exist).

- [ ] **Step 3: Implement the weighted methods**

In `src/eval/pawn_structure.rs`, replace `evaluate_with_phase` and `evaluate_pawns` (originally lines 111-207) with:

```rust
    pub fn evaluate_with_phase(&self, board: &Board, phase: i32) -> i32 {
        self.evaluate_with_phase_and_weights(board, phase, &super::EvalWeights::DEFAULT)
    }

    pub fn evaluate_with_phase_and_weights(
        &self,
        board: &Board,
        phase: i32,
        weights: &super::EvalWeights,
    ) -> i32 {
        let mut mg_score = 0;
        let mut eg_score = 0;

        let white_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);
        let black_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        let (w_mg, w_eg) =
            self.evaluate_pawns_with_weights(white_pawns, black_pawns, Color::White, weights);
        mg_score += w_mg;
        eg_score += w_eg;

        let (b_mg, b_eg) =
            self.evaluate_pawns_with_weights(black_pawns, white_pawns, Color::Black, weights);
        mg_score -= b_mg;
        eg_score -= b_eg;

        self.phase.taper(mg_score, eg_score, phase)
    }

    fn evaluate_pawns(
        &self,
        our_pawns: Bitboard,
        enemy_pawns: Bitboard,
        color: Color,
    ) -> (i32, i32) {
        self.evaluate_pawns_with_weights(our_pawns, enemy_pawns, color, &super::EvalWeights::DEFAULT)
    }

    fn evaluate_pawns_with_weights(
        &self,
        our_pawns: Bitboard,
        enemy_pawns: Bitboard,
        color: Color,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg_score = 0;
        let mut eg_score = 0;

        // Hoisted per-file occupancy: 8 ANDs + popcounts once per color,
        // instead of `our_pawns & FILES[file]` / `& ADJACENT_FILES[file]`
        // per pawn. Adjacent-file presence is derived from neighbour file
        // counts, which is exactly `(our & ADJACENT_FILES[f]).is_empty()`:
        // file 0 neighbours only file 1, file 7 only file 6.
        let mut file_counts = [0u32; 8];
        let mut f = 0usize;
        while f < 8 {
            file_counts[f] = (our_pawns.0 & FILES[f].0).count_ones();
            f += 1;
        }
        let mut has_adjacent = [false; 8];
        let mut g = 0usize;
        while g < 8 {
            has_adjacent[g] = if g == 0 {
                file_counts[1] > 0
            } else if g == 7 {
                file_counts[6] > 0
            } else {
                file_counts[g - 1] > 0 || file_counts[g + 1] > 0
            };
            g += 1;
        }

        let mut pawns = our_pawns;

        while pawns.0 != 0 {
            let sq = pawns.pop_lsb().unwrap();
            let file = sq.file() as usize;
            let rank = sq.rank() as usize;

            let eval_rank = if color == Color::White {
                rank
            } else {
                7 - rank
            };

            //doubled pawn check
            if file_counts[file] > 1 {
                mg_score += weights.doubled_pawn_mg;
                eg_score += weights.doubled_pawn_eg;
            }

            //iso pawn check
            if !has_adjacent[file] {
                mg_score += weights.isolated_pawn_mg;
                eg_score += weights.isolated_pawn_eg;
            }

            //Passed pawn check
            if self.is_passed_pawn(sq, enemy_pawns, color) {
                mg_score += weights.passed_pawn_mg[eval_rank];
                eg_score += weights.passed_pawn_eg[eval_rank];
            }

            //Connected pawn check
            if self.is_connected_pawn(sq, our_pawns, color) {
                mg_score += weights.connected_pawn_mg;
                eg_score += weights.connected_pawn_eg;
            }

            //Backward pawn check
            if self.is_backward_pawn(sq, our_pawns, enemy_pawns, color) {
                mg_score += weights.backward_pawn_mg;
                eg_score += weights.backward_pawn_eg;
            }
        }

        (mg_score, eg_score)
    }
```

(The `return (mg_score, eg_score);` in the original became a plain trailing expression — equivalent, matches the style of the other 3 modules' `_with_weights` helpers written in later tasks.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib eval::pawn_structure -- --nocapture`
Expected: PASS — all pre-existing pawn structure tests (50+, per `CLAUDE.md`) plus the 2 new ones.

- [ ] **Step 5: Run the full test suite**

Run: `cargo test --lib`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/eval/pawn_structure.rs
git commit -m "feat: add weighted pawn structure evaluation for the upcoming tuner"
```

---

### Task 4: King safety module — weighted evaluation

**Files:**
- Modify: `src/eval/king_safety.rs:19-148` (the `KingSafetyEval` impl block)

**Interfaces:**
- Consumes: `EvalWeights` (Task 1) — king-safety fields.
- Produces: `KingSafetyEval::evaluate_with_phase_and_weights(&self, board: &Board, phase: i32, weights: &EvalWeights) -> i32`, used by Task 6.

- [ ] **Step 1: Write the failing test**

Append inside the existing `#[cfg(test)] mod tests` block in `src/eval/king_safety.rs`:

```rust
    #[test]
    fn weighted_default_matches_unweighted() {
        use crate::eval::EvalWeights;

        let board = Board::from_fen("8/8/8/8/8/8/5PPP/6K1 w - - 0 1").unwrap();
        let eval = KingSafetyEval::new();

        let default = eval.evaluate_king(&board, Color::White);
        let weighted =
            eval.evaluate_king_with_weights(&board, Color::White, &EvalWeights::DEFAULT);

        assert_eq!(default, weighted);
    }

    #[test]
    fn known_fen_scores_unchanged() {
        let cases = [
            (
                "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
                -14,
            ),
            (
                "r1bqk2r/pp1nbppp/2p2n2/3p4/3P4/2N1PN2/PPP1BPPP/R1BQK2R w KQ - 0 1",
                18,
            ),
        ];
        let eval = KingSafetyEval::new();
        for (fen, expected) in cases {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(eval.evaluate(&board), expected, "mismatch for {}", fen);
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib eval::king_safety -- --nocapture`
Expected: FAIL to compile (`evaluate_king_with_weights` doesn't exist).

- [ ] **Step 3: Implement the weighted methods**

Replace `evaluate_with_phase`, `evaluate_king`, `evaluate_pawn_shield`, and `evaluate_open_files` (originally lines 41-147) with:

```rust
    pub fn evaluate_with_phase(&self, board: &Board, phase: i32) -> i32 {
        self.evaluate_with_phase_and_weights(board, phase, &super::EvalWeights::DEFAULT)
    }

    pub fn evaluate_with_phase_and_weights(
        &self,
        board: &Board,
        phase: i32,
        weights: &super::EvalWeights,
    ) -> i32 {
        let (w_mg, w_eg) = self.evaluate_king_with_weights(board, Color::White, weights);
        let (b_mg, b_eg) = self.evaluate_king_with_weights(board, Color::Black, weights);

        let mg_score = w_mg - b_mg;
        let eg_score = w_eg - b_eg;

        self.phase.taper(mg_score, eg_score, phase)
    }

    fn evaluate_king(&self, board: &Board, color: Color) -> (i32, i32) {
        self.evaluate_king_with_weights(board, color, &super::EvalWeights::DEFAULT)
    }

    fn evaluate_king_with_weights(
        &self,
        board: &Board,
        color: Color,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let king_sq = self.get_king_square(board, color);

        let our_pawns = board.piece_bitboard(color, crate::PieceType::Pawn);
        let enemy_pawns = board.piece_bitboard(color.opposite(), crate::PieceType::Pawn);

        let (shield_mg, shield_eg) =
            self.evaluate_pawn_shield_with_weights(king_sq, our_pawns, color, weights);
        mg += shield_mg;
        eg += shield_eg;

        let (files_mg, files_eg) =
            self.evaluate_open_files_with_weights(king_sq, our_pawns, enemy_pawns, weights);
        mg += files_mg;
        eg += files_eg;

        (mg, eg)
    }

    fn evaluate_pawn_shield_with_weights(
        &self,
        king_sq: Square,
        our_pawns: Bitboard,
        color: Color,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let king_file = king_sq.file() as usize;
        let king_rank = king_sq.rank();

        let shield_files = FILES[king_file] | ADJACENT_FILES[king_file];
        let shield_pawns = our_pawns & shield_files;

        for pawn_sq in shield_pawns {
            let pawn_rank = pawn_sq.rank();

            let rank_diff = if color == Color::White {
                pawn_rank as i32 - king_rank as i32
            } else {
                king_rank as i32 - pawn_rank as i32
            };

            if rank_diff == 1 {
                mg += weights.pawn_shield_close_mg;
                eg += weights.pawn_shield_close_eg;
            } else if rank_diff == 2 {
                mg += weights.pawn_shield_far_mg;
                eg += weights.pawn_shield_far_eg;
            }
        }

        (mg, eg)
    }

    fn evaluate_open_files_with_weights(
        &self,
        king_sq: Square,
        our_pawns: Bitboard,
        enemy_pawns: Bitboard,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let king_file = king_sq.file() as usize;

        let files_to_check = [
            king_file.saturating_sub(1),
            king_file,
            (king_file + 1).min(7),
        ];

        for &file in &files_to_check {
            let file_mask = FILES[file];
            let our_on_file = (our_pawns & file_mask).is_not_empty();
            let enemy_on_file = (enemy_pawns & file_mask).is_not_empty();

            if !our_on_file && !enemy_on_file {
                mg += weights.open_file_near_king_mg;
                eg += weights.open_file_near_king_eg;
            } else if !our_on_file {
                mg += weights.semi_open_file_near_king_mg;
                eg += weights.semi_open_file_near_king_eg;
            }
        }

        (mg, eg)
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib eval::king_safety -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run the full test suite**

Run: `cargo test --lib`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/eval/king_safety.rs
git commit -m "feat: add weighted king safety evaluation for the upcoming tuner"
```

---

### Task 5: Mobility module — weighted evaluation

**Files:**
- Modify: `src/eval/mobility.rs:44-227` (the `MobilityEval` impl block)

**Interfaces:**
- Consumes: `EvalWeights` (Task 1) — mobility fields.
- Produces: `MobilityEval::evaluate_with_phase_and_weights(&self, board: &Board, phase: i32, weights: &EvalWeights) -> i32`, used by Task 6.

- [ ] **Step 1: Write the failing test**

Append inside the existing `#[cfg(test)] mod tests` block in `src/eval/mobility.rs`:

```rust
    #[test]
    fn weighted_default_matches_unweighted() {
        use crate::eval::EvalWeights;

        let board = Board::from_fen("8/8/8/8/4N3/8/8/4K2k w - - 0 1").unwrap();
        let eval = MobilityEval::new();

        let default = eval.evaluate(&board);
        let phase = eval.phase.calculate(&board);
        let weighted = eval.evaluate_with_phase_and_weights(&board, phase, &EvalWeights::DEFAULT);

        assert_eq!(default, weighted);
    }

    #[test]
    fn known_fen_scores_unchanged() {
        let cases = [
            (
                "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
                130,
            ),
            (
                "r1bqk2r/pp1nbppp/2p2n2/3p4/3P4/2N1PN2/PPP1BPPP/R1BQK2R w KQ - 0 1",
                39,
            ),
        ];
        let eval = MobilityEval::new();
        for (fen, expected) in cases {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(eval.evaluate(&board), expected, "mismatch for {}", fen);
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib eval::mobility -- --nocapture`
Expected: FAIL to compile (`evaluate_with_phase_and_weights` doesn't exist).

- [ ] **Step 3: Implement the weighted methods**

Replace `evaluate_with_phase`, `evaluate_color`, `evaluate_knight_mobility`, `evaluate_bishop_mobility`, `evaluate_rook_mobility`, and `evaluate_queen_mobility` (originally lines 62-226) with:

```rust
    pub fn evaluate_with_phase(&self, board: &Board, phase: i32) -> i32 {
        self.evaluate_with_phase_and_weights(board, phase, &super::EvalWeights::DEFAULT)
    }

    pub fn evaluate_with_phase_and_weights(
        &self,
        board: &Board,
        phase: i32,
        weights: &super::EvalWeights,
    ) -> i32 {
        let (w_mg, w_eg) = self.evaluate_color_with_weights(board, Color::White, weights);
        let (b_mg, b_eg) = self.evaluate_color_with_weights(board, Color::Black, weights);

        let mg_score = w_mg - b_mg;
        let eg_score = w_eg - b_eg;

        self.phase.taper(mg_score, eg_score, phase)
    }

    fn evaluate_color_with_weights(
        &self,
        board: &Board,
        color: Color,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        // Get friendly pieces to exclude from mobility count.
        // `color_bitboard` is the maintained union of all six per-piece
        // bitboards; the debug assertion below guards that invariant.
        let friendly = board.color_bitboard(color);
        debug_assert_eq!(
            friendly,
            board.piece_bitboard(color, PieceType::Pawn)
                | board.piece_bitboard(color, PieceType::Knight)
                | board.piece_bitboard(color, PieceType::Bishop)
                | board.piece_bitboard(color, PieceType::Rook)
                | board.piece_bitboard(color, PieceType::Queen)
                | board.piece_bitboard(color, PieceType::King),
            "color_bitboard must equal the union of per-piece bitboards"
        );

        let blockers = board.all_pieces();

        let (knight_mg, knight_eg) =
            self.evaluate_knight_mobility_with_weights(board, color, friendly, weights);
        mg += knight_mg;
        eg += knight_eg;

        let (bishop_mg, bishop_eg) =
            self.evaluate_bishop_mobility_with_weights(board, color, friendly, blockers, weights);
        mg += bishop_mg;
        eg += bishop_eg;

        let (rook_mg, rook_eg) =
            self.evaluate_rook_mobility_with_weights(board, color, friendly, blockers, weights);
        mg += rook_mg;
        eg += rook_eg;

        let (queen_mg, queen_eg) =
            self.evaluate_queen_mobility_with_weights(board, color, friendly, blockers, weights);
        mg += queen_mg;
        eg += queen_eg;

        (mg, eg)
    }

    fn evaluate_knight_mobility_with_weights(
        &self,
        board: &Board,
        color: Color,
        friendly: Bitboard,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let mut knights = board.piece_bitboard(color, PieceType::Knight);
        if knights.is_empty() {
            return (0, 0);
        }
        while knights.0 != 0 {
            let sq = knights.pop_lsb().unwrap();
            let attacks = ATTACK_TABLE.knight_attacks(sq);
            let mobility = (attacks & !friendly).count_pieces() as usize;
            let mobility = mobility.min(weights.knight_mobility_mg.len() - 1);

            mg += weights.knight_mobility_mg[mobility];
            eg += weights.knight_mobility_eg[mobility];
        }

        (mg, eg)
    }

    fn evaluate_bishop_mobility_with_weights(
        &self,
        board: &Board,
        color: Color,
        friendly: Bitboard,
        blockers: Bitboard,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let mut bishops = board.piece_bitboard(color, PieceType::Bishop);
        if bishops.is_empty() {
            return (0, 0);
        }
        while bishops.0 != 0 {
            let sq = bishops.pop_lsb().unwrap();
            let attacks = ATTACK_TABLE.bishop_attacks(sq, blockers);
            let mobility = (attacks & !friendly).count_pieces() as usize;
            let mobility = mobility.min(weights.bishop_mobility_mg.len() - 1);

            mg += weights.bishop_mobility_mg[mobility];
            eg += weights.bishop_mobility_eg[mobility];
        }

        (mg, eg)
    }

    fn evaluate_rook_mobility_with_weights(
        &self,
        board: &Board,
        color: Color,
        friendly: Bitboard,
        blockers: Bitboard,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let mut rooks = board.piece_bitboard(color, PieceType::Rook);
        if rooks.is_empty() {
            return (0, 0);
        }
        while rooks.0 != 0 {
            let sq = rooks.pop_lsb().unwrap();
            let attacks = ATTACK_TABLE.rook_attacks(sq, blockers);
            let mobility = (attacks & !friendly).count_pieces() as usize;
            let mobility = mobility.min(weights.rook_mobility_mg.len() - 1);

            mg += weights.rook_mobility_mg[mobility];
            eg += weights.rook_mobility_eg[mobility];
        }

        (mg, eg)
    }

    fn evaluate_queen_mobility_with_weights(
        &self,
        board: &Board,
        color: Color,
        friendly: Bitboard,
        blockers: Bitboard,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let mut queens = board.piece_bitboard(color, PieceType::Queen);
        if queens.is_empty() {
            return (0, 0);
        }
        while queens.0 != 0 {
            let sq = queens.pop_lsb().unwrap();
            let attacks = ATTACK_TABLE.queen_attacks(sq, blockers);
            let mobility = (attacks & !friendly).count_pieces() as usize;
            let mobility = mobility.min(weights.queen_mobility_mg.len() - 1);

            mg += weights.queen_mobility_mg[mobility];
            eg += weights.queen_mobility_eg[mobility];
        }

        (mg, eg)
    }
```

Note: `evaluate_color` (the old unweighted private helper) is deleted entirely rather than kept as a wrapper, since nothing outside `evaluate_with_phase` called it and `evaluate_with_phase` now goes straight to `evaluate_color_with_weights` via `evaluate_with_phase_and_weights`. Same for the 4 old per-piece `evaluate_*_mobility` methods — none are called directly by the existing test module (confirmed: mobility.rs's tests only call `MobilityEval::new().evaluate(&board)`), so there's no wrapper needed for them either.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib eval::mobility -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run the full test suite**

Run: `cargo test --lib`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/eval/mobility.rs
git commit -m "feat: add weighted mobility evaluation for the upcoming tuner"
```

---

### Task 6: `Evaluator` bridge and end-to-end parity test

**Files:**
- Modify: `src/eval/mod.rs:27-66` (the `Evaluator` impl block)

**Interfaces:**
- Consumes: `EvalWeights` (Task 1); `evaluate_with_phase_and_weights` from PST, pawn structure, king safety, and mobility (Tasks 2-5).
- Produces: `Evaluator::static_eval_white_pov(&self, board: &Board, weights: &EvalWeights) -> i32` — the function the tuner (Tasks 7-12) calls for every trial.

- [ ] **Step 1: Write the failing test**

Append to `src/eval/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Board;

    const KNOWN_CASES: [(&str, i32); 5] = [
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 0),
        (
            "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
            449,
        ),
        ("8/8/8/8/8/4k3/8/4K3 w - - 0 1", -34),
        ("r3k2r/ppp2ppp/8/8/8/8/PPP2PPP/R3K2R w KQkq - 0 1", 0),
        (
            "r1bqk2r/pp1nbppp/2p2n2/3p4/3P4/2N1PN2/PPP1BPPP/R1BQK2R w KQ - 0 1",
            245,
        ),
    ];

    #[test]
    fn evaluate_matches_pre_refactor_values() {
        let evaluator = Evaluator::new();
        for (fen, expected) in KNOWN_CASES {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(evaluator.evaluate(&board), expected, "mismatch for {}", fen);
        }
    }

    #[test]
    fn static_eval_white_pov_matches_known_values() {
        // All KNOWN_CASES FENs have White to move, so evaluate() and
        // static_eval_white_pov() coincide (no side-to-move flip applies).
        let evaluator = Evaluator::new();
        for (fen, expected) in KNOWN_CASES {
            let board = Board::from_fen(fen).unwrap();
            let score = evaluator.static_eval_white_pov(&board, &EvalWeights::DEFAULT);
            assert_eq!(score, expected, "white-POV mismatch for {}", fen);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib eval:: -- --nocapture`
Expected: FAIL to compile (`static_eval_white_pov` doesn't exist).

- [ ] **Step 3: Implement `static_eval_white_pov` and rewrite `evaluate` to use it**

Replace the `evaluate` method (originally lines 39-55) with:

```rust
    pub fn evaluate(&self, board: &Board) -> i32 {
        let score = self.static_eval_white_pov(board, &EvalWeights::DEFAULT);

        // Return score from side-to-move's perspective for negamax
        if board.side_to_move() == Color::White {
            score
        } else {
            -score
        }
    }

    /// The tapered evaluation score from White's perspective (no
    /// side-to-move flip), computed with an explicit `weights` set.
    /// Used by the tuner to score training positions consistently
    /// regardless of which side is to move.
    pub fn static_eval_white_pov(&self, board: &Board, weights: &EvalWeights) -> i32 {
        let phase = self.phase.calculate(board);
        self.pst.evaluate_with_phase_and_weights(board, phase, weights)
            + self
                .pawn_structure
                .evaluate_with_phase_and_weights(board, phase, weights)
            + self
                .king_safety
                .evaluate_with_phase_and_weights(board, phase, weights)
            + self
                .mobility
                .evaluate_with_phase_and_weights(board, phase, weights)
    }
```

`EvalWeights` is already in scope in this file via the `pub use weights::EvalWeights;` added in Task 1.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib eval:: -- --nocapture`
Expected: PASS (all eval module tests, including the 2 new ones in this task).

- [ ] **Step 5: Run the full test suite**

Run: `cargo test --lib`
Expected: PASS, full suite green — this is the checkpoint the spec calls out as required before the tuner binary is built.

- [ ] **Step 6: Commit**

```bash
git add src/eval/mod.rs
git commit -m "feat: add Evaluator::static_eval_white_pov for the tuner, verified bit-identical to pre-refactor scores"
```

---

### Task 7: Tuner scaffold and dataset loader

**Files:**
- Create: `src/bin/tuner/main.rs`
- Create: `src/bin/tuner/dataset.rs`

**Interfaces:**
- Consumes: `lemonate::Board` (from the lib crate, via `pub use board::*;` in `src/lib.rs`).
- Produces: `pub struct LabeledPosition { pub board: Board, pub result: f64 }`, `pub fn load_dataset(path: &str) -> Vec<LabeledPosition>`, used by Task 8 onward.

- [ ] **Step 1: Create the binary scaffold**

Create `src/bin/tuner/main.rs`:

```rust
mod dataset;

fn main() {
    eprintln!("tuner: not yet implemented");
    std::process::exit(1);
}
```

Run: `cargo build --bin tuner`
Expected: fails (no `dataset.rs` yet) — this is just confirming Cargo auto-discovers the new binary target before writing its content.

- [ ] **Step 2: Write the failing tests**

Create `src/bin/tuner/dataset.rs`:

```rust
use lemonate::Board;

/// One labeled training position: a board plus its game result from
/// White's perspective (1.0 = White won, 0.5 = draw, 0.0 = Black won).
pub struct LabeledPosition {
    pub board: Board,
    pub result: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_lines() {
        let sample = concat!(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"1-0\";\n",
            "8/8/8/8/8/4k3/8/4K3 w - - 0 1 c9 \"1/2-1/2\";\n",
            "8/8/8/8/8/4k3/8/4K3 b - - 0 1 c9 \"0-1\";\n",
        );
        let dir = std::env::temp_dir();
        let path = dir.join("lemonate_tuner_dataset_test_valid.epd");
        std::fs::write(&path, sample).unwrap();

        let positions = load_dataset(path.to_str().unwrap());

        std::fs::remove_file(&path).unwrap();

        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0].result, 1.0);
        assert_eq!(positions[1].result, 0.5);
        assert_eq!(positions[2].result, 0.0);
    }

    #[test]
    fn skips_blank_and_comment_lines() {
        let sample = concat!(
            "\n",
            "# comment\n",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"1-0\";\n",
        );
        let dir = std::env::temp_dir();
        let path = dir.join("lemonate_tuner_dataset_test_blanks.epd");
        std::fs::write(&path, sample).unwrap();

        let positions = load_dataset(path.to_str().unwrap());

        std::fs::remove_file(&path).unwrap();

        assert_eq!(positions.len(), 1);
    }

    #[test]
    fn skips_unparseable_lines_without_panicking() {
        let sample = concat!(
            "not a real epd line\n",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"1-0\";\n",
        );
        let dir = std::env::temp_dir();
        let path = dir.join("lemonate_tuner_dataset_test_garbage.epd");
        std::fs::write(&path, sample).unwrap();

        let positions = load_dataset(path.to_str().unwrap());

        std::fs::remove_file(&path).unwrap();

        assert_eq!(positions.len(), 1);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --bin tuner -- --nocapture`
Expected: FAIL to compile (`load_dataset` doesn't exist).

- [ ] **Step 4: Implement `load_dataset`**

Add to `src/bin/tuner/dataset.rs` (after the `LabeledPosition` struct, before the test module):

```rust
/// Parses a quiet-labeled EPD dataset (zurichess-tuner format:
/// `<FEN> c9 "<result>";` per line, e.g.
/// `... w - - 0 1 c9 "1-0";`). Blank lines and lines starting with `#`
/// are skipped; lines that fail to parse are skipped with a warning
/// rather than aborting the whole load.
pub fn load_dataset(path: &str) -> Vec<LabeledPosition> {
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read dataset {path}: {e}"));

    let mut positions = Vec::new();
    for (line_no, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse_line(line) {
            Some(pos) => positions.push(pos),
            None => eprintln!("skipping unparseable line {}: {line}", line_no + 1),
        }
    }
    positions
}

fn parse_line(line: &str) -> Option<LabeledPosition> {
    let marker = "c9 \"";
    let marker_idx = line.find(marker)?;
    let fen = line[..marker_idx].trim();

    let after_marker = &line[marker_idx + marker.len()..];
    let end_quote = after_marker.find('"')?;
    let result_str = &after_marker[..end_quote];

    let result = match result_str {
        "1-0" => 1.0,
        "0-1" => 0.0,
        "1/2-1/2" => 0.5,
        _ => return None,
    };

    let board = Board::from_fen(fen).ok()?;
    Some(LabeledPosition { board, result })
}
```

Then update `src/bin/tuner/main.rs` to call it (still a stub end-to-end, filled in for real in Task 11):

```rust
mod dataset;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: tuner <dataset.epd>");
        std::process::exit(1);
    }
    let positions = dataset::load_dataset(&args[1]);
    println!("loaded {} positions", positions.len());
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --bin tuner -- --nocapture`
Expected: PASS (3 tests).

- [ ] **Step 6: Build the binary to confirm it runs**

Run: `cargo build --bin tuner && cargo run --bin tuner -- /nonexistent-path-for-smoke-test 2>&1 | head -5`
Expected: prints a "failed to read dataset" panic message (confirms the binary builds and runs; the panic is expected since the path doesn't exist — this is just a build/wiring smoke test, not a real dataset run).

- [ ] **Step 7: Commit**

```bash
git add src/bin/tuner/main.rs src/bin/tuner/dataset.rs
git commit -m "feat: add tuner binary scaffold with quiet-labeled EPD dataset loader"
```

---

### Task 8: Optimizer — sigmoid, MSE, and K-fitting (single-threaded)

**Files:**
- Create: `src/bin/tuner/optimizer.rs`
- Modify: `src/bin/tuner/main.rs` (add `mod optimizer;`)

**Interfaces:**
- Consumes: `LabeledPosition` (Task 7); `lemonate::{EvalWeights, Evaluator}`.
- Produces: `pub fn sigmoid(eval: f64, k: f64) -> f64`, `pub fn mean_squared_error(positions: &[LabeledPosition], weights: &EvalWeights, k: f64) -> f64`, `pub fn fit_k(positions: &[LabeledPosition], weights: &EvalWeights) -> f64`, used by Task 9 onward.

- [ ] **Step 1: Write the failing tests**

Create `src/bin/tuner/optimizer.rs`:

```rust
use crate::dataset::LabeledPosition;
use lemonate::{Board, EvalWeights, Evaluator};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mse_is_near_zero_for_drawn_symmetric_position() {
        // Starting position evaluates to 0 (symmetric), so
        // sigmoid(0, k) = 0.5 for any k - matching a labeled draw
        // (result = 0.5) exactly.
        let board = Board::starting_position();
        let positions = vec![
            LabeledPosition {
                board: board.clone(),
                result: 0.5,
            },
            LabeledPosition { board, result: 0.5 },
        ];
        let error = mean_squared_error(&positions, &EvalWeights::DEFAULT, 1.0);
        assert!(error < 1e-9, "expected ~0 error, got {error}");
    }

    #[test]
    fn mse_is_positive_for_mismatched_predictions() {
        // Starting position (eval 0, predicted 0.5) labeled as a White
        // win (result 1.0) - a deliberate mismatch, so error must be
        // clearly positive.
        let board = Board::starting_position();
        let positions = vec![LabeledPosition { board, result: 1.0 }];
        let error = mean_squared_error(&positions, &EvalWeights::DEFAULT, 1.0);
        assert!(error > 0.2, "expected clearly positive error, got {error}");
    }

    #[test]
    fn fit_k_stays_in_search_range() {
        let board = Board::starting_position();
        let positions = vec![LabeledPosition { board, result: 0.5 }];
        let k = fit_k(&positions, &EvalWeights::DEFAULT);
        assert!(k > 0.0 && k < 3.0, "k out of expected range: {k}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin tuner optimizer -- --nocapture`
Expected: FAIL to compile (`mean_squared_error`, `fit_k` don't exist).

- [ ] **Step 3: Implement `sigmoid`, `mean_squared_error`, and `fit_k`**

Add to `src/bin/tuner/optimizer.rs` (before the test module):

```rust
pub fn sigmoid(eval: f64, k: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-k * eval / 400.0))
}

/// Mean squared error between `sigmoid(k * eval)` and each position's
/// game result, scoring every position with `weights`.
pub fn mean_squared_error(positions: &[LabeledPosition], weights: &EvalWeights, k: f64) -> f64 {
    let evaluator = Evaluator::new();
    let mut total = 0.0;
    for pos in positions {
        let eval = evaluator.static_eval_white_pov(&pos.board, weights) as f64;
        let predicted = sigmoid(eval, k);
        let diff = pos.result - predicted;
        total += diff * diff;
    }
    total / positions.len() as f64
}

/// Coarse-to-fine grid search for the sigmoid scaling constant `K`
/// that best fits `weights` to the dataset's game results.
pub fn fit_k(positions: &[LabeledPosition], weights: &EvalWeights) -> f64 {
    let mut best_k = 1.0;
    let mut best_error = mean_squared_error(positions, weights, best_k);

    let mut low = 0.1;
    let mut high = 2.0;
    for _ in 0..6 {
        let step = (high - low) / 20.0;
        let mut k = low;
        while k <= high {
            let error = mean_squared_error(positions, weights, k);
            if error < best_error {
                best_error = error;
                best_k = k;
            }
            k += step;
        }
        low = (best_k - step * 2.0).max(0.001);
        high = best_k + step * 2.0;
    }

    best_k
}
```

Add `mod optimizer;` to `src/bin/tuner/main.rs` (next to `mod dataset;`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tuner optimizer -- --nocapture`
Expected: PASS (3 tests).

- [ ] **Step 5: Run the full test suite**

Run: `cargo test`
Expected: PASS (lib tests + tuner bin tests).

- [ ] **Step 6: Commit**

```bash
git add src/bin/tuner/optimizer.rs src/bin/tuner/main.rs
git commit -m "feat: add sigmoid MSE scoring and K grid-search to the tuner"
```

---

### Task 9: Optimizer — coordinate descent

**Files:**
- Modify: `src/bin/tuner/optimizer.rs`

**Interfaces:**
- Consumes: `mean_squared_error`, `EvalWeights::to_vec`/`from_vec` (Task 1).
- Produces: `pub fn coordinate_descent(positions: &[LabeledPosition], initial: &EvalWeights, k: f64, max_sweeps: usize) -> (EvalWeights, f64)`, used by Task 11 and 12.

- [ ] **Step 1: Write the failing tests**

Append to the `#[cfg(test)] mod tests` block in `src/bin/tuner/optimizer.rs`:

```rust
    #[test]
    fn coordinate_descent_never_increases_error() {
        let board = Board::starting_position();
        let mut positions = Vec::new();
        for i in 0..10 {
            let result = if i % 2 == 0 { 0.5 } else { 0.6 };
            positions.push(LabeledPosition {
                board: board.clone(),
                result,
            });
        }

        let k = fit_k(&positions, &EvalWeights::DEFAULT);
        let initial_error = mean_squared_error(&positions, &EvalWeights::DEFAULT, k);
        let (_tuned, final_error) = coordinate_descent(&positions, &EvalWeights::DEFAULT, k, 3);

        assert!(
            final_error <= initial_error,
            "error increased: {initial_error} -> {final_error}"
        );
    }

    #[test]
    fn coordinate_descent_with_zero_sweeps_is_a_no_op() {
        let board = Board::starting_position();
        let positions = vec![LabeledPosition { board, result: 0.5 }];
        let k = 1.0;
        let initial_error = mean_squared_error(&positions, &EvalWeights::DEFAULT, k);

        let (tuned, final_error) = coordinate_descent(&positions, &EvalWeights::DEFAULT, k, 0);

        assert_eq!(final_error, initial_error);
        assert_eq!(tuned.to_vec(), EvalWeights::DEFAULT.to_vec());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin tuner optimizer -- --nocapture`
Expected: FAIL to compile (`coordinate_descent` doesn't exist).

- [ ] **Step 3: Implement `coordinate_descent`**

Add to `src/bin/tuner/optimizer.rs`:

```rust
/// Classic Texel-style local search: for each tunable scalar, try +1
/// then -1, keeping whichever (if any) reduces the dataset's mean
/// squared error. Repeats full sweeps over every parameter until a
/// sweep makes no improvement, or `max_sweeps` is reached.
pub fn coordinate_descent(
    positions: &[LabeledPosition],
    initial: &EvalWeights,
    k: f64,
    max_sweeps: usize,
) -> (EvalWeights, f64) {
    let mut params = initial.to_vec();
    let mut best_error = mean_squared_error(positions, &EvalWeights::from_vec(&params), k);

    for sweep in 0..max_sweeps {
        let mut improved_this_sweep = false;

        for i in 0..params.len() {
            let original = params[i];

            params[i] = original + 1;
            let mut error = mean_squared_error(positions, &EvalWeights::from_vec(&params), k);
            if error < best_error {
                best_error = error;
                improved_this_sweep = true;
                continue;
            }

            params[i] = original - 1;
            error = mean_squared_error(positions, &EvalWeights::from_vec(&params), k);
            if error < best_error {
                best_error = error;
                improved_this_sweep = true;
                continue;
            }

            params[i] = original;
        }

        eprintln!("sweep {}: error = {:.6}", sweep + 1, best_error);
        if !improved_this_sweep {
            break;
        }
    }

    (EvalWeights::from_vec(&params), best_error)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tuner optimizer -- --nocapture`
Expected: PASS (5 tests total in this file).

- [ ] **Step 5: Run the full test suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/bin/tuner/optimizer.rs
git commit -m "feat: add coordinate descent optimizer to the tuner"
```

---

### Task 10: Parallelize `mean_squared_error` with `std::thread::scope`

**Files:**
- Modify: `src/bin/tuner/optimizer.rs` (the `mean_squared_error` function from Task 8)

**Interfaces:**
- Consumes: nothing new.
- Produces: same signature as before (`mean_squared_error`) — this task only changes the implementation, not the API, so `fit_k` and `coordinate_descent` (Tasks 8-9) need no changes.

- [ ] **Step 1: Write the failing test**

Append to the `#[cfg(test)] mod tests` block in `src/bin/tuner/optimizer.rs`:

```rust
    #[test]
    fn parallel_mse_matches_sequential_reference() {
        let board = Board::starting_position();
        let mut positions = Vec::new();
        for i in 0..50 {
            let result = match i % 3 {
                0 => 1.0,
                1 => 0.0,
                _ => 0.5,
            };
            positions.push(LabeledPosition {
                board: board.clone(),
                result,
            });
        }

        let evaluator = Evaluator::new();
        let mut sequential_total = 0.0;
        for pos in &positions {
            let eval = evaluator.static_eval_white_pov(&pos.board, &EvalWeights::DEFAULT) as f64;
            let predicted = sigmoid(eval, 1.0);
            let diff = pos.result - predicted;
            sequential_total += diff * diff;
        }
        let sequential_mse = sequential_total / positions.len() as f64;

        let parallel_mse = mean_squared_error(&positions, &EvalWeights::DEFAULT, 1.0);
        assert!(
            (sequential_mse - parallel_mse).abs() < 1e-12,
            "sequential {sequential_mse} vs parallel {parallel_mse}"
        );
    }
```

- [ ] **Step 2: Run test to verify it passes against the current (single-threaded) implementation first**

Run: `cargo test --bin tuner optimizer::tests::parallel_mse_matches_sequential_reference -- --nocapture`
Expected: PASS (the single-threaded `mean_squared_error` from Task 8 already computes this correctly — this test is a correctness baseline that must keep passing after parallelizing).

- [ ] **Step 3: Replace `mean_squared_error` with a parallel implementation**

Replace the `mean_squared_error` function in `src/bin/tuner/optimizer.rs` with:

```rust
/// Mean squared error between `sigmoid(k * eval)` and each position's
/// game result, scoring every position with `weights`. Splits the
/// dataset across the available CPU cores using `std::thread::scope`
/// (no external dependency) since this runs on every optimizer trial.
pub fn mean_squared_error(positions: &[LabeledPosition], weights: &EvalWeights, k: f64) -> f64 {
    if positions.is_empty() {
        return 0.0;
    }

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(positions.len());

    if num_threads <= 1 {
        return mean_squared_error_chunk(positions, weights, k) / positions.len() as f64;
    }

    let chunk_size = positions.len().div_ceil(num_threads);
    let total: f64 = std::thread::scope(|scope| {
        let handles: Vec<_> = positions
            .chunks(chunk_size)
            .map(|chunk| scope.spawn(|| mean_squared_error_chunk(chunk, weights, k)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    });

    total / positions.len() as f64
}

fn mean_squared_error_chunk(positions: &[LabeledPosition], weights: &EvalWeights, k: f64) -> f64 {
    let evaluator = Evaluator::new();
    let mut total = 0.0;
    for pos in positions {
        let eval = evaluator.static_eval_white_pov(&pos.board, weights) as f64;
        let predicted = sigmoid(eval, k);
        let diff = pos.result - predicted;
        total += diff * diff;
    }
    total
}
```

`std::thread::scope` borrows `positions`, `weights`, and `k` for the duration of the call without needing `'static` or `Arc` — each `EvalWeights` candidate the optimizer tries is a plain stack value, never leaked.

- [ ] **Step 4: Run all optimizer tests to verify they still pass**

Run: `cargo test --bin tuner optimizer -- --nocapture`
Expected: PASS (all 6 tests, including `mse_is_near_zero_for_drawn_symmetric_position` and `coordinate_descent_never_increases_error` from earlier tasks, plus the new parallel-vs-sequential test).

- [ ] **Step 5: Run the full test suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/bin/tuner/optimizer.rs
git commit -m "perf: parallelize tuner MSE computation across CPU cores with std::thread::scope"
```

---

### Task 11: Output formatting and CLI wiring

**Files:**
- Modify: `src/bin/tuner/optimizer.rs` (add `format_weights` and helpers)
- Modify: `src/bin/tuner/main.rs` (real CLI logic)

**Interfaces:**
- Consumes: `EvalWeights`, `fit_k`, `coordinate_descent` (Tasks 8-9).
- Produces: `pub fn format_weights(weights: &EvalWeights) -> String`, and a working `tuner` binary CLI (`tuner <dataset.epd> [max_sweeps]`).

- [ ] **Step 1: Write the failing tests**

Append to the `#[cfg(test)] mod tests` block in `src/bin/tuner/optimizer.rs`:

```rust
    #[test]
    fn format_weights_contains_all_const_names() {
        let output = format_weights(&EvalWeights::DEFAULT);
        for name in [
            "MG_PAWN_TABLE",
            "EG_KING_TABLE",
            "DOUBLED_PAWN_MG",
            "PASSED_PAWN_EG",
            "PAWN_SHIELD_CLOSE_MG",
            "SEMI_OPEN_FILE_NEAR_KING_EG",
            "KNIGHT_MOBILITY_MG",
            "QUEEN_MOBILITY_EG",
        ] {
            assert!(output.contains(name), "missing {name} in output:\n{output}");
        }
    }

    #[test]
    fn format_weights_has_expected_line_count() {
        // 12 PST arrays + 10 pawn-structure consts + 8 king-safety
        // scalars + 8 mobility arrays = 38 lines.
        let output = format_weights(&EvalWeights::DEFAULT);
        assert_eq!(output.lines().count(), 38);
    }

    #[test]
    fn format_array_round_trips_through_rust_syntax() {
        let formatted = format_array("TEST_ARRAY", &[1, -2, 3]);
        assert_eq!(formatted, "pub const TEST_ARRAY: [i32; 3] = [1, -2, 3];\n");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin tuner optimizer -- --nocapture`
Expected: FAIL to compile (`format_weights`, `format_array` don't exist).

- [ ] **Step 3: Implement `format_weights` and its helpers**

Add to `src/bin/tuner/optimizer.rs`:

```rust
/// Formats every tunable value in `weights` as ready-to-paste Rust
/// `pub const` blocks, using the same names and shapes as the
/// originals in `pst.rs`/`pawn_structure.rs`/`king_safety.rs`/
/// `mobility.rs`.
pub fn format_weights(weights: &EvalWeights) -> String {
    let mut out = String::new();

    let pst_tables: [(&str, &[i32; 64]); 12] = [
        ("MG_PAWN_TABLE", &weights.pst_mg[0]),
        ("MG_KNIGHT_TABLE", &weights.pst_mg[1]),
        ("MG_BISHOP_TABLE", &weights.pst_mg[2]),
        ("MG_ROOK_TABLE", &weights.pst_mg[3]),
        ("MG_QUEEN_TABLE", &weights.pst_mg[4]),
        ("MG_KING_TABLE", &weights.pst_mg[5]),
        ("EG_PAWN_TABLE", &weights.pst_eg[0]),
        ("EG_KNIGHT_TABLE", &weights.pst_eg[1]),
        ("EG_BISHOP_TABLE", &weights.pst_eg[2]),
        ("EG_ROOK_TABLE", &weights.pst_eg[3]),
        ("EG_QUEEN_TABLE", &weights.pst_eg[4]),
        ("EG_KING_TABLE", &weights.pst_eg[5]),
    ];
    for (name, table) in pst_tables {
        out.push_str(&format_array(name, table));
    }

    out.push_str(&format_scalar("DOUBLED_PAWN_MG", weights.doubled_pawn_mg));
    out.push_str(&format_scalar("DOUBLED_PAWN_EG", weights.doubled_pawn_eg));
    out.push_str(&format_scalar("ISOLATED_PAWN_MG", weights.isolated_pawn_mg));
    out.push_str(&format_scalar("ISOLATED_PAWN_EG", weights.isolated_pawn_eg));
    out.push_str(&format_scalar("BACKWARD_PAWN_MG", weights.backward_pawn_mg));
    out.push_str(&format_scalar("BACKWARD_PAWN_EG", weights.backward_pawn_eg));
    out.push_str(&format_array("PASSED_PAWN_MG", &weights.passed_pawn_mg));
    out.push_str(&format_array("PASSED_PAWN_EG", &weights.passed_pawn_eg));
    out.push_str(&format_scalar("CONNECTED_PAWN_MG", weights.connected_pawn_mg));
    out.push_str(&format_scalar("CONNECTED_PAWN_EG", weights.connected_pawn_eg));

    out.push_str(&format_scalar("PAWN_SHIELD_CLOSE_MG", weights.pawn_shield_close_mg));
    out.push_str(&format_scalar("PAWN_SHIELD_CLOSE_EG", weights.pawn_shield_close_eg));
    out.push_str(&format_scalar("PAWN_SHIELD_FAR_MG", weights.pawn_shield_far_mg));
    out.push_str(&format_scalar("PAWN_SHIELD_FAR_EG", weights.pawn_shield_far_eg));
    out.push_str(&format_scalar(
        "OPEN_FILE_NEAR_KING_MG",
        weights.open_file_near_king_mg,
    ));
    out.push_str(&format_scalar(
        "OPEN_FILE_NEAR_KING_EG",
        weights.open_file_near_king_eg,
    ));
    out.push_str(&format_scalar(
        "SEMI_OPEN_FILE_NEAR_KING_MG",
        weights.semi_open_file_near_king_mg,
    ));
    out.push_str(&format_scalar(
        "SEMI_OPEN_FILE_NEAR_KING_EG",
        weights.semi_open_file_near_king_eg,
    ));

    out.push_str(&format_array("KNIGHT_MOBILITY_MG", &weights.knight_mobility_mg));
    out.push_str(&format_array("KNIGHT_MOBILITY_EG", &weights.knight_mobility_eg));
    out.push_str(&format_array("BISHOP_MOBILITY_MG", &weights.bishop_mobility_mg));
    out.push_str(&format_array("BISHOP_MOBILITY_EG", &weights.bishop_mobility_eg));
    out.push_str(&format_array("ROOK_MOBILITY_MG", &weights.rook_mobility_mg));
    out.push_str(&format_array("ROOK_MOBILITY_EG", &weights.rook_mobility_eg));
    out.push_str(&format_array("QUEEN_MOBILITY_MG", &weights.queen_mobility_mg));
    out.push_str(&format_array("QUEEN_MOBILITY_EG", &weights.queen_mobility_eg));

    out
}

fn format_scalar(name: &str, value: i32) -> String {
    format!("pub const {name}: i32 = {value};\n")
}

fn format_array(name: &str, values: &[i32]) -> String {
    let body = values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("pub const {name}: [i32; {}] = [{body}];\n", values.len())
}
```

Then replace `src/bin/tuner/main.rs` entirely with:

```rust
mod dataset;
mod optimizer;

use dataset::load_dataset;
use lemonate::EvalWeights;
use optimizer::{coordinate_descent, fit_k, format_weights};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: tuner <dataset.epd> [max_sweeps]");
        std::process::exit(1);
    }
    let dataset_path = &args[1];
    let max_sweeps: usize = args
        .get(2)
        .map(|s| s.parse().expect("max_sweeps must be a non-negative integer"))
        .unwrap_or(50);

    println!("loading dataset from {dataset_path}...");
    let positions = load_dataset(dataset_path);
    println!("loaded {} positions", positions.len());
    if positions.is_empty() {
        eprintln!("no usable positions loaded, aborting");
        std::process::exit(1);
    }

    println!("fitting K...");
    let k = fit_k(&positions, &EvalWeights::DEFAULT);
    println!("K = {k:.4}");

    println!("running coordinate descent (max {max_sweeps} sweeps)...");
    let (tuned, final_error) =
        coordinate_descent(&positions, &EvalWeights::DEFAULT, k, max_sweeps);
    println!("final error: {final_error:.6}");

    println!("\n--- paste the following into pst.rs / pawn_structure.rs / king_safety.rs / mobility.rs ---\n");
    print!("{}", format_weights(&tuned));
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --bin tuner optimizer -- --nocapture`
Expected: PASS (3 new tests plus all prior optimizer tests).

- [ ] **Step 5: Build and smoke-test the CLI end-to-end with a tiny hand-written dataset**

```bash
cargo build --release --bin tuner
cat > /tmp/tuner_smoke.epd << 'EOF'
rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 "1-0";
rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 "0-1";
8/8/8/8/8/4k3/8/4K3 w - - 0 1 c9 "1/2-1/2";
EOF
./target/release/tuner /tmp/tuner_smoke.epd 2
```

Expected: prints "loaded 3 positions", a fitted K, sweep progress lines, "final error: ...", then 38 lines of `pub const` blocks.

- [ ] **Step 6: Run the full test suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/bin/tuner/optimizer.rs src/bin/tuner/main.rs
git commit -m "feat: wire up tuner CLI with paste-ready const-block output"
```

---

### Task 12: End-to-end synthetic-dataset pipeline test

**Files:**
- Modify: `src/bin/tuner/main.rs` (add a `#[cfg(test)]` module)

**Interfaces:**
- Consumes: everything from Tasks 7-11 (`load_dataset`, `fit_k`, `coordinate_descent`, `format_weights`).
- Produces: nothing new — this is a regression test proving the whole pipeline works together, per the spec's required "small synthetic-dataset test on the tuner's error/K-fitting math."

- [ ] **Step 1: Write the test**

Append to `src/bin/tuner/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use dataset::LabeledPosition;
    use lemonate::Board;

    #[test]
    fn pipeline_reduces_or_maintains_error_on_synthetic_dataset() {
        let fens_and_results = [
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 0.5),
            (
                "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
                1.0,
            ),
            (
                "rnbqkb1r/pppp1ppp/5n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
                0.5,
            ),
            ("8/8/8/8/8/4k3/8/4K3 w - - 0 1", 0.5),
            ("r3k2r/ppp2ppp/8/8/8/8/PPP2PPP/R3K2R w KQkq - 0 1", 0.5),
        ];

        let mut positions = Vec::new();
        for (fen, result) in fens_and_results {
            let board = Board::from_fen(fen).unwrap();
            positions.push(LabeledPosition { board, result });
        }

        let k = fit_k(&positions, &EvalWeights::DEFAULT);
        let initial_error = optimizer::mean_squared_error(&positions, &EvalWeights::DEFAULT, k);
        let (tuned, final_error) = coordinate_descent(&positions, &EvalWeights::DEFAULT, k, 3);

        assert!(
            final_error <= initial_error,
            "error increased: {initial_error} -> {final_error}"
        );

        let output = format_weights(&tuned);
        assert!(output.contains("MG_PAWN_TABLE"));
        assert!(output.contains("QUEEN_MOBILITY_EG"));
        assert_eq!(output.lines().count(), 38);
    }

    #[test]
    fn pipeline_round_trips_through_a_real_epd_file() {
        let sample = concat!(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"1-0\";\n",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"0-1\";\n",
            "8/8/8/8/8/4k3/8/4K3 w - - 0 1 c9 \"1/2-1/2\";\n",
        );
        let dir = std::env::temp_dir();
        let path = dir.join("lemonate_tuner_pipeline_test.epd");
        std::fs::write(&path, sample).unwrap();

        let positions = load_dataset(path.to_str().unwrap());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(positions.len(), 3);

        let k = fit_k(&positions, &EvalWeights::DEFAULT);
        let (_tuned, final_error) = coordinate_descent(&positions, &EvalWeights::DEFAULT, k, 1);
        assert!(final_error.is_finite());
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --bin tuner -- --nocapture`
Expected: PASS (2 new tests plus every test from Tasks 7-11 in this binary).

- [ ] **Step 3: Run the full test suite one final time**

Run: `cargo test`
Expected: PASS — lib tests (Tasks 1-6) and tuner binary tests (Tasks 7-12) all green.

- [ ] **Step 4: Commit**

```bash
git add src/bin/tuner/main.rs
git commit -m "test: add end-to-end synthetic-dataset pipeline test for the tuner"
```

---

## After this plan

Once all 12 tasks are merged, the actual tuning run happens outside this plan's scope, per the spec's validation workflow: sync the repo to the Proxmox container, download a real quiet-labeled EPD dataset, run `tuner <dataset> <max_sweeps>` there (parallelized across its 12 cores), paste the printed const blocks into `pst.rs`/`pawn_structure.rs`/`king_safety.rs`/`mobility.rs`, rebuild, and SPRT the tuned build against the current one with the existing cutechess-cli + ordo pipeline.
