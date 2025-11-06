use crate::Piece;
use crate::types::Square;

pub fn zobrist_piece_hash(_square: Square, _piece: Piece) -> u64 {
    // TODO: Implement proper Zobrist hashing
    // For now, return 0 - this means position hashing won't work correctly
    // but it won't crash the move generation
    0
}
