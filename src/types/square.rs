#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Square(u8);

impl Square {
    pub const fn from_coords(file: u8, rank: u8) -> Self {
        // This will wrap back if invalid inputs, EX:
        // from_coords(9,10) = from_coords(1,2) instead of panicing
        Square((rank & 7) * 8 + (file & 7))
    }

    pub const fn from_index(index: usize) -> Self {
        Square(index as u8)
    }
    pub const fn file(self) -> u8 {
        self.0 & 7
    }
    pub const fn rank(self) -> u8 {
        self.0 >> 3
    }
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Parse a square from algebraic notation (e.g., "e4", "a1")
    pub fn from_algebraic(s: &str) -> Result<Self, crate::FenError> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return Err(crate::FenError::InvalidPiecePlacement);
        }

        let file = bytes[0];
        let rank = bytes[1];

        if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
            return Err(crate::FenError::InvalidPiecePlacement);
        }

        let file_idx = file - b'a';
        let rank_idx = rank - b'1';

        Ok(Square::from_coords(file_idx, rank_idx))
    }
}
