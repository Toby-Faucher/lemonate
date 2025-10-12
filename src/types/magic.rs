use crate::bitboard::Square;
use crate::Bitboard;

#[derive(Clone, Copy)]
pub struct Magic {
    pub mask: Bitboard,
    pub magic: u64,
    pub shift: u32,
    pub offset: u32,
}

impl Magic {
    pub fn new(mask: Bitboard, magic: u64, offset: u32) -> Self {
        let shift = 64 - mask.count_pieces();
        Self {
            mask,
            magic,
            shift,
            offset,
        }
    }

    pub fn hash(&self, blockers: Bitboard) -> usize {
        let relevant = blockers & self.mask;

        let hash = relevant.0.wrapping_mul(self.magic) >> self.shift;

        hash as usize
    }

    pub fn table_size(&self) -> usize {
        1 << self.mask.count_pieces()
    }
}
