#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Square(u8);

impl Square {
    pub const fn from_coords(file: u8, rank: u8) -> Self {
        // This will wrap back if invalid inputs, EX:
        // from_coords(9,10) = from_coords(1,2) instead of panicing
        Square((rank & 7) * 8 + (file & 7))
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
}
