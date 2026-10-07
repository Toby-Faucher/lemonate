# Backlog

One line per candidate hypothesis. Pick one, run `scripts/attempt.sh <id>`, and mark it
`[x]` once its experiment has a result. Line numbers refer to `src/search/mod.rs` at the
time of writing.

- [ ] 0001-aspiration-windows: re-enable aspiration windows (disabled near line 408 as "TEMPORARILY DISABLED ... to debug queen blunder"); the blunder may have been fixed elsewhere
- [ ] 0002-aspiration-narrow: shrink `ASPIRATION_WINDOW` from 50 to 25 (only meaningful once 0001 is settled)
- [ ] 0003-null-move-reduction: lower `NULL_MOVE_REDUCTION` from 3 to 2, or change the `depth / 6` term in the null-move reduction
- [ ] 0004-null-move-min-depth: lower `NULL_MOVE_MIN_DEPTH` from 3 to 2
- [ ] 0005-lmr-full-depth: reduce `LMR_FULL_DEPTH_MOVES` from 4 to 3 so reductions start earlier
- [ ] 0006-lmr-divisor: change the LMR table divisor (`ln(depth) * ln(move_count) / 2.0`) to 2.25
- [ ] 0007-futility-margin: change `FUTILITY_MARGIN_BASE` from 150 to 120, and separately to 200
- [ ] 0008-reverse-futility: add static-eval beta pruning (reverse futility) at shallow depth in non-PV nodes, a technique the search does not have yet
- [ ] 0009-history-gravity: replace the history update in `src/search/history.rs` with a gravity-style update that decays old values in proportion to the bonus
- [ ] 0010-check-extension: extend the search by one ply when the side to move is in check
- [ ] 0011-tt-replacement: change the transposition table replacement scheme in `src/search/transposition.rs` to age-aware replacement
- [ ] 0012-iir: internal iterative reduction: reduce depth at nodes with no hash move
- [ ] 0013-retune-mobility: re-run `src/bin/tuner` on a larger quiet-position dataset and adopt the new mobility weights
