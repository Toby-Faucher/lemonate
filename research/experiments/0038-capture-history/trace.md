## Summary
Added a [color][attacker][to][captured] capture history, added to the MVV-LVA capture score (/64), bonused on capture cutoffs and maluses for failed captures, to try proven captures first and cut nodes.

## Tried
1. Table in `HistoryTable` (gravity update, aged with the rest, cleared on new game).
2. Capture ordering: provisional score = GOOD_CAPTURE + MVV-LVA + capthist/64; SEE still resolves lazily, good captures keep that score, bad ones drop to BAD_CAPTURE + SEE.
3. In `negamax`, track captures that failed (score <= alpha); on any beta cutoff, bonus the cutoff capture (if it was one) and penalise the failed captures (also on quiet cutoffs).
4. `cargo test --release --lib --bins` passes.

## Dropped
- Capture history in quiescence: not asked, and qsearch has its own ordering path; skipped to keep the diff small.
- Tuning the divisor (64) and a separate history for bad captures: no way to measure without the gate.

## Surprises
- No CLAUDE.md exists in the worktree or at the repo HEAD.
- Because the provisional capture score is already the final good-capture score, the SEE resolution no longer recomputes it.
