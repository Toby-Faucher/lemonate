# Late move pruning: skip late quiet moves at depth <= 4

## Change
In non-PV nodes at depth <= 4, once LMP_BASE + depth^2 quiet moves have been tried, the remaining quiet moves are skipped (not when in check, not for captures or promotions).

## Mechanism
Late quiet moves almost never raise alpha after good ordering; skipping them saves nodes that the extra depth then reinvests.
