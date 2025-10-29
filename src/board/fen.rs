use crate::{Board, CastlingRights, Color, FenError, Square};
impl Board {
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            return Err(FenError::InvalidFormat);
        }

        let mut board = Board::new();

        board.parse_piece_placement(parts[0])?;

        board.side_to_move = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err(FenError::InvalidActiveColor),
        };

        board.castling_rights = CastlingRights::from_fen(parts[2])?;

        board.en_passant_square = if parts[3] == "-" {
            None
        } else {
            Some(Square::from_algebraic(parts[3])?)
        };

        board.halfmove_clock == parts[4].parse().map_err(|_| FenError::InvalidHalfMove)?;
        board.fullmove_number == parts[5].parse().map_err(|_| FenError::InvalidFullMove)?;

        board.recalculate_occupancy();
        board.recalculate_hash();

        Ok(board)
    }

    fn parse_piece_placement(&mut self, placement: &str) -> Result<(), FenError> {
        unimplemented!()
    }
}
