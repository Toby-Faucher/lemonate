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

#[repr(align(64))]
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

    mailbox: [Option<Piece>; 64],
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
            mailbox: [None; 64],
        }
    }

    pub fn starting_position() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
    }

    #[inline(always)]
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        unsafe { *self.mailbox.get_unchecked(square.index()) }
    }

    pub fn place_piece(&mut self, square: Square, piece: Piece) {
        self.piece_bitboards[piece.color as usize][piece.piece_type as usize].set(square);

        self.color_bitboard[piece.color as usize].set(square);

        self.all_pieces.set(square);

        self.mailbox[square.index()] = Some(piece);

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

    pub fn color_bitboard(&self, color: Color) -> Bitboard {
        self.color_bitboard[color as usize]
    }

    /// Check if the side to move is in check.
    pub fn is_in_check(&self) -> bool {
        let color = self.side_to_move;
        let king_bb = self.piece_bitboard(color, PieceType::King);

        if king_bb.is_empty() {
            return false;
        }

        let king_square = king_bb.into_iter().next().unwrap();
        self.is_square_attacked(king_square, color.opposite())
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
