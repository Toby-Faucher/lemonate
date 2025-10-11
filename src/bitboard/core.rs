use crate::square::Square;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Self = Self(0);
    pub const FULL: Self = Self(u64::MAX);

    // Set ops
    pub fn is_set(self, square: Square) -> bool {
        let bitmask = 1u64 << square.index();
        // Any non-zero u64 is truthly
        self.0 & bitmask != 0
    }

    pub fn set(&mut self, square: Square) {
        let bitmask = 1u64 << square.index();
        self.0 |= bitmask
    }

    pub fn clear(&mut self, square: Square) {
        let bitmask = 1u64 << square.index();
        self.0 &= !bitmask
    }

    pub fn toggle(&mut self, square: Square) {
        let bitmask = 1u64 << square.index();
        self.0 ^= bitmask
    }
}
