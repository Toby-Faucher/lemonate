use crate::itboard::Bitboard;
use crate::Magic;

pub struct AttackTable {
    pub rook_attacks: Box<[Bitboard]>,
    pub bishop_attacks: Box<[Bitboard]>,
    pub rook_magics: [Magic; 64],
    pub bishop_magics: [Magic; 64],
}
