use crate::bitboard::Bitboard;
use crate::magic::AttackTable;
use crate::types::{Color, PieceType};
use crate::Board;
use once_cell::sync::Lazy;

use super::phase::GamePhase;

// Global attack table for mobility calculation.
static ATTACK_TABLE: Lazy<AttackTable> = Lazy::new(AttackTable::new);

// Mobility bonuses per square attacked (centipawns).
// Index corresponds to number of attacked squares.
// Values inspired by Stockfish/CPW tuning.

// Knights: max 8 squares, high bonus per square in middlegame.
pub const KNIGHT_MOBILITY_MG: [i32; 9] = [-62, -53, -12, -4, 3, 13, 22, 28, 33];
pub const KNIGHT_MOBILITY_EG: [i32; 9] = [-81, -56, -30, -14, 8, 15, 23, 27, 33];

// Bishops: max 13 squares on open board, long diagonal bonus.
pub const BISHOP_MOBILITY_MG: [i32; 14] = [-48, -20, 16, 26, 38, 51, 55, 63, 63, 68, 81, 81, 91, 98];
pub const BISHOP_MOBILITY_EG: [i32; 14] = [-59, -23, -3, 13, 24, 42, 54, 57, 65, 73, 78, 86, 88, 97];

// Rooks: max 14 squares, more important in endgame.
pub const ROOK_MOBILITY_MG: [i32; 15] =
    [-60, -20, 2, 3, 3, 11, 22, 31, 40, 40, 41, 48, 57, 57, 62];
pub const ROOK_MOBILITY_EG: [i32; 15] =
    [-78, -17, 23, 39, 70, 99, 103, 121, 134, 139, 158, 164, 168, 169, 172];

// Queens: max 27 squares, but lower bonus per square since queens are inherently mobile.
pub const QUEEN_MOBILITY_MG: [i32; 28] = [
    -30, -12, -8, -9, 20, 23, 23, 35, 38, 53, 64, 65, 65, 66, 67, 67, 72, 72, 77, 79, 93, 108, 108,
    108, 110, 114, 114, 116,
];
pub const QUEEN_MOBILITY_EG: [i32; 28] = [
    -48, -30, -7, 19, 40, 55, 59, 75, 78, 96, 96, 100, 121, 127, 131, 133, 136, 141, 147, 150, 151,
    168, 168, 171, 182, 182, 192, 219,
];

pub struct MobilityEval {
    phase: GamePhase,
}

impl MobilityEval {
    pub fn new() -> Self {
        Self {
            phase: GamePhase::new(),
        }
    }

    pub fn evaluate(&self, board: &Board) -> i32 {
        let phase = self.phase.calculate(board);
        self.evaluate_with_phase(board, phase)
    }

    /// Evaluate mobility with a pre-computed game phase.
    ///
    /// Skips the internal `phase.calculate` call so callers that already
    /// know the phase can share one calculation across all eval terms.
    /// Scores are bit-identical to `evaluate` when passed
    /// `self.phase.calculate(board)`.
    pub fn evaluate_with_phase(&self, board: &Board, phase: i32) -> i32 {
        self.evaluate_with_phase_and_weights(board, phase, &super::EvalWeights::DEFAULT)
    }

    pub fn evaluate_with_phase_and_weights(
        &self,
        board: &Board,
        phase: i32,
        weights: &super::EvalWeights,
    ) -> i32 {
        let (w_mg, w_eg) = self.evaluate_color_with_weights(board, Color::White, weights);
        let (b_mg, b_eg) = self.evaluate_color_with_weights(board, Color::Black, weights);

        let mg_score = w_mg - b_mg;
        let eg_score = w_eg - b_eg;

        self.phase.taper(mg_score, eg_score, phase)
    }

    fn evaluate_color_with_weights(
        &self,
        board: &Board,
        color: Color,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        // Get friendly pieces to exclude from mobility count.
        // `color_bitboard` is the maintained union of all six per-piece
        // bitboards; the debug assertion below guards that invariant.
        let friendly = board.color_bitboard(color);
        debug_assert_eq!(
            friendly,
            board.piece_bitboard(color, PieceType::Pawn)
                | board.piece_bitboard(color, PieceType::Knight)
                | board.piece_bitboard(color, PieceType::Bishop)
                | board.piece_bitboard(color, PieceType::Rook)
                | board.piece_bitboard(color, PieceType::Queen)
                | board.piece_bitboard(color, PieceType::King),
            "color_bitboard must equal the union of per-piece bitboards"
        );

        let blockers = board.all_pieces();

        let (knight_mg, knight_eg) =
            self.evaluate_knight_mobility_with_weights(board, color, friendly, weights);
        mg += knight_mg;
        eg += knight_eg;

        let (bishop_mg, bishop_eg) =
            self.evaluate_bishop_mobility_with_weights(board, color, friendly, blockers, weights);
        mg += bishop_mg;
        eg += bishop_eg;

        let (rook_mg, rook_eg) =
            self.evaluate_rook_mobility_with_weights(board, color, friendly, blockers, weights);
        mg += rook_mg;
        eg += rook_eg;

        let (queen_mg, queen_eg) =
            self.evaluate_queen_mobility_with_weights(board, color, friendly, blockers, weights);
        mg += queen_mg;
        eg += queen_eg;

        (mg, eg)
    }

    fn evaluate_knight_mobility_with_weights(
        &self,
        board: &Board,
        color: Color,
        friendly: Bitboard,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let mut knights = board.piece_bitboard(color, PieceType::Knight);
        if knights.is_empty() {
            return (0, 0);
        }
        while knights.0 != 0 {
            let sq = knights.pop_lsb().unwrap();
            let attacks = ATTACK_TABLE.knight_attacks(sq);
            let mobility = (attacks & !friendly).count_pieces() as usize;
            let mobility = mobility.min(weights.knight_mobility_mg.len() - 1);

            mg += weights.knight_mobility_mg[mobility];
            eg += weights.knight_mobility_eg[mobility];
        }

        (mg, eg)
    }

    fn evaluate_bishop_mobility_with_weights(
        &self,
        board: &Board,
        color: Color,
        friendly: Bitboard,
        blockers: Bitboard,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let mut bishops = board.piece_bitboard(color, PieceType::Bishop);
        if bishops.is_empty() {
            return (0, 0);
        }
        while bishops.0 != 0 {
            let sq = bishops.pop_lsb().unwrap();
            let attacks = ATTACK_TABLE.bishop_attacks(sq, blockers);
            let mobility = (attacks & !friendly).count_pieces() as usize;
            let mobility = mobility.min(weights.bishop_mobility_mg.len() - 1);

            mg += weights.bishop_mobility_mg[mobility];
            eg += weights.bishop_mobility_eg[mobility];
        }

        (mg, eg)
    }

    fn evaluate_rook_mobility_with_weights(
        &self,
        board: &Board,
        color: Color,
        friendly: Bitboard,
        blockers: Bitboard,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let mut rooks = board.piece_bitboard(color, PieceType::Rook);
        if rooks.is_empty() {
            return (0, 0);
        }
        while rooks.0 != 0 {
            let sq = rooks.pop_lsb().unwrap();
            let attacks = ATTACK_TABLE.rook_attacks(sq, blockers);
            let mobility = (attacks & !friendly).count_pieces() as usize;
            let mobility = mobility.min(weights.rook_mobility_mg.len() - 1);

            mg += weights.rook_mobility_mg[mobility];
            eg += weights.rook_mobility_eg[mobility];
        }

        (mg, eg)
    }

    fn evaluate_queen_mobility_with_weights(
        &self,
        board: &Board,
        color: Color,
        friendly: Bitboard,
        blockers: Bitboard,
        weights: &super::EvalWeights,
    ) -> (i32, i32) {
        let mut mg = 0;
        let mut eg = 0;

        let mut queens = board.piece_bitboard(color, PieceType::Queen);
        if queens.is_empty() {
            return (0, 0);
        }
        while queens.0 != 0 {
            let sq = queens.pop_lsb().unwrap();
            let attacks = ATTACK_TABLE.queen_attacks(sq, blockers);
            let mobility = (attacks & !friendly).count_pieces() as usize;
            let mobility = mobility.min(weights.queen_mobility_mg.len() - 1);

            mg += weights.queen_mobility_mg[mobility];
            eg += weights.queen_mobility_eg[mobility];
        }

        (mg, eg)
    }
}

impl Default for MobilityEval {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position_symmetric() {
        let board = Board::starting_position();
        let eval = MobilityEval::new();
        let score = eval.evaluate(&board);

        // Starting position is symmetric, score should be near zero.
        assert!(
            score.abs() < 10,
            "Starting position should be symmetric, got {}",
            score
        );
    }

    #[test]
    fn test_knight_in_center_more_mobile() {
        // Knight on e4 vs knight on a1.
        let board_center =
            Board::from_fen("8/8/8/8/4N3/8/8/4K2k w - - 0 1").unwrap();
        let board_corner =
            Board::from_fen("N7/8/8/8/8/8/8/4K2k w - - 0 1").unwrap();

        let eval = MobilityEval::new();
        let center_score = eval.evaluate(&board_center);
        let corner_score = eval.evaluate(&board_corner);

        // Knight in center should have better mobility score.
        assert!(
            center_score > corner_score,
            "Center knight should score higher: {} vs {}",
            center_score,
            corner_score
        );
    }

    #[test]
    fn test_bishop_on_open_diagonal() {
        // Bishop on open diagonal vs blocked bishop.
        let board_open =
            Board::from_fen("8/8/8/8/8/8/8/B3K2k w - - 0 1").unwrap();
        let board_blocked =
            Board::from_fen("8/8/8/8/8/8/1P6/B3K2k w - - 0 1").unwrap();

        let eval = MobilityEval::new();
        let open_score = eval.evaluate(&board_open);
        let blocked_score = eval.evaluate(&board_blocked);

        // Open bishop should have better mobility.
        assert!(
            open_score > blocked_score,
            "Open bishop should score higher: {} vs {}",
            open_score,
            blocked_score
        );
    }

    #[test]
    fn test_rook_on_open_file() {
        // Rook on open file vs blocked rook.
        let board_open =
            Board::from_fen("8/8/8/8/8/8/8/R3K2k w - - 0 1").unwrap();
        let board_blocked =
            Board::from_fen("8/8/8/8/8/8/P7/R3K2k w - - 0 1").unwrap();

        let eval = MobilityEval::new();
        let open_score = eval.evaluate(&board_open);
        let blocked_score = eval.evaluate(&board_blocked);

        // Open rook should have better mobility.
        assert!(
            open_score > blocked_score,
            "Open rook should score higher: {} vs {}",
            open_score,
            blocked_score
        );
    }

    #[test]
    fn test_queen_mobility() {
        // Queen on d4 has many moves available.
        let board = Board::from_fen("8/8/8/8/3Q4/8/8/4K2k w - - 0 1").unwrap();
        let eval = MobilityEval::new();
        let score = eval.evaluate(&board);

        // Queen in center should have significant positive score.
        assert!(
            score > 50,
            "Centralized queen should have good mobility: {}",
            score
        );
    }

    #[test]
    fn test_multiple_pieces_mobility() {
        // White has more mobile pieces.
        let board = Board::from_fen(
            "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
        )
        .unwrap();
        let eval = MobilityEval::new();
        let score = eval.evaluate(&board);

        // White's developed knight should give slight mobility advantage.
        assert!(
            score > 0,
            "Developed white should have mobility advantage: {}",
            score
        );
    }

    #[test]
    fn test_no_pieces_zero_mobility() {
        // Only kings on the board.
        let board = Board::from_fen("8/8/8/8/8/4k3/8/4K3 w - - 0 1").unwrap();
        let eval = MobilityEval::new();
        let score = eval.evaluate(&board);

        // No mobile pieces (kings don't count), should be zero.
        assert_eq!(score, 0, "Only kings should have zero mobility score");
    }

    #[test]
    fn test_trapped_pieces_penalty() {
        // White bishop trapped behind pawns.
        let board =
            Board::from_fen("8/8/8/8/8/8/PPP5/1B2K2k w - - 0 1").unwrap();
        let eval = MobilityEval::new();
        let score = eval.evaluate(&board);

        // Trapped bishop should have low/negative score.
        assert!(
            score < 20,
            "Trapped bishop should have poor mobility: {}",
            score
        );
    }

    #[test]
    fn test_color_bitboard_equals_piece_union() {
        // Debug-assertion backing: `color_bitboard` must equal the manual
        // union of the six per-piece bitboards, so `evaluate_color` can
        // use the cached aggregate.
        let positions = [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
            "8/8/8/8/8/8/PPP5/1B2K2k w - - 0 1",
            "8/8/8/8/8/4k3/8/4K3 w - - 0 1",
        ];
        for fen in positions {
            let board = Board::from_fen(fen).unwrap();
            for color in [Color::White, Color::Black] {
                let manual = board.piece_bitboard(color, PieceType::Pawn)
                    | board.piece_bitboard(color, PieceType::Knight)
                    | board.piece_bitboard(color, PieceType::Bishop)
                    | board.piece_bitboard(color, PieceType::Rook)
                    | board.piece_bitboard(color, PieceType::Queen)
                    | board.piece_bitboard(color, PieceType::King);
                assert_eq!(
                    board.color_bitboard(color),
                    manual,
                    "color_bitboard mismatch for {:?} in {}",
                    color,
                    fen
                );
            }
        }
    }

    #[test]
    fn weighted_default_matches_unweighted() {
        use crate::eval::EvalWeights;

        let board = Board::from_fen("8/8/8/8/4N3/8/8/4K2k w - - 0 1").unwrap();
        let eval = MobilityEval::new();

        let default = eval.evaluate(&board);
        let phase = eval.phase.calculate(&board);
        let weighted = eval.evaluate_with_phase_and_weights(&board, phase, &EvalWeights::DEFAULT);

        assert_eq!(default, weighted);
    }

    #[test]
    fn known_fen_scores_unchanged() {
        let cases = [
            (
                "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
                130,
            ),
            (
                "r1bqk2r/pp1nbppp/2p2n2/3p4/3P4/2N1PN2/PPP1BPPP/R1BQK2R w KQ - 0 1",
                39,
            ),
        ];
        let eval = MobilityEval::new();
        for (fen, expected) in cases {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(eval.evaluate(&board), expected, "mismatch for {}", fen);
        }
    }
}
