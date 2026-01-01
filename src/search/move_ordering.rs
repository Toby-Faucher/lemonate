//! Move Ordering
//!
//! Sorts moves to maximize alpha-beta cutoffs.
//! Good move ordering can reduce search tree from O(b^d) to O(b^(d/2)).
//!
//! # Move Priority Order
//! 1. Hash move (from transposition table)
//! 2. Good captures (MVV-LVA, winning/equal exchanges)
//! 3. Killer moves (quiet moves that caused cutoffs at this ply)
//! 4. Counter move (response to opponent's last move)
//! 5. Quiet moves ordered by history heuristic
//! 6. Bad captures (losing exchanges)

use crate::board::Move;
use crate::types::PieceType;
use crate::Board;

use super::history::HistoryTable;
use super::killer_moves::KillerMoveTable;

/// Score constants for move ordering.
pub mod scores {
    /// Hash move from transposition table.
    pub const HASH_MOVE: i32 = 100_000_000;
    /// Good capture base score.
    pub const GOOD_CAPTURE: i32 = 50_000_000;
    /// Killer move bonus.
    pub const KILLER_PRIMARY: i32 = 40_000_000;
    pub const KILLER_SECONDARY: i32 = 39_000_000;
    /// Counter move bonus.
    pub const COUNTER_MOVE: i32 = 38_000_000;
    /// Base score for quiet moves (history added to this).
    pub const QUIET_BASE: i32 = 0;
    /// Bad capture penalty.
    pub const BAD_CAPTURE: i32 = -50_000_000;
}

/// MVV-LVA (Most Valuable Victim - Least Valuable Aggressor) values.
///
/// Higher scores for capturing valuable pieces with cheap pieces.
pub const MVV_LVA: [[i32; 6]; 6] = [
    // Victim:    P    N    B    R    Q    K
    /* P */ [105, 205, 305, 405, 505, 605],
    /* N */ [104, 204, 304, 404, 504, 604],
    /* B */ [103, 203, 303, 403, 503, 603],
    /* R */ [102, 202, 302, 402, 502, 602],
    /* Q */ [101, 201, 301, 401, 501, 601],
    /* K */ [100, 200, 300, 400, 500, 600],
];

/// A move with an associated score for sorting.
#[derive(Clone, Copy)]
pub struct ScoredMove {
    pub mv: Move,
    pub score: i32,
}

impl ScoredMove {
    pub fn new(mv: Move, score: i32) -> Self {
        Self { mv, score }
    }
}

/// Move ordering context for a single node.
pub struct MoveOrderer<'a> {
    /// The list of moves to order.
    moves: Vec<ScoredMove>,
    /// Current index in the move list.
    current: usize,
    /// Hash move to prioritize.
    hash_move: Option<Move>,
    /// Reference to killer move table.
    killers: &'a KillerMoveTable,
    /// Reference to history table.
    history: &'a HistoryTable,
    /// Current ply for killer move lookup.
    ply: u8,
}

impl<'a> MoveOrderer<'a> {
    /// Create a new move orderer for the given position.
    ///
    /// # Arguments
    /// * `board` - The current position
    /// * `moves` - Legal moves to order
    /// * `hash_move` - Best move from transposition table (if any)
    /// * `killers` - Killer move table reference
    /// * `history` - History table reference
    /// * `ply` - Current search ply
    pub fn new(
        board: &Board,
        moves: Vec<Move>,
        hash_move: Option<Move>,
        killers: &'a KillerMoveTable,
        history: &'a HistoryTable,
        ply: u8,
    ) -> Self {
        todo!("Implement MoveOrderer::new")
    }

    /// Score all moves for ordering.
    fn score_moves(&mut self, board: &Board) {
        todo!("Implement MoveOrderer::score_moves")
    }

    /// Get the next best move using partial sorting.
    ///
    /// Uses selection sort to find the best remaining move,
    /// which is more efficient than full sorting when we expect
    /// early cutoffs.
    pub fn next(&mut self) -> Option<Move> {
        todo!("Implement MoveOrderer::next")
    }

    /// Check if there are more moves to try.
    pub fn has_moves(&self) -> bool {
        self.current < self.moves.len()
    }
}

/// Calculate MVV-LVA score for a capture.
///
/// # Arguments
/// * `attacker` - The piece type making the capture
/// * `victim` - The piece type being captured
pub fn mvv_lva_score(attacker: PieceType, victim: PieceType) -> i32 {
    todo!("Implement mvv_lva_score")
}

/// Perform Static Exchange Evaluation (SEE) on a capture.
///
/// Determines if a capture sequence is winning, losing, or equal.
/// Used to separate good captures from bad captures.
///
/// # Arguments
/// * `board` - The current position
/// * `mv` - The capture move to evaluate
///
/// # Returns
/// The material balance after all recaptures (positive = winning).
pub fn see(board: &Board, mv: &Move) -> i32 {
    todo!("Implement SEE (Static Exchange Evaluation)")
}

/// Check if a capture is likely good (winning or equal exchange).
///
/// Fast approximation of SEE for move ordering.
pub fn is_good_capture(board: &Board, mv: &Move) -> bool {
    todo!("Implement is_good_capture")
}

/// Order moves for quiescence search (captures only).
///
/// Uses MVV-LVA ordering without killer/history heuristics.
pub fn order_captures(board: &Board, captures: Vec<Move>) -> Vec<Move> {
    todo!("Implement order_captures for quiescence search")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mvv_lva_ordering() {
        // TODO: Test that QxP scores higher than PxQ
    }

    #[test]
    fn test_hash_move_first() {
        // TODO: Test hash move is returned first
    }

    #[test]
    fn test_killer_move_priority() {
        // TODO: Test killer moves are prioritized
    }

    #[test]
    fn test_see_winning_capture() {
        // TODO: Test SEE identifies winning captures
    }

    #[test]
    fn test_see_losing_capture() {
        // TODO: Test SEE identifies losing captures
    }
}
