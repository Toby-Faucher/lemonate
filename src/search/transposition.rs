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

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

/// Default table size in megabytes.
pub const DEFAULT_TABLE_SIZE_MB: usize = 64;

/// Minimum table size in entries.
const MIN_TABLE_SIZE: usize = 1024;

/// Threshold for considering a score as a mate score.
const MATE_THRESHOLD: i32 = MATE_SCORE - 256;

/// Depth credit (plies) granted per search of age distance when deciding
/// whether a new entry may replace an existing same-position entry.
const AGE_WEIGHT: i32 = 2;

/// Wrapping distance between the current search age and an entry's age.
#[inline]
fn age_distance(current_age: u8, entry_age: u8) -> i32 {
    current_age.wrapping_sub(entry_age) as i32
}

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
    /// - Always replace if the slot holds a different position
    /// - Same position: replace if `depth + AGE_WEIGHT * age_distance >= existing depth`
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
        // Key insight: We must handle hash collisions (different positions mapping
        // to same index) differently from same-position updates.
        // - Different position (collision): always replace to store our position
        // - Same position: only replace if new depth >= existing depth
        //   (prevents shallow aspiration re-searches from overwriting deeper results)
        let should_replace = existing.is_empty()
            || existing.hash != hash  // Different position (hash collision), must replace
            // Same position: depth plus an age bonus must reach the old depth.
            || depth as i32 + AGE_WEIGHT * age_distance(self.current_age, existing.age)
                >= existing.depth as i32;

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

/// One seqlock-protected slot of the shared table.
///
/// Readers take no lock: they snapshot `seq`, copy the payload, then
/// re-check `seq`. Writers CAS `seq` from even to odd (brief per-entry
/// exclusion), publish the payload, then mark it even again. A reader
/// whose two `seq` samples differ (or saw an odd value) retries, so it
/// can never observe a torn entry.
struct SharedEntry {
    seq: AtomicU32,
    data: UnsafeCell<TranspositionEntry>,
}

// SAFETY: all payload access is mediated by the seqlock above;
// `TranspositionEntry` itself is plain `Copy` data.
unsafe impl Send for SharedEntry {}
unsafe impl Sync for SharedEntry {}

impl SharedEntry {
    fn new() -> Self {
        Self {
            seq: AtomicU32::new(0),
            data: UnsafeCell::new(TranspositionEntry::default()),
        }
    }

    /// Optimistic read. Spins while a writer holds the slot and retries
    /// on torn copies.
    fn load(&self) -> TranspositionEntry {
        loop {
            let s1 = self.seq.load(Ordering::Acquire);
            if s1 & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            // SAFETY: `seq` is even, so no writer is publishing right now.
            // If a writer starts mid-copy the trailing `seq` check fails
            // and the (possibly torn) copy is discarded.
            let copy: TranspositionEntry =
                unsafe { std::ptr::read_volatile(self.data.get()) };
            std::sync::atomic::fence(Ordering::Acquire);
            let s2 = self.seq.load(Ordering::Acquire);
            if s1 == s2 {
                return copy;
            }
        }
    }

    /// Lock the slot for writing. Returns the locked sequence number to
    /// pass to [`SharedEntry::unlock`]. Spins briefly on contention; only
    /// writers racing on the *same index* wait here.
    fn lock(&self) -> u32 {
        loop {
            let s = self.seq.load(Ordering::Acquire);
            if s & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            match self.seq.compare_exchange_weak(
                s,
                s.wrapping_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return s.wrapping_add(1),
                Err(_) => {
                    std::hint::spin_loop();
                }
            }
        }
    }

    /// Publish a payload previously prepared while holding the lock.
    /// Caller must have locked via [`SharedEntry::lock`].
    unsafe fn publish(&self, locked: u32, entry: TranspositionEntry) {
        // SAFETY: caller holds the slot exclusively (odd `seq` written by
        // us); readers either spin or discard via the `seq` mismatch check.
        unsafe { std::ptr::write_volatile(self.data.get(), entry) };
        self.seq.store(locked.wrapping_add(1), Ordering::Release);
    }

    /// Release a lock taken via [`SharedEntry::lock`] without changing
    /// the payload.
    fn unlock(&self, locked: u32) {
        self.seq.store(locked.wrapping_add(1), Ordering::Release);
    }
}

/// Thread-safe transposition table for Lazy SMP search.
///
/// Entry layout and replacement policy are identical to
/// [`TranspositionTable`]; only the synchronization differs (per-entry
/// seqlock instead of `&mut` exclusivity). The table size is fixed at
/// construction; resizing means building a new table (done while no
/// search is running).
pub struct SharedTT {
    entries: Box<[SharedEntry]>,
    mask: usize,
    age: AtomicU8,
}

// SAFETY: `entries` is immutable after construction and each slot is
// seqlock-synchronized; `age` is atomic.
unsafe impl Send for SharedTT {}
unsafe impl Sync for SharedTT {}

impl SharedTT {
    /// Create a new shared table with the given size in MB.
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<TranspositionEntry>();
        let bytes = size_mb.saturating_mul(1024 * 1024);
        let num_entries = (bytes / entry_size).max(MIN_TABLE_SIZE);

        // Round down to power of 2 for efficient masking (same as
        // `TranspositionTable::new`).
        let size = num_entries.next_power_of_two() >> 1;
        let size = size.max(MIN_TABLE_SIZE);

        let mut entries = Vec::with_capacity(size);
        for _ in 0..size {
            entries.push(SharedEntry::new());
        }

        Self {
            entries: entries.into_boxed_slice(),
            mask: size - 1,
            age: AtomicU8::new(0),
        }
    }

    /// Clear all entries (call when no search is running).
    pub fn clear(&self) {
        for entry in self.entries.iter() {
            let locked = entry.lock();
            // SAFETY: slot locked for writing.
            unsafe { entry.publish(locked, TranspositionEntry::default()) };
        }
        self.age.store(0, Ordering::Release);
    }

    /// Increment the age counter for a new search.
    ///
    /// Call exactly once per search (by the coordinator), not once per
    /// worker, so shared replacement behaves like the single-threaded
    /// table.
    pub fn new_search(&self) {
        self.age.fetch_add(1, Ordering::AcqRel);
    }

    /// Get the index for a given hash.
    #[inline]
    fn index(&self, hash: u64) -> usize {
        (hash as usize) & self.mask
    }

    /// Probe the table, returning an owned copy of the entry on a full
    /// hash match. Never returns torn data.
    #[inline]
    pub fn probe(&self, hash: u64) -> Option<TranspositionEntry> {
        let index = self.index(hash);
        let entry = self.entries[index].load();

        if entry.hash == hash && !entry.is_empty() {
            Some(entry)
        } else {
            None
        }
    }

    /// Store a new entry, using the same depth-preferred replacement
    /// with age consideration as [`TranspositionTable::store`].
    pub fn store(
        &self,
        hash: u64,
        score: i32,
        depth: u8,
        entry_type: EntryType,
        best_move: Option<Move>,
    ) {
        let index = self.index(hash);
        let slot = &self.entries[index];
        let current_age = self.age.load(Ordering::Acquire);

        let locked = slot.lock();
        // Re-read under the lock: another worker may have stored a deeper
        // entry after our last look.
        // SAFETY: slot locked for writing.
        let existing: TranspositionEntry =
            unsafe { std::ptr::read_volatile(slot.data.get()) };

        let should_replace = existing.is_empty()
            || existing.hash != hash
            || depth as i32 + AGE_WEIGHT * age_distance(current_age, existing.age)
                >= existing.depth as i32;

        if should_replace {
            let best_move = if best_move.is_some() {
                best_move
            } else if existing.hash == hash {
                existing.best_move
            } else {
                None
            };

            let entry =
                TranspositionEntry::new(hash, score, depth, entry_type, best_move, current_age);
            // SAFETY: slot locked for writing.
            unsafe { slot.publish(locked, entry) };
        } else {
            slot.unlock(locked);
        }
    }

    /// Get the hash table usage permille (0-1000), sampling the first
    /// 1000 entries like [`TranspositionTable::hashfull`].
    pub fn hashfull(&self) -> u16 {
        let current_age = self.age.load(Ordering::Acquire);
        let sample_size = 1000.min(self.entries.len());
        let filled = self.entries[..sample_size]
            .iter()
            .filter(|e| {
                let entry = e.load();
                !entry.is_empty() && entry.age == current_age
            })
            .count();

        ((filled * 1000) / sample_size) as u16
    }

    /// Get the number of entries in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the table is empty (no entries stored).
    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|e| e.load().is_empty())
    }

    /// Get the current age counter.
    pub fn age(&self) -> u8 {
        self.age.load(Ordering::Acquire)
    }
}

impl Default for SharedTT {
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

        // Try to store at depth 2 - should NOT replace (lower depth, same position).
        // This prevents shallow aspiration re-searches from overwriting deeper results.
        tt.store(hash, 200, 2, EntryType::Exact, None);
        assert_eq!(tt.probe(hash).unwrap().score, 100); // Original score preserved
        assert_eq!(tt.probe(hash).unwrap().depth, 3);   // Original depth preserved

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

        // One search old: 3 + 2*1 = 5 < 10, so the deep entry is kept.
        tt.store(hash, 200, 3, EntryType::Exact, None);
        assert_eq!(tt.probe(hash).unwrap().score, 100);

        // Four searches old: 3 + 2*4 = 11 >= 10, so it is replaced.
        for _ in 0..3 {
            tt.new_search();
        }
        tt.store(hash, 200, 3, EntryType::Exact, None);
        assert_eq!(tt.probe(hash).unwrap().score, 200);
    }

    #[test]
    fn test_age_distance_wraps() {
        assert_eq!(age_distance(5, 3), 2);
        assert_eq!(age_distance(1, 255), 2);
        assert_eq!(age_distance(7, 7), 0);
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

    #[test]
    fn test_shared_tt_single_thread_matches_local() {
        // Same stores through the shared table must give the same visible
        // results as the single-threaded table.
        let shared = SharedTT::new(1);
        let mut local = TranspositionTable::new(1);
        let mv = make_test_move();

        let stores = [
            (0x123456789ABCDEF0u64, 100, 5, EntryType::Exact, Some(mv)),
            (0x123456789ABCDEF0u64, 200, 2, EntryType::Exact, None), // shallower: kept
            (0xFEDCBA9876543210u64, -50, 3, EntryType::LowerBound, None),
            (0x0BADF00DDEADBEEFu64, 300, 8, EntryType::UpperBound, Some(mv)),
        ];
        for (hash, score, depth, ty, bm) in stores {
            local.store(hash, score, depth, ty, bm);
            shared.store(hash, score, depth, ty, bm);
        }

        for (hash, _, _, _, _) in stores {
            match (local.probe(hash), shared.probe(hash)) {
                (Some(l), Some(s)) => {
                    assert_eq!(l.hash, s.hash);
                    assert_eq!(l.score, s.score);
                    assert_eq!(l.depth, s.depth);
                    assert_eq!(l.entry_type, s.entry_type);
                    assert_eq!(l.best_move, s.best_move);
                }
                (None, None) => {}
                (l, s) => panic!("probe mismatch for {:x}: {:?} vs {:?}", hash, l, s),
            }
        }
        assert!(shared.probe(0xDEADDEADDEADDEAD).is_none());
    }

    #[test]
    fn test_shared_tt_concurrent_no_tear_same_entry() {
        use std::sync::Arc;

        // All writers publish byte-identical entries for one hash while
        // readers probe it. Any torn copy would differ from the expected
        // entry, so exact equality on every `Some` proves tear-free reads.
        let shared = Arc::new(SharedTT::new(1));
        let mv = make_test_move();
        let hash: u64 = 0x123456789ABCDEF0;
        shared.store(hash, 777, 9, EntryType::Exact, Some(mv));
        let expected = shared.probe(hash).unwrap();

        let mut handles = Vec::new();
        for _ in 0..8 {
            let tt = Arc::clone(&shared);
            handles.push(std::thread::spawn(move || {
                for _ in 0..2000 {
                    tt.store(hash, 777, 9, EntryType::Exact, Some(mv));
                }
            }));
        }
        for _ in 0..4 {
            let tt = Arc::clone(&shared);
            let exp = expected;
            handles.push(std::thread::spawn(move || {
                for _ in 0..4000 {
                    if let Some(got) = tt.probe(hash) {
                        assert_eq!(got.hash, exp.hash);
                        assert_eq!(got.score, exp.score);
                        assert_eq!(got.depth, exp.depth);
                        assert_eq!(got.entry_type, exp.entry_type);
                        assert_eq!(got.best_move, exp.best_move);
                        assert_eq!(got.age, exp.age);
                    }
                }
            }));
        }
        for h in handles {
            h.join().expect("worker panicked");
        }
    }

    #[test]
    fn test_shared_tt_concurrent_distinct_hashes() {
        use std::sync::Arc;

        // Distinct hashes hammered from many threads: every probe must
        // return either a miss or the exact entry for that hash — never a
        // mix of two positions.
        let shared = Arc::new(SharedTT::new(4));
        let mut handles = Vec::new();
        for t in 0..8u64 {
            let tt = Arc::clone(&shared);
            handles.push(std::thread::spawn(move || {
                for i in 0..2000u64 {
                    let hash = (t << 56) ^ (i.wrapping_mul(0x9E3779B97F4A7C15)) ^ 0x100;
                    assert_ne!(hash, 0);
                    let score = (hash ^ (hash >> 32)) as i32;
                    tt.store(hash, score, (i % 16) as u8, EntryType::Exact, None);
                    if let Some(entry) = tt.probe(hash) {
                        // Either our entry survived or a colliding position
                        // replaced it (probe verifies the full hash, so a
                        // hit always belongs to `hash`).
                        assert_eq!(entry.hash, hash);
                        if entry.depth == (i % 16) as u8 {
                            assert_eq!(entry.score, score);
                        }
                    }
                    // Unrelated probes must never return a wrong entry.
                    let other = hash ^ ((tt.len() as u64) << 3) ^ 0xFF;
                    if let Some(entry) = tt.probe(other) {
                        assert_eq!(entry.hash, other);
                    }
                }
            }));
        }
        for h in handles {
            h.join().expect("worker panicked");
        }
        // Table still fully usable afterwards.
        shared.store(0xABCD, 42, 4, EntryType::Exact, None);
        assert_eq!(shared.probe(0xABCD).unwrap().score, 42);
    }

    #[test]
    fn test_shared_tt_age_and_clear() {
        let shared = SharedTT::new(1);
        let hash: u64 = 0x123456789ABCDEF0;

        shared.store(hash, 100, 10, EntryType::Exact, None);
        shared.new_search();
        assert_eq!(shared.age(), 1);
        // One search old: 3 + 2*1 < 10, deep entry kept.
        shared.store(hash, 200, 3, EntryType::Exact, None);
        assert_eq!(shared.probe(hash).unwrap().score, 100);
        // Four searches old: 3 + 2*4 >= 10, replaced.
        for _ in 0..3 {
            shared.new_search();
        }
        shared.store(hash, 200, 3, EntryType::Exact, None);
        assert_eq!(shared.probe(hash).unwrap().score, 200);

        assert!(!shared.is_empty());
        shared.clear();
        assert!(shared.is_empty());
        assert_eq!(shared.age(), 0);
        assert!(shared.probe(hash).is_none());
    }
}
