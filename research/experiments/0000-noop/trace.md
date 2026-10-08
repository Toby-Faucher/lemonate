## Summary

Empty patch: no gain detected (+2.5 ± 7.5 Elo over 4000 games, LLR 0.006), so the gate correctly did not accept it.

## Tried

Ran the unmodified baseline as the candidate through the full gate on the chess-test container.

## Dropped

Nothing.

## Surprises

The SPRT (elo0=0, elo1=5) does not finish for a true-zero change: it hits the 4000-game cap and reports `inconclusive`, never `accepted`.
