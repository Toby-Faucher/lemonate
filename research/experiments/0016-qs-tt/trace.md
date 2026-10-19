## Summary
Quiescence probes and stores the transposition table, with stores that cannot evict deeper entries: +30.7 Elo on top of PVS.

## Tried
1. Probe on entry to `quiescence` (exact scores, or bounds that already decide the window); store a lower bound on stand-pat and capture cutoffs and an exact value or upper bound at the end. Mate-range scores are skipped because quiescence has no mate distance.
2. Added `store_qs` to both tables. The normal store replaces any colliding entry, and quiescence nodes outnumber interior nodes, so using it would evict the deeper entries the main search depends on; `store_qs` only takes slots that are empty, stale or depth 0.
3. Three table tests (does not evict a deeper entry, replaces empty/stale/shallow slots, keeps the existing best move for the same position).
4. Rebased onto the PVS branch and ran the SPRT against it, because PVS shrinks the search enough to change what a quiescence cache is worth.

## Dropped
Storing quiescence best moves: a capture that is best in quiescence is a poor hash move for a main-search node at the same position.

## Surprises
The early estimate (+89 after 8 games, +45 after 285) fell to +25 around 1,600 games before the SPRT settled at +30.7, a reminder of how little a few hundred games say.
