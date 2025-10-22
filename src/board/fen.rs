use crate::{FenError, board::Board};
impl Board {
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        unimplemented!()
    }

    fn parse_piece_placement(&mut self, placement: &str) -> Result<(), FenError> {
        unimplemented!()
    }
}
