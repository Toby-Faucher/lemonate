# ProbCut: in non-PV nodes at depth >= 5, a reduced search of captures at beta+margin cuts when it fails high

## Change

In `negamax`, after null move pruning, in non-PV, not-in-check nodes at depth >= 5 with a non-mate beta: try each capture with a null-window search at `beta + 200`, depth `depth - 4`. If any returns >= `beta + 200`, return that score (the node fails high).

## Mechanism

A shallow search that beats beta by a clear margin almost always means the full-depth search beats beta too. Cutting there skips the whole move loop at expensive nodes. Captures are the moves most likely to produce such a swing, so only they are tried, which keeps the added cost small.

## Expected Elo

+10 to +20 at short time controls; small gains at best because null move and reverse futility already prune most of these nodes.
