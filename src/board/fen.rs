use crate::types::piece::PieceType;
use crate::{Board, CastlingRights, Color, FenError, Piece, Square};
use super::zobrist::{zobrist_en_passant_hash, zobrist_side_to_move_hash};
impl Board {
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        // The halfmove clock and fullmove number are optional: EPD-style records and many
        // book and puzzle files stop after the en passant field.
        if parts.len() != 4 && parts.len() != 6 {
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

        if parts.len() == 6 {
            board.halfmove_clock = parts[4].parse().map_err(|_| FenError::InvalidHalfMove)?;
            board.fullmove_number = parts[5].parse().map_err(|_| FenError::InvalidFullMove)?;
        } else {
            board.halfmove_clock = 0;
            board.fullmove_number = 1;
        }

        // An en passant square only means something when the pawn that just made the double
        // step is really behind it. Move generation trusts the field, so a bogus square would
        // let a pawn "capture" a pawn that is not there and corrupt the board on unmake.
        if let Some(ep) = board.en_passant_square {
            let (ep_rank, behind, pawn_color) = match board.side_to_move {
                Color::White => (5, ep.index().wrapping_sub(8), Color::Black),
                Color::Black => (2, ep.index() + 8, Color::White),
            };
            let valid = ep.rank() == ep_rank
                && board.piece_at(ep).is_none()
                && behind < 64
                && matches!(
                    board.piece_at(Square::from_index(behind)),
                    Some(Piece { piece_type: PieceType::Pawn, color }) if color == pawn_color
                );
            if !valid {
                board.en_passant_square = None;
            }
        }

        // Fold side-to-move, castling rights, and en passant square into the
        // Zobrist hash so positions differing only in these fields hash
        // differently (matching the incremental updates in make/unmake).
        if board.side_to_move == Color::Black {
            board.position_hash ^= zobrist_side_to_move_hash();
        }
        board.position_hash ^= board.castling_rights_hash();
        if let Some(ep) = board.en_passant_square {
            board.position_hash ^= zobrist_en_passant_hash(Some(ep.file()));
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fen_without_move_counters_is_accepted() {
        let board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -").unwrap();
        assert_eq!(board.halfmove_clock(), 0);
        assert!(Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0").is_err());
        assert!(Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w").is_err());
    }

    #[test]
    fn en_passant_square_without_a_pawn_behind_it_is_dropped() {
        // Black pawn on d4, but no white pawn on e4: e3 cannot be a real en passant target.
        let bogus = Board::from_fen("4k3/8/8/8/3p4/8/8/4K3 b - e3 0 1").unwrap();
        let plain = Board::from_fen("4k3/8/8/8/3p4/8/8/4K3 b - - 0 1").unwrap();
        assert_eq!(bogus.position_hash, plain.position_hash);
        // Wrong rank for the side to move.
        let wrong_rank = Board::from_fen("4k3/8/8/8/3pP3/8/8/4K3 b - e6 0 1").unwrap();
        assert_eq!(wrong_rank.position_hash, Board::from_fen("4k3/8/8/8/3pP3/8/8/4K3 b - - 0 1").unwrap().position_hash);
        // A genuine square survives.
        let real = Board::from_fen("4k3/8/8/8/3pP3/8/8/4K3 b - e3 0 1").unwrap();
        assert_ne!(real.position_hash, Board::from_fen("4k3/8/8/8/3pP3/8/8/4K3 b - - 0 1").unwrap().position_hash);
    }

    #[test]
    fn test_fen_hash_distinguishes_side_to_move() {
        let w = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let b = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1").unwrap();
        assert_ne!(w.position_hash, b.position_hash);
    }

    #[test]
    fn test_fen_hash_distinguishes_castling_rights() {
        let full =
            Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let none = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1").unwrap();
        assert_ne!(full.position_hash, none.position_hash);
    }

    #[test]
    fn test_fen_hash_distinguishes_en_passant() {
        let with_ep =
            Board::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1").unwrap();
        let without_ep =
            Board::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1").unwrap();
        assert_ne!(with_ep.position_hash, without_ep.position_hash);
    }
}
