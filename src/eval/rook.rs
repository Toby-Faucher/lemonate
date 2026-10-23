use crate::bitboard::{Bitboard, FILES, RANK_1, RANK_2};
use crate::types::Color;
use crate::{Board, PieceType};

use super::phase::GamePhase;

// Fixed (not tuner-controlled) centipawn bonuses, so `EvalWeights` is unchanged.
const OPEN_FILE_MG: i32 = 20;
const OPEN_FILE_EG: i32 = 10;
const SEMI_OPEN_FILE_MG: i32 = 10;
const SEMI_OPEN_FILE_EG: i32 = 5;
const SEVENTH_MG: i32 = 20;
const SEVENTH_EG: i32 = 30;
const RANK_7: Bitboard = Bitboard(0x00FF_0000_0000_0000);
const RANK_8: Bitboard = Bitboard(0xFF00_0000_0000_0000);

/// Side-to-move bonus, added in `Evaluator::evaluate` only.
pub const TEMPO: i32 = 10;

fn side(board: &Board, color: Color) -> (i32, i32) {
    let ours = board.piece_bitboard(color, PieceType::Pawn);
    let theirs = board.piece_bitboard(color.opposite(), PieceType::Pawn);
    let (seventh, back) = if color == Color::White { (RANK_7, RANK_8) } else { (RANK_2, RANK_1) };
    let enemy_king = board.piece_bitboard(color.opposite(), PieceType::King);
    let seventh_matters = !(theirs & seventh).is_empty() || !(enemy_king & back).is_empty();
    let (mut mg, mut eg) = (0, 0);
    for sq in board.piece_bitboard(color, PieceType::Rook) {
        let file = FILES[sq.file() as usize];
        if (ours & file).is_empty() {
            if (theirs & file).is_empty() {
                mg += OPEN_FILE_MG;
                eg += OPEN_FILE_EG;
            } else {
                mg += SEMI_OPEN_FILE_MG;
                eg += SEMI_OPEN_FILE_EG;
            }
        }
        if seventh_matters && !(Bitboard(1u64 << sq.index()) & seventh).is_empty() {
            mg += SEVENTH_MG;
            eg += SEVENTH_EG;
        }
    }
    (mg, eg)
}

/// Rook placement term, White's point of view, tapered by `phase`.
pub fn evaluate(board: &Board, phase: i32) -> i32 {
    let (wm, we) = side(board, Color::White);
    let (bm, be) = side(board, Color::Black);
    GamePhase::new().taper(wm - bm, we - be, phase)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(fen: &str) -> i32 {
        evaluate(&Board::from_fen(fen).unwrap(), 24)
    }

    #[test]
    fn open_file_rook_scores_and_closed_does_not() {
        assert_eq!(score("4k3/8/8/8/8/8/8/R3K3 w - - 0 1"), OPEN_FILE_MG);
        assert_eq!(score("4k3/p7/8/8/8/8/P7/R3K3 w - - 0 1"), 0);
        // Only the enemy has a pawn on the file: semi-open.
        assert_eq!(score("4k3/p7/8/8/8/8/8/R3K3 w - - 0 1"), SEMI_OPEN_FILE_MG);
    }

    #[test]
    fn seventh_rank_rook_is_mirrored() {
        let w = score("4k3/R7/8/8/8/8/8/4K3 w - - 0 1");
        let b = score("4k3/8/8/8/8/8/r7/4K3 w - - 0 1");
        assert_eq!(w, -b);
        assert_eq!(w, OPEN_FILE_MG + SEVENTH_MG);
    }
}
