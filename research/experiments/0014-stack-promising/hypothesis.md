# Stack the four unresolved small gains (0001, 0003, 0008, 0012) on top of the merged LMR change

## Change
Cherry-pick, in order, the candidates of 0001 (aspiration windows from depth 5), 0003 (null-move base reduction 3 -> 2), 0008 (reverse futility pruning, margin 100*depth, depth <= 6) and 0012 (internal iterative reduction at depth >= 4 without a hash move) onto the branch tip. 0012 conflicted with 0008 in `negamax`; resolved by keeping both blocks, reverse futility first, then internal iterative reduction, both before null-move pruning.

## Mechanism
Each of the four measured +7 to +9 Elo individually (LLR 1.6 to 2.3 of the 2.94 accept bound), too small for the SPRT to confirm in 4000 games. If those gains are real and roughly independent, the stack should be worth about +20 to +30 Elo and clear the bar on its own.

## Expected Elo
About +20 (less than the sum, since pruning and reduction ideas overlap). Not a combination anyone has measured: interactions between null-move, reverse futility and internal iterative reduction may eat part of the gain.
