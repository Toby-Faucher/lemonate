use crate::types::Square;
use crate::Piece;

// Zobrist table layout:
// - Indices 0-767: Piece-square combinations (64 squares * 12 piece types)
// - Indices 768-771: Castling rights (4 combinations)
// - Indices 772-779: En passant files (8 files)
// - Index 780: Side to move
const MAX_ZOBRIST_LEN: usize = 800;

const CASTLING_OFFSET: usize = 768;
const EN_PASSANT_OFFSET: usize = 772;
const SIDE_TO_MOVE_OFFSET: usize = 780;

const fn generate_zobrist_tables() -> [u64; MAX_ZOBRIST_LEN] {
    let mut table = [0u64; MAX_ZOBRIST_LEN];

    let mut seed = 0x123456789abcdef0u64;
    let mut i = 0;

    while i < MAX_ZOBRIST_LEN {
        seed ^= seed << 13;
        seed ^= seed >> 7; // Fixed: right shift instead of left shift
        seed ^= seed << 17;
        table[i] = seed;
        i += 1;
    }

    table
}

static ZOBRIST_TABLE: [u64; MAX_ZOBRIST_LEN] = generate_zobrist_tables();

pub fn zobrist_piece_hash(square: Square, piece: Piece) -> u64 {
    let piece_type = piece.piece_type as usize;
    let color = piece.color as usize;
    let square_idx = square.index();

    let index = square_idx * 12 + piece_type * 2 + color;

    ZOBRIST_TABLE[index]
}

/// Hash for castling rights
/// bits: 0 = white kingside, 1 = white queenside, 2 = black kingside, 3 = black queenside
pub fn zobrist_castling_hash(castling_rights: u8) -> u64 {
    let mut hash = 0u64;

    for i in 0..4 {
        if castling_rights & (1 << i) != 0 {
            hash ^= ZOBRIST_TABLE[CASTLING_OFFSET + i];
        }
    }

    hash
}

/// Hash for en passant file (0-7 for files a-h)
/// Returns 0 if file is None (no en passant available)
pub fn zobrist_en_passant_hash(file: Option<u8>) -> u64 {
    match file {
        Some(f) if f < 8 => ZOBRIST_TABLE[EN_PASSANT_OFFSET + f as usize],
        _ => 0,
    }
}

/// Hash for side to move (XOR this when it's black's turn)
pub fn zobrist_side_to_move_hash() -> u64 {
    ZOBRIST_TABLE[SIDE_TO_MOVE_OFFSET]
}
