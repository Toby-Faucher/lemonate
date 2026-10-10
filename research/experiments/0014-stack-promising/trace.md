## Summary

Controller-assembled stack of four unresolved small gains (0001, 0003, 0008, 0012) on top of the merged LMR change; no new code beyond resolving one merge conflict.

## Tried

Cherry-picked the four experiment commits with `-x`. 0001, 0003 and 0008 applied cleanly. 0012 conflicted with 0008 in `negamax` (both insert a block after `in_check` is computed); kept both blocks, reverse futility first, then internal iterative reduction, before null-move pruning.

## Dropped

Nothing.

## Surprises

None yet.
