## Summary
Added internal iterative reduction in `negamax`: when not in check, depth >= IIR_MIN_DEPTH (4) and the TT probe gave no hash move, depth -= 1 (PV and non-PV alike), expecting fewer nodes at equal depth with ordering fed back through the TT.

## Tried
1. Confirmed no existing IIR in `negamax`. Made `depth` a `mut` parameter and added the reduction right after `in_check` is computed (after TT probe and move generation, before null move pruning), so null move, futility, LMR, history updates and the TT store all use the reduced depth consistently.
2. `cargo test --release --lib --bins`: 200 + 14 tests passed.
3. UCI `go depth 12` (single thread, default hash): startpos 133466 nodes, bestmove g1f3, score cp 1; kiwipete 654848 nodes, bestmove e2a6, score cp -8. No divergence (sensible moves and scores). No baseline run was made for comparison.

## Dropped
Placing IIR after null move pruning, or limiting to PV nodes: not done, per the one-change, no-tuning brief.

## Surprises
PV extraction only reads best_move from the TT and never entry depth, so the reduced stored depth does not affect it. The "no hash move" test uses hash_move, so a TT hit entry without best_move (e.g. all-fail upper bound) also triggers IIR.
