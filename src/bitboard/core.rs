use crate::types::Square;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Self = Self(0);
    pub const FULL: Self = Self(u64::MAX);

    #[inline(always)]
    pub fn is_set(self, square: Square) -> bool {
        let bitmask = 1u64 << square.index();
        // Any non-zero u64 is truthly
        self.0 & bitmask != 0
    }

    #[inline(always)]
    pub fn set(&mut self, square: Square) {
        let bitmask = 1u64 << square.index();
        self.0 |= bitmask
    }

    #[inline(always)]
    pub fn clear(&mut self, square: Square) {
        let bitmask = 1u64 << square.index();
        self.0 &= !bitmask
    }

    #[inline(always)]
    pub fn toggle(&mut self, square: Square) {
        let bitmask = 1u64 << square.index();
        self.0 ^= bitmask
    }

    #[inline(always)]
    pub fn count_pieces(&self) -> u32 {
        self.0.count_ones()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub fn is_not_empty(&self) -> bool {
        self.0 != 0
    }

    #[inline(always)]
    pub fn pop_lsb(&mut self) -> Option<Square> {
        if self.is_empty() {
            None
        } else {
            #[cfg(target_feature = "bmi1")]
            {
                use std::arch::x86_64::_blsi_u64;
                let lsb = unsafe { _blsi_u64(self.0) };

                let square_index = self.0.trailing_zeros() as usize;

                self.0 ^= lsb;

                Some(Square::from_index(square_index))
            }
            #[cfg(not(target_feature = "bmi1"))]
            {
                let square_index = self.0.trailing_zeros() as usize;
                self.0 &= self.0.wrapping_sub(1);
                Some(Square::from_index(square_index))
            }
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
            Some(Square::from_index(self.0.trailing_zeros() as usize))
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

pub const FILE_A: Bitboard = Bitboard(0x0101010101010101);
pub const FILE_B: Bitboard = Bitboard(0x0202020202020202);
pub const FILE_C: Bitboard = Bitboard(0x0404040404040404);
pub const FILE_D: Bitboard = Bitboard(0x0808080808080808);
pub const FILE_E: Bitboard = Bitboard(0x1010101010101010);
pub const FILE_F: Bitboard = Bitboard(0x2020202020202020);
pub const FILE_G: Bitboard = Bitboard(0x4040404040404040);
pub const FILE_H: Bitboard = Bitboard(0x8080808080808080);

pub const FILES: [Bitboard; 8] = [
    FILE_A, FILE_B, FILE_C, FILE_D, FILE_E, FILE_F, FILE_G, FILE_H,
];

pub const ADJACENT_FILES: [Bitboard; 8] = [
    Bitboard(FILE_B.0),            // A: only B
    Bitboard(FILE_A.0 | FILE_C.0), // B: A and C
    Bitboard(FILE_B.0 | FILE_D.0), // C: B and D
    Bitboard(FILE_C.0 | FILE_E.0), // D: C and E
    Bitboard(FILE_D.0 | FILE_F.0), // E: D and F
    Bitboard(FILE_E.0 | FILE_G.0), // F: E and G
    Bitboard(FILE_F.0 | FILE_H.0), // G: F and H
    Bitboard(FILE_G.0),            // H: only G
];

pub const RANK_1: Bitboard = Bitboard(0x00000000000000FF);
pub const RANK_2: Bitboard = Bitboard(0x000000000000FF00);
pub const RANK_3: Bitboard = Bitboard(0x0000000000FF0000);
pub const RANK_4: Bitboard = Bitboard(0x00000000FF000000);
pub const RANK_5: Bitboard = Bitboard(0x000000FF00000000);
pub const RANK_6: Bitboard = Bitboard(0x0000FF0000000000);
pub const RANK_7: Bitboard = Bitboard(0x00FF000000000000);
pub const RANK_8: Bitboard = Bitboard(0xFF00000000000000);

pub const RANKS: [Bitboard; 8] = [
    RANK_1, RANK_2, RANK_3, RANK_4, RANK_5, RANK_6, RANK_7, RANK_8,
];
