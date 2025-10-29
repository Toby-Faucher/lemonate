use crate::{types::color::Color, FenError};

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
    pub fn from_fen_char(ch: char) -> Result<Self, FenError> {
        let color = if ch.is_uppercase() {
            Color::White
        } else {
            Color::Black
        };

        let piece_type = match ch.to_ascii_lowercase() {
            'p' => PieceType::Pawn,
            'n' => PieceType::Knight,
            'b' => PieceType::Bishop,
            'r' => PieceType::Rook,
            'q' => PieceType::Queen,
            'k' => PieceType::King,
            _ => return Err(FenError::InvalidPiece),
        };
        Ok(Piece { piece_type, color })
    }
}
