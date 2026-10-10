## Summary
Lowered NULL_MOVE_REDUCTION from 3 to 2 (R = 2 + depth/6) so the null search is one ply deeper and prunes less wrongly.

## Tried
1. Chose base 3 -> 2 with depth/6 kept; ran `cargo test --release --lib --bins` (200 + 14 passed, no test pinned the old value).

## Dropped
- Larger divisor for depth term: a second knob with less clear direction; kept to a single change.
- Touching NULL_MOVE_MIN_DEPTH: separate backlog item.

## Surprises
Total null-search reduction was depth-4-depth/6 (the constant plus the extra 1 from depth-1), deeper than it looks.
