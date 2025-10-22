#[derive(Clone, Copy, Debug)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,

    pub black_kingside: bool,
    pub black_queenside: bool,
}

impl CastlingRights {
    pub const fn all() -> Self {
        Self {
            white_kingside: true,
            white_queenside: true,

            black_kingside: true,
            black_queenside: true,
        }
    }
}
