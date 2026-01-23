//! Opening Book Support
//!
//! This module provides Polyglot (.bin) opening book support for the Lemonate
//! chess engine. Polyglot is the de facto standard format for chess opening books,
//! providing access to a large ecosystem of pre-built books.
//!
//! # Usage
//!
//! ```ignore
//! use lemonate::book::{BookManager, polyglot_hash};
//!
//! let mut manager = BookManager::new();
//! manager.load("path/to/book.bin")?;
//!
//! // Probe for a book move
//! if let Some(mv) = manager.probe(&board) {
//!     println!("Book move: {:?}", mv);
//! }
//! ```

mod polyglot_hash;

pub use polyglot_hash::polyglot_hash;

use crate::types::{Color, PieceType, Square};
use crate::board::{Board, Move, MoveType};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

/// A single entry from a Polyglot opening book.
#[derive(Clone, Copy, Debug)]
pub struct BookEntry {
    /// Polyglot Zobrist hash of the position
    pub key: u64,
    /// Encoded move (Polyglot format)
    pub raw_move: u16,
    /// Move weight (higher = more frequently played)
    pub weight: u16,
    /// Learn data (usually unused)
    pub learn: u32,
}

/// Move selection strategy when multiple book moves exist.
#[derive(Clone, Copy, Debug, Default)]
pub enum BookMoveSelection {
    /// Select randomly weighted by move weight (default)
    #[default]
    WeightedRandom,
    /// Always select the move with highest weight
    Best,
    /// Select uniformly at random among all book moves
    UniformRandom,
}

/// Manager for Polyglot opening books.
pub struct BookManager {
    /// Sorted list of book entries (sorted by key for binary search)
    entries: Vec<BookEntry>,
    /// Move selection strategy
    pub selection: BookMoveSelection,
}

impl Default for BookManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BookManager {
    /// Create a new empty BookManager.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selection: BookMoveSelection::default(),
        }
    }

    /// Load a Polyglot opening book from the given path.
    pub fn load<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut buf = [0u8; 16];

        self.entries.clear();

        while reader.read_exact(&mut buf).is_ok() {
            let entry = BookEntry {
                key: u64::from_be_bytes(buf[0..8].try_into().unwrap()),
                raw_move: u16::from_be_bytes(buf[8..10].try_into().unwrap()),
                weight: u16::from_be_bytes(buf[10..12].try_into().unwrap()),
                learn: u32::from_be_bytes(buf[12..16].try_into().unwrap()),
            };
            self.entries.push(entry);
        }

        // Sort by key for binary search
        self.entries.sort_by_key(|e| e.key);

        Ok(())
    }

    /// Returns true if a book is loaded and has entries.
    pub fn is_loaded(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Returns the number of entries in the loaded book.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if no book is loaded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Find all book entries matching the given position.
    pub fn probe_entries(&self, board: &Board) -> Vec<BookEntry> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        let key = polyglot_hash(board);

        // Binary search to find first matching entry
        let idx = match self.entries.binary_search_by_key(&key, |e| e.key) {
            Ok(i) => i,
            Err(_) => return Vec::new(),
        };

        // Collect all entries with matching key (they're contiguous after sort)
        let mut results = Vec::new();

        // Search backwards for first entry with this key
        let mut start = idx;
        while start > 0 && self.entries[start - 1].key == key {
            start -= 1;
        }

        // Collect all entries with this key
        let mut i = start;
        while i < self.entries.len() && self.entries[i].key == key {
            results.push(self.entries[i]);
            i += 1;
        }

        results
    }

    /// Probe the book and return a move if found.
    ///
    /// The move is selected based on the current `selection` strategy.
    /// Returns `None` if no book move is found for the position.
    pub fn probe(&self, board: &Board) -> Option<Move> {
        let entries = self.probe_entries(board);
        if entries.is_empty() {
            return None;
        }

        let entry = match self.selection {
            BookMoveSelection::Best => {
                entries.iter().max_by_key(|e| e.weight).copied()
            }
            BookMoveSelection::WeightedRandom => {
                weighted_random_select(&entries)
            }
            BookMoveSelection::UniformRandom => {
                uniform_random_select(&entries)
            }
        }?;

        decode_polyglot_move(entry.raw_move, board)
    }

    /// Probe and return all book moves with their weights.
    pub fn probe_all(&self, board: &Board) -> Vec<(Move, u16)> {
        let entries = self.probe_entries(board);
        let mut moves = Vec::with_capacity(entries.len());

        for entry in entries {
            if let Some(mv) = decode_polyglot_move(entry.raw_move, board) {
                moves.push((mv, entry.weight));
            }
        }

        // Sort by weight descending
        moves.sort_by(|a, b| b.1.cmp(&a.1));
        moves
    }
}

/// Decode a Polyglot move encoding into a Move.
///
/// Polyglot move encoding:
/// - bits 0-2: to file
/// - bits 3-5: to rank
/// - bits 6-8: from file
/// - bits 9-11: from rank
/// - bits 12-14: promotion piece (0=none, 1=knight, 2=bishop, 3=rook, 4=queen)
fn decode_polyglot_move(raw: u16, board: &Board) -> Option<Move> {
    let to_file = (raw & 0x7) as u8;
    let to_rank = ((raw >> 3) & 0x7) as u8;
    let from_file = ((raw >> 6) & 0x7) as u8;
    let from_rank = ((raw >> 9) & 0x7) as u8;
    let promotion = ((raw >> 12) & 0x7) as u8;

    let from = Square::from_coords(from_file, from_rank);
    let to = Square::from_coords(to_file, to_rank);

    // Get the piece that's moving
    let piece = board.piece_at(from)?;
    let captured = board.piece_at(to);

    // Handle special case: Polyglot encodes castling as king captures rook
    // We need to detect this and convert to standard castling notation
    if piece.piece_type == PieceType::King {
        // Check if this looks like castling (king moving to rook square)
        let is_castling = match (board.side_to_move(), from_file, to_file) {
            // White castling: e1 to a1 (queenside) or e1 to h1 (kingside)
            (Color::White, 4, 0) if from_rank == 0 => true,  // Queenside
            (Color::White, 4, 7) if from_rank == 0 => true,  // Kingside
            // Black castling: e8 to a8 (queenside) or e8 to h8 (kingside)
            (Color::Black, 4, 0) if from_rank == 7 => true,  // Queenside
            (Color::Black, 4, 7) if from_rank == 7 => true,  // Kingside
            _ => false,
        };

        if is_castling {
            // Convert to standard castling notation (king moves 2 squares)
            let castle_to = if to_file == 0 {
                // Queenside: king goes to c-file
                Square::from_coords(2, from_rank)
            } else {
                // Kingside: king goes to g-file
                Square::from_coords(6, from_rank)
            };

            return Some(Move {
                from,
                to: castle_to,
                move_type: MoveType::Castle,
                piece,
                captured: None,
            });
        }
    }

    // Determine move type
    let move_type = if promotion > 0 {
        let promo_piece = match promotion {
            1 => PieceType::Knight,
            2 => PieceType::Bishop,
            3 => PieceType::Rook,
            4 => PieceType::Queen,
            _ => return None,
        };
        MoveType::Promotion(promo_piece)
    } else if piece.piece_type == PieceType::Pawn && board.en_passant_square() == Some(to) {
        MoveType::EnPassant
    } else if captured.is_some() {
        MoveType::Capture
    } else {
        MoveType::Normal
    };

    Some(Move {
        from,
        to,
        move_type,
        piece,
        captured,
    })
}

/// Select a random entry weighted by move weight.
fn weighted_random_select(entries: &[BookEntry]) -> Option<BookEntry> {
    if entries.is_empty() {
        return None;
    }

    let total_weight: u32 = entries.iter().map(|e| e.weight as u32).sum();
    if total_weight == 0 {
        // If all weights are 0, fall back to uniform random
        return uniform_random_select(entries);
    }

    // Simple PRNG using current time
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let random = (seed ^ (seed >> 33)).wrapping_mul(0xff51afd7ed558ccd);
    let threshold = (random % total_weight as u64) as u32;

    let mut cumulative = 0u32;
    for entry in entries {
        cumulative += entry.weight as u32;
        if cumulative > threshold {
            return Some(*entry);
        }
    }

    Some(entries[entries.len() - 1])
}

/// Select a random entry uniformly.
fn uniform_random_select(entries: &[BookEntry]) -> Option<BookEntry> {
    if entries.is_empty() {
        return None;
    }

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let random = (seed ^ (seed >> 33)).wrapping_mul(0xff51afd7ed558ccd);
    let idx = (random as usize) % entries.len();

    Some(entries[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Path to the test opening book
    const TEST_BOOK_PATH: &str = "bins/Perfect2021.bin";

    #[test]
    fn test_load_perfect2021_book() {
        let mut manager = BookManager::new();
        let result = manager.load(TEST_BOOK_PATH);
        assert!(result.is_ok(), "Failed to load book: {:?}", result.err());
        assert!(manager.is_loaded());
        assert!(manager.len() > 0);
        println!("Loaded {} book entries", manager.len());
    }

    #[test]
    fn test_probe_starting_position() {
        let mut manager = BookManager::new();
        manager.load(TEST_BOOK_PATH).expect("Failed to load book");

        let board = Board::starting_position();
        let entries = manager.probe_entries(&board);

        // The starting position should have multiple book moves
        assert!(!entries.is_empty(), "No book entries for starting position");
        println!(
            "Found {} book entries for starting position",
            entries.len()
        );

        // Get all moves with weights
        let moves = manager.probe_all(&board);
        assert!(!moves.is_empty());

        for (mv, weight) in &moves {
            println!(
                "  {}{} (weight: {})",
                mv.from.to_algebraic(),
                mv.to.to_algebraic(),
                weight
            );
        }
    }

    #[test]
    fn test_probe_after_e4() {
        let mut manager = BookManager::new();
        manager.load(TEST_BOOK_PATH).expect("Failed to load book");

        // Position after 1. e4
        let board =
            Board::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1").unwrap();
        let moves = manager.probe_all(&board);

        assert!(!moves.is_empty(), "No book entries after 1. e4");
        println!("Book moves after 1. e4:");
        for (mv, weight) in &moves {
            println!(
                "  {}{} (weight: {})",
                mv.from.to_algebraic(),
                mv.to.to_algebraic(),
                weight
            );
        }
    }

    #[test]
    fn test_book_move_selection_best() {
        let mut manager = BookManager::new();
        manager.load(TEST_BOOK_PATH).expect("Failed to load book");
        manager.selection = BookMoveSelection::Best;

        let board = Board::starting_position();
        let mv = manager.probe(&board);
        assert!(mv.is_some(), "Should find a book move for starting position");

        // When using Best selection, we should get the highest weight move
        let all_moves = manager.probe_all(&board);
        if let Some(best_move) = mv {
            // The best move should match the first in probe_all (which is sorted by weight)
            assert_eq!(best_move.from, all_moves[0].0.from);
            assert_eq!(best_move.to, all_moves[0].0.to);
        }
    }

    #[test]
    fn test_decode_polyglot_move_e2e4() {
        // e2e4: from e2 (file=4, rank=1), to e4 (file=4, rank=3)
        // raw = (0 << 12) | (1 << 9) | (4 << 6) | (3 << 3) | 4
        //     = 0 | 512 | 256 | 24 | 4 = 796
        let raw = 796u16;
        let board = Board::starting_position();
        let mv = decode_polyglot_move(raw, &board).unwrap();

        assert_eq!(mv.from.file(), 4); // e-file
        assert_eq!(mv.from.rank(), 1); // rank 2
        assert_eq!(mv.to.file(), 4);   // e-file
        assert_eq!(mv.to.rank(), 3);   // rank 4
        assert_eq!(mv.piece.piece_type, PieceType::Pawn);
    }

    #[test]
    fn test_decode_polyglot_move_d2d4() {
        // d2d4: from d2 (file=3, rank=1), to d4 (file=3, rank=3)
        // raw = (0 << 12) | (1 << 9) | (3 << 6) | (3 << 3) | 3
        //     = 0 | 512 | 192 | 24 | 3 = 731
        let raw = 731u16;
        let board = Board::starting_position();
        let mv = decode_polyglot_move(raw, &board).unwrap();

        assert_eq!(mv.from.file(), 3); // d-file
        assert_eq!(mv.from.rank(), 1); // rank 2
        assert_eq!(mv.to.file(), 3);   // d-file
        assert_eq!(mv.to.rank(), 3);   // rank 4
    }

    #[test]
    fn test_book_manager_new() {
        let manager = BookManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn test_book_entry_decode() {
        // Test that raw bytes decode correctly
        let bytes: [u8; 16] = [
            0x46, 0x3B, 0x96, 0x18, 0x16, 0x91, 0xFC, 0x9C, // key (starting position)
            0x03, 0x1C, // move (e2e4 = 796)
            0x00, 0x64, // weight = 100
            0x00, 0x00, 0x00, 0x00, // learn = 0
        ];

        let entry = BookEntry {
            key: u64::from_be_bytes(bytes[0..8].try_into().unwrap()),
            raw_move: u16::from_be_bytes(bytes[8..10].try_into().unwrap()),
            weight: u16::from_be_bytes(bytes[10..12].try_into().unwrap()),
            learn: u32::from_be_bytes(bytes[12..16].try_into().unwrap()),
        };

        assert_eq!(entry.key, 0x463B96181691FC9C);
        assert_eq!(entry.raw_move, 796);
        assert_eq!(entry.weight, 100);
    }
}
