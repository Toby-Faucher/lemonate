use crate::bitboard::Bitboard;
use crate::square::Square;
use crate::types::Color;

#[derive(Clone, Debug)]
pub struct Board {
    piece_bitboards: [[Bitboard; 6]; 2],
    color_bitboard: [Bitboard; 2],
    all_pieces: Bitboard,

    side_to_move: Color,
    castling_rights: CastlingRights,
    en_passant_square: Option<Square>,
    halfmove_clock: u16,
    fullmove_number: u16,

    position_hash: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,

    pub black_kingside: bool,
    pub black_queenside: bool,
}

impl Board {
    pub fn new() -> Self {
        Self {
            piece_bitboards: [[Bitboard::EMPTY; 6]; 2],
            color_bitboard: [Bitboard::EMPTY; 2],
            all_pieces: Bitboard::EMPTY,
            side_to_move: Color::White,
            //TODO: impl all
            castling_rights: CastlingRights::all(),
            en_passant_square: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            position_hash: 0,
        }
    }
}
