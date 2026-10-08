# Self-test: capping the search depth at 4 must lose clearly

## Change
Cap iterative deepening at depth 4 (`max_depth.unwrap_or(MAX_DEPTH).min(4)`).

## Mechanism
A depth-4 search is far shallower than what 10+0.1 allows, so the engine plays much weaker.

## Expected Elo
Strongly negative (hundreds of Elo); the gate must reject it within a few hundred games.
