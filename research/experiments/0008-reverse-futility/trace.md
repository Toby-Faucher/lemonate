## Summary
Added reverse futility pruning (margin 100*depth, depth 1..=6, non-PV, not in check, non-mate beta, non-pawn material) before null move, to cut shallow fail-high subtrees cheaply.

## Tried
1. Grepped for existing static-eval beta cutoffs: none. Added consts and the check using the already computed static_eval, fail-soft return. Ran lib/bin tests: all pass.

## Dropped
- Returning beta instead of static_eval - margin: fail-soft kept for more informative bounds.
- Unit test for the constant: skipped, not valuable.

## Surprises
None; can_null_move was reusable as the zugzwang guard. static_eval is computed before TT-independent checks, so no double evaluation.
