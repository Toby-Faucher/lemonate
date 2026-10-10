# Extend the search by one ply when the side to move is in check (non-root)

## Change
Confirmed from src/search/mod.rs: `negamax` has no extension logic today (no `extension`/`extend` anywhere; only quiescence resolves captures, and in-check nodes at depth 0 drop straight into quiescence). Change: compute `in_check` at the top of `negamax` (before the `depth <= 0` leaf test) and, for non-root nodes with `ply < MAX_DEPTH`, do `depth += 1`. The later duplicate `board.is_in_check()` call is replaced by the hoisted variable. Doing it before the leaf test, TT probe and pruning means the TT depth comparison, null move, futility, and LMR all see the extended depth.

## Mechanism
Ply is not changed by the extension, so PV/killer/history/move_buffers indexing (all keyed by ply, bounded by the ply >= MAX_DEPTH guard) cannot go out of bounds. Depth is never increased beyond root depth + 1 along a path (a check node does -1 for the child then +1), so the extension cannot explode depth and TT `depth as u8` casts stay in range. Mate scores use ply, unaffected. Null move, futility and reverse futility pruning are already skipped when in check; LMR excludes in-check nodes and checking moves. Forcing check sequences are searched deeper instead of hitting the horizon (and being handed to quiescence while in check), improving tactical accuracy.

## Expected Elo
+10 to +25 (standard check extension gain; some node cost).
