## Summary
Lowered FUTILITY_MARGIN_BASE 150 -> 120 (margin = base * depth) for more futility pruning, following the LMR result that aggressive pruning/reduction is near optimum.

## Tried
1. Read futility_margin (base * depth, used once at src/search/mod.rs:642).
2. Changed the constant to 120. Tests: all pass, no test pinned the old value.

## Dropped
Nothing; only the single value 120 was in scope.

## Surprises
None. The margin is purely linear in depth, with a single use site.
