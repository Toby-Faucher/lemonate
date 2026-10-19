# Backlog

One line per candidate hypothesis. Pick one, run `scripts/attempt.sh <id>`, and mark it
`[x]` once its experiment has a result. Line numbers refer to `src/search/mod.rs` at the
time of writing.

- [x] 0001-aspiration-windows: re-enable aspiration windows (disabled near line 408 as "TEMPORARILY DISABLED ... to debug queen blunder"); the blunder may have been fixed elsewhere **Result: inconclusive, +8.9 ± 7.4 Elo (LOS 99.1%, LLR 2.25 of 2.94), 4000 games; promising, needs a longer confirmation run. Not merged.**
- [ ] 0002-aspiration-narrow: shrink `ASPIRATION_WINDOW` from 50 to 25 (only meaningful once 0001 is settled)
- [x] 0003-null-move-reduction: lower `NULL_MOVE_REDUCTION` from 3 to 2, or change the `depth / 6` term in the null-move reduction **Result: inconclusive, +8.1 ± 7.6 Elo, 4000 games, LLR 1.83. Not merged.**
- [x] 0004-null-move-min-depth: lower `NULL_MOVE_MIN_DEPTH` from 3 to 2 **Result: inconclusive, -5.0 ± 7.6 Elo, 4000 games, LLR -2.5. Not merged.**
- [x] 0005-lmr-full-depth: reduce `LMR_FULL_DEPTH_MOVES` from 4 to 3 so reductions start earlier **Result: accepted, +19.8 ± 10.6 Elo, 1998 games, LLR 2.95. Merged.**
- [x] 0006-lmr-divisor: change the LMR table divisor (`ln(depth) * ln(move_count) / 2.0`) to 2.25 **Result: rejected, -14.0 ± 10.3 Elo, 2040 games, LLR -2.96. Not merged.**
- [x] 0007-futility-margin: change `FUTILITY_MARGIN_BASE` from 150 to 120, and separately to 200 **Result: inconclusive, -1.0 ± 7.4 Elo, 4000 games, LLR -1.24. Not merged.**
- [x] 0008-reverse-futility: add static-eval beta pruning (reverse futility) at shallow depth in non-PV nodes, a technique the search does not have yet **Result: inconclusive, +7.1 ± 7.3 Elo, 4000 games, LLR 1.64. Not merged.**
- [x] 0009-history-bonus: test history bonus formula in `src/search/history.rs`: change bonus from `depth * depth` to `depth * (depth + 1) / 2` (triangular instead of quadratic), or test different exponents **Result: inconclusive, -1.1 ± 7.3 Elo, 4000 games, LLR -1.3. Not merged.**
- [x] 0010-check-extension: extend the search by one ply when the side to move is in check **Result: rejected, -27.4 ± 14.0 Elo, 1104 games, LLR -2.97. Not merged.**
- [x] 0011-tt-depth-age: test alternative TT replacement weighting in `src/search/transposition.rs`: combine depth and age into a unified score for replacement decisions instead of treating them separately **Result: inconclusive, +2.2 ± 7.3 Elo, 4000 games, LLR -0.12. Not merged.**
- [x] 0012-iir: internal iterative reduction: reduce depth at nodes with no hash move **Result: inconclusive, +8.9 ± 7.4 Elo, 4000 games, LLR 2.29. Not merged.**
- [ ] 0013-retune-mobility: re-run `src/bin/tuner` on a larger quiet-position dataset and adopt the new mobility weights
- [x] 0014-stack-promising: stack the four unresolved small gains (0001 aspiration windows, 0003 null-move R=2, 0008 reverse futility, 0012 internal iterative reduction) on top of the merged LMR change (added after the backlog, at the owner's request). **Result: accepted, +29.5 ± 13.3 Elo, 1241 games, LLR 2.94. Merged.**
- [x] 0015-pvs: principal variation search **Result: accepted, +102.1 ± 26.3 Elo, 504 games, LLR 2.95. Merged.**
- [x] 0016-qs-tt: transposition table in quiescence **Result: accepted, +30.7 ± 13.5 Elo, 1715 games, LLR 2.96. Merged.**
- [x] 0017-null-no-consecutive: no null move straight after a null move **Result: inconclusive, stopped by hand at 1025 games. Not merged.**
- [x] 0018-null-eval-r: scale the null-move reduction by eval over beta **Result: inconclusive, about +1 Elo, stopped by hand at about 990 games. Not merged.**
- [x] 0019-move-overhead: Move Overhead UCI option (see its result.json)
- [x] 0020-fast-legality: skip make/unmake legality for non-pinned, non-king moves **Result: accepted, +54.3 ± 18.5 Elo, 923 games, LLR 2.95. Merged.**
- [x] 0021-combined-main: rating check of main against 9359b37 **Result: +210.1 ± 41.8 Elo, 272 games.**
- [x] 0022-late-move-pruning **Result: accepted, +56.3 ± 18.9 Elo, 928 games, LLR 2.96. Merged in #75 (stacked as 0029).**
- [x] 0023-see-pruning **Result: inconclusive, +11.6 ± 8.9 Elo, 4000 games, LLR 2.21. Not merged.**
- [x] 0024-improving **Result: accepted, +16.5 ± 9.5 Elo, 3487 games, LLR 2.94. Merged in #75 (stacked as 0029).**
- [x] 0025-razoring **Result: inconclusive, +13.6 ± 9.0 Elo, 4000 games, LLR 2.64. Worth a re-run.**
- [x] 0026-mate-distance **Result: inconclusive, -1.1 ± 8.9 Elo, 4000 games. Not merged.**
- [x] 0027-continuation-history **Result: inconclusive, +1.0 ± 8.9 Elo, 4000 games. Not merged.**
- [x] 0028-see-attacker-fix: correctness fix **Result: inconclusive, -0.8 ± 8.9 Elo, 4000 games (no strength change expected). Not merged.**
- [x] 0029-lmp-improving-stack **Result: accepted, +75.2 ± 22.1 Elo, 662 games, LLR 2.95. Merged (#75).**
- [x] 0030-main-vs-prev: merged main 9e89100 against e492c25 **Result: accepted, +65.7 ± 20.6 Elo, 787 games, LLR 2.95.**
