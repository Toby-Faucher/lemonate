## Summary
Changed the LMR formula divisor from 2.0 to 1.75 (larger reductions), because the accepted 0005 (earlier LMR start) suggests this engine benefits from more aggressive LMR.

## Tried
1. Filled hypothesis.md.
2. Changed `/ 2.0` to `/ 1.75` in the static LMR table construction (src/search/mod.rs line 78).
3. Changed the same expression in the unit test `lmr_table_matches_formula` (line 1278). This is the mandatory mirror of the table change, not a weakened assertion: the test recomputes the formula and compares it to the table, so it must use the same divisor. Grep for `ln_move_count` confirmed only these two copies exist.
4. `cargo test --release --lib --bins`: all pass.

## Dropped
The backlog's suggested 2.25 (less reduction): contradicts the direction indicated by 0005's accepted result.

## Surprises
None.
