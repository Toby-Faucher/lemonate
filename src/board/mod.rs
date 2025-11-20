use crate::bitboard::Bitboard;
use crate::types::castling::CastlingRights;
use crate::types::Color;
use crate::types::Square;
use crate::Piece;
use crate::PieceType;

mod make_move;
pub use make_move::BoardState;

mod zobrist;
use zobrist::zobrist_piece_hash;

mod fen;

mod moves;
pub use moves::{Move, MoveType};

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

    // Move history for make/unmake operations
    move_history: Option<Vec<(Move, BoardState)>>,
}

impl Board {
    pub fn new() -> Self {
        Self {
            piece_bitboards: [[Bitboard::EMPTY; 6]; 2],
            color_bitboard: [Bitboard::EMPTY; 2],
            all_pieces: Bitboard::EMPTY,
            side_to_move: Color::White,
            castling_rights: CastlingRights::all(),
            en_passant_square: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            position_hash: 0,
            move_history: None,
        }
    }

    pub fn peice_at(&self, square: Square) -> Option<Piece> {
        if !self.all_pieces.is_set(square) {
            return None;
        }

        for color in [Color::White, Color::Black] {
            if self.color_bitboard[color as usize].is_set(square) {
                for piece_type in [
                    PieceType::Pawn,
                    PieceType::Knight,
                    PieceType::Bishop,
                    PieceType::Rook,
                    PieceType::Queen,
                    PieceType::King,
                ] {
                    if self.piece_bitboards[color as usize][piece_type as usize].is_set(square) {
                        return Some(Piece { piece_type, color });
                    }
                }
            }
        }
        None
    }

    pub fn place_piece(&mut self, square: Square, piece: Piece) {
        self.piece_bitboards[piece.color as usize][piece.piece_type as usize].set(square);

        self.color_bitboard[piece.color as usize].set(square);

        self.all_pieces.set(square);

        self.position_hash ^= zobrist_piece_hash(square, piece);
    }

    pub fn all_pieces(&self) -> Bitboard {
        self.all_pieces
    }

    pub fn position_hash(&self) -> u64 {
        self.position_hash
    }

    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    pub fn piece_bitboard(&self, color: Color, piece_type: PieceType) -> Bitboard {
        self.piece_bitboards[color as usize][piece_type as usize]
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum FenError {
    InvalidFormat,
    InvalidPiecePlacement,
    InvalidActiveColor,
    InvalidCastlingRights,
    InvalidPiece,
    InvalidHalfMove,
    InvalidFullMove,
}
