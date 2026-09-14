use crate::bitboard::{Bitboard, ADJACENT_FILES, FILES, RANKS};
use crate::types::{Color, Square};
use crate::Board;

use super::phase::GamePhase;

// Precomputed attack-span masks, indexed `[color as usize][square]` with
// White = 0, Black = 1 (matching the `Color` discriminant order).
//
// `PASSED_MASK` holds `FILES[f] | ADJACENT_FILES[f]` intersected with the
// ranks in front of the pawn: ranks above (`rank+1..8`) for White, ranks
// below (`0..rank`) for Black. `CONNECTED_MASK` holds the exact 4 squares
// `is_connected_pawn` used to probe (same-rank neighbours plus one
// support-rank neighbour per side), including edge-file behavior where
// off-board squares contribute no bits.
const fn build_passed_masks() -> [[u64; 64]; 2] {
    let mut tables = [[0u64; 64]; 2];
    let mut sq = 0usize;
    while sq < 64 {
        let file = sq & 7;
        let rank = sq >> 3;
        let blocking = FILES[file].0 | ADJACENT_FILES[file].0;
        // White front span: ranks above the pawn.
        let mut white_front = 0u64;
        let mut r = rank + 1;
        while r < 8 {
            white_front |= RANKS[r].0;
            r += 1;
        }
        tables[0][sq] = blocking & white_front;
        // Black front span: ranks below the pawn.
        let mut black_front = 0u64;
        let mut r2 = 0usize;
        while r2 < rank {
            black_front |= RANKS[r2].0;
            r2 += 1;
        }
        tables[1][sq] = blocking & black_front;
        sq += 1;
    }
    tables
}

const fn connected_bit(file: isize, rank: isize) -> u64 {
    if file >= 0 && file < 8 && rank >= 0 && rank < 8 {
        1u64 << ((rank as usize) * 8 + (file as usize))
    } else {
        0
    }
}

const fn build_connected_masks() -> [[u64; 64]; 2] {
    let mut tables = [[0u64; 64]; 2];
    let mut sq = 0usize;
    while sq < 64 {
        let file = (sq & 7) as isize;
        let rank = (sq >> 3) as isize;
        // White support rank is one below, Black one above; same-rank
        // neighbours count for both colors.
        tables[0][sq] = connected_bit(file - 1, rank)
            | connected_bit(file - 1, rank - 1)
            | connected_bit(file + 1, rank)
            | connected_bit(file + 1, rank - 1);
        tables[1][sq] = connected_bit(file - 1, rank)
            | connected_bit(file - 1, rank + 1)
            | connected_bit(file + 1, rank)
            | connected_bit(file + 1, rank + 1);
        sq += 1;
    }
    tables
}

static PASSED_MASK: [[u64; 64]; 2] = build_passed_masks();
static CONNECTED_MASK: [[u64; 64]; 2] = build_connected_masks();

// Penalty/bonus values (centipawns)
// Tuned values - adjust based on testing
pub const DOUBLED_PAWN_MG: i32 = -10;
pub const DOUBLED_PAWN_EG: i32 = -20;

pub const ISOLATED_PAWN_MG: i32 = -15;
pub const ISOLATED_PAWN_EG: i32 = -10;

pub const BACKWARD_PAWN_MG: i32 = -10;
pub const BACKWARD_PAWN_EG: i32 = -15;

// Rank 1 and 8 are impossible for pawns
pub const PASSED_PAWN_MG: [i32; 8] = [0, 5, 10, 20, 35, 60, 100, 0];
pub const PASSED_PAWN_EG: [i32; 8] = [0, 10, 20, 40, 70, 120, 200, 0];

pub const CONNECTED_PAWN_MG: i32 = 5;
pub const CONNECTED_PAWN_EG: i32 = 10;

pub struct PawnStructureEval {
    phase: GamePhase,
}

impl PawnStructureEval {
    pub fn new() -> Self {
        Self {
            phase: GamePhase::new(),
        }
    }

    //TODO: ill deal with this later
    pub fn evaluate(&self, board: &Board) -> i32 {
        let phase = self.phase.calculate(board);
        self.evaluate_with_phase(board, phase)
    }

    pub fn evaluate_with_phase(&self, board: &Board, phase: i32) -> i32 {
        let mut mg_score = 0;
        let mut eg_score = 0;

        let white_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);
        let black_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        let (w_mg, w_eg) = self.evaluate_pawns(white_pawns, black_pawns, Color::White);
        mg_score += w_mg;
        eg_score += w_eg;

        let (b_mg, b_eg) = self.evaluate_pawns(black_pawns, white_pawns, Color::Black);
        mg_score -= b_mg;
        eg_score -= b_eg;

        self.phase.taper(mg_score, eg_score, phase)
    }

    fn evaluate_pawns(
        &self,
        our_pawns: Bitboard,
        enemy_pawns: Bitboard,
        color: Color,
    ) -> (i32, i32) {
        let mut mg_score = 0;
        let mut eg_score = 0;

        // Hoisted per-file occupancy: 8 ANDs + popcounts once per color,
        // instead of `our_pawns & FILES[file]` / `& ADJACENT_FILES[file]`
        // per pawn. Adjacent-file presence is derived from neighbour file
        // counts, which is exactly `(our & ADJACENT_FILES[f]).is_empty()`:
        // file 0 neighbours only file 1, file 7 only file 6.
        let mut file_counts = [0u32; 8];
        let mut f = 0usize;
        while f < 8 {
            file_counts[f] = (our_pawns.0 & FILES[f].0).count_ones();
            f += 1;
        }
        let mut has_adjacent = [false; 8];
        let mut g = 0usize;
        while g < 8 {
            has_adjacent[g] = if g == 0 {
                file_counts[1] > 0
            } else if g == 7 {
                file_counts[6] > 0
            } else {
                file_counts[g - 1] > 0 || file_counts[g + 1] > 0
            };
            g += 1;
        }

        let mut pawns = our_pawns;

        while pawns.0 != 0 {
            let sq = pawns.pop_lsb().unwrap();
            let file = sq.file() as usize;
            let rank = sq.rank() as usize;

            let eval_rank = if color == Color::White {
                rank
            } else {
                7 - rank
            };

            //doubled pawn check
            if file_counts[file] > 1 {
                mg_score += DOUBLED_PAWN_MG;
                eg_score += DOUBLED_PAWN_EG;
            }

            //iso pawn check
            if !has_adjacent[file] {
                mg_score += ISOLATED_PAWN_MG;
                eg_score += ISOLATED_PAWN_EG;
            }

            //Passed pawn check
            if self.is_passed_pawn(sq, enemy_pawns, color) {
                mg_score += PASSED_PAWN_MG[eval_rank];
                eg_score += PASSED_PAWN_EG[eval_rank];
            }

            //Connected pawn check
            if self.is_connected_pawn(sq, our_pawns, color) {
                mg_score += CONNECTED_PAWN_MG;
                eg_score += CONNECTED_PAWN_EG;
            }

            //Backward pawn check
            if self.is_backward_pawn(sq, our_pawns, enemy_pawns, color) {
                mg_score += BACKWARD_PAWN_MG;
                eg_score += BACKWARD_PAWN_EG;
            }
        }

        return (mg_score, eg_score);
    }

    fn is_passed_pawn(&self, sq: Square, enemy_pawns: Bitboard, color: Color) -> bool {
        (enemy_pawns.0 & PASSED_MASK[color as usize][sq.index()]) == 0
    }

    fn is_connected_pawn(&self, sq: Square, our_pawns: Bitboard, color: Color) -> bool {
        (our_pawns.0 & CONNECTED_MASK[color as usize][sq.index()]) != 0
    }

    fn is_backward_pawn(
        &self,
        sq: Square,
        our_pawns: Bitboard,
        enemy_pawns: Bitboard,
        color: Color,
    ) -> bool {
        let file = sq.file() as usize;
        let rank = sq.rank();

        if self.is_connected_pawn(sq, our_pawns, color) {
            return false;
        }

        let adjacent = our_pawns & ADJACENT_FILES[file];
        if adjacent.is_empty() {
            return false; // Isolated, not backward
        }

        let mut adj = adjacent;
        while adj.0 != 0 {
            let adj_sq = adj.pop_lsb().unwrap();
            let adj_rank = adj_sq.rank();

            let is_ahead = if color == Color::White {
                adj_rank > rank
            } else {
                adj_rank < rank
            };

            if !is_ahead {
                return false; // Found a pawn not ahead, so not backward
            }
        }

        let stop_rank = if color == Color::White {
            rank + 1
        } else {
            rank.wrapping_sub(1)
        };
        if stop_rank >= 8 {
            return false;
        }

        let enemy_attack_files = ADJACENT_FILES[file];
        let enemy_attack_rank = if color == Color::White {
            if stop_rank + 1 < 8 {
                RANKS[(stop_rank + 1) as usize]
            } else {
                Bitboard::EMPTY
            }
        } else {
            if stop_rank > 0 {
                RANKS[(stop_rank - 1) as usize]
            } else {
                Bitboard::EMPTY
            }
        };

        let enemy_attackers = enemy_pawns & enemy_attack_files & enemy_attack_rank;
        enemy_attackers.is_not_empty()
    }
}

impl Default for PawnStructureEval {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Color, Square};
    use crate::Board;

    // ==================== Doubled Pawns Tests ====================

    #[test]
    fn test_doubled_pawns_detected() {
        // Two white pawns on the e-file (e2 and e4) with black pawns to block passed pawn bonus
        let board = Board::from_fen("8/4p3/8/8/4P3/8/4P3/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let (mg, eg) = eval.evaluate_pawns(
            board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );

        // Both pawns are doubled and isolated
        // e2: doubled (-10 MG) + isolated (-15 MG) = -25 MG
        // e4: doubled (-10 MG) + isolated (-15 MG) = -25 MG
        // Total: -50 MG
        let expected_mg = 2 * (DOUBLED_PAWN_MG + ISOLATED_PAWN_MG);
        let expected_eg = 2 * (DOUBLED_PAWN_EG + ISOLATED_PAWN_EG);
        assert_eq!(mg, expected_mg);
        assert_eq!(eg, expected_eg);
    }

    #[test]
    fn test_tripled_pawns_detected() {
        // Three white pawns on the e-file, black pawn to block passed
        let board = Board::from_fen("8/4p3/8/4P3/4P3/8/4P3/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let (mg, eg) = eval.evaluate_pawns(
            board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );

        // All three pawns are doubled and isolated
        let expected_mg = 3 * (DOUBLED_PAWN_MG + ISOLATED_PAWN_MG);
        let expected_eg = 3 * (DOUBLED_PAWN_EG + ISOLATED_PAWN_EG);
        assert_eq!(mg, expected_mg);
        assert_eq!(eg, expected_eg);
    }

    #[test]
    fn test_no_doubled_pawns_with_blockers() {
        // Pawns on different files with enemy blockers
        let board = Board::from_fen("8/p1p1p1p1/8/8/8/8/P1P1P1P1/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let (mg, eg) = eval.evaluate_pawns(
            board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );

        // Pawns on a, c, e, g files - all isolated, no doubled, not passed (blocked)
        let expected_mg = 4 * ISOLATED_PAWN_MG;
        let expected_eg = 4 * ISOLATED_PAWN_EG;
        assert_eq!(mg, expected_mg);
        assert_eq!(eg, expected_eg);
    }

    // ==================== Isolated Pawns Tests ====================

    #[test]
    fn test_isolated_pawn_detected() {
        // Single pawn on a-file with black pawn blocking
        let board = Board::from_fen("8/p7/8/8/8/8/P7/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let (mg, eg) = eval.evaluate_pawns(
            board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );

        // Isolated pawn penalty only
        assert_eq!(mg, ISOLATED_PAWN_MG);
        assert_eq!(eg, ISOLATED_PAWN_EG);
    }

    #[test]
    fn test_not_isolated_with_adjacent_pawn() {
        // Pawns on adjacent files (d2 and e2) with blockers
        let board = Board::from_fen("8/3pp3/8/8/8/8/3PP3/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let (mg, eg) = eval.evaluate_pawns(
            board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );

        // Connected pawns bonus, no isolated penalty
        let expected_mg = 2 * CONNECTED_PAWN_MG;
        let expected_eg = 2 * CONNECTED_PAWN_EG;
        assert_eq!(mg, expected_mg);
        assert_eq!(eg, expected_eg);
    }

    #[test]
    fn test_isolated_pawn_on_h_file() {
        // Single pawn on h-file with blocker
        let board = Board::from_fen("8/7p/8/8/8/8/7P/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let (mg, eg) = eval.evaluate_pawns(
            board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );

        // Isolated pawn penalty only
        assert_eq!(mg, ISOLATED_PAWN_MG);
        assert_eq!(eg, ISOLATED_PAWN_EG);
    }

    // ==================== Passed Pawns Tests ====================

    #[test]
    fn test_passed_pawn_no_blockers() {
        // White pawn on e5, no black pawns
        let board = Board::from_fen("8/8/8/4P3/8/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let sq = Square::from_algebraic("e5").unwrap();
        let enemy_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        assert!(eval.is_passed_pawn(sq, enemy_pawns, Color::White));
    }

    #[test]
    fn test_passed_pawn_blocked_by_enemy() {
        // White pawn on e5, black pawn on e6
        let board = Board::from_fen("8/8/4p3/4P3/8/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let sq = Square::from_algebraic("e5").unwrap();
        let enemy_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        assert!(!eval.is_passed_pawn(sq, enemy_pawns, Color::White));
    }

    #[test]
    fn test_passed_pawn_blocked_on_adjacent_file() {
        // White pawn on e5, black pawn on d6
        let board = Board::from_fen("8/8/3p4/4P3/8/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let sq = Square::from_algebraic("e5").unwrap();
        let enemy_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        assert!(!eval.is_passed_pawn(sq, enemy_pawns, Color::White));
    }

    #[test]
    fn test_passed_pawn_enemy_behind() {
        // White pawn on e5, black pawn on e4 (behind)
        let board = Board::from_fen("8/8/8/4P3/4p3/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let sq = Square::from_algebraic("e5").unwrap();
        let enemy_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        assert!(eval.is_passed_pawn(sq, enemy_pawns, Color::White));
    }

    #[test]
    fn test_passed_pawn_black() {
        // Black pawn on e4, no white pawns
        let board = Board::from_fen("8/8/8/8/4p3/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let sq = Square::from_algebraic("e4").unwrap();
        let enemy_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);

        assert!(eval.is_passed_pawn(sq, enemy_pawns, Color::Black));
    }

    #[test]
    fn test_passed_pawn_bonus_by_rank_white() {
        // Passed pawn on 6th rank (eval_rank 5 for white)
        let board = Board::from_fen("8/8/4P3/8/8/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let (mg, eg) = eval.evaluate_pawns(
            board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );

        // Pawn on rank 6 (eval_rank 5 for white) plus isolated penalty
        let expected_mg = PASSED_PAWN_MG[5] + ISOLATED_PAWN_MG;
        let expected_eg = PASSED_PAWN_EG[5] + ISOLATED_PAWN_EG;
        assert_eq!(mg, expected_mg);
        assert_eq!(eg, expected_eg);
    }

    // ==================== Connected Pawns Tests ====================

    #[test]
    fn test_connected_pawns_side_by_side() {
        // Pawns on d4 and e4
        let board = Board::from_fen("8/8/8/8/3PP3/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let d4 = Square::from_algebraic("d4").unwrap();
        let e4 = Square::from_algebraic("e4").unwrap();
        let our_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);

        assert!(eval.is_connected_pawn(d4, our_pawns, Color::White));
        assert!(eval.is_connected_pawn(e4, our_pawns, Color::White));
    }

    #[test]
    fn test_connected_pawns_defender_behind() {
        // Pawn on e4 defended by d3
        // is_connected_pawn checks (file-1, rank) and (file-1, rank-1)
        // e4 checks d4, d3, f4, f3 - d3 is at (3, 2)
        let board = Board::from_fen("8/8/8/8/4P3/3P4/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let d3 = Square::from_algebraic("d3").unwrap();
        let e4 = Square::from_algebraic("e4").unwrap();
        let our_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);

        // e4 finds d3 (checking one rank behind on adjacent file)
        assert!(eval.is_connected_pawn(e4, our_pawns, Color::White));
        // d3 checks c3, c2, e3, e2 - e4 is not checked, so d3 is NOT connected
        assert!(!eval.is_connected_pawn(d3, our_pawns, Color::White));
    }

    #[test]
    fn test_not_connected_pawns() {
        // Pawns on d2 and f2 (not connected - gap between)
        let board = Board::from_fen("8/8/8/8/8/8/3P1P2/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let d2 = Square::from_algebraic("d2").unwrap();
        let f2 = Square::from_algebraic("f2").unwrap();
        let our_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);

        assert!(!eval.is_connected_pawn(d2, our_pawns, Color::White));
        assert!(!eval.is_connected_pawn(f2, our_pawns, Color::White));
    }

    #[test]
    fn test_connected_pawn_chain() {
        // Pawn chain: c3, d4, e5
        // c3 checks b3, b2, d3, d2 - none present, NOT connected
        // d4 checks c4, c3, e4, e3 - c3 present, CONNECTED
        // e5 checks d5, d4, f5, f4 - d4 present, CONNECTED
        let board = Board::from_fen("8/8/8/4P3/3P4/2P5/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let c3 = Square::from_algebraic("c3").unwrap();
        let d4 = Square::from_algebraic("d4").unwrap();
        let e5 = Square::from_algebraic("e5").unwrap();
        let our_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);

        // c3 is the base of the chain - has no defender behind
        assert!(!eval.is_connected_pawn(c3, our_pawns, Color::White));
        // d4 is defended by c3
        assert!(eval.is_connected_pawn(d4, our_pawns, Color::White));
        // e5 is defended by d4
        assert!(eval.is_connected_pawn(e5, our_pawns, Color::White));
    }

    // ==================== Backward Pawns Tests ====================

    #[test]
    fn test_backward_pawn_basic() {
        // White pawn on e3, friendly pawns on d4 and f4 (both ahead)
        // Black pawn on d5 attacks e4 (the stop square)
        let board = Board::from_fen("8/8/8/3p4/3P1P2/4P3/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let e3 = Square::from_algebraic("e3").unwrap();
        let our_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);
        let enemy_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        assert!(eval.is_backward_pawn(e3, our_pawns, enemy_pawns, Color::White));
    }

    #[test]
    fn test_not_backward_when_connected() {
        // Pawn on e4 with pawn on d4 - connected, so not backward
        let board = Board::from_fen("8/8/8/8/3PP3/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let e4 = Square::from_algebraic("e4").unwrap();
        let our_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);
        let enemy_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        assert!(!eval.is_backward_pawn(e4, our_pawns, enemy_pawns, Color::White));
    }

    #[test]
    fn test_not_backward_when_isolated() {
        // Single isolated pawn - isolated, not backward
        let board = Board::from_fen("8/8/8/8/4P3/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let e4 = Square::from_algebraic("e4").unwrap();
        let our_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);
        let enemy_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        assert!(!eval.is_backward_pawn(e4, our_pawns, enemy_pawns, Color::White));
    }

    #[test]
    fn test_not_backward_friendly_pawn_behind() {
        // e4 with d3 behind - not backward since d3 is behind e4
        let board = Board::from_fen("8/8/8/8/4P3/3P4/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let e4 = Square::from_algebraic("e4").unwrap();
        let our_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);
        let enemy_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

        assert!(!eval.is_backward_pawn(e4, our_pawns, enemy_pawns, Color::White));
    }

    #[test]
    fn test_backward_pawn_black() {
        // Black pawn on e6 with an adjacent own pawn ahead on d5,
        // and a white pawn on d4 attacking the e5 stop square.
        // e6 is not defended from behind (black defenders would be on
        // rank 7), so it is backward.
        let board = Board::from_fen("8/8/4p3/3p4/3P4/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();

        let e6 = Square::from_algebraic("e6").unwrap();
        let our_pawns = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);
        let enemy_pawns = board.piece_bitboard(Color::White, crate::PieceType::Pawn);

        // e6 (file 4, rank 5) checks: d6, d7, f6, f7 - none present, not connected
        // adjacent pawn d5 (rank 4) is ahead of e6 for Black
        // stop square e5 is attacked by white pawn d4
        assert!(eval.is_backward_pawn(e6, our_pawns, enemy_pawns, Color::Black));

        // Mirror check: white pawn on e3 with adjacent own pawn ahead on d4
        // and black pawn on d5 attacking e4 is also backward (symmetry).
        let board_w = Board::from_fen("8/8/8/3p4/3P4/4P3/8/8 w - - 0 1").unwrap();
        let e3 = Square::from_algebraic("e3").unwrap();
        let our_w = board_w.piece_bitboard(Color::White, crate::PieceType::Pawn);
        let enemy_w = board_w.piece_bitboard(Color::Black, crate::PieceType::Pawn);
        assert!(eval.is_backward_pawn(e3, our_w, enemy_w, Color::White));
    }

    // ==================== Full Evaluation Tests ====================

    #[test]
    fn test_evaluate_symmetric_position() {
        // Symmetric pawn structure: blocked pawn rows for both sides.
        // Neither side has passed pawns (each pawn is blocked by an enemy
        // pawn), and connected bonuses cancel out, so the score must be 0.
        let board = Board::from_fen("8/pppppppp/8/8/8/8/PPPPPPPP/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let score = eval.evaluate(&board);

        assert_eq!(score, 0, "Symmetric position should evaluate to 0, got {}", score);
    }

    #[test]
    fn test_evaluate_white_passed_pawn_advantage() {
        // White has a passed pawn on e6
        // Use a position where the passed pawn advantage is clear
        let board = Board::from_fen("8/8/4P3/8/8/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let score = eval.evaluate(&board);

        // Single white passed pawn should give white advantage
        // e6 (rank 5, eval_rank 5): passed (+60 MG) + isolated (-15 MG) = +45 MG
        assert!(score > 0, "White with passed pawn should have positive eval, got {}", score);
    }

    #[test]
    fn test_evaluate_white_doubled_pawns_disadvantage() {
        // White has doubled pawns on e-file
        let board = Board::from_fen("8/pppppppp/8/8/4P3/8/PPPPPPPP/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let score = eval.evaluate(&board);

        // White should have negative score due to doubled pawns
        assert!(score < 0, "White with doubled pawns should have negative eval, got {}", score);
    }

    #[test]
    fn test_evaluate_black_isolated_pawn_disadvantage() {
        // Black has isolated a-pawn, white has all pawns connected
        let board = Board::from_fen("8/p4ppp/8/8/8/8/PPPPPPPP/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let score = eval.evaluate(&board);

        // White should have positive score (black has weakness)
        assert!(score > 0, "Black with isolated pawn should give white positive eval, got {}", score);
    }

    #[test]
    fn test_evaluate_complex_position() {
        // Complex position with multiple features
        let board = Board::from_fen("8/2pp2p1/2p5/P7/3PP3/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let score = eval.evaluate(&board);

        // Just verify it produces a reasonable score
        assert!(score.abs() < 500, "Score should be reasonable, got {}", score);
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_empty_board() {
        // No pawns at all
        let board = Board::from_fen("8/8/8/8/8/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let score = eval.evaluate(&board);

        assert_eq!(score, 0, "Empty board should have zero eval");
    }

    #[test]
    fn test_single_white_pawn() {
        // Single white pawn - passed and isolated
        let board = Board::from_fen("8/8/8/8/4P3/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let (mg, eg) = eval.evaluate_pawns(
            board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );

        // e4 (rank 3, eval_rank 3): passed (+20 MG) + isolated (-15 MG) = +5 MG
        let expected_mg = PASSED_PAWN_MG[3] + ISOLATED_PAWN_MG;
        let expected_eg = PASSED_PAWN_EG[3] + ISOLATED_PAWN_EG;
        assert_eq!(mg, expected_mg);
        assert_eq!(eg, expected_eg);
    }

    #[test]
    fn test_pawn_on_7th_rank() {
        // White pawn on 7th rank - about to promote
        let board = Board::from_fen("8/4P3/8/8/8/8/8/8 w - - 0 1").unwrap();
        let eval = PawnStructureEval::new();
        let (mg, eg) = eval.evaluate_pawns(
            board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );

        // e7 (rank 6, eval_rank 6): passed (+100 MG) + isolated (-15 MG) = +85 MG
        let expected_mg = PASSED_PAWN_MG[6] + ISOLATED_PAWN_MG;
        let expected_eg = PASSED_PAWN_EG[6] + ISOLATED_PAWN_EG;
        assert_eq!(mg, expected_mg);
        assert_eq!(eg, expected_eg);
    }

    // ==================== Mirror Symmetry Tests ====================

    /// Flip a pawn bitboard vertically (rank r -> 7 - r), preserving files.
    fn mirror_pawns(bb: Bitboard) -> Bitboard {
        let mut out = 0u64;
        let mut b = bb.0;
        while b != 0 {
            let i = b.trailing_zeros() as usize;
            b &= b - 1;
            let f = i & 7;
            let r = i >> 3;
            out |= 1u64 << ((7 - r) * 8 + f);
        }
        Bitboard(out)
    }

    #[test]
    fn test_mirrored_pawn_evaluation_symmetry() {
        // Differential test for the table-driven rewrite: pawn evaluation is
        // color-symmetric, so evaluating a setup from its own side must equal
        // evaluating the rank-flipped, color-swapped setup from the opposite
        // side. Positions cover edge files (a/h), 7th-rank pawns, and blocked
        // chains with pawns for both colors.
        let eval = PawnStructureEval::new();
        let fens = [
            // Edge files a/h, both colors, blocked files (no passed pawns).
            "8/p6p/8/8/8/8/P6P/8 w - - 0 1",
            // 7th-rank white pawn plus 2nd-rank black pawn.
            "8/4P3/8/8/8/8/3p4/8 w - - 0 1",
            // Blocked/interlocked center pawns.
            "8/8/8/2pPp3/3P4/8/8/8 w - - 0 1",
            // Blocked chains on several files plus edge pawns.
            "8/pp4pp/8/3p4/3P4/8/PP4PP/8 w - - 0 1",
            // Complex position with edge, passed, and blocked pawns.
            "8/2pp2p1/2p5/P7/3PP3/8/8/8 w - - 0 1",
            // 7th-rank edge pawns facing each other, blocked chains.
            "8/P5p1/8/2p5/2P5/1p6/P5P1/8 w - - 0 1",
        ];

        for fen in fens {
            let board = Board::from_fen(fen).unwrap();
            let wp = board.piece_bitboard(Color::White, crate::PieceType::Pawn);
            let bp = board.piece_bitboard(Color::Black, crate::PieceType::Pawn);

            let (w_mg, w_eg) = eval.evaluate_pawns(wp, bp, Color::White);
            let (b_mg, b_eg) = eval.evaluate_pawns(bp, wp, Color::Black);

            // Rank-flipped, color-swapped setup.
            let wp_m = mirror_pawns(bp);
            let bp_m = mirror_pawns(wp);
            let (w_mg_m, w_eg_m) = eval.evaluate_pawns(wp_m, bp_m, Color::White);
            let (b_mg_m, b_eg_m) = eval.evaluate_pawns(bp_m, wp_m, Color::Black);

            assert_eq!(
                (w_mg, w_eg),
                (b_mg_m, b_eg_m),
                "white setup {} evaluated as White must equal its mirror evaluated as Black",
                fen
            );
            assert_eq!(
                (b_mg, b_eg),
                (w_mg_m, w_eg_m),
                "black setup {} evaluated as Black must equal its mirror evaluated as White",
                fen
            );
        }

        // Explicit white-setup vs rank-flipped black-setup pair, evaluated
        // from their respective sides to move (edge a-file + 7th-rank pawn).
        let white_board =
            Board::from_fen("8/4P3/8/2P5/8/1P6/P7/8 w - - 0 1").unwrap();
        let black_board =
            Board::from_fen("8/p7/1p6/8/2p5/8/4p3/8 b - - 0 1").unwrap();
        let (w_mg, w_eg) = eval.evaluate_pawns(
            white_board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            white_board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            Color::White,
        );
        let (b_mg, b_eg) = eval.evaluate_pawns(
            black_board.piece_bitboard(Color::Black, crate::PieceType::Pawn),
            black_board.piece_bitboard(Color::White, crate::PieceType::Pawn),
            Color::Black,
        );
        assert_eq!((w_mg, w_eg), (b_mg, b_eg));

        // Full-evaluation mirror check: color-swapped, rank-flipped boards
        // must score exact opposites (same magnitude, flipped sign).
        assert_eq!(
            eval.evaluate(&white_board),
            -eval.evaluate(&black_board)
        );
    }
}
