## Summary

Disabling null-move pruning (NULL_MOVE_MIN_DEPTH=99) did not lose measurably (+9 ± 7.7 Elo over 4000 games, inconclusive), so it is not a usable sabotage; the gate was right not to accept it.

## Tried

Set NULL_MOVE_MIN_DEPTH to 99. Confirmed the patch is real: at fixed depth 9 from the start position the node count rose from 23,588 to 41,184 (+75%) and the principal variation changed.

## Dropped

Using null-move removal as the "must lose" self-test; replaced by the depth-cap sabotages (0000-sabotage-depth, 0000-sabotage-depth5).

## Surprises

In this engine, null-move pruning gives no measurable strength at 10+0.1 despite cutting nodes by ~43%. That is an engine finding worth a backlog item (check the null-move conditions and reduction).
