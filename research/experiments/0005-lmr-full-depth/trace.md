## Summary
LMR_FULL_DEPTH_MOVES 4 -> 3 so reductions begin at the 4th move instead of the 5th, saving nodes on late, rarely-best moves.

## Tried
1. Checked the comparison: `move_count > LMR_FULL_DEPTH_MOVES` with 1-based move_count, so the constant is the count of full-depth moves.
2. Changed the constant to 3. Tests: 200 lib + 14 bin tests pass, none pinned the old value.

## Dropped
Nothing; the LMR divisor and table were deliberately left alone (separate backlog item).

## Surprises
None. The LMR table test uses its own reference formula and does not reference the constant.
