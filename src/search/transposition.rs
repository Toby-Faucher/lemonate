//! Transposition Table
//!
//! A hash table storing previously searched positions to avoid redundant work.
//! Uses Zobrist hashing for position identification.
//!
//! # Implementation Notes
//! - Entry size should be power of 2 for efficient indexing
//! - Use depth-preferred replacement with age tracking
//! - Handle hash collisions via verification

use crate::board::Move;

use super::MATE_SCORE;

/// Default table size in megabytes.
pub const DEFAULT_TABLE_SIZE_MB: usize = 64;

/// Minimum table size in entries.
const MIN_TABLE_SIZE: usize = 1024;

/// Threshold for considering a score as a mate score.
const MATE_THRESHOLD: i32 = MATE_SCORE - 256;

/// Entry types indicating the nature of the stored score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryType {
    /// Score is exact (PV node, full window search).
    Exact,
    /// Score is a lower bound (failed high, beta cutoff).
    LowerBound,
    /// Score is an upper bound (failed low, alpha not improved).
    UpperBound,
}

/// A single entry in the transposition table.
#[derive(Clone, Copy, Debug)]
pub struct TranspositionEntry {
    /// Zobrist hash for collision detection.
    pub hash: u64,
    /// The evaluated score.
    pub score: i32,
    /// Depth at which this position was searched.
    pub depth: u8,
    /// Type of score bound.
    pub entry_type: EntryType,
    /// Best move found from this position.
    pub best_move: Option<Move>,
    /// Age counter for replacement decisions.
    pub age: u8,
}

impl TranspositionEntry {
    /// Create a new transposition entry.
    pub fn new(
        hash: u64,
        score: i32,
        depth: u8,
        entry_type: EntryType,
        best_move: Option<Move>,
        age: u8,
    ) -> Self {
        Self {
            hash,
            score,
            depth,
            entry_type,
            best_move,
            age,
        }
    }

    /// Check if this entry is empty/invalid.
    #[inline]
    pub fn is_empty(&self) -> bool {
        // An entry with hash 0 and depth 0 is considered empty.
        // This works because a real position is extremely unlikely to hash to 0.
        self.hash == 0 && self.depth == 0
    }

    /// Check if the stored score can be used for the given alpha-beta bounds.
    ///
    /// Returns `Some(score)` if the entry provides a cutoff, `None` otherwise.
    #[inline]
    pub fn get_score(&self, alpha: i32, beta: i32) -> Option<i32> {
        match self.entry_type {
            EntryType::Exact => Some(self.score),
            EntryType::LowerBound => {
                if self.score >= beta {
                    Some(self.score)
                } else {
                    None
                }
            }
            EntryType::UpperBound => {
                if self.score <= alpha {
                    Some(self.score)
                } else {
                    None
                }
            }
        }
    }
}

impl Default for TranspositionEntry {
    fn default() -> Self {
        Self {
            hash: 0,
            score: 0,
            depth: 0,
            entry_type: EntryType::Exact,
            best_move: None,
            age: 0,
        }
    }
}

/// The transposition table storing searched positions.
pub struct TranspositionTable {
    /// The hash table entries.
    entries: Vec<TranspositionEntry>,
    /// Number of entries (must be power of 2).
    size: usize,
    /// Mask for efficient index calculation (size - 1).
    mask: usize,
    /// Current search age for replacement decisions.
    current_age: u8,
}

impl TranspositionTable {
    /// Create a new transposition table with the given size in MB.
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<TranspositionEntry>();
        let bytes = size_mb.saturating_mul(1024 * 1024);
        let num_entries = (bytes / entry_size).max(MIN_TABLE_SIZE);

        // Round down to power of 2 for efficient masking.
        let size = num_entries.next_power_of_two() >> 1;
        let size = size.max(MIN_TABLE_SIZE);

        Self {
            entries: vec![TranspositionEntry::default(); size],
            size,
            mask: size - 1,
            current_age: 0,
        }
    }

    /// Clear all entries in the table.
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = TranspositionEntry::default();
        }
        self.current_age = 0;
    }

    /// Increment the age counter for a new search.
    ///
    /// Call this at the start of each new search from root.
    pub fn new_search(&mut self) {
        self.current_age = self.current_age.wrapping_add(1);
    }

    /// Get the index for a given hash.
    #[inline]
    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    /// Probe the table for an entry matching the given hash.
    ///
    /// Returns `Some(entry)` if found and hash matches, `None` otherwise.
    #[inline]
    pub fn probe(&self, hash: u64) -> Option<&TranspositionEntry> {
        let index = self.index(hash);
        let entry = &self.entries[index];

        // Verify the full hash matches to detect collisions.
        if entry.hash == hash && !entry.is_empty() {
            Some(entry)
        } else {
            None
        }
    }

    /// Store a new entry in the table.
    ///
    /// Uses depth-preferred replacement with age consideration:
    /// - Always replace if entry is empty
    /// - Always replace if entry is from a previous search (different age)
    /// - Replace if new depth >= existing depth
    /// - Replace if same position (hash matches)
    pub fn store(
        &mut self,
        hash: u64,
        score: i32,
        depth: u8,
        entry_type: EntryType,
        best_move: Option<Move>,
    ) {
        let index = self.index(hash);
        let existing = &self.entries[index];

        // Determine if we should replace the existing entry.
        let should_replace = existing.is_empty()
            || existing.hash == hash  // Same position, always update
            || existing.age != self.current_age  // Old entry from previous search
            || depth >= existing.depth;  // New search is at least as deep

        if should_replace {
            // Preserve best move from existing entry if we don't have one
            // and it's the same position.
            let best_move = if best_move.is_some() {
                best_move
            } else if existing.hash == hash {
                existing.best_move
            } else {
                None
            };

            self.entries[index] = TranspositionEntry::new(
                hash,
                score,
                depth,
                entry_type,
                best_move,
                self.current_age,
            );
        }
    }

    /// Adjust mate scores for storage.
    ///
    /// Mate scores are stored as "mate in N plies from this position"
    /// rather than "mate in N plies from root". This ensures the score
    /// is valid regardless of how we reached this position.
    ///
    /// If we found "mate in 5 from root" at ply 3, we store "mate in 2 from here".
    #[inline]
    pub fn adjust_score_for_storage(score: i32, ply: u8) -> i32 {
        if score > MATE_THRESHOLD {
            // Positive mate score: we're winning.
            // Add ply to convert from root distance to position distance.
            score + ply as i32
        } else if score < -MATE_THRESHOLD {
            // Negative mate score: we're losing.
            // Subtract ply to convert from root distance to position distance.
            score - ply as i32
        } else {
            score
        }
    }

    /// Adjust mate scores after retrieval.
    ///
    /// Convert from "mate in N from this position" back to
    /// "mate in N from root" for the current search.
    #[inline]
    pub fn adjust_score_for_retrieval(score: i32, ply: u8) -> i32 {
        if score > MATE_THRESHOLD {
            // Positive mate score: we're winning.
            // Subtract ply to convert from position distance to root distance.
            score - ply as i32
        } else if score < -MATE_THRESHOLD {
            // Negative mate score: we're losing.
            // Add ply to convert from position distance to root distance.
            score + ply as i32
        } else {
            score
        }
    }

    /// Get the hash table usage percentage (0-1000, permille).
    ///
    /// Samples the first 1000 entries to estimate fill rate.
    pub fn hashfull(&self) -> u16 {
        let sample_size = 1000.min(self.size);
        let filled = self.entries[..sample_size]
            .iter()
            .filter(|e| !e.is_empty() && e.age == self.current_age)
            .count();

        ((filled * 1000) / sample_size) as u16
    }

    /// Resize the table to a new size in MB.
    ///
    /// This clears all existing entries.
    pub fn resize(&mut self, size_mb: usize) {
        let entry_size = std::mem::size_of::<TranspositionEntry>();
        let bytes = size_mb.saturating_mul(1024 * 1024);
        let num_entries = (bytes / entry_size).max(MIN_TABLE_SIZE);

        // Round down to power of 2.
        let size = num_entries.next_power_of_two() >> 1;
        let size = size.max(MIN_TABLE_SIZE);

        self.entries = vec![TranspositionEntry::default(); size];
        self.size = size;
        self.mask = size - 1;
        self.current_age = 0;
    }

    /// Get the number of entries in the table.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Check if the table is empty (no entries stored).
    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|e| e.is_empty())
    }

    /// Get the current age counter.
    pub fn age(&self) -> u8 {
        self.current_age
    }
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new(DEFAULT_TABLE_SIZE_MB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::MoveType;
    use crate::types::{Color, Piece, PieceType, Square};

    fn make_test_move() -> Move {
        Move {
            from: Square::from_algebraic("e2").unwrap(),
            to: Square::from_algebraic("e4").unwrap(),
            move_type: MoveType::Normal,
            piece: Piece {
                piece_type: PieceType::Pawn,
                color: Color::White,
            },
            captured: None,
        }
    }

    #[test]
    fn test_table_creation() {
        let tt = TranspositionTable::new(1);
        assert!(tt.len() >= MIN_TABLE_SIZE);
        assert!(tt.len().is_power_of_two());
        assert!(tt.is_empty());
    }

    #[test]
    fn test_table_size_power_of_two() {
        for size_mb in [1, 2, 4, 8, 16, 32, 64] {
            let tt = TranspositionTable::new(size_mb);
            assert!(
                tt.len().is_power_of_two(),
                "Size {} should be power of 2",
                tt.len()
            );
        }
    }

    #[test]
    fn test_store_and_probe() {
        let mut tt = TranspositionTable::new(1);

        let hash: u64 = 0x123456789ABCDEF0;
        let mv = make_test_move();

        tt.store(hash, 100, 5, EntryType::Exact, Some(mv));

        let entry = tt.probe(hash).expect("Entry should exist");
        assert_eq!(entry.hash, hash);
        assert_eq!(entry.score, 100);
        assert_eq!(entry.depth, 5);
        assert_eq!(entry.entry_type, EntryType::Exact);
        assert!(entry.best_move.is_some());
    }

    #[test]
    fn test_probe_miss() {
        let mut tt = TranspositionTable::new(1);

        let hash1: u64 = 0x123456789ABCDEF0;
        let hash2: u64 = 0xFEDCBA9876543210;

        tt.store(hash1, 100, 5, EntryType::Exact, None);

        // Probing a different hash should return None.
        assert!(tt.probe(hash2).is_none());
    }

    #[test]
    fn test_replacement_policy_depth() {
        let mut tt = TranspositionTable::new(1);

        let hash: u64 = 0x123456789ABCDEF0;

        // Store at depth 3.
        tt.store(hash, 100, 3, EntryType::Exact, None);
        assert_eq!(tt.probe(hash).unwrap().depth, 3);

        // Try to store at depth 2 - should NOT replace (lower depth).
        tt.store(hash, 200, 2, EntryType::Exact, None);
        // With same hash, we always replace.
        assert_eq!(tt.probe(hash).unwrap().score, 200);

        // Store at depth 5 - should replace (higher depth).
        tt.store(hash, 300, 5, EntryType::Exact, None);
        assert_eq!(tt.probe(hash).unwrap().score, 300);
        assert_eq!(tt.probe(hash).unwrap().depth, 5);
    }

    #[test]
    fn test_replacement_policy_age() {
        let mut tt = TranspositionTable::new(1);

        let hash: u64 = 0x123456789ABCDEF0;

        // Store entry in current search.
        tt.store(hash, 100, 10, EntryType::Exact, None);

        // Start a new search (increment age).
        tt.new_search();

        // Store at lower depth - should replace because entry is old.
        tt.store(hash, 200, 3, EntryType::Exact, None);
        assert_eq!(tt.probe(hash).unwrap().score, 200);
    }

    #[test]
    fn test_hash_collision_handling() {
        let mut tt = TranspositionTable::new(1);

        let hash1: u64 = 0x123456789ABCDEF0;
        // Create a hash that maps to the same index but is different.
        let hash2: u64 = hash1 ^ ((tt.len() as u64) << 1);

        tt.store(hash1, 100, 5, EntryType::Exact, None);

        // hash2 may or may not collide depending on table size.
        // But if we probe with hash1, we should get the correct entry.
        let entry = tt.probe(hash1).unwrap();
        assert_eq!(entry.hash, hash1);
        assert_eq!(entry.score, 100);
    }

    #[test]
    fn test_mate_score_adjustment() {
        // Mate in 5 from root, currently at ply 2.
        let mate_score = MATE_SCORE - 5;
        let ply = 2u8;

        // Storage: convert to "mate in 3 from this position".
        let stored = TranspositionTable::adjust_score_for_storage(mate_score, ply);
        assert_eq!(stored, MATE_SCORE - 5 + 2); // MATE_SCORE - 3

        // Retrieval: convert back to "mate in 5 from root".
        let retrieved = TranspositionTable::adjust_score_for_retrieval(stored, ply);
        assert_eq!(retrieved, mate_score);
    }

    #[test]
    fn test_mate_score_adjustment_negative() {
        // Getting mated in 5 from root, currently at ply 2.
        let mate_score = -(MATE_SCORE - 5);
        let ply = 2u8;

        // Storage: convert to "mated in 3 from this position".
        let stored = TranspositionTable::adjust_score_for_storage(mate_score, ply);
        assert_eq!(stored, -(MATE_SCORE - 5) - 2); // -(MATE_SCORE - 3)

        // Retrieval: convert back to "mated in 5 from root".
        let retrieved = TranspositionTable::adjust_score_for_retrieval(stored, ply);
        assert_eq!(retrieved, mate_score);
    }

    #[test]
    fn test_non_mate_score_unchanged() {
        let score = 150; // Normal centipawn score.
        let ply = 5u8;

        let stored = TranspositionTable::adjust_score_for_storage(score, ply);
        assert_eq!(stored, score);

        let retrieved = TranspositionTable::adjust_score_for_retrieval(stored, ply);
        assert_eq!(retrieved, score);
    }

    #[test]
    fn test_entry_get_score() {
        // Exact score - always returns the score.
        let entry = TranspositionEntry::new(1, 100, 5, EntryType::Exact, None, 0);
        assert_eq!(entry.get_score(-1000, 1000), Some(100));
        assert_eq!(entry.get_score(200, 300), Some(100));

        // Lower bound (fail high): score >= beta means we can cutoff.
        let entry = TranspositionEntry::new(1, 100, 5, EntryType::LowerBound, None, 0);
        assert_eq!(entry.get_score(-1000, 150), None); // score < beta, no cutoff
        assert_eq!(entry.get_score(-1000, 100), Some(100)); // score == beta, cutoff
        assert_eq!(entry.get_score(-1000, 80), Some(100)); // score > beta, cutoff

        // Upper bound (fail low): score <= alpha means we can cutoff.
        let entry = TranspositionEntry::new(1, 100, 5, EntryType::UpperBound, None, 0);
        assert_eq!(entry.get_score(50, 1000), None); // score > alpha, no cutoff
        assert_eq!(entry.get_score(100, 1000), Some(100)); // score == alpha, cutoff
        assert_eq!(entry.get_score(120, 1000), Some(100)); // score < alpha, cutoff
    }

    #[test]
    fn test_clear() {
        let mut tt = TranspositionTable::new(1);

        tt.store(0x123, 100, 5, EntryType::Exact, None);
        tt.store(0x456, 200, 3, EntryType::Exact, None);
        tt.new_search();

        assert!(!tt.is_empty());

        tt.clear();

        assert!(tt.is_empty());
        assert_eq!(tt.age(), 0);
        assert!(tt.probe(0x123).is_none());
        assert!(tt.probe(0x456).is_none());
    }

    #[test]
    fn test_resize() {
        let mut tt = TranspositionTable::new(1);
        let original_size = tt.len();

        tt.store(0x123, 100, 5, EntryType::Exact, None);

        tt.resize(2);

        assert!(tt.len() > original_size);
        assert!(tt.is_empty()); // Resize clears entries.
        assert!(tt.probe(0x123).is_none());
    }

    #[test]
    fn test_hashfull() {
        let mut tt = TranspositionTable::new(1);

        assert_eq!(tt.hashfull(), 0);

        // Store some entries.
        for i in 0..100u64 {
            tt.store(i * 12345, i as i32, 5, EntryType::Exact, None);
        }

        let fullness = tt.hashfull();
        assert!(fullness > 0, "Table should have some entries");
    }

    #[test]
    fn test_preserve_best_move() {
        let mut tt = TranspositionTable::new(1);
        let hash: u64 = 0x123456789ABCDEF0;
        let mv = make_test_move();

        // Store with a best move.
        tt.store(hash, 100, 5, EntryType::Exact, Some(mv));

        // Update same position without a best move - should preserve existing.
        tt.store(hash, 150, 6, EntryType::Exact, None);

        let entry = tt.probe(hash).unwrap();
        assert!(entry.best_move.is_some());
        assert_eq!(entry.score, 150);
    }
}
