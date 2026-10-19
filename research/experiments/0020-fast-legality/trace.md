## Summary
Pinned pieces are computed once per node, so legal moves by unpinned non-king pieces (when not in check) skip the make/unmake test: same search tree, 1.38x faster.

## Tried
1. Profiled a fixed-depth search with callgrind: 49% of instructions in legal move generation, 29% of them in the per-move do/undo.
2. Added `legality_context` (in check or no king -> test everything; otherwise the pinned set from rook/bishop x-rays) and `is_legal_with`, which keeps the make-and-test path for king moves, en passant and pinned pieces.
3. Compared perft(4) with the old implementation on 3,971 random positions (identical), and the fast filter against the make-a-copy reference on 17 hand-picked positions.

## Dropped
Caching `is_in_check` per move and skipping the static eval when in check: small, and they touch the move loop that PVS also edits.

## Surprises
The gain is larger than the instruction share suggested: 7282 ms to 5280 ms for the benchmark suite, which the SPRT valued at +54 Elo.
