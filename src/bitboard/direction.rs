#[derive(Clone, Copy, Debug)]
pub struct Direction(i8);

impl Direction {
    pub const NORTH: Self = Self(8);
    pub const SOUTH: Self = Self(-8);
    pub const EAST: Self = Self(1);
    pub const WEST: Self = Self(-1);
    pub const NORTHEAST: Self = Self(9);
    pub const NORTHWEST: Self = Self(7);
    pub const SOUTHEAST: Self = Self(-7);
    pub const SOUTHWEST: Self = Self(-9);
}
