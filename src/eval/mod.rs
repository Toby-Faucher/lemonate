use crate::Board;

mod material;
mod phase;
mod pst;

pub use material::MaterialEvaluator;
pub use phase::GamePhase;
pub use pst::PieceSquareTableEval;

// TODO: remove this
pub struct PawnStructureEval;

pub struct Evaluator {
    material: MaterialEvaluator,
    pst: PieceSquareTableEval,
    phase: GamePhase,
    pawn_structure: PawnStructureEval,
    // king_safety: KingSafetyEval,  // TODO: Implement
    // mobility: MobilityEval,        // TODO: Implement
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            material: MaterialEvaluator::new(),
            pst: PieceSquareTableEval::new(),
            phase: GamePhase::new(),
            pawn_structure: PawnStructureEval,
        }
    }

    pub fn evaluate(&self, board: &Board) -> i32 {
        // Material is included in PST evaluation
        self.pst.evaluate(board)
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

pub fn evaluate(board: &Board) -> i32 {
    let evaluator = Evaluator::new();
    evaluator.evaluate(board)
}
