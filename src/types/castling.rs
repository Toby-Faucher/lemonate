#[derive(Clone, Copy, Debug)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,

    pub black_kingside: bool,
    pub black_queenside: bool,
}
