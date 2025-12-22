use crate::bitboard::{Bitboard, ADJACENT_FILES, FILES};
use crate::types::{Color, Square};
use crate::Board;

use super::phase::GamePhase;

// Pawn shield bonuses (centipawns).
pub const PAWN_SHIELD_CLOSE_MG: i32 = 15;
pub const PAWN_SHIELD_CLOSE_EG: i32 = 5;
pub const PAWN_SHIELD_FAR_MG: i32 = 8;
pub const PAWN_SHIELD_FAR_EG: i32 = 2;

// Open file penalties near king.
pub const OPEN_FILE_NEAR_KING_MG: i32 = -25;
pub const OPEN_FILE_NEAR_KING_EG: i32 = -5;
pub const SEMI_OPEN_FILE_NEAR_KING_MG: i32 = -10;
pub const SEMI_OPEN_FILE_NEAR_KING_EG: i32 = -2;

pub struct KingSafetyEval {
    phase: GamePhase,
}

impl KingSafetyEval {
    pub fn new() -> Self {
        Self {
            phase: GamePhase::new(),
        }
    }

    pub fn evaluate(&self, board: &Board) -> i32 {
        let phase = self.phase.calculate(board);

        let (w_mg, w_eg) = self.evaluate_king(board, Color::White);
        let (b_mg, b_eg) = self.evaluate_king(board, Color::Black);

        let mg_score = w_mg - b_mg;
        let eg_score = w_eg - b_eg;

        self.phase.taper(mg_score, eg_score, phase)
    }

    fn evaluate_king(&self, board: &Board, color: Color) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let king_sq = self.get_king_square(board, color);

        let our_pawns = board.piece_bitboard(color, crate::PieceType::Pawn);
        let enemy_pawns = board.piece_bitboard(color.opposite(), crate::PieceType::Pawn);

        // Pawn shield evaluation.
        let (shield_mg, shield_eg) = self.evaluate_pawn_shield(king_sq, our_pawns, color);
        mg += shield_mg;
        eg += shield_eg;

        // Open files near king.
        let (files_mg, files_eg) = self.evaluate_open_files(king_sq, our_pawns, enemy_pawns);
        mg += files_mg;
        eg += files_eg;

        (mg, eg)
    }

    fn get_king_square(&self, board: &Board, color: Color) -> Square {
        let king_bb = board.piece_bitboard(color, crate::PieceType::King);

        Square::from_index(king_bb.0.trailing_zeros() as usize)
    }

    fn evaluate_pawn_shield(
        &self,
        king_sq: Square,
        our_pawns: Bitboard,
        color: Color,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let king_file = king_sq.file() as usize;
        let king_rank = king_sq.rank();

        let shield_files = FILES[king_file] | ADJACENT_FILES[king_file];
        let shield_pawns = our_pawns & shield_files;

        for pawn_sq in shield_pawns {
            let pawn_rank = pawn_sq.rank();

            let rank_diff = if color == Color::White {
                pawn_rank as i32 - king_rank as i32
            } else {
                king_rank as i32 - pawn_rank as i32
            };

            if rank_diff == 1 {
                mg += PAWN_SHIELD_CLOSE_MG;
                eg += PAWN_SHIELD_CLOSE_EG;
            } else if rank_diff == 2 {
                mg += PAWN_SHIELD_FAR_MG;
                eg += PAWN_SHIELD_FAR_EG;
            }
        }

        (mg, eg)
    }

    fn evaluate_open_files(
        &self,
        king_sq: Square,
        our_pawns: Bitboard,
        enemy_pawns: Bitboard,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let king_file = king_sq.file() as usize;

        let files_to_check = [
            king_file.saturating_sub(1),
            king_file,
            (king_file + 1).min(7),
        ];

        for &file in &files_to_check {
            let file_mask = FILES[file];
            let our_on_file = (our_pawns & file_mask).is_not_empty();
            let enemy_on_file = (enemy_pawns & file_mask).is_not_empty();

            if !our_on_file && !enemy_on_file {
                mg += OPEN_FILE_NEAR_KING_MG;
                eg += OPEN_FILE_NEAR_KING_EG;
            } else if !our_on_file {
                mg += SEMI_OPEN_FILE_NEAR_KING_MG;
                eg += SEMI_OPEN_FILE_NEAR_KING_EG;
            }
        }

        (mg, eg)
    }
}

impl Default for KingSafetyEval {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_castled_king_with_shield() {
        // White king on g1 with pawns on f2, g2, h2.
        let board = Board::from_fen("8/8/8/8/8/8/5PPP/6K1 w - - 0 1").unwrap();
        let eval = KingSafetyEval::new();
        let (mg, eg) = eval.evaluate_king(&board, Color::White);

        // Three close shield pawns.
        assert_eq!(mg, 3 * PAWN_SHIELD_CLOSE_MG);
        assert_eq!(eg, 3 * PAWN_SHIELD_CLOSE_EG);
    }

    #[test]
    fn test_open_file_penalty() {
        // White king on e1, no pawns on e-file.
        let board = Board::from_fen("8/8/8/8/8/8/PPP2PPP/4K3 w - - 0 1").unwrap();
        let eval = KingSafetyEval::new();
        let (mg, _) = eval.evaluate_king(&board, Color::White);

        // Open e-file penalty included.
        assert!(mg < 0);
    }

    #[test]
    fn test_symmetric_position() {
        let board = Board::from_fen("r3k2r/ppp2ppp/8/8/8/8/PPP2PPP/R3K2R w KQkq - 0 1").unwrap();
        let eval = KingSafetyEval::new();
        let score = eval.evaluate(&board);

        // Symmetric structure gives balanced score.
        assert!(score.abs() < 10);
    }
}
