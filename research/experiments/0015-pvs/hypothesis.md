# Principal variation search: scout every move after the first with a null window

## Change
In `negamax` the first move of a node is searched with the full window. Every later move is
scouted with the null window (alpha, alpha+1), reduced when LMR applies, and re-searched only if
it beats alpha: first at full depth with the null window, then with the full window if it lands
strictly inside (alpha, beta). Before, any move LMR did not reduce was searched as a PV node.

## Mechanism
Scouts are non-PV nodes, so the pruning that is switched off in PV nodes (TT cutoffs, reverse
futility, null move, futility) applies to them. With good move ordering most later moves fail
low, and a null-window search proves that with far fewer nodes than a full-window search.

## Expected Elo
Large for an engine that searched every non-reduced move at full width: +40 to +100.
