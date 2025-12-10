// Game Phase Evaluation
// Determines whether a position is in opening, middlegame, or endgame
// Used for tapered evaluation where different evaluation terms have different weights

use crate::types::piece::PieceType;
use crate::types::Color;
use crate::Board;

/// Maximum phase value (opening position with all pieces)
pub const MAX_PHASE: i32 = 24;

/// Phase increments for each piece type
/// Pawn=0, Knight=1, Bishop=1, Rook=2, Queen=4, King=0
pub const PHASE_INCREMENT: [i32; 6] = [0, 1, 1, 2, 4, 0];

pub struct GamePhase {
    phase_increment: [i32; 6],
    max_phase: i32,
}

impl GamePhase {
    pub fn new() -> Self {
        Self {
            phase_increment: PHASE_INCREMENT,
            max_phase: MAX_PHASE,
        }
    }

    /// Calculate the current game phase
    /// Returns a value from 0 (endgame) to MAX_PHASE (opening/middlegame)
    pub fn calculate(&self, board: &Board) -> i32 {
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

            phase += self.phase_increment[piece_type as usize] * total_count as i32;
        }

        // Cap at max_phase to avoid distortions from early promotions
        phase.min(self.max_phase)
    }

    /// Get the middlegame phase weight (0 to MAX_PHASE)
    pub fn mg_phase(&self, phase: i32) -> i32 {
        phase
    }

    /// Get the endgame phase weight (0 to MAX_PHASE)
    pub fn eg_phase(&self, phase: i32) -> i32 {
        self.max_phase - phase
    }

    /// Calculate tapered score from mg and eg scores
    pub fn taper(&self, mg_score: i32, eg_score: i32, phase: i32) -> i32 {
        let mg_phase = self.mg_phase(phase);
        let eg_phase = self.eg_phase(phase);
        (mg_score * mg_phase + eg_score * eg_phase) / self.max_phase
    }

    /// Check if position is in the endgame
    pub fn is_endgame(&self, phase: i32) -> bool {
        phase < self.max_phase / 3
    }

    /// Check if position is in the middlegame
    pub fn is_middlegame(&self, phase: i32) -> bool {
        phase > self.max_phase * 2 / 3
    }

    /// Check if position is at starting position
    pub fn is_opening(&self, phase: i32) -> bool {
        phase == self.max_phase
    }
}

impl Default for GamePhase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position_max_phase() {
        let board = Board::starting_position();
        let phase = GamePhase::new();
        let calculated_phase = phase.calculate(&board);

        // Starting position should have max phase
        assert_eq!(calculated_phase, MAX_PHASE);
    }

    #[test]
    fn test_endgame_position_low_phase() {
        // King vs King (no other pieces)
        let board = Board::from_fen("8/8/8/8/8/5k2/8/5K2 w - - 0 1").unwrap();
        let phase = GamePhase::new();
        let calculated_phase = phase.calculate(&board);

        // Endgame with only kings should have phase 0 (kings don't contribute)
        assert_eq!(calculated_phase, 0);
    }

    #[test]
    fn test_phase_decreases_with_captures() {
        let phase = GamePhase::new();

        let board_start = Board::starting_position();
        let phase_start = phase.calculate(&board_start);

        // After both queens are removed
        let board_after =
            Board::from_fen("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNB1KBNR w KQkq - 0 1").unwrap();
        let phase_after = phase.calculate(&board_after);

        // Phase should decrease (2 queens gone = -8)
        assert_eq!(phase_start - phase_after, 8);
    }

    #[test]
    fn test_taper_evaluation() {
        let phase_calc = GamePhase::new();

        // Middlegame position (phase = 24)
        let mg_score = 100;
        let eg_score = -50;
        let result = phase_calc.taper(mg_score, eg_score, 24);
        assert_eq!(result, mg_score); // Pure middlegame

        // Endgame position (phase = 0)
        let result = phase_calc.taper(mg_score, eg_score, 0);
        assert_eq!(result, eg_score); // Pure endgame

        // Middle phase (phase = 12)
        let result = phase_calc.taper(mg_score, eg_score, 12);
        let expected = (mg_score * 12 + eg_score * 12) / 24;
        assert_eq!(result, expected);
    }

    #[test]
    fn test_phase_capped_at_max() {
        // Even with promotions, phase shouldn't exceed MAX_PHASE
        let board =
            Board::from_fen("QQQQQQQQ/QQQQQQQQ/8/8/8/8/qqqqqqqq/qqqqqqqq w - - 0 1").unwrap();
        let phase = GamePhase::new();
        let calculated_phase = phase.calculate(&board);

        assert_eq!(calculated_phase, MAX_PHASE);
    }

    #[test]
    fn test_mg_and_eg_phase_weights() {
        let phase_calc = GamePhase::new();

        // At phase 18 (relatively early game)
        let mg_phase = phase_calc.mg_phase(18);
        let eg_phase = phase_calc.eg_phase(18);

        assert_eq!(mg_phase, 18);
        assert_eq!(eg_phase, 6);
        assert_eq!(mg_phase + eg_phase, MAX_PHASE);
    }

    #[test]
    fn test_is_endgame() {
        let phase = GamePhase::new();

        assert!(phase.is_endgame(0));
        assert!(phase.is_endgame(7));
        assert!(!phase.is_endgame(8));
        assert!(!phase.is_endgame(24));
    }

    #[test]
    fn test_is_middlegame() {
        let phase = GamePhase::new();

        assert!(phase.is_middlegame(17));
        assert!(phase.is_middlegame(24));
        assert!(!phase.is_middlegame(16));
        assert!(!phase.is_middlegame(0));
    }

    #[test]
    fn test_is_opening() {
        let phase = GamePhase::new();

        assert!(phase.is_opening(24));
        assert!(!phase.is_opening(23));
        assert!(!phase.is_opening(0));
    }

    #[test]
    fn test_single_piece_values() {
        let phase = GamePhase::new();

        // King vs King + Knight
        let board = Board::from_fen("8/8/8/8/8/5k2/8/5KN1 w - - 0 1").unwrap();
        let calculated_phase = phase.calculate(&board);
        assert_eq!(calculated_phase, 1); // 1 knight = 1 phase

        // King vs King + Rook
        let board = Board::from_fen("8/8/8/8/8/5k2/8/5KR1 w - - 0 1").unwrap();
        let calculated_phase = phase.calculate(&board);
        assert_eq!(calculated_phase, 2); // 1 rook = 2 phase

        // King vs King + Queen
        let board = Board::from_fen("8/8/8/8/8/5k2/8/5KQ1 w - - 0 1").unwrap();
        let calculated_phase = phase.calculate(&board);
        assert_eq!(calculated_phase, 4); // 1 queen = 4 phase
    }
}
