## Summary
Re-enabled aspiration windows (depth >= 5, non-mate previous score) with a sound widening loop, to cut nodes per iteration and gain depth.

## Tried
1. Read iterative_deepening/aspiration_search: the call was simply disabled with a full-window search; the old function was unused. No concrete cause of the queen blunder found in the window code itself; the old loop had weaknesses (beta not pulled in on fail-low, no guard against a full window re-looping, bounds could sit in mate range).
2. Rewrote aspiration_search widening, added ASPIRATION_MIN_DEPTH = 5, skipped for mate scores, and made a full window terminate the loop. cargo test --release --lib --bins: all pass; no tests changed.

## Dropped
Tuning ASPIRATION_WINDOW (separate backlog item).

## Surprises
The root is always a PV node (beta-alpha > 1), so TT cutoffs cannot hide a fail at the root. No real bug explaining the queen blunder was found; it may be unrelated or fixed elsewhere. The hypothesis/CATALOG files live in the perf-run tree, not the experiment worktree.
