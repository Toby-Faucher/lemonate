## Summary
Added a one-ply check extension in negamax (non-root, ply < MAX_DEPTH): in_check is hoisted above the leaf test and depth is bumped by 1, so forcing check lines are not cut off at the horizon.

## Tried
1. Confirmed there was no extension logic in negamax. 2. Hoisted `in_check` to the top of negamax and extended depth before leaf/TT/pruning logic. 3. cargo test --release --lib --bins: 200 + 14 pass. 4. UCI go depth 12 (release build): startpos 122k nodes, kiwipete 2.2M nodes (7.6s, seldepth 37), CPW pos 3 288k nodes, back-rank mate-in-1 332k nodes. No panics, no runaway counts.

## Dropped
Extending only the child search of checking moves: needs a post-move is_in_check per move and does not cover evasions at depth 0; node-level extension is simpler. No extension cap parameters (no tuning).

## Surprises
Mate-in-1 position at depth 12 still takes 332k nodes (iterative deepening continues past the mate). Kiwipete seldepth reaches 37, as expected with check extensions plus quiescence.
