# Re-enable aspiration windows (depth >= 5, non-mate previous score)

## Change
Iterative deepening calls aspiration_search again from depth 5 using the previous iteration's score (not for mate scores). The re-search logic is made sound: fail-low pulls beta toward alpha, fail-high widens beta, delta doubles, and the window opens fully past delta 200 or when a bound reaches mate range.

## Mechanism
A narrow root window around the expected score yields more beta cutoffs and fewer nodes per iteration, so more depth in the same time. Shallow iterations stay full-window.

## Expected Elo
+10 to +30
