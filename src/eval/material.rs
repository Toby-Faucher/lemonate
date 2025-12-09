use crate::types::piece::PieceType;
use crate::types::Color;
use crate::Board;

pub struct Material {
    pub pawn: u16,
    pub knight: u16,
    pub bishop: u16,
    pub rook: u16,
    pub queen: u16,
    pub king: u16,
}

impl Material {
    pub const fn default() -> Self {
        Self {
            pawn: 100,
            knight: 320,
            bishop: 330,
            rook: 500,
            queen: 900,
            king: 0,
        }
    }

    /// Create material values using PeSTO's middlegame values
    pub const fn pesto_mg() -> Self {
        Self {
            pawn: 82,
            knight: 337,
            bishop: 365,
            rook: 477,
            queen: 1025,
            king: 0,
        }
    }

    /// Create material values using PeSTO's endgame values
    pub const fn pesto_eg() -> Self {
        Self {
            pawn: 94,
            knight: 281,
            bishop: 297,
            rook: 512,
            queen: 936,
            king: 0,
        }
    }

    pub fn value(&self, piece_type: PieceType) -> u16 {
        match piece_type {
            PieceType::Pawn => self.pawn,
            PieceType::Knight => self.knight,
            PieceType::Bishop => self.bishop,
            PieceType::Rook => self.rook,
            PieceType::Queen => self.queen,
            PieceType::King => self.king,
        }
    }
}

pub struct MaterialEvaluator {
    material: Material,
}

impl MaterialEvaluator {
    pub fn new() -> Self {
        Self {
            material: Material::default(),
        }
    }

    pub fn evaluate(&self, board: &Board) -> i32 {
        let mut score = 0i32;

        for piece_type in [
            PieceType::Pawn,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
            PieceType::King,
        ] {
            let white_count = board.piece_bitboard(Color::White, piece_type).count_pieces();
            let black_count = board.piece_bitboard(Color::Black, piece_type).count_pieces();

            let piece_value = self.material.value(piece_type) as i32;
            score += piece_value * white_count as i32;
            score -= piece_value * black_count as i32;
        }

        score
    }
}

