# Self-test: capping the search depth at 5 must lose clearly

## Change
Cap iterative deepening at depth 5 (`max_depth.unwrap_or(MAX_DEPTH).min(5)`). Depth 4 was tried first and
was rejected at the test stage (`test_four_threads_fixed_depth_completes` expects depth 5), so depth 5 is the
smallest cap that passes the unit tests and reaches the match.

## Mechanism
A depth-5 search is far shallower than what 10+0.1 allows (the engine normally reaches depth 14+), so it plays much weaker.

## Expected Elo
Strongly negative; the gate must reject it via the SPRT (H0 accepted).
