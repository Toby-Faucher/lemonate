# Lower the LMR formula divisor from 2.0 to 1.75 for larger late-move reductions

## Change

In src/search/mod.rs, the LMR table formula `ln(depth) * ln(move_count) / 2.0` becomes `/ 1.75`. The mirrored copy in the unit test `lmr_table_matches_formula` is updated identically. No other parameter changes.

## Mechanism

The backlog suggested 2.25 (less reduction). However, the preceding experiment (0005, LMR_FULL_DEPTH_MOVES 4 -> 3, reductions starting one move earlier) was accepted at +19.8 Elo, which suggests this engine benefits from more aggressive LMR. So the opposite direction is tested: a smaller divisor gives larger reductions at depth and for late moves, saving nodes on moves unlikely to be best and reaching deeper on the main line.

## Expected Elo

Roughly +5 to +10 Elo; risk is missed tactics from over-reduction.
