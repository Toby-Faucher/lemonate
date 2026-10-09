# Triangular history bonus depth*(depth+1)/2 instead of depth*depth

## Change
In src/search/history.rs, `history_bonus(depth)` goes from `depth * depth` to `depth * (depth + 1) / 2`. Nothing else changes (gravity constant, clamp MAX_HISTORY_SCORE = 16384, malus rule which reuses the same formula, aging).

## Mechanism
Update is `h += b - h*|b|/16384`, clamped to +-16384. With b = d^2, a single deep cutoff (d >= 10, b >= 100) moves the entry noticeably and at d near 128 b alone equals the clamp, so a few deep cutoffs/maluses saturate entries and wash out earlier information. The triangular bonus is about half of d^2 at large depth (d=12: 78 vs 144; d=5: 15 vs 25) while keeping the same ordering in depth, so entries saturate more slowly and retain finer resolution between moves; the malus shrinks identically so bonus/malus balance is preserved. Safety: max b at MAX_DEPTH=128 is 128*129/2 = 8256 <= 16384 (old formula reached exactly 16384); max intermediate h*|b| = 16384*8256 ~ 1.35e8, far below i32::MAX (2.1e9), and 128*129 = 16512 does not overflow either. Checked by arithmetic and by the existing clamp test.

## Expected Elo
Roughly 0 to +5 Elo; small, within noise at short time controls.
