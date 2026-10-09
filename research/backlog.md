# Backlog

One line per candidate hypothesis. Pick one, run `scripts/attempt.sh <id>`, and mark it
`[x]` once its experiment has a result. Line numbers refer to `src/search/mod.rs` at the
time of writing.

- [x] 0001-aspiration-windows: re-enable aspiration windows (disabled near line 408 as "TEMPORARILY DISABLED ... to debug queen blunder"); the blunder may have been fixed elsewhere **Result: inconclusive, +8.9 ± 7.4 Elo (LOS 99.1%, LLR 2.25 of 2.94), 4000 games; promising, needs a longer confirmation run. Not merged.**
- [ ] 0002-aspiration-narrow: shrink `ASPIRATION_WINDOW` from 50 to 25 (only meaningful once 0001 is settled)
- [x] 0003-null-move-reduction: lower `NULL_MOVE_REDUCTION` from 3 to 2, or change the `depth / 6` term in the null-move reduction **Result: inconclusive, +8.1 ± 7.6 Elo, 4000 games, LLR 1.83. Not merged.**
- [x] 0004-null-move-min-depth: lower `NULL_MOVE_MIN_DEPTH` from 3 to 2 **Result: inconclusive, -5.0 ± 7.6 Elo, 4000 games, LLR -2.5. Not merged.**
- [x] 0005-lmr-full-depth: reduce `LMR_FULL_DEPTH_MOVES` from 4 to 3 so reductions start earlier **Result: accepted, +19.8 ± 10.6 Elo, 1998 games, LLR 2.95. Merged.**
- [ ] 0006-lmr-divisor: change the LMR table divisor (`ln(depth) * ln(move_count) / 2.0`) to 2.25
- [ ] 0007-futility-margin: change `FUTILITY_MARGIN_BASE` from 150 to 120, and separately to 200
- [ ] 0008-reverse-futility: add static-eval beta pruning (reverse futility) at shallow depth in non-PV nodes, a technique the search does not have yet
- [ ] 0009-history-bonus: test history bonus formula in `src/search/history.rs`: change bonus from `depth * depth` to `depth * (depth + 1) / 2` (triangular instead of quadratic), or test different exponents
- [ ] 0010-check-extension: extend the search by one ply when the side to move is in check
- [ ] 0011-tt-depth-age: test alternative TT replacement weighting in `src/search/transposition.rs`: combine depth and age into a unified score for replacement decisions instead of treating them separately
- [ ] 0012-iir: internal iterative reduction: reduce depth at nodes with no hash move
- [ ] 0013-retune-mobility: re-run `src/bin/tuner` on a larger quiet-position dataset and adopt the new mobility weights
