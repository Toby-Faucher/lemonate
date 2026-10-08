# Start null-move pruning one ply shallower (NULL_MOVE_MIN_DEPTH 3 -> 2)

## Change
In src/search/mod.rs, lower `NULL_MOVE_MIN_DEPTH` from 3 to 2. Nothing else changes (NULL_MOVE_REDUCTION stays 3).

## Mechanism
Null-move pruning now also fires at depth-2 nodes (non-PV, not in check, static_eval >= beta, side has a non-pawn piece). These are plentiful, so cheap fail-high cutoffs happen earlier. Safety: the null search depth is `depth - 1 - (3 + depth/6)`; at depth 2 that is -2 (already -1 at depth 3, which is existing behaviour). negamax handles `depth <= 0` by dropping into quiescence before any node accounting, so a negative depth is not a problem; the null search at depth 2 is effectively a qsearch with a null-move-null window. In-check is excluded, can_null_move requires a knight/bishop/rook/queen (pawn-only zugzwang guard), and mate scores from the null search return beta instead of the mate score. Risk: slightly more zugzwang/tactical misses at low depth.

## Expected Elo
About +3 (range -5 to +10); likely small, since depth-2 nodes were already cheap.
