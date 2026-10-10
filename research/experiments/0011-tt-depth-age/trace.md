## Summary
Unified TT replacement into `depth + 2*age_distance >= existing.depth` (plus empty / different-hash) in both TranspositionTable and SharedTT, so deep entries from a recent search are no longer always overwritten.

## Tried
1. Added const AGE_WEIGHT = 2 and wrap-safe `age_distance` helper; replaced the age/depth OR-conditions in both store() functions.
2. Updated two tests (test_replacement_policy_age, test_shared_tt_age_and_clear) which pinned "older entry always replaced": with depth 10 stored then one new_search, a depth-3 store gives 3+2=5 < 10, so the old entry (score 100) is now kept (exact assertion); after 3 more new_search (age distance 4), 3+8=11 >= 10 and it is replaced (score 200). Added test_age_distance_wraps.
3. cargo test --release --lib --bins: all pass.

## Dropped
No exact-bound protection for same-position updates (existing code has none; mirrored as instructed). No AGE_WEIGHT tuning.

## Surprises
Existing code already replaced on equal depth at the same age; age distance 0 is therefore identical to the old behaviour.
