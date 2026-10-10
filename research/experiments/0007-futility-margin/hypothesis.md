# Lower FUTILITY_MARGIN_BASE from 150 to 120 for more futility pruning

## Change
`FUTILITY_MARGIN_BASE` in src/search/mod.rs: 150 -> 120. `futility_margin(depth)` returns `FUTILITY_MARGIN_BASE * depth`, so the margin goes from 150/300/450... to 120/240/360... cp at depth 1/2/3.... A move is pruned when static_eval + margin <= alpha. No other constant or condition changes.

## Mechanism
A smaller margin makes the prune condition true for more nodes (eval needs to be only 120*depth below alpha instead of 150*depth), so more quiet late moves are skipped and the tree shrinks. The engine just accepted more aggressive LMR (+19.8 Elo) and rejected an even more aggressive step (-14 Elo), so the pruning/reduction balance appears near an optimum on the aggressive side; a modest step to 120 is plausible to gain.

## Expected Elo
About +3 (range -8 to +10); a small, noisy effect.
