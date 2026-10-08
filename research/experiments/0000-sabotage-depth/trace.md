## Summary

A search-depth cap of 4 was rejected at the test stage: test_four_threads_fixed_depth_completes expects a depth-5 search to reach depth 5.

## Tried

Capped iterative deepening at depth 4 (`max_depth.unwrap_or(MAX_DEPTH).min(4)`).

## Dropped

Depth 4 as the sabotage, because it fails a unit test and never reaches the match; see 0000-sabotage-depth5.

## Surprises

None. The gate rejecting it at the test stage, before any match, is the intended behavior.
