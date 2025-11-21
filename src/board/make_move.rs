use super::zobrist::{
    zobrist_castling_hash, zobrist_en_passant_hash, zobrist_piece_hash, zobrist_side_to_move_hash,
};
use super::{Board, Move, MoveType};
use crate::types::castling::CastlingRights;
use crate::types::{Color, Piece, PieceType, Square};
use crate::Bitboard;

/// Stores all reversible board state that needs to be saved before making a move
#[derive(Clone, Debug)]
pub struct BoardState {
    /// Castling rights before the move
    pub castling_rights: CastlingRights,
    /// En passant square before the move
    pub en_passant_square: Option<Square>,
    /// Halfmove clock before the move
    pub halfmove_clock: u16,
    /// Position hash before the move
    pub position_hash: u64,
    /// Captured piece (if any)
    pub captured_piece: Option<Piece>,
}

impl Board {
    /// Makes a move on the board, updating all state and Zobrist hash
    /// Returns true if the move was legal and executed, false otherwise
    pub fn make_move(&mut self, mv: Move) -> bool {
        // Check if move is legal
        if !self.is_legal_move(mv) {
            return false;
        }

        // Save current state
        let state = BoardState {
            castling_rights: self.castling_rights,
            en_passant_square: self.en_passant_square,
            halfmove_clock: self.halfmove_clock,
            position_hash: self.position_hash,
            captured_piece: mv.captured,
        };

        // Execute the move with state updates
        self.execute_move(mv);

        // Store move and state in history
        if !self.move_history.is_some() {
            self.move_history = Some(Vec::new());
        }
        if let Some(ref mut history) = self.move_history {
            history.push((mv, state));
        }

        true
    }

    /// Unmakes the last move, restoring the previous board state
    /// Returns true if successful, false if no moves to unmake
    pub fn unmake_move(&mut self) -> bool {
        // Get the last move and state from history
        let (mv, state) = match &mut self.move_history {
            Some(history) if !history.is_empty() => history.pop().unwrap(),
            _ => return false,
        };

        // Reverse the move execution
        self.reverse_move(mv, &state);

        // Restore the saved state
        self.castling_rights = state.castling_rights;
        self.en_passant_square = state.en_passant_square;
        self.halfmove_clock = state.halfmove_clock;
        self.position_hash = state.position_hash;

        true
    }

    /// Execute a move, updating board state and hash (internal use only)
    fn execute_move(&mut self, mv: Move) {
        let color = mv.piece.color;

        // Remove piece from source and update hash
        self.remove_piece(mv.from, mv.piece);

        // Handle captures
        if let Some(captured) = mv.captured {
            let capture_square = if mv.move_type == MoveType::EnPassant {
                // En passant captures on a different square
                let direction = if color == Color::White { -1i8 } else { 1i8 };
                let capture_rank = (mv.to.rank() as i8 + direction) as u8;
                Square::from_coords(mv.to.file(), capture_rank)
            } else {
                mv.to
            };

            // Remove captured piece and update hash
            self.remove_piece(capture_square, captured);
        }

        // Place piece on destination
        let final_piece = match mv.move_type {
            MoveType::Promotion(promo_type) => Piece {
                piece_type: promo_type,
                color,
            },
            _ => mv.piece,
        };
        self.place_piece_no_hash(mv.to, final_piece);
        self.position_hash ^= zobrist_piece_hash(mv.to, final_piece);

        // Handle castling - move the rook
        if mv.move_type == MoveType::Castle {
            let (rook_from, rook_to) = self.get_castling_rook_squares(color, mv.to);
            let rook = Piece {
                piece_type: PieceType::Rook,
                color,
            };

            self.remove_piece(rook_from, rook);
            self.place_piece_no_hash(rook_to, rook);
            self.position_hash ^= zobrist_piece_hash(rook_to, rook);
        }

        // Update castling rights and hash
        self.update_castling_rights(mv);

        // Update en passant square and hash
        self.update_en_passant_square(mv);

        // Update halfmove clock
        self.update_halfmove_clock(mv);

        // Update fullmove number
        if color == Color::Black {
            self.fullmove_number += 1;
        }

        // Switch side to move and update hash
        self.side_to_move = color.opposite();
        self.position_hash ^= zobrist_side_to_move_hash();
    }

    /// Reverse a move, restoring piece positions (internal use only)
    fn reverse_move(&mut self, mv: Move, state: &BoardState) {
        let color = mv.piece.color;

        // Switch side back
        self.side_to_move = color;

        // Restore fullmove number
        if color == Color::Black {
            self.fullmove_number -= 1;
        }

        // Remove piece from destination
        let final_piece = match mv.move_type {
            MoveType::Promotion(promo_type) => Piece {
                piece_type: promo_type,
                color,
            },
            _ => mv.piece,
        };
        self.remove_piece_no_hash(mv.to, final_piece);

        // Restore captured piece if any
        if let Some(captured) = state.captured_piece {
            let capture_square = if mv.move_type == MoveType::EnPassant {
                // En passant captures on a different square
                let direction = if color == Color::White { -1i8 } else { 1i8 };
                let capture_rank = (mv.to.rank() as i8 + direction) as u8;
                Square::from_coords(mv.to.file(), capture_rank)
            } else {
                mv.to
            };

            self.place_piece_no_hash(capture_square, captured);
        }

        // Place piece back on source square
        self.place_piece_no_hash(mv.from, mv.piece);

        // Handle castling - move rook back
        if mv.move_type == MoveType::Castle {
            let (rook_from, rook_to) = self.get_castling_rook_squares(color, mv.to);
            let rook = Piece {
                piece_type: PieceType::Rook,
                color,
            };

            self.remove_piece_no_hash(rook_to, rook);
            self.place_piece_no_hash(rook_from, rook);
        }
    }

    /// Remove a piece from a square, updating bitboards and hash
    fn remove_piece(&mut self, square: Square, piece: Piece) {
        self.piece_bitboards[piece.color as usize][piece.piece_type as usize].clear(square);
        self.color_bitboard[piece.color as usize].clear(square);
        self.all_pieces.clear(square);
        self.mailbox[square.index()] = None;
        self.position_hash ^= zobrist_piece_hash(square, piece);
    }

    /// Place a piece without updating hash (used in move reversal)
    #[inline(always)]
    fn place_piece_no_hash(&mut self, square: Square, piece: Piece) {
        let mask = Bitboard(1u64 << square.index());
        self.piece_bitboards[piece.color as usize][piece.piece_type as usize] |= mask;
        self.color_bitboard[piece.color as usize] |= mask;
        self.all_pieces |= mask;
        self.mailbox[square.index()] = Some(piece);
    }

    /// Remove a piece without updating hash (used in move reversal)
    #[inline(always)]
    fn remove_piece_no_hash(&mut self, square: Square, piece: Piece) {
        let mask = !(Bitboard(1u64 << square.index()));
        self.piece_bitboards[piece.color as usize][piece.piece_type as usize] &= mask;
        self.color_bitboard[piece.color as usize] &= mask;
        self.all_pieces &= mask;
    }

    /// Get rook squares for castling move
    fn get_castling_rook_squares(&self, color: Color, king_to: Square) -> (Square, Square) {
        match (color, king_to.file()) {
            (Color::White, 6) => {
                // White kingside
                (Square::from_coords(7, 0), Square::from_coords(5, 0))
            }
            (Color::White, 2) => {
                // White queenside
                (Square::from_coords(0, 0), Square::from_coords(3, 0))
            }
            (Color::Black, 6) => {
                // Black kingside
                (Square::from_coords(7, 7), Square::from_coords(5, 7))
            }
            (Color::Black, 2) => {
                // Black queenside
                (Square::from_coords(0, 7), Square::from_coords(3, 7))
            }
            _ => panic!("Invalid castling move"),
        }
    }

    /// Update castling rights based on the move
    fn update_castling_rights(&mut self, mv: Move) {
        // XOR out old castling rights from hash
        self.position_hash ^= self.castling_rights_hash();

        let color = mv.piece.color;

        // King move removes all castling rights for that color
        if mv.piece.piece_type == PieceType::King {
            match color {
                Color::White => {
                    self.castling_rights.white_kingside = false;
                    self.castling_rights.white_queenside = false;
                }
                Color::Black => {
                    self.castling_rights.black_kingside = false;
                    self.castling_rights.black_queenside = false;
                }
            }
        }

        // Rook move from starting square removes that side's castling
        if mv.piece.piece_type == PieceType::Rook {
            match (color, mv.from.file(), mv.from.rank()) {
                (Color::White, 0, 0) => self.castling_rights.white_queenside = false,
                (Color::White, 7, 0) => self.castling_rights.white_kingside = false,
                (Color::Black, 0, 7) => self.castling_rights.black_queenside = false,
                (Color::Black, 7, 7) => self.castling_rights.black_kingside = false,
                _ => {}
            }
        }

        // Rook captured on starting square removes that side's castling
        if let Some(captured) = mv.captured {
            if captured.piece_type == PieceType::Rook {
                match (captured.color, mv.to.file(), mv.to.rank()) {
                    (Color::White, 0, 0) => self.castling_rights.white_queenside = false,
                    (Color::White, 7, 0) => self.castling_rights.white_kingside = false,
                    (Color::Black, 0, 7) => self.castling_rights.black_queenside = false,
                    (Color::Black, 7, 7) => self.castling_rights.black_kingside = false,
                    _ => {}
                }
            }
        }

        // XOR in new castling rights to hash
        self.position_hash ^= self.castling_rights_hash();
    }

    /// Update en passant square based on the move
    fn update_en_passant_square(&mut self, mv: Move) {
        // XOR out old en passant from hash
        if let Some(old_ep) = self.en_passant_square {
            self.position_hash ^= zobrist_en_passant_hash(Some(old_ep.file()));
        }

        // Set new en passant square if pawn double push
        if mv.piece.piece_type == PieceType::Pawn {
            let rank_diff = (mv.to.rank() as i8 - mv.from.rank() as i8).abs();
            if rank_diff == 2 {
                // Pawn double push
                let ep_rank = if mv.piece.color == Color::White {
                    mv.from.rank() + 1
                } else {
                    mv.from.rank() - 1
                };
                self.en_passant_square = Some(Square::from_coords(mv.from.file(), ep_rank));

                // XOR in new en passant to hash
                self.position_hash ^= zobrist_en_passant_hash(Some(mv.from.file()));
            } else {
                self.en_passant_square = None;
            }
        } else {
            self.en_passant_square = None;
        }
    }

    /// Update halfmove clock (50-move rule)
    fn update_halfmove_clock(&mut self, mv: Move) {
        // Reset on pawn move or capture
        if mv.piece.piece_type == PieceType::Pawn || mv.captured.is_some() {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }
    }

    /// Get hash for current castling rights
    fn castling_rights_hash(&self) -> u64 {
        let mut bits = 0u8;
        if self.castling_rights.white_kingside {
            bits |= 1 << 0;
        }
        if self.castling_rights.white_queenside {
            bits |= 1 << 1;
        }
        if self.castling_rights.black_kingside {
            bits |= 1 << 2;
        }
        if self.castling_rights.black_queenside {
            bits |= 1 << 3;
        }
        zobrist_castling_hash(bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_unmake_simple_move() {
        let mut board =
            Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();

        let initial_hash = board.position_hash;

        // Generate a legal move
        let moves = board.generate_legal_moves();
        assert!(!moves.is_empty());

        let mv = moves[0];

        // Make the move
        assert!(board.make_move(mv));

        // Hash should have changed
        assert_ne!(board.position_hash, initial_hash);

        // Unmake the move
        assert!(board.unmake_move());

        // Hash should be back to initial
        assert_eq!(board.position_hash, initial_hash);
    }

    #[test]
    fn test_unmake_empty_history() {
        let mut board = Board::new();
        assert!(!board.unmake_move());
    }
}
