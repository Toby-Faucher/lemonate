use crate::{Board, CastlingRights, Color, FenError, Piece, Square};
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
        let ranks: Vec<&str> = placement.split('/').collect();

        if ranks.len() != 8 {
            return Err(FenError::InvalidPiecePlacement);
        }

        for (rank_idx, rank_str) in ranks.iter().enumerate() {
            let rank = 7 - rank_idx;
            let mut file = 0;

            for ch in rank_str.chars() {
                if ch.is_ascii_digit() {
                    let empty_count = ch.to_digit(10).unwrap() as u8;
                    file += empty_count;
                } else {
                    let piece = Piece::from_fen_char(ch)?;

                    let square = Square::from_coords(file, rank as u8);

                    self.place_piece(square, piece);

                    file += 1;
                }

                if file > 8 {
                    return Err(FenError::InvalidPiecePlacement);
                }
            }
            if file != 8 {
                return Err(FenError::InvalidPiecePlacement);
            }
        }

        Ok(())
    }
}
