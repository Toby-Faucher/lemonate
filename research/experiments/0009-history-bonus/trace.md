## Summary
history_bonus changed from depth*depth to depth*(depth+1)/2 so deep cutoffs/maluses saturate history entries less quickly (max 8256 at depth 128, half the clamp).

## Tried
1. Read update_cutoff/update_penalty: h += b - h*|b|/16384, clamp +-16384; malus reuses history_bonus. Wrote hypothesis, made the one-line change.
2. Updated test_history_update_cutoff: expected 25 -> 15 (5*6/2). This mirrors the new formula for depth 5 from zero state; the assertion is not weakened (still exact equality).
3. cargo test --release --lib --bins: all pass.

## Dropped
Other formulas (e.g. min(d*d, cap), linear d*300-250) were not tried; the task allowed exactly one change.

## Surprises
Old formula reached exactly 16384 (the clamp) at MAX_DEPTH 128, so a single bonus could saturate an entry. Overflow check: max h*|b| = 16384*8256 ~ 1.35e8 < i32::MAX.
