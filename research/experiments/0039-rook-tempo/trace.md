## Summary
Added fixed-weight rook terms (open/semi-open file, 7th rank) in new src/eval/rook.rs plus a +10 side-to-move tempo in Evaluator::evaluate, expecting better rook activity and score stability.

## Tried
1. Filled hypothesis.md.
2. Wrote src/eval/rook.rs (tapered, White-POV, mirrored for Black) with unit tests; wired into static_eval_white_pov.
3. Added TEMPO in evaluate() only (not in the white-POV function the tuner uses).
4. Updated two eval/mod.rs tests for the +TEMPO shift. cargo test --release --lib --bins passes.

## Dropped
- Making the new weights tunable: would need EvalWeights, to_vec/from_vec, format_weights and test-length edits; the constants are fixed instead, so the tuner vector is unchanged (it does score the rook terms, just does not tune them).
- Bishop pair (belongs to 0035).

## Surprises
- The prompt names src/bin/tuner/weights.rs; the real file is src/eval/weights.rs.
- The worktree had no research/experiments/0039 dir (untracked in main), so hypothesis/trace live in the main checkout.
- evaluate_detailed/EvalDetails does not include the rook term (debug output only).
