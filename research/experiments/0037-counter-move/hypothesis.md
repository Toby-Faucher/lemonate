# Wire in the unused CounterMoveTable: store on quiet beta cutoffs, rank in move ordering

## Change
Track the move played at each ply (`move_stack`, None after a null move). On a quiet beta cutoff, store the cutting move as the counter to the opponent's previous move. In move ordering, a quiet move equal to the counter move scores `COUNTER_MOVE` (below killers, above history).

## Mechanism
A quiet refutation of a given opponent move (piece type, to-square) tends to repeat across siblings. Ranking it right after the killers gets cutoffs earlier at nodes where the killers miss, so fewer moves are searched before the cutoff.

## Expected Elo
+5 to +15; counter moves are a standard small gain on top of killers and history.
