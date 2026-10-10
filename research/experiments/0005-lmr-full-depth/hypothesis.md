# Start late-move reductions one move earlier (LMR_FULL_DEPTH_MOVES 4 -> 3)

## Change
In src/search/mod.rs, `LMR_FULL_DEPTH_MOVES` goes from 4 to 3. It is used as `move_count > LMR_FULL_DEPTH_MOVES` with `move_count` 1-based (incremented before the check), so the first N moves are searched at full depth. Reductions therefore begin at move 5 before and at move 4 after: one move earlier. LMR table formula and all other parameters are untouched.

## Mechanism
With good move ordering (TT move, captures, killers, history), the 4th move is rarely best. Reducing it saves nodes, which buys extra depth at fixed time. Moves that raise alpha are still re-searched at full depth, limiting the risk.

## Expected Elo
Small, roughly +0 to +8 Elo; could be neutral if ordering is not strong enough for the 4th move.
