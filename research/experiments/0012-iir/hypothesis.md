# Internal iterative reduction: search one ply shallower at depth>=4 nodes with no hash move

## Change
Confirmed from `negamax` in src/search/mod.rs that no such reduction exists today: after the TT probe the only uses of the hash move are move ordering and the non-PV cutoff; `depth` is never lowered for missing hash moves. Add `IIR_MIN_DEPTH = 4` next to the other search constants and make `depth` mutable in `negamax`. Right after `in_check` is computed (after the TT probe and move generation, before null move pruning and the move loop), if `!in_check && depth >= IIR_MIN_DEPTH && hash_move.is_none()` then `depth -= 1`. Applies to PV and non-PV nodes. The reduced depth is used consistently by null move, futility, LMR, move loop, history updates and TT store. Threshold 4 guarantees depth stays >= 3.

## Mechanism
Without a hash move, move ordering is poor, so a full-depth search is expensive and less likely to find a cutoff early. Searching one ply shallower is cheaper, and the result stored in the TT (with a best move) gives better ordering on later visits and iterations. PV extraction only reads best_move from the TT (never entry depth), and the TT stores the depth actually searched (depth-1), which is honest, so stored depth semantics are not broken.

## Expected Elo
Roughly +5 to +15 Elo from a lower node count at equal depth.
