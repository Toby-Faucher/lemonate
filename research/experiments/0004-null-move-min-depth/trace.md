## Summary
Lowered NULL_MOVE_MIN_DEPTH from 3 to 2 so depth-2 non-PV nodes also try null-move pruning, giving earlier cheap cutoffs.

## Tried
1. Read the null-move condition and null_move_prune; confirmed depth 2 gives null search depth 2-1-3 = -2, which negamax routes to quiescence (depth <= 0 check precedes node accounting).
2. Changed the constant; ran cargo test --release --lib --bins: all pass, no test pinned the old value.

## Dropped
Nothing; no other parameters touched, per instructions.

## Surprises
Reduction is already larger than depth at depth 3 (null depth -1), so the null search at depth 3 was already pure qsearch; depth 2 differs only in that it is now also qsearch.
