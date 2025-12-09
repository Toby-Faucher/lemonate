// PeSTO's Evaluation Function
// Source: https://www.chessprogramming.org/PeSTO%27s_Evaluation_Function
// Values from Rofchade, tuned using Texel's tuning method

use crate::types::piece::PieceType;
use crate::types::Color;
use crate::Board;

// Material values (middlegame, endgame)
pub const MG_VALUE: [i32; 6] = [82, 337, 365, 477, 1025, 0];
pub const EG_VALUE: [i32; 6] = [94, 281, 297, 512, 936, 0];

// Game phase increments for each piece type
pub const GAMEPHASE_INC: [i32; 6] = [0, 1, 1, 2, 4, 0];

// Middlegame piece-square tables (from White's perspective, rank 8 to rank 1)
pub const MG_PAWN_TABLE: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 98, 134, 61, 95, 68, 126, 34, -11, -6, 7, 26, 31, 65, 56, 25, -20, -14,
    13, 6, 21, 23, 12, 17, -23, -27, -2, -5, 12, 17, 6, 10, -25, -26, -4, -4, -10, 3, 3, 33, -12,
    -35, -1, -20, -23, -15, 24, 38, -22, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub const MG_KNIGHT_TABLE: [i32; 64] = [
    -167, -89, -34, -49, 61, -97, -15, -107, -73, -41, 72, 36, 23, 62, 7, -17, -47, 60, 37, 65, 84,
    129, 73, 44, -9, 17, 19, 53, 37, 69, 18, 22, -13, 4, 16, 13, 28, 19, 21, -8, -23, -9, 12, 10,
    19, 17, 25, -16, -29, -53, -12, -3, -1, 18, -14, -19, -105, -21, -58, -33, -17, -28, -19, -23,
];

pub const MG_BISHOP_TABLE: [i32; 64] = [
    -29, 4, -82, -37, -25, -42, 7, -8, -26, 16, -18, -13, 30, 59, 18, -47, -16, 37, 43, 40, 35, 50,
    37, -2, -4, 5, 19, 50, 37, 37, 7, -2, -6, 13, 13, 26, 34, 12, 10, 4, 0, 15, 15, 15, 14, 27, 18,
    10, 4, 15, 16, 0, 7, 21, 33, 1, -33, -3, -14, -21, -13, -12, -39, -21,
];

pub const MG_ROOK_TABLE: [i32; 64] = [
    32, 42, 32, 51, 63, 9, 31, 43, 27, 32, 58, 62, 80, 67, 26, 44, -5, 19, 26, 36, 17, 45, 61, 16,
    -24, -11, 7, 26, 24, 35, -8, -20, -36, -26, -12, -1, 9, -7, 6, -23, -45, -25, -16, -17, 3, 0,
    -5, -33, -44, -16, -20, -9, -1, 11, -6, -71, -19, -13, 1, 17, 16, 7, -37, -26,
];

pub const MG_QUEEN_TABLE: [i32; 64] = [
    -28, 0, 29, 12, 59, 44, 43, 45, -24, -39, -5, 1, -16, 57, 28, 54, -13, -17, 7, 8, 29, 56, 47,
    57, -27, -27, -16, -16, -1, 17, -2, 1, -9, -26, -9, -10, -2, -4, 3, -3, -14, 2, -11, -2, -5, 2,
    14, 5, -35, -8, 11, 2, 8, 15, -3, 1, -1, -18, -9, 10, -15, -25, -31, -50,
];

pub const MG_KING_TABLE: [i32; 64] = [
    -65, 23, 16, -15, -56, -34, 2, 13, 29, -1, -20, -7, -8, -4, -38, -29, -9, 24, 2, -16, -20, 6,
    22, -22, -17, -20, -12, -27, -30, -25, -14, -36, -49, -1, -27, -39, -46, -44, -33, -51, -14,
    -14, -22, -46, -44, -30, -15, -27, 1, 7, -8, -64, -43, -16, 9, 8, -15, 36, 12, -54, 8, -28, 24,
    14,
];

// Endgame piece-square tables
pub const EG_PAWN_TABLE: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 178, 173, 158, 134, 147, 132, 165, 187, 94, 100, 85, 67, 56, 53, 82,
    84, 32, 24, 13, 5, -2, 4, 17, 17, 13, 9, -3, -7, -7, -8, 3, -1, 4, 7, -6, 1, 0, -5, -1, -8, 13,
    8, 8, 10, 13, 0, 2, -7, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub const EG_KNIGHT_TABLE: [i32; 64] = [
    -58, -38, -13, -28, -31, -27, -63, -99, -25, -8, -25, -2, -9, -25, -24, -52, -24, -20, 10, 9,
    -1, -9, -19, -41, -17, 3, 22, 22, 22, 11, 8, -18, -18, -6, 16, 25, 16, 17, 4, -18, -23, -3, -1,
    15, 10, -3, -20, -22, -42, -20, -10, -5, -2, -20, -23, -44, -29, -51, -23, -15, -22, -18, -50,
    -64,
];

pub const EG_BISHOP_TABLE: [i32; 64] = [
    -14, -21, -11, -8, -7, -9, -17, -24, -8, -4, 7, -12, -3, -13, -4, -14, 2, -8, 0, -1, -2, 6, 0,
    4, -3, 9, 12, 9, 14, 10, 3, 2, -6, 3, 13, 19, 7, 10, -3, -9, -12, -3, 8, 10, 13, 3, -7, -15,
    -14, -18, -7, -1, 4, -9, -15, -27, -23, -9, -23, -5, -9, -16, -5, -17,
];

pub const EG_ROOK_TABLE: [i32; 64] = [
    13, 10, 18, 15, 12, 12, 8, 5, 11, 13, 13, 11, -3, 3, 8, 3, 7, 7, 7, 5, 4, -3, -5, -3, 4, 3, 13,
    1, 2, 1, -1, 2, 3, 5, 8, 4, -5, -6, -8, -11, -4, 0, -5, -1, -7, -12, -8, -16, -6, -6, 0, 2, -9,
    -9, -11, -3, -9, 2, 3, -1, -5, -13, 4, -20,
];

pub const EG_QUEEN_TABLE: [i32; 64] = [
    -9, 22, 22, 27, 27, 19, 10, 20, -17, 20, 32, 41, 58, 25, 30, 0, -20, 6, 9, 49, 47, 35, 19, 9,
    3, 22, 24, 45, 57, 40, 57, 36, -18, 28, 19, 47, 31, 34, 39, 23, -16, -27, 15, 6, 9, 17, 10, 5,
    -22, -23, -30, -16, -16, -23, -36, -32, -33, -28, -22, -43, -5, -32, -20, -41,
];

pub const EG_KING_TABLE: [i32; 64] = [
    -74, -35, -18, -18, -11, 15, 4, -17, -12, 17, 14, 17, 17, 38, 23, 11, 10, 17, 23, 15, 20, 45,
    44, 13, -8, 22, 24, 27, 26, 33, 26, 3, -18, -4, 21, 24, 27, 23, 9, -11, -19, -3, 11, 21, 23,
    16, 7, -9, -27, -11, 4, 13, 14, 4, -5, -17, -53, -34, -21, -11, -28, -14, -24, -43,
];

pub struct PieceSquareTableEval {
    mg_table: [&'static [i32; 64]; 6],
    eg_table: [&'static [i32; 64]; 6],
}

impl PieceSquareTableEval {
    pub fn new() -> Self {
        Self {
            mg_table: [
                &MG_PAWN_TABLE,
                &MG_KNIGHT_TABLE,
                &MG_BISHOP_TABLE,
                &MG_ROOK_TABLE,
                &MG_QUEEN_TABLE,
                &MG_KING_TABLE,
            ],
            eg_table: [
                &EG_PAWN_TABLE,
                &EG_KNIGHT_TABLE,
                &EG_BISHOP_TABLE,
                &EG_ROOK_TABLE,
                &EG_QUEEN_TABLE,
                &EG_KING_TABLE,
            ],
        }
    }

    /// Calculate the game phase (0 = endgame, 24 = opening)
    pub fn game_phase(&self, board: &Board) -> i32 {
        let mut phase = 0;

        for piece_type in [
            PieceType::Pawn,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
            PieceType::King,
        ] {
            let white_count = board
                .piece_bitboard(Color::White, piece_type)
                .count_pieces();
            let black_count = board
                .piece_bitboard(Color::Black, piece_type)
                .count_pieces();
            let total_count = white_count + black_count;

            phase += GAMEPHASE_INC[piece_type as usize] * total_count as i32;
        }

        phase.min(24) // Cap at 24 to avoid distortions from early promotions
    }

    /// Get the PST value for a piece on a square (from White's perspective)
    fn get_piece_value(&self, piece_type: PieceType, square: u8, is_mg: bool) -> i32 {
        let table = if is_mg {
            self.mg_table[piece_type as usize]
        } else {
            self.eg_table[piece_type as usize]
        };

        table[square as usize]
    }

    /// Evaluate the position using tapered evaluation
    pub fn evaluate(&self, board: &Board) -> i32 {
        let mut mg_score = 0;
        let mut eg_score = 0;

        for piece_type in [
            PieceType::Pawn,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
            PieceType::King,
        ] {
            let piece_idx = piece_type as usize;

            // White pieces
            let mut white_bb = board.piece_bitboard(Color::White, piece_type);
            while white_bb.0 != 0 {
                let square = white_bb.pop_lsb().unwrap().index() as u8;
                mg_score += MG_VALUE[piece_idx];
                eg_score += EG_VALUE[piece_idx];
                mg_score += self.get_piece_value(piece_type, square, true);
                eg_score += self.get_piece_value(piece_type, square, false);
            }

            // Black pieces (flip square vertically for Black's perspective)
            let mut black_bb = board.piece_bitboard(Color::Black, piece_type);
            while black_bb.0 != 0 {
                let square = black_bb.pop_lsb().unwrap().index() as u8;
                let flipped_square = square ^ 56; // Flip rank
                mg_score -= MG_VALUE[piece_idx];
                eg_score -= EG_VALUE[piece_idx];
                mg_score -= self.get_piece_value(piece_type, flipped_square, true);
                eg_score -= self.get_piece_value(piece_type, flipped_square, false);
            }
        }

        // Tapered evaluation
        let phase = self.game_phase(board);
        let mg_phase = phase;
        let eg_phase = 24 - phase;

        (mg_score * mg_phase + eg_score * eg_phase) / 24
    }
}

impl Default for PieceSquareTableEval {
    fn default() -> Self {
        Self::new()
    }
}
