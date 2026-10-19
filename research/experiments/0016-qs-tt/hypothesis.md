# Probe and store the transposition table in quiescence

## Change
`quiescence` probes the table on entry (exact scores, or bounds that already decide the window)
and stores its own result at depth 0: a lower bound on stand-pat and capture cutoffs, an exact
value or an upper bound at the end. Mate-range scores are not stored. Stores use `store_qs`,
which only takes slots that are empty, stale or depth 0, so quiescence cannot evict the deeper
entries the main search relies on.

## Mechanism
The same position is reached through many capture orders and by many main-search nodes.
Reusing the result saves repeated quiescence trees, at the price of a table access per node.

## Expected Elo
Small to moderate: +5 to +25 if the cache hit rate is high enough to pay for the memory access.
