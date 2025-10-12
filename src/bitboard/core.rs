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

    pub fn count_pieces(&self) -> u32 {
        self.0.count_ones()
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn is_not_empty(&self) -> bool {
        self.0 != 0
    }

    pub fn pop_lsb(&mut self) -> Option<Square> {
        if self.is_empty() {
            None
        } else {
            let square_index = self.0.trailing_zeros() as usize;
            self.0 &= self.0 - 1;
            Some(Square::from_index(square_index))
        }
    }
    /// Returns the number of leading zeros.
    /// Returns 64 if the bitboard is empty.
    pub fn leading_zeros(&self) -> u32 {
        self.0.leading_zeros()
    }

    pub fn trailing_zeros(&self) -> u32 {
        self.0.trailing_zeros()
    }

    pub fn first_square(&self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square::from_index(self.0.leading_zeros() as usize))
        }
    }
}

impl std::ops::BitOr for Bitboard {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for Bitboard {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::BitXor for Bitboard {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl std::ops::Not for Bitboard {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl std::ops::BitOrAssign for Bitboard {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0
    }
}

impl std::ops::BitAndAssign for Bitboard {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0
    }
}

impl std::ops::BitXorAssign for Bitboard {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0
    }
}

impl Iterator for Bitboard {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        self.pop_lsb()
    }
}

impl std::ops::Shl<u32> for Bitboard {
    type Output = Self;
    fn shl(self, rhs: u32) -> Self::Output {
        Self(self.0 << rhs)
    }
}

impl std::ops::Shr<u32> for Bitboard {
    type Output = Self;
    fn shr(self, rhs: u32) -> Self::Output {
        Self(self.0 >> rhs)
    }
}

impl std::fmt::Display for Bitboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for rank in (0..8).rev() {
            for file in 0..8 {
                let square_index = rank * 8 + file;

                let bit = (self.0 >> square_index) & 1;
                write!(f, "{} ", if bit == 1 { "1" } else { "." })?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
