use crate::Board;

mod material;
mod pawn_structure;
mod phase;
mod pst;

pub use material::MaterialEvaluator;
pub use pawn_structure::PawnStructureEval;
pub use phase::GamePhase;
pub use pst::PieceSquareTableEval;

pub struct Evaluator {
    material: MaterialEvaluator,
    pst: PieceSquareTableEval,
    phase: GamePhase,
    pawn_structure: PawnStructureEval,
    // king_safety: KingSafetyEval,        // TODO: Implement
    // mobility: MobilityEval,             // TODO: Implement
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            material: MaterialEvaluator::new(),
            pst: PieceSquareTableEval::new(),
            phase: GamePhase::new(),
            pawn_structure: PawnStructureEval::new(),
        }
    }

    pub fn evaluate(&self, board: &Board) -> i32 {
        let pst_score = self.pst.evaluate(board);
        let pawn_score = self.pawn_structure.evaluate(board);
        pst_score + pawn_score
    }

    pub fn evaluate_detailed(&self, board: &Board) -> EvalDetails {
        EvalDetails {
            pst: self.pst.evaluate(board),
            pawn_structure: self.pawn_structure.evaluate(board),
            phase: self.phase.calculate(board),
        }
    }
}

pub struct EvalDetails {
    pub pst: i32,
    pub pawn_structure: i32,
    pub phase: i32,
}

impl EvalDetails {
    pub fn total(&self) -> i32 {
        self.pst + self.pawn_structure
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
