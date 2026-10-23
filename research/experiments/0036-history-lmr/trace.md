## Summary
LMR reduction is now 1 ply less for quiets with history > 4096 and 1 ply more for history < -4096 (clamped to [1, depth-1]), because history predicts which late quiets turn out good.

## Tried
1. Read history, LMR and PVS code in src/search/mod.rs; wrote hypothesis.
2. Read the history score before do_move, added LMR_HISTORY_THRESHOLD = 4096 and the adjustment; `cargo test --release --lib --bins` passes.

## Dropped
- PV/cut-node tweaks: no cut-node flag exists, and PVS scouts are all non-PV, so a PV tweak would be a no-op on late moves.
- Continuous scaling (history / divisor): threshold steps are simpler and the history range is small.

## Surprises
- The old reduction could exceed depth-1 (table cap plus the !improving +1); the new clamp changes that slightly, a side effect.
- Thresholds untuned, since no matches may be run.
