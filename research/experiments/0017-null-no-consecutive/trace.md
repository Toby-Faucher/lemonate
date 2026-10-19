## Summary
No measurable effect: stopped by hand at 1,025 games, +3 Elo (95% interval roughly +/-18), with an evidence score of 0.03 against the 2.94 needed.

## Tried
1. Added an `after_null` flag per ply: the parent sets the child's flag when it reaches it through a null move, and the child consumes it on entry and skips its own null-move pruning.
2. Test that no flag outlives a search (every flag a parent sets is consumed by the child).
3. Ran the SPRT against the PVS branch (bounds 0 to 5) rather than the old main, since PVS changes what the other tweaks are worth.

## Dropped
Left unmerged. Two nulls in a row should waste nodes, but any gain is too small to separate from luck at this game budget. Why it is small was not investigated.

## Surprises
The evidence score stayed near zero at every check (0.08, 0.02, 0.03), the shape of a true effect close to zero: the test would have run to its 4,000-game cap without deciding.
