# Self-test: disabling null-move pruning must lose

## Change
Set NULL_MOVE_MIN_DEPTH to 99 so null-move pruning never fires.

## Mechanism
Removing a major pruning technique makes the search shallower at equal time.

## Expected Elo
Strongly negative.
