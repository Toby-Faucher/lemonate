# Continuation history for quiet move ordering

## Change
Keep a history table indexed by the previous move's piece and target square, and add its score when ordering quiet moves.

## Mechanism
A quiet move that worked after the same opponent move earlier in the search is likely to work again.
