# Unify TT depth and age into one replacement score (depth + 2*age_distance >= existing depth)

## Change
In both TranspositionTable::store and SharedTT::store, replace the separate OR-conditions `existing.age != current_age || depth >= existing.depth` with a single rule: replace if the slot is empty, the hash differs, or `depth + AGE_WEIGHT * age_distance >= existing.depth`, with AGE_WEIGHT = 2 and age_distance = current_age.wrapping_sub(existing.age) (u8, wrap-safe). Same-position handling (best move preservation, no exact-bound special case) is unchanged. Today any entry from an older search is always replaced, even a much deeper one; now an entry that is 1 search old and 5+ plies deeper than the new store survives, while age 0 behaves exactly as before.

## Mechanism
Deep entries from the previous move's search (common in game play, where the next search revisits the same subtree) are no longer clobbered by shallow stores of the same position, and they still age out gradually: each search of staleness is worth 2 plies of depth. This keeps valuable deep data longer without letting stale entries squat forever.

## Expected Elo
Small positive, roughly +0 to +5 Elo; effect is mostly at larger hash pressure and longer games.
