# Rook open/semi-open file, rook on the 7th, and a tempo bonus in the eval

## Change

New `src/eval/rook.rs`: per rook, +MG/EG bonus on an open file (no pawns) or semi-open file (no own pawn), and a bonus on the 7th rank (2nd for Black) when enemy pawns sit there or the enemy king is on the back rank. `Evaluator::evaluate` also adds a fixed side-to-move tempo bonus. The constants are fixed (not in `EvalWeights`), so the tuner vector is unchanged. No bishop pair (that is 0035).

## Mechanism

The eval has mobility but no notion that rooks want open files or the 7th, so it undervalues rook activity the PST only crudely captures. A tempo bonus models the side-to-move advantage and stabilises the score between iterations.

## Expected Elo

+5 to +15 Elo. Small, eval-only terms, with untuned weights and a slight nps cost.
