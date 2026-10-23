# History-scaled late-move reductions: reduce high-history quiets less, low-history quiets more

## Change

In the LMR branch of `negamax`, adjust the reduction by the quiet move's history score (read before the move is made): -1 when history > 4096, +1 when history < -4096 (MAX_HISTORY_SCORE is 16384). Reduction is clamped to [1, depth-1] so a reduced move is never searched below depth 1 more than before. The PV/cut-node tweak is left out: the engine has no cut-node flag and PVS scouts are all non-PV.

## Mechanism

History records which quiets caused beta cutoffs and which failed. A quiet with a strong record is more likely to be good, so a shallower reduction avoids re-searches and misses; one with a bad record is likely refuted, so a deeper reduction saves nodes. Ordering alone only places the move; this feeds the same signal into depth.

## Expected Elo

+10 to +20 (standard history-LMR gain; small because the history table is a plain from/to table).
