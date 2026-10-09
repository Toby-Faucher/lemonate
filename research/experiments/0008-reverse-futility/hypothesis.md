# Reverse futility pruning (static-eval beta cutoff) at depth <= 6 in non-PV nodes

## Change
Confirmed by grep of src/search/mod.rs that no static-eval based beta cutoff exists (static_eval is used only for the null-move gate `static_eval >= beta` and for futility pruning against alpha). Add `REVERSE_FUTILITY_MARGIN = 100` and, in `negamax` right after `static_eval`/`in_check` are computed and before the null-move block, return early when: non-PV, not in check, 1 <= depth <= 6, beta not in mate range, side has non-pawn material (reusing `can_null_move`), and `static_eval - 100*depth >= beta`. Returns the fail-soft value `static_eval - margin`, which is >= beta and carries more information than `beta` while staying a sound lower bound under the pruning assumption.

## Mechanism
At shallow depth a position whose static eval exceeds beta by a depth-scaled margin will almost certainly still fail high after a real search, so the whole subtree (move generation, ordering, null move) is skipped. This saves nodes, reaching greater depth in the same time.

## Expected Elo
+30 to +60
