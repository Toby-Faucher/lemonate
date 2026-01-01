//! Killer Moves Heuristic
//!
//! Tracks quiet moves that caused beta cutoffs at each ply depth.
//! These moves are likely to be good in sibling positions at the same depth.
//!
//! # Implementation Notes
//! - Store 2 killer moves per ply (most engines use 2)
//! - Only store quiet moves (not captures)
//! - Use FIFO replacement when adding new killers

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
        todo!("Implement KillerMoveTable::clear")
    }

    /// Store a killer move at the given ply.
    ///
    /// Uses FIFO replacement: new killer goes to slot 0,
    /// previous slot 0 moves to slot 1, slot 1 is discarded.
    ///
    /// # Arguments
    /// * `ply` - The current search ply
    /// * `mv` - The move that caused a beta cutoff
    pub fn store(&mut self, ply: u8, mv: Move) {
        todo!("Implement KillerMoveTable::store")
    }

    /// Check if a move is a killer at the given ply.
    ///
    /// Returns the killer slot (0 or 1) if found, None otherwise.
    pub fn is_killer(&self, ply: u8, mv: &Move) -> Option<usize> {
        todo!("Implement KillerMoveTable::is_killer")
    }

    /// Get the primary killer move at the given ply.
    pub fn get_primary(&self, ply: u8) -> Option<Move> {
        todo!("Implement KillerMoveTable::get_primary")
    }

    /// Get the secondary killer move at the given ply.
    pub fn get_secondary(&self, ply: u8) -> Option<Move> {
        todo!("Implement KillerMoveTable::get_secondary")
    }

    /// Get both killer moves at the given ply.
    pub fn get_killers(&self, ply: u8) -> [Option<Move>; KILLERS_PER_PLY] {
        todo!("Implement KillerMoveTable::get_killers")
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

    #[test]
    fn test_store_and_retrieve() {
        // TODO: Test storing and retrieving killer moves
    }

    #[test]
    fn test_fifo_replacement() {
        // TODO: Test that FIFO replacement works correctly
    }

    #[test]
    fn test_is_killer() {
        // TODO: Test killer move detection
    }

    #[test]
    fn test_clear() {
        // TODO: Test clearing the table
    }
}
