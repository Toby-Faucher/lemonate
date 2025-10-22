use crate::{FenError, types::color::Color};

#[derive(Clone, Copy, Debug)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub piece_type: PieceType,
    pub color: Color,
}

impl Piece {
    fn from_fen_char(ch: char) -> Result<Self, FenError> {
        unimplemented!()
    }
}
