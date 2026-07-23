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

use crate::bitboard::Bitboard;
use crate::board::{Move, MoveType};
use crate::magic::AttackTable;
use crate::types::{Color, PieceType, Square};
use crate::Board;
use once_cell::sync::Lazy;

use super::history::{ContinuationHistory, HistoryTable};
use super::killer_moves::KillerMoveTable;

/// Lazy-initialized attack table for SEE calculations.
static ATTACK_TABLE: Lazy<AttackTable> = Lazy::new(AttackTable::new);

/// Shared empty continuation history used by [`MoveOrderer::new`], which
/// doesn't take a continuation history table (e.g. in tests or call sites
/// that don't track the previous move).
static EMPTY_CONTINUATION_HISTORY: Lazy<ContinuationHistory> = Lazy::new(ContinuationHistory::new);

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

/// Piece values for SEE (Static Exchange Evaluation).
/// Using standard centipawn values.
pub const SEE_PIECE_VALUES: [i32; 6] = [
    100,   // Pawn
    320,   // Knight
    330,   // Bishop
    500,   // Rook
    900,   // Queen
    20000, // King (very high to prevent king captures)
];

/// MVV-LVA (Most Valuable Victim - Least Valuable Aggressor) values.
///
/// Higher scores for capturing valuable pieces with cheap pieces.
/// Format: MVV_LVA[attacker][victim]
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
#[derive(Clone, Copy, Debug)]
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
    /// Reference to continuation history table.
    continuation_history: &'a ContinuationHistory,
    /// The opponent's previous move, if known (absent at the root).
    previous_move: Option<Move>,
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
        Self::with_continuation_history(
            board,
            moves,
            hash_move,
            killers,
            history,
            &EMPTY_CONTINUATION_HISTORY,
            None,
            ply,
        )
    }

    /// Create a new move orderer, additionally scoring quiet moves with
    /// continuation history (how good `mv` has historically been as a reply
    /// to `previous_move`).
    ///
    /// # Arguments
    /// * `board` - The current position
    /// * `moves` - Legal moves to order
    /// * `hash_move` - Best move from transposition table (if any)
    /// * `killers` - Killer move table reference
    /// * `history` - History table reference
    /// * `continuation_history` - Continuation history table reference
    /// * `previous_move` - The opponent's previous move, if known
    /// * `ply` - Current search ply
    #[allow(clippy::too_many_arguments)]
    pub fn with_continuation_history(
        board: &Board,
        moves: Vec<Move>,
        hash_move: Option<Move>,
        killers: &'a KillerMoveTable,
        history: &'a HistoryTable,
        continuation_history: &'a ContinuationHistory,
        previous_move: Option<Move>,
        ply: u8,
    ) -> Self {
        let mut orderer = Self {
            moves: moves.into_iter().map(|mv| ScoredMove::new(mv, 0)).collect(),
            current: 0,
            hash_move,
            killers,
            history,
            continuation_history,
            previous_move,
            ply,
        };
        orderer.score_moves(board);
        // Sort once up front so `next()` is O(1) instead of doing a fresh
        // linear scan for the max on every call (was O(n^2) per node via
        // selection sort). Use a stable sort so ties (e.g. quiet moves with
        // identical history score) keep natural move-generation order rather
        // than an arbitrary unstable order.
        orderer.moves.sort_by(|a, b| b.score.cmp(&a.score));
        orderer
    }

    /// Score all moves for ordering.
    fn score_moves(&mut self, board: &Board) {
        let color = board.side_to_move();

        for scored_move in &mut self.moves {
            let mv = &scored_move.mv;

            // Hash move gets highest priority.
            if let Some(hash_mv) = &self.hash_move {
                if mv == hash_mv {
                    scored_move.score = scores::HASH_MOVE;
                    continue;
                }
            }

            // Captures are scored by MVV-LVA; SEE is only invoked when the
            // attacker is worth more than the victim, since those are the
            // only captures that can plausibly lose material (avoids paying
            // for a full SEE walk on every capture, which dominated node
            // cost in profiling).
            if let Some(victim) = mv.captured {
                let attacker_value = SEE_PIECE_VALUES[mv.piece.piece_type as usize];
                let victim_value = SEE_PIECE_VALUES[victim.piece_type as usize];

                let see_score = if attacker_value > victim_value {
                    Some(see(board, mv))
                } else {
                    None
                };

                if see_score.is_none_or(|s| s >= 0) {
                    // Good capture: base score + MVV-LVA for ordering among good captures.
                    scored_move.score =
                        scores::GOOD_CAPTURE + mvv_lva_score(mv.piece.piece_type, victim.piece_type);
                } else {
                    // Bad capture: negative score.
                    scored_move.score = scores::BAD_CAPTURE + see_score.unwrap();
                }
                continue;
            }

            // Promotions are valuable.
            if let MoveType::Promotion(promo_type) = mv.move_type {
                scored_move.score = scores::GOOD_CAPTURE + SEE_PIECE_VALUES[promo_type as usize];
                continue;
            }

            // Killer moves get priority for quiet moves.
            if let Some(slot) = self.killers.is_killer(self.ply, mv) {
                scored_move.score = if slot == 0 {
                    scores::KILLER_PRIMARY
                } else {
                    scores::KILLER_SECONDARY
                };
                continue;
            }

            // Quiet moves use history heuristic, plus continuation history
            // (how good this move has been as a reply to the previous move).
            // Both live on the same MAX_HISTORY_SCORE scale, so they're
            // simply added together.
            let continuation_score = match &self.previous_move {
                Some(prev) => self.continuation_history.get(prev, mv),
                None => 0,
            };
            scored_move.score = scores::QUIET_BASE + self.history.get(color, mv) + continuation_score;
        }
    }

    /// Get the next best move.
    ///
    /// Moves are sorted by score once in `new()`, so this is just a linear
    /// walk through the already-ordered list.
    pub fn next(&mut self) -> Option<Move> {
        if self.current >= self.moves.len() {
            return None;
        }

        let mv = self.moves[self.current].mv;
        self.current += 1;

        Some(mv)
    }

    /// Check if there are more moves to try.
    pub fn has_moves(&self) -> bool {
        self.current < self.moves.len()
    }

    /// Get the number of remaining moves.
    pub fn remaining(&self) -> usize {
        self.moves.len() - self.current
    }
}

/// Calculate MVV-LVA score for a capture.
///
/// # Arguments
/// * `attacker` - The piece type making the capture
/// * `victim` - The piece type being captured
#[inline]
pub fn mvv_lva_score(attacker: PieceType, victim: PieceType) -> i32 {
    MVV_LVA[attacker as usize][victim as usize]
}

/// Get all attackers to a square for a given color.
///
/// Returns a bitboard of all pieces of the given color that attack the square.
fn get_attackers(board: &Board, square: Square, by_color: Color, occupied: u64) -> u64 {
    let occupied_bb = Bitboard(occupied);

    // Pawn attackers
    let pawn_attacks = ATTACK_TABLE.pawn_attacks(square, by_color.opposite());
    let pawns = board.piece_bitboard(by_color, PieceType::Pawn).0 & pawn_attacks.0;

    // Knight attackers
    let knight_attacks = ATTACK_TABLE.knight_attacks(square);
    let knights = board.piece_bitboard(by_color, PieceType::Knight).0 & knight_attacks.0;

    // Bishop/Queen attackers (diagonal)
    let bishop_attacks = ATTACK_TABLE.bishop_attacks(square, occupied_bb);
    let bishops = board.piece_bitboard(by_color, PieceType::Bishop).0 & bishop_attacks.0;
    let queens_diag = board.piece_bitboard(by_color, PieceType::Queen).0 & bishop_attacks.0;

    // Rook/Queen attackers (straight)
    let rook_attacks = ATTACK_TABLE.rook_attacks(square, occupied_bb);
    let rooks = board.piece_bitboard(by_color, PieceType::Rook).0 & rook_attacks.0;
    let queens_straight = board.piece_bitboard(by_color, PieceType::Queen).0 & rook_attacks.0;

    // King attackers
    let king_attacks = ATTACK_TABLE.king_attacks(square);
    let king = board.piece_bitboard(by_color, PieceType::King).0 & king_attacks.0;

    pawns | knights | bishops | rooks | (queens_diag | queens_straight) | king
}

/// Get the least valuable attacker from a set of attackers.
///
/// Returns the piece type and clears it from the attackers bitboard.
fn get_least_valuable_attacker(
    board: &Board,
    attackers: &mut u64,
    color: Color,
) -> Option<PieceType> {
    // Check pieces in order of value (least to most).
    for piece_type in [
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ] {
        let pieces = board.piece_bitboard(color, piece_type).0;
        let overlap = *attackers & pieces;
        if overlap != 0 {
            // Remove the least significant bit (one attacker).
            let lsb = overlap & overlap.wrapping_neg();
            *attackers &= !lsb;
            return Some(piece_type);
        }
    }
    None
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
    // Non-captures always have SEE of 0.
    let captured = match mv.captured {
        Some(c) => c,
        None => return 0,
    };

    let target_square = mv.to;
    let mut occupied = board.all_pieces().0;

    // Remove the attacking piece from occupied.
    occupied &= !(1u64 << mv.from.index());

    // Get the value of the initial capture.
    let mut gain = [0i32; 32];
    let mut depth = 0;
    gain[depth] = SEE_PIECE_VALUES[captured.piece_type as usize];

    // The piece on the target square after the initial capture.
    let mut attacker_piece = mv.piece.piece_type;
    let mut side_to_move = mv.piece.color.opposite();

    // Get all attackers to the target square.
    let mut attackers = get_attackers(board, target_square, Color::White, occupied)
        | get_attackers(board, target_square, Color::Black, occupied);

    loop {
        depth += 1;
        if depth >= 32 {
            break;
        }

        // Negamax the gain.
        gain[depth] = SEE_PIECE_VALUES[attacker_piece as usize] - gain[depth - 1];

        // Pruning: if we can't improve, stop.
        if (-gain[depth - 1]).max(gain[depth]) < 0 {
            break;
        }

        // Get attackers for current side.
        let side_attackers = attackers & board.color_bitboard(side_to_move).0 & occupied;

        if side_attackers == 0 {
            break;
        }

        // Find the least valuable attacker.
        let mut temp_attackers = side_attackers;
        let next_attacker = get_least_valuable_attacker(board, &mut temp_attackers, side_to_move);

        match next_attacker {
            Some(piece) => {
                // Remove this attacker from occupied and attackers.
                let lsb = side_attackers & side_attackers.wrapping_neg();
                occupied &= !lsb;
                attackers &= occupied;

                // Add any x-ray attackers that are now revealed.
                let new_attackers = get_attackers(board, target_square, Color::White, occupied)
                    | get_attackers(board, target_square, Color::Black, occupied);
                attackers |= new_attackers & occupied;

                attacker_piece = piece;
                side_to_move = side_to_move.opposite();
            }
            None => break,
        }
    }

    // Unwind the gain stack.
    while depth > 1 {
        depth -= 1;
        gain[depth - 1] = -(-gain[depth - 1]).max(gain[depth]);
    }

    gain[0]
}

/// Check if a capture is likely good (winning or equal exchange).
///
/// Fast approximation of SEE for move ordering.
#[inline]
pub fn is_good_capture(board: &Board, mv: &Move) -> bool {
    see(board, mv) >= 0
}

/// Order moves for quiescence search (captures only).
///
/// Uses MVV-LVA ordering without killer/history heuristics.
pub fn order_captures(captures: Vec<Move>) -> Vec<Move> {
    let mut scored: Vec<ScoredMove> = captures
        .into_iter()
        .map(|mv| {
            let score = if let Some(captured) = mv.captured {
                mvv_lva_score(mv.piece.piece_type, captured.piece_type)
            } else {
                // Promotions without captures.
                if let MoveType::Promotion(promo_type) = mv.move_type {
                    SEE_PIECE_VALUES[promo_type as usize]
                } else {
                    0
                }
            };
            ScoredMove::new(mv, score)
        })
        .collect();

    // Sort by score descending.
    scored.sort_by(|a, b| b.score.cmp(&a.score));

    scored.into_iter().map(|sm| sm.mv).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::MoveType;
    use crate::types::Piece;

    fn make_move(from: &str, to: &str, piece_type: PieceType, color: Color) -> Move {
        Move {
            from: Square::from_algebraic(from).unwrap(),
            to: Square::from_algebraic(to).unwrap(),
            move_type: MoveType::Normal,
            piece: Piece { piece_type, color },
            captured: None,
        }
    }

    fn make_capture(
        from: &str,
        to: &str,
        attacker: PieceType,
        victim: PieceType,
        color: Color,
    ) -> Move {
        Move {
            from: Square::from_algebraic(from).unwrap(),
            to: Square::from_algebraic(to).unwrap(),
            move_type: MoveType::Capture,
            piece: Piece {
                piece_type: attacker,
                color,
            },
            captured: Some(Piece {
                piece_type: victim,
                color: color.opposite(),
            }),
        }
    }

    // ==================== MVV-LVA Tests ====================

    #[test]
    fn test_mvv_lva_pawn_takes_queen() {
        let score = mvv_lva_score(PieceType::Pawn, PieceType::Queen);
        assert_eq!(score, 505);
    }

    #[test]
    fn test_mvv_lva_queen_takes_pawn() {
        let score = mvv_lva_score(PieceType::Queen, PieceType::Pawn);
        assert_eq!(score, 101);
    }

    #[test]
    fn test_mvv_lva_ordering() {
        // PxQ should score higher than QxP.
        let pxq = mvv_lva_score(PieceType::Pawn, PieceType::Queen);
        let qxp = mvv_lva_score(PieceType::Queen, PieceType::Pawn);
        assert!(pxq > qxp);

        // NxQ should score higher than NxP.
        let nxq = mvv_lva_score(PieceType::Knight, PieceType::Queen);
        let nxp = mvv_lva_score(PieceType::Knight, PieceType::Pawn);
        assert!(nxq > nxp);

        // PxN should score higher than QxN (same victim, cheaper attacker).
        let pxn = mvv_lva_score(PieceType::Pawn, PieceType::Knight);
        let qxn = mvv_lva_score(PieceType::Queen, PieceType::Knight);
        assert!(pxn > qxn);
    }

    // ==================== MoveOrderer Tests ====================

    #[test]
    fn test_hash_move_first() {
        let board = Board::default();
        let killers = KillerMoveTable::new();
        let history = HistoryTable::new();

        let mv1 = make_move("e2", "e4", PieceType::Pawn, Color::White);
        let mv2 = make_move("d2", "d4", PieceType::Pawn, Color::White);
        let mv3 = make_move("g1", "f3", PieceType::Knight, Color::White);

        let moves = vec![mv1, mv2, mv3];
        let hash_move = Some(mv2);

        let mut orderer = MoveOrderer::new(&board, moves, hash_move, &killers, &history, 0);

        // Hash move should be returned first.
        assert_eq!(orderer.next(), Some(mv2));
    }

    #[test]
    fn test_killer_move_priority() {
        let board = Board::default();
        let mut killers = KillerMoveTable::new();
        let history = HistoryTable::new();

        let quiet1 = make_move("e2", "e4", PieceType::Pawn, Color::White);
        let quiet2 = make_move("d2", "d4", PieceType::Pawn, Color::White);
        let quiet3 = make_move("g1", "f3", PieceType::Knight, Color::White);

        // Store quiet2 as a killer move at ply 5.
        killers.store(5, quiet2);

        let moves = vec![quiet1, quiet2, quiet3];
        let mut orderer = MoveOrderer::new(&board, moves, None, &killers, &history, 5);

        // Killer move should be returned first.
        assert_eq!(orderer.next(), Some(quiet2));
    }

    #[test]
    fn test_capture_ordering() {
        // Test that good captures are ordered by MVV-LVA.
        let pxq = mvv_lva_score(PieceType::Pawn, PieceType::Queen);
        let nxr = mvv_lva_score(PieceType::Knight, PieceType::Rook);
        let bxn = mvv_lva_score(PieceType::Bishop, PieceType::Knight);

        assert!(pxq > nxr);
        assert!(nxr > bxn);
    }

    #[test]
    fn test_orderer_exhaustion() {
        let board = Board::default();
        let killers = KillerMoveTable::new();
        let history = HistoryTable::new();

        let mv1 = make_move("e2", "e4", PieceType::Pawn, Color::White);
        let mv2 = make_move("d2", "d4", PieceType::Pawn, Color::White);

        let moves = vec![mv1, mv2];
        let mut orderer = MoveOrderer::new(&board, moves, None, &killers, &history, 0);

        assert!(orderer.has_moves());
        assert_eq!(orderer.remaining(), 2);

        orderer.next();
        assert!(orderer.has_moves());
        assert_eq!(orderer.remaining(), 1);

        orderer.next();
        assert!(!orderer.has_moves());
        assert_eq!(orderer.remaining(), 0);

        assert_eq!(orderer.next(), None);
    }

    #[test]
    fn test_order_captures_mvv_lva() {
        let captures = vec![
            make_capture("e4", "d5", PieceType::Pawn, PieceType::Pawn, Color::White),
            make_capture("d1", "d5", PieceType::Queen, PieceType::Pawn, Color::White),
            make_capture("e4", "d5", PieceType::Pawn, PieceType::Queen, Color::White),
        ];

        let ordered = order_captures(captures);

        // PxQ should be first (score 505).
        assert_eq!(ordered[0].piece.piece_type, PieceType::Pawn);
        assert_eq!(ordered[0].captured.unwrap().piece_type, PieceType::Queen);

        // PxP should be before QxP (cheaper attacker).
        let pxp_idx = ordered
            .iter()
            .position(|m| {
                m.piece.piece_type == PieceType::Pawn
                    && m.captured.unwrap().piece_type == PieceType::Pawn
            })
            .unwrap();
        let qxp_idx = ordered
            .iter()
            .position(|m| {
                m.piece.piece_type == PieceType::Queen
                    && m.captured.unwrap().piece_type == PieceType::Pawn
            })
            .unwrap();
        assert!(pxp_idx < qxp_idx);
    }

    // ==================== SEE Tests ====================

    #[test]
    fn test_see_simple_winning() {
        // Pawn takes undefended knight - should be positive.
        let board = Board::from_fen("4k3/8/8/3n4/4P3/8/8/4K3 w - - 0 1").unwrap();
        let mv = make_capture("e4", "d5", PieceType::Pawn, PieceType::Knight, Color::White);

        let see_score = see(&board, &mv);
        assert!(see_score > 0, "SEE should be positive: {}", see_score);
    }

    #[test]
    fn test_see_equal_exchange() {
        // Knight takes knight, defended by another knight.
        // Position: White knight e3, Black knight d5 defended by knight on b4.
        let board = Board::from_fen("4k3/8/8/3n4/1n6/4N3/8/4K3 w - - 0 1").unwrap();
        let mv = make_capture("e3", "d5", PieceType::Knight, PieceType::Knight, Color::White);

        // White Nxd5 (+320), Black Nxd5 (-320) = 0
        let see_score = see(&board, &mv);
        assert_eq!(see_score, 0, "Equal exchange should be 0: {}", see_score);
    }

    #[test]
    fn test_see_losing_capture() {
        // Queen takes pawn defended by knight - losing.
        // Position: White queen d1, Black pawn e4 defended by knight on f6.
        let board = Board::from_fen("4k3/8/5n2/8/4p3/8/8/3QK3 w - - 0 1").unwrap();
        let mv = make_capture("d1", "e4", PieceType::Queen, PieceType::Pawn, Color::White);

        // White Qxe4 (+100), Black Nxe4 (-900) = -800
        let see_score = see(&board, &mv);
        assert!(see_score < 0, "SEE should be negative: {}", see_score);
    }

    #[test]
    fn test_is_good_capture() {
        // Pawn takes undefended rook.
        let board = Board::from_fen("4k3/8/8/3r4/4P3/8/8/4K3 w - - 0 1").unwrap();
        let mv = make_capture("e4", "d5", PieceType::Pawn, PieceType::Rook, Color::White);

        assert!(is_good_capture(&board, &mv));
    }

    #[test]
    fn test_non_capture_see_zero() {
        let board = Board::default();
        let mv = make_move("e2", "e4", PieceType::Pawn, Color::White);

        assert_eq!(see(&board, &mv), 0);
    }

    // ==================== History Integration Test ====================

    #[test]
    fn test_history_affects_ordering() {
        let board = Board::default();
        let killers = KillerMoveTable::new();
        let mut history = HistoryTable::new();

        let mv1 = make_move("e2", "e4", PieceType::Pawn, Color::White);
        let mv2 = make_move("d2", "d4", PieceType::Pawn, Color::White);
        let mv3 = make_move("g1", "f3", PieceType::Knight, Color::White);

        // Give mv3 a high history score.
        for _ in 0..10 {
            history.update_cutoff(Color::White, &mv3, 5);
        }

        let moves = vec![mv1, mv2, mv3];
        let mut orderer = MoveOrderer::new(&board, moves, None, &killers, &history, 0);

        // mv3 should be first due to high history score.
        assert_eq!(orderer.next(), Some(mv3));
    }

    // ==================== Mixed Priority Tests ====================

    #[test]
    fn test_capture_beats_killer() {
        // Good captures should be ordered before killers.
        let board = Board::from_fen("4k3/8/8/3r4/4P3/8/8/4K3 w - - 0 1").unwrap();
        let mut killers = KillerMoveTable::new();
        let history = HistoryTable::new();

        let quiet = make_move("e1", "e2", PieceType::King, Color::White);
        let capture = make_capture("e4", "d5", PieceType::Pawn, PieceType::Rook, Color::White);

        killers.store(0, quiet);

        let moves = vec![quiet, capture];
        let mut orderer = MoveOrderer::new(&board, moves, None, &killers, &history, 0);

        // Capture should be first.
        assert_eq!(orderer.next(), Some(capture));
    }

    #[test]
    fn test_hash_move_beats_capture() {
        // Hash move should beat even good captures.
        let board = Board::from_fen("4k3/8/8/3r4/4P3/8/8/4K3 w - - 0 1").unwrap();
        let killers = KillerMoveTable::new();
        let history = HistoryTable::new();

        let quiet = make_move("e1", "e2", PieceType::King, Color::White);
        let capture = make_capture("e4", "d5", PieceType::Pawn, PieceType::Rook, Color::White);

        let moves = vec![capture, quiet];
        let mut orderer = MoveOrderer::new(&board, moves, Some(quiet), &killers, &history, 0);

        // Hash move should be first.
        assert_eq!(orderer.next(), Some(quiet));
    }
}
