# Mate distance pruning

## Change
Clamp alpha and beta to the best and worst mate scores reachable at the current ply, and cut off when the window closes.

## Mechanism
A node cannot score better than mating now or worse than being mated now, so shorter mates are found without searching longer lines.
