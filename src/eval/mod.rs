use crate::types::Color;
use crate::Board;

mod king_safety;
mod material;
mod mobility;
mod pawn_structure;
mod phase;
mod pst;

pub use king_safety::KingSafetyEval;
pub use material::MaterialEvaluator;
pub use mobility::MobilityEval;
pub use pawn_structure::PawnStructureEval;
pub use phase::GamePhase;
pub use pst::PieceSquareTableEval;

pub struct Evaluator {
    material: MaterialEvaluator,
    pst: PieceSquareTableEval,
    phase: GamePhase,
    pawn_structure: PawnStructureEval,
    king_safety: KingSafetyEval,
    mobility: MobilityEval,
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            material: MaterialEvaluator::new(),
            pst: PieceSquareTableEval::new(),
            phase: GamePhase::new(),
            pawn_structure: PawnStructureEval::new(),
            king_safety: KingSafetyEval::new(),
            mobility: MobilityEval::new(),
        }
    }

    pub fn evaluate(&self, board: &Board) -> i32 {
        let pst_score = self.pst.evaluate(board);
        let pawn_score = self.pawn_structure.evaluate(board);
        let king_score = self.king_safety.evaluate(board);
        let mobility_score = self.mobility.evaluate(board);
        let score = pst_score + pawn_score + king_score + mobility_score;

        // Return score from side-to-move's perspective for negamax
        if board.side_to_move() == Color::White {
            score
        } else {
            -score
        }
    }

    pub fn evaluate_detailed(&self, board: &Board) -> EvalDetails {
        EvalDetails {
            pst: self.pst.evaluate(board),
            pawn_structure: self.pawn_structure.evaluate(board),
            king_safety: self.king_safety.evaluate(board),
            mobility: self.mobility.evaluate(board),
            phase: self.phase.calculate(board),
        }
    }
}

pub struct EvalDetails {
    pub pst: i32,
    pub pawn_structure: i32,
    pub king_safety: i32,
    pub mobility: i32,
    pub phase: i32,
}

impl EvalDetails {
    pub fn total(&self) -> i32 {
        self.pst + self.pawn_structure + self.king_safety + self.mobility
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
