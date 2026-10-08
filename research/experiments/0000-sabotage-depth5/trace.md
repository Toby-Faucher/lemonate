## Summary

A search-depth cap of 5 was rejected by the SPRT (H0) after 67 games: 0 wins, 47 losses, 20 draws, about -302 Elo. This is the self-test showing the gate rejects a clearly worse engine.

## Tried

Capped iterative deepening at depth 5 (`max_depth.unwrap_or(MAX_DEPTH).min(5)`), the smallest cap that passes the unit tests.

## Dropped

Nothing.

## Surprises

None. LLR reached -2.98 (lower bound -2.94) within 67 games.
