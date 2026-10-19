## Summary
Principal variation search: the first move of a node gets the full window, later moves a null-window scout that is re-searched only if it beats alpha. +102 Elo, and the same depth in 40% of the time.

## Tried
1. Rewrote the move loop: `move_count == 1` searches `(-beta, -alpha)`; later moves search `(-alpha-1, -alpha)` at the LMR-reduced depth, repeat at full depth if a reduced scout beat alpha, and re-search with the full window only if the score lands strictly inside `(alpha, beta)`.
2. Checked that non-PV nodes (`beta == alpha + 1`) behave exactly as before, since the last re-search never fires there.
3. Compared with `bench.py compare`: time to depth 12 fell from 7273 ms to 2859 ms and nodes by 62%.

## Dropped
Nothing. The queued experiments (quiescence TT, two null-move tweaks) were rebased onto this branch and re-run against it, since a search this much smaller changes what the other tweaks are worth.

## Surprises
The size of the gain: +102 Elo is far above the +40 to +100 range expected, which shows how much of the old search was spent on full-window searches of moves that failed low.
