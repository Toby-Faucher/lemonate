//! History Heuristic
//!
//! Tracks the success of quiet moves across the search tree.
//! Moves that frequently cause cutoffs are prioritized in move ordering.
//!
//! # Implementation Notes
//! - Index by [color][from_square][to_square]
//! - Update on beta cutoffs with depth-weighted bonus
//! - Apply aging/scaling to prevent overflow
//! - Use "gravity" approach: bonus for good moves, penalty for bad moves

use crate::board::Move;
use crate::types::Color;

/// Maximum history score before scaling.
pub const MAX_HISTORY_SCORE: i32 = 16384;

/// History bonus formula: depth * depth
#[inline]
fn history_bonus(depth: i32) -> i32 {
    depth * depth
}

/// History table for quiet move ordering.
pub struct HistoryTable {
    /// History scores: [color][from_square][to_square]
    history: [[[i32; 64]; 64]; 2],
}

impl HistoryTable {
    /// Create a new empty history table.
    pub fn new() -> Self {
        Self {
            history: [[[0; 64]; 64]; 2],
        }
    }

    /// Clear all history scores (call at start of new game).
    pub fn clear(&mut self) {
        for color in &mut self.history {
            for from in color {
                for to in from {
                    *to = 0;
                }
            }
        }
    }

    /// Get the history score for a move.
    #[inline]
    pub fn get(&self, color: Color, mv: &Move) -> i32 {
        let from = mv.from.index();
        let to = mv.to.index();
        self.history[color as usize][from][to]
    }

    /// Update history score for a move that caused a beta cutoff.
    ///
    /// Uses the "gravity" approach where scores are clamped and
    /// scaled to prevent unbounded growth.
    ///
    /// # Arguments
    /// * `color` - The side that made the move
    /// * `mv` - The move that caused the cutoff
    /// * `depth` - The remaining search depth
    pub fn update_cutoff(&mut self, color: Color, mv: &Move, depth: i32) {
        let from = mv.from.index();
        let to = mv.to.index();
        let bonus = history_bonus(depth);

        // Apply bonus with gravity scaling to prevent overflow.
        let current = self.history[color as usize][from][to];
        let new_score = current + bonus - (current * bonus.abs() / MAX_HISTORY_SCORE);
        self.history[color as usize][from][to] = new_score.clamp(-MAX_HISTORY_SCORE, MAX_HISTORY_SCORE);
    }

    /// Apply a penalty to moves that didn't cause a cutoff.
    ///
    /// Called for quiet moves tried before the cutoff move.
    /// This helps differentiate between moves that were tried and failed
    /// vs moves that were never tried.
    ///
    /// # Arguments
    /// * `color` - The side that made the move
    /// * `mv` - The move that failed to cause a cutoff
    /// * `depth` - The remaining search depth
    pub fn update_penalty(&mut self, color: Color, mv: &Move, depth: i32) {
        let from = mv.from.index();
        let to = mv.to.index();
        let penalty = history_bonus(depth);

        // Apply penalty with gravity scaling.
        let current = self.history[color as usize][from][to];
        let new_score = current - penalty - (current * penalty.abs() / MAX_HISTORY_SCORE);
        self.history[color as usize][from][to] = new_score.clamp(-MAX_HISTORY_SCORE, MAX_HISTORY_SCORE);
    }

    /// Age/scale all history scores.
    ///
    /// Called periodically (e.g., at start of new search) to prevent
    /// overflow and maintain relevance of recent moves.
    /// Divides all scores by 2.
    pub fn age(&mut self) {
        for color in &mut self.history {
            for from in color {
                for to in from {
                    *to /= 2;
                }
            }
        }
    }

    /// Update history for a batch of quiet moves that failed to cause cutoff,
    /// plus the move that did cause the cutoff.
    ///
    /// This is the typical usage pattern during search.
    ///
    /// # Arguments
    /// * `color` - The side that made the moves
    /// * `best_move` - The move that caused the cutoff (gets bonus)
    /// * `failed_quiets` - Quiet moves tried before best_move (get penalty)
    /// * `depth` - The remaining search depth
    pub fn update(&mut self, color: Color, best_move: &Move, failed_quiets: &[Move], depth: i32) {
        // Give bonus to the move that caused cutoff.
        self.update_cutoff(color, best_move, depth);

        // Penalize moves that failed to cause cutoff.
        for mv in failed_quiets {
            if mv != best_move {
                self.update_penalty(color, mv, depth);
            }
        }
    }
}

impl Default for HistoryTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Counter move heuristic.
///
/// Tracks which move is a good response to the opponent's previous move.
/// If move A is often followed by a strong move B, store B as counter to A.
/// Indexed by [piece_type][to_square] of the previous move.
pub struct CounterMoveTable {
    /// Counter moves: [piece_type][to_square] -> best response
    counters: [[Option<Move>; 64]; 6],
}

impl CounterMoveTable {
    /// Create a new empty counter move table.
    pub fn new() -> Self {
        Self {
            counters: [[None; 64]; 6],
        }
    }

    /// Clear all counter moves.
    pub fn clear(&mut self) {
        for piece in &mut self.counters {
            for sq in piece {
                *sq = None;
            }
        }
    }

    /// Get the counter move for the opponent's previous move.
    ///
    /// Returns the move that has historically been a good response
    /// to the given previous move.
    #[inline]
    pub fn get(&self, previous_move: &Move) -> Option<Move> {
        let piece_type = previous_move.piece.piece_type as usize;
        let to_sq = previous_move.to.index();
        self.counters[piece_type][to_sq]
    }

    /// Store a counter move.
    ///
    /// Records that `counter` is a good response when the opponent
    /// plays `previous_move`.
    ///
    /// # Arguments
    /// * `previous_move` - The opponent's move we're responding to
    /// * `counter` - The move that worked well as a response
    #[inline]
    pub fn store(&mut self, previous_move: &Move, counter: Move) {
        let piece_type = previous_move.piece.piece_type as usize;
        let to_sq = previous_move.to.index();
        self.counters[piece_type][to_sq] = Some(counter);
    }

    /// Check if a move is the counter move for the given previous move.
    #[inline]
    pub fn is_counter(&self, previous_move: &Move, mv: &Move) -> bool {
        self.get(previous_move).map_or(false, |counter| counter == *mv)
    }
}

impl Default for CounterMoveTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::MoveType;
    use crate::types::{Piece, PieceType, Square};

    fn make_move(from: &str, to: &str, piece_type: PieceType) -> Move {
        Move {
            from: Square::from_algebraic(from).unwrap(),
            to: Square::from_algebraic(to).unwrap(),
            move_type: MoveType::Normal,
            piece: Piece {
                piece_type,
                color: Color::White,
            },
            captured: None,
        }
    }

    // ==================== HistoryTable Tests ====================

    #[test]
    fn test_new_history_table_is_zero() {
        let table = HistoryTable::new();
        let mv = make_move("e2", "e4", PieceType::Pawn);

        assert_eq!(table.get(Color::White, &mv), 0);
        assert_eq!(table.get(Color::Black, &mv), 0);
    }

    #[test]
    fn test_history_update_cutoff() {
        let mut table = HistoryTable::new();
        let mv = make_move("e2", "e4", PieceType::Pawn);

        table.update_cutoff(Color::White, &mv, 5);

        let score = table.get(Color::White, &mv);
        assert!(score > 0, "Score should be positive after cutoff: {}", score);
        assert_eq!(score, 25); // depth * depth = 5 * 5 = 25
    }

    #[test]
    fn test_history_update_penalty() {
        let mut table = HistoryTable::new();
        let mv = make_move("e2", "e4", PieceType::Pawn);

        table.update_penalty(Color::White, &mv, 5);

        let score = table.get(Color::White, &mv);
        assert!(score < 0, "Score should be negative after penalty: {}", score);
    }

    #[test]
    fn test_history_accumulates() {
        let mut table = HistoryTable::new();
        let mv = make_move("e2", "e4", PieceType::Pawn);

        table.update_cutoff(Color::White, &mv, 3);
        let score1 = table.get(Color::White, &mv);

        table.update_cutoff(Color::White, &mv, 3);
        let score2 = table.get(Color::White, &mv);

        assert!(score2 > score1, "Score should accumulate: {} > {}", score2, score1);
    }

    #[test]
    fn test_history_colors_independent() {
        let mut table = HistoryTable::new();
        let mv = make_move("e2", "e4", PieceType::Pawn);

        table.update_cutoff(Color::White, &mv, 5);

        assert!(table.get(Color::White, &mv) > 0);
        assert_eq!(table.get(Color::Black, &mv), 0);
    }

    #[test]
    fn test_history_moves_independent() {
        let mut table = HistoryTable::new();
        let mv1 = make_move("e2", "e4", PieceType::Pawn);
        let mv2 = make_move("d2", "d4", PieceType::Pawn);

        table.update_cutoff(Color::White, &mv1, 5);

        assert!(table.get(Color::White, &mv1) > 0);
        assert_eq!(table.get(Color::White, &mv2), 0);
    }

    #[test]
    fn test_history_clamped() {
        let mut table = HistoryTable::new();
        let mv = make_move("e2", "e4", PieceType::Pawn);

        // Apply many large bonuses.
        for _ in 0..1000 {
            table.update_cutoff(Color::White, &mv, 20);
        }

        let score = table.get(Color::White, &mv);
        assert!(
            score <= MAX_HISTORY_SCORE,
            "Score should be clamped: {} <= {}",
            score,
            MAX_HISTORY_SCORE
        );
    }

    #[test]
    fn test_history_aging() {
        let mut table = HistoryTable::new();
        let mv = make_move("e2", "e4", PieceType::Pawn);

        table.update_cutoff(Color::White, &mv, 10);
        let score_before = table.get(Color::White, &mv);

        table.age();
        let score_after = table.get(Color::White, &mv);

        assert_eq!(score_after, score_before / 2);
    }

    #[test]
    fn test_history_clear() {
        let mut table = HistoryTable::new();
        let mv = make_move("e2", "e4", PieceType::Pawn);

        table.update_cutoff(Color::White, &mv, 10);
        assert!(table.get(Color::White, &mv) > 0);

        table.clear();
        assert_eq!(table.get(Color::White, &mv), 0);
    }

    #[test]
    fn test_history_batch_update() {
        let mut table = HistoryTable::new();
        let best = make_move("e2", "e4", PieceType::Pawn);
        let failed1 = make_move("d2", "d4", PieceType::Pawn);
        let failed2 = make_move("c2", "c4", PieceType::Pawn);

        table.update(Color::White, &best, &[failed1, failed2], 5);

        assert!(table.get(Color::White, &best) > 0);
        assert!(table.get(Color::White, &failed1) < 0);
        assert!(table.get(Color::White, &failed2) < 0);
    }

    // ==================== CounterMoveTable Tests ====================

    #[test]
    fn test_new_counter_table_is_empty() {
        let table = CounterMoveTable::new();
        let prev = make_move("e2", "e4", PieceType::Pawn);

        assert!(table.get(&prev).is_none());
    }

    #[test]
    fn test_counter_store_and_retrieve() {
        let mut table = CounterMoveTable::new();
        let prev = make_move("e7", "e5", PieceType::Pawn);
        let counter = make_move("g1", "f3", PieceType::Knight);

        table.store(&prev, counter);

        assert_eq!(table.get(&prev), Some(counter));
    }

    #[test]
    fn test_counter_overwrites() {
        let mut table = CounterMoveTable::new();
        let prev = make_move("e7", "e5", PieceType::Pawn);
        let counter1 = make_move("g1", "f3", PieceType::Knight);
        let counter2 = make_move("d2", "d4", PieceType::Pawn);

        table.store(&prev, counter1);
        table.store(&prev, counter2);

        assert_eq!(table.get(&prev), Some(counter2));
    }

    #[test]
    fn test_counter_is_counter() {
        let mut table = CounterMoveTable::new();
        let prev = make_move("e7", "e5", PieceType::Pawn);
        let counter = make_move("g1", "f3", PieceType::Knight);
        let other = make_move("d2", "d4", PieceType::Pawn);

        table.store(&prev, counter);

        assert!(table.is_counter(&prev, &counter));
        assert!(!table.is_counter(&prev, &other));
    }

    #[test]
    fn test_counter_clear() {
        let mut table = CounterMoveTable::new();
        let prev = make_move("e7", "e5", PieceType::Pawn);
        let counter = make_move("g1", "f3", PieceType::Knight);

        table.store(&prev, counter);
        assert!(table.get(&prev).is_some());

        table.clear();
        assert!(table.get(&prev).is_none());
    }

    #[test]
    fn test_counter_different_pieces_independent() {
        let mut table = CounterMoveTable::new();
        // Same target square, different piece types.
        let prev_pawn = make_move("e7", "e5", PieceType::Pawn);
        let prev_knight = make_move("g8", "e5", PieceType::Knight);
        let counter1 = make_move("g1", "f3", PieceType::Knight);
        let counter2 = make_move("d2", "d4", PieceType::Pawn);

        table.store(&prev_pawn, counter1);
        table.store(&prev_knight, counter2);

        assert_eq!(table.get(&prev_pawn), Some(counter1));
        assert_eq!(table.get(&prev_knight), Some(counter2));
    }

    #[test]
    fn test_counter_different_squares_independent() {
        let mut table = CounterMoveTable::new();
        let prev1 = make_move("e7", "e5", PieceType::Pawn);
        let prev2 = make_move("d7", "d5", PieceType::Pawn);
        let counter1 = make_move("g1", "f3", PieceType::Knight);
        let counter2 = make_move("c2", "c4", PieceType::Pawn);

        table.store(&prev1, counter1);
        table.store(&prev2, counter2);

        assert_eq!(table.get(&prev1), Some(counter1));
        assert_eq!(table.get(&prev2), Some(counter2));
    }
}
