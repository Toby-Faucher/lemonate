use crate::FenError;

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

    pub const fn none() -> Self {
        Self {
            white_kingside: false,
            white_queenside: false,

            black_kingside: false,
            black_queenside: false,
        }
    }

    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        if fen == "-" {
            return Ok(Self::none());
        }

        let mut rights = Self::none();
        for ch in fen.chars() {
            match ch {
                'K' => rights.white_kingside = true,
                'Q' => rights.white_queenside = true,

                'k' => rights.black_kingside = true,
                'q' => rights.black_queenside = true,
                _ => return Err(FenError::InvalidCastlingRights),
            }
        }
        Ok(rights)
    }
}
