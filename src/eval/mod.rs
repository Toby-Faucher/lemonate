use crate::Board;

mod material;
pub use material::MaterialEvaluator;

pub struct Evaluator {
    material: MaterialEvaluator,
    pst: PieceSquareTableEval,
    pawn_structure: PawnStructureEval,
    king_safety: KingSafetyEval,
    mobility: MobilityEval,
}

pub fn evaluate(board: &Board) -> i32 {
    unimplemented!()
}
