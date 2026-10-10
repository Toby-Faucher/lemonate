# Lower null-move base reduction from 3 to 2 (R = 2 + depth/6)

## Change
`NULL_MOVE_REDUCTION` in src/search/mod.rs: 3 -> 2. The null search depth becomes `depth - 1 - (2 + depth/6)` instead of `depth - 1 - (3 + depth/6)`. The `depth / 6` term, NULL_MOVE_MIN_DEPTH and all other parameters are unchanged.

## Mechanism
The total reduction was depth-4-depth/6, which is deep for a conventional R=2..3 scheme. Disabling null move entirely did not lose strength here, which suggests the verification search is too shallow and cuts off wrongly (zugzwang, tactical threats). Reducing one ply less makes the null search more accurate and cuts fewer lines incorrectly. The chosen variant is the smallest change in the less-aggressive direction and keeps the depth-scaled term, so null move stays cheap at high depth.

## Expected Elo
+3 to +10 at 10+0.1; the null-move-disabled result gave +9 +- 7.7, so some headroom from a less aggressive reduction is plausible.
