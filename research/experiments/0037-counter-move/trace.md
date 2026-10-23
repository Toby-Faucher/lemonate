## Summary
Wired the existing CounterMoveTable into search: store the quiet cutoff move against the opponent's previous move, and order it just below killers, to get earlier cutoffs.

## Tried
1. Added `move_stack` in SearchEngine, set before each child search and cleared (None) for null moves.
2. Stored the counter in `update_cutoff_heuristics` (quiet cutoffs only, ply > 0).
3. Added `MoveOrderer::with_counter` (`new` delegates with None, so existing tests are untouched); counter scores `scores::COUNTER_MOVE`, checked after killers.
4. `cargo test --release --lib --bins` passes.

## Dropped
- Counter-move bonus into history score or LMR adjustments: not part of the hypothesis.
- Per-thread/side-to-move indexing: the table is already per engine (per SMP worker).

## Surprises
CounterMoveTable, the COUNTER_MOVE score and the `counters` field existed but were never read or written in search. The table is keyed by piece type and to-square only, not by color, which is fine since the previous move is always the opponent's.
