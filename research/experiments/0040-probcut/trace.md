## Summary
Added ProbCut (non-PV, not in check, depth >= 5): null-window capture searches at beta+200, depth-4; a fail high cuts the node, because a shallow search that clears beta by a margin predicts the full search will.

## Tried
1. Single implementation after null move pruning: margin 200, reduction 4, captures only, no qsearch pre-check, no SEE filter. Unit tests pass.

## Dropped
- Quiescence pre-check before the reduced search, TT-based skip, and SEE filtering of captures: extra code, and I cannot tune against matches.

## Surprises
- No CLAUDE.md exists in the repo or worktree. Captures are copied into a Vec per ProbCut call (small heap allocation at depth >= 5 only).
