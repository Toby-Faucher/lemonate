//! Killer Moves Heuristic
//!
//! Tracks quiet moves that caused beta cutoffs at each ply depth.
//! These moves are likely to be good in sibling positions at the same depth.
//!
//! # Implementation Notes
//! - Store 2 killer moves per ply (most engines use 2)
//! - Only store quiet moves (not captures)
//! - Use FIFO replacement when adding new killers
//! - Avoid storing duplicate moves

use crate::board::Move;

/// Maximum ply depth for killer move storage.
pub const MAX_PLY: usize = 128;

/// Number of killer moves to store per ply.
pub const KILLERS_PER_PLY: usize = 2;

/// Table storing killer moves indexed by ply.
pub struct KillerMoveTable {
    /// Killer moves array: [ply][slot]
    killers: [[Option<Move>; KILLERS_PER_PLY]; MAX_PLY],
}

impl KillerMoveTable {
    /// Create a new empty killer move table.
    pub fn new() -> Self {
        Self {
            killers: [[None; KILLERS_PER_PLY]; MAX_PLY],
        }
    }

    /// Clear all killer moves (call at start of new search).
    pub fn clear(&mut self) {
        for ply_killers in &mut self.killers {
            *ply_killers = [None; KILLERS_PER_PLY];
        }
    }

    /// Store a killer move at the given ply.
    ///
    /// Uses FIFO replacement: new killer goes to slot 0,
    /// previous slot 0 moves to slot 1, slot 1 is discarded.
    ///
    /// Does nothing if:
    /// - The ply is out of bounds
    /// - The move is already the primary killer (avoid duplicates)
    ///
    /// # Arguments
    /// * `ply` - The current search ply
    /// * `mv` - The move that caused a beta cutoff
    pub fn store(&mut self, ply: u8, mv: Move) {
        let ply = ply as usize;
        if ply >= MAX_PLY {
            return;
        }

        // Don't store if it's already the primary killer.
        if let Some(primary) = self.killers[ply][0] {
            if primary == mv {
                return;
            }
        }

        // FIFO: shift slot 0 to slot 1, put new move in slot 0.
        self.killers[ply][1] = self.killers[ply][0];
        self.killers[ply][0] = Some(mv);
    }

    /// Check if a move is a killer at the given ply.
    ///
    /// Returns the killer slot (0 or 1) if found, None otherwise.
    #[inline]
    pub fn is_killer(&self, ply: u8, mv: &Move) -> Option<usize> {
        let ply = ply as usize;
        if ply >= MAX_PLY {
            return None;
        }

        for (slot, killer) in self.killers[ply].iter().enumerate() {
            if let Some(k) = killer {
                if k == mv {
                    return Some(slot);
                }
            }
        }
        None
    }

    /// Get the primary killer move at the given ply.
    #[inline]
    pub fn get_primary(&self, ply: u8) -> Option<Move> {
        let ply = ply as usize;
        if ply >= MAX_PLY {
            return None;
        }
        self.killers[ply][0]
    }

    /// Get the secondary killer move at the given ply.
    #[inline]
    pub fn get_secondary(&self, ply: u8) -> Option<Move> {
        let ply = ply as usize;
        if ply >= MAX_PLY {
            return None;
        }
        self.killers[ply][1]
    }

    /// Get both killer moves at the given ply.
    #[inline]
    pub fn get_killers(&self, ply: u8) -> [Option<Move>; KILLERS_PER_PLY] {
        let ply = ply as usize;
        if ply >= MAX_PLY {
            return [None; KILLERS_PER_PLY];
        }
        self.killers[ply]
    }

    /// Check if there are any killer moves at the given ply.
    #[inline]
    pub fn has_killers(&self, ply: u8) -> bool {
        let ply = ply as usize;
        if ply >= MAX_PLY {
            return false;
        }
        self.killers[ply][0].is_some()
    }
}

impl Default for KillerMoveTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::MoveType;
    use crate::types::{Color, Piece, PieceType, Square};

    fn make_move(from: &str, to: &str) -> Move {
        Move {
            from: Square::from_algebraic(from).unwrap(),
            to: Square::from_algebraic(to).unwrap(),
            move_type: MoveType::Normal,
            piece: Piece {
                piece_type: PieceType::Knight,
                color: Color::White,
            },
            captured: None,
        }
    }

    #[test]
    fn test_new_table_is_empty() {
        let table = KillerMoveTable::new();

        for ply in 0..10u8 {
            assert!(table.get_primary(ply).is_none());
            assert!(table.get_secondary(ply).is_none());
            assert!(!table.has_killers(ply));
        }
    }

    #[test]
    fn test_store_and_retrieve() {
        let mut table = KillerMoveTable::new();
        let mv = make_move("e2", "e4");

        table.store(5, mv);

        assert_eq!(table.get_primary(5), Some(mv));
        assert!(table.get_secondary(5).is_none());
        assert!(table.has_killers(5));
    }

    #[test]
    fn test_fifo_replacement() {
        let mut table = KillerMoveTable::new();
        let mv1 = make_move("e2", "e4");
        let mv2 = make_move("d2", "d4");
        let mv3 = make_move("c2", "c4");

        // Store first move.
        table.store(3, mv1);
        assert_eq!(table.get_primary(3), Some(mv1));
        assert!(table.get_secondary(3).is_none());

        // Store second move - mv1 should shift to secondary.
        table.store(3, mv2);
        assert_eq!(table.get_primary(3), Some(mv2));
        assert_eq!(table.get_secondary(3), Some(mv1));

        // Store third move - mv2 shifts to secondary, mv1 is discarded.
        table.store(3, mv3);
        assert_eq!(table.get_primary(3), Some(mv3));
        assert_eq!(table.get_secondary(3), Some(mv2));
    }

    #[test]
    fn test_no_duplicate_primary() {
        let mut table = KillerMoveTable::new();
        let mv1 = make_move("e2", "e4");
        let mv2 = make_move("d2", "d4");

        table.store(3, mv1);
        table.store(3, mv2);
        assert_eq!(table.get_primary(3), Some(mv2));
        assert_eq!(table.get_secondary(3), Some(mv1));

        // Try to store mv2 again - should be ignored.
        table.store(3, mv2);
        assert_eq!(table.get_primary(3), Some(mv2));
        assert_eq!(table.get_secondary(3), Some(mv1));
    }

    #[test]
    fn test_is_killer() {
        let mut table = KillerMoveTable::new();
        let mv1 = make_move("e2", "e4");
        let mv2 = make_move("d2", "d4");
        let mv3 = make_move("c2", "c4");

        table.store(5, mv1);
        table.store(5, mv2);

        assert_eq!(table.is_killer(5, &mv2), Some(0)); // Primary
        assert_eq!(table.is_killer(5, &mv1), Some(1)); // Secondary
        assert_eq!(table.is_killer(5, &mv3), None); // Not a killer

        // Check different ply.
        assert_eq!(table.is_killer(6, &mv1), None);
    }

    #[test]
    fn test_get_killers() {
        let mut table = KillerMoveTable::new();
        let mv1 = make_move("e2", "e4");
        let mv2 = make_move("d2", "d4");

        table.store(7, mv1);
        table.store(7, mv2);

        let killers = table.get_killers(7);
        assert_eq!(killers[0], Some(mv2));
        assert_eq!(killers[1], Some(mv1));
    }

    #[test]
    fn test_clear() {
        let mut table = KillerMoveTable::new();
        let mv1 = make_move("e2", "e4");
        let mv2 = make_move("d2", "d4");

        table.store(3, mv1);
        table.store(5, mv2);

        assert!(table.has_killers(3));
        assert!(table.has_killers(5));

        table.clear();

        assert!(!table.has_killers(3));
        assert!(!table.has_killers(5));
        assert!(table.get_primary(3).is_none());
        assert!(table.get_primary(5).is_none());
    }

    #[test]
    fn test_different_plies_independent() {
        let mut table = KillerMoveTable::new();
        let mv1 = make_move("e2", "e4");
        let mv2 = make_move("d2", "d4");

        table.store(3, mv1);
        table.store(5, mv2);

        // Killers at different plies should be independent.
        assert_eq!(table.get_primary(3), Some(mv1));
        assert_eq!(table.get_primary(5), Some(mv2));
        assert!(table.get_secondary(3).is_none());
        assert!(table.get_secondary(5).is_none());
    }

    #[test]
    fn test_out_of_bounds_ply() {
        let mut table = KillerMoveTable::new();
        let mv = make_move("e2", "e4");

        // Should not panic, just be ignored.
        table.store(255, mv);

        assert!(table.get_primary(255).is_none());
        assert!(table.is_killer(255, &mv).is_none());
        assert!(!table.has_killers(255));
    }

    #[test]
    fn test_boundary_ply() {
        let mut table = KillerMoveTable::new();
        let mv = make_move("e2", "e4");

        // MAX_PLY - 1 should work.
        let max_valid_ply = (MAX_PLY - 1) as u8;
        table.store(max_valid_ply, mv);
        assert_eq!(table.get_primary(max_valid_ply), Some(mv));

        // MAX_PLY should be out of bounds.
        let out_of_bounds = MAX_PLY as u8;
        table.store(out_of_bounds, mv);
        assert!(table.get_primary(out_of_bounds).is_none());
    }
}
