use crate::{AttackTable, Board, Color, Piece, PieceType, Square};
use once_cell::sync::Lazy;

// Global static attack table - initialized once and shared across all boards
static ATTACK_TABLE: Lazy<AttackTable> = Lazy::new(|| AttackTable::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub move_type: MoveType,
    pub piece: Piece,
    pub captured: Option<Piece>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveType {
    Normal,
    Capture,
    EnPassant,
    Castle,
    Promotion(PieceType), // Queen, Rook, Bishop, Knight
}

impl Board {
    pub fn generate_legal_moves(&self) -> Vec<Move> {
        let mut legal_moves = Vec::new();

        let pseudo_legal = self.generate_pseudo_legal_moves();

        for mv in pseudo_legal {
            if self.is_legal_move(mv) {
                legal_moves.push(mv);
            }
        }
        legal_moves
    }

    pub fn generate_pseudo_legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();

        let color = self.side_to_move;
        let color_idx = color as usize;

        for piece_type in [
            PieceType::Pawn,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Queen,
            PieceType::King,
        ] {
            let piece_bb = self.piece_bitboards[color_idx][piece_type as usize];

            for square in piece_bb {
                match piece_type {
                    PieceType::Pawn => self.generate_pawn_moves(square, &mut moves),
                    PieceType::Knight => self.generate_knight_moves(square, &mut moves),
                    PieceType::Bishop => self.generate_bishop_moves(square, &mut moves),
                    PieceType::Rook => self.generate_rook_moves(square, &mut moves),
                    PieceType::Queen => self.generate_queen_moves(square, &mut moves),
                    PieceType::King => self.generate_king_moves(square, &mut moves),
                }
            }
        }
        moves
    }

    fn generate_knight_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.side_to_move;
        let piece = Piece {
            piece_type: PieceType::Knight,
            color,
        };

        // Get all knight attacks from the attack table
        let attacks = ATTACK_TABLE.knight_attacks(from);

        // Filter out squares with friendly pieces
        let friendly = self.color_bitboard[color as usize];
        let valid_targets = attacks & !friendly;

        // Generate moves for each valid target
        for to in valid_targets {
            let captured = self.piece_at(to);
            let move_type = if captured.is_some() {
                MoveType::Capture
            } else {
                MoveType::Normal
            };

            moves.push(Move {
                from,
                to,
                move_type,
                piece,
                captured,
            });
        }
    }

    fn generate_bishop_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.side_to_move;
        let piece = Piece {
            piece_type: PieceType::Bishop,
            color,
        };

        // Get bishop attacks considering blockers
        let attacks = ATTACK_TABLE.bishop_attacks(from, self.all_pieces);

        // Filter out squares with friendly pieces
        let friendly = self.color_bitboard[color as usize];
        let valid_targets = attacks & !friendly;

        // Generate moves for each valid target
        for to in valid_targets {
            let captured = self.piece_at(to);
            let move_type = if captured.is_some() {
                MoveType::Capture
            } else {
                MoveType::Normal
            };

            moves.push(Move {
                from,
                to,
                move_type,
                piece,
                captured,
            });
        }
    }

    fn generate_rook_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.side_to_move;
        let piece = Piece {
            piece_type: PieceType::Rook,
            color,
        };

        // Get rook attacks considering blockers
        let attacks = ATTACK_TABLE.rook_attacks(from, self.all_pieces);

        // Filter out squares with friendly pieces
        let friendly = self.color_bitboard[color as usize];
        let valid_targets = attacks & !friendly;

        // Generate moves for each valid target
        for to in valid_targets {
            let captured = self.piece_at(to);
            let move_type = if captured.is_some() {
                MoveType::Capture
            } else {
                MoveType::Normal
            };

            moves.push(Move {
                from,
                to,
                move_type,
                piece,
                captured,
            });
        }
    }

    fn generate_queen_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.side_to_move;
        let piece = Piece {
            piece_type: PieceType::Queen,
            color,
        };

        // Queen moves = rook moves + bishop moves
        let attacks = ATTACK_TABLE.queen_attacks(from, self.all_pieces);

        // Filter out squares with friendly pieces
        let friendly = self.color_bitboard[color as usize];
        let valid_targets = attacks & !friendly;

        // Generate moves for each valid target
        for to in valid_targets {
            let captured = self.piece_at(to);
            let move_type = if captured.is_some() {
                MoveType::Capture
            } else {
                MoveType::Normal
            };

            moves.push(Move {
                from,
                to,
                move_type,
                piece,
                captured,
            });
        }
    }

    fn generate_king_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.side_to_move;
        let piece = Piece {
            piece_type: PieceType::King,
            color,
        };

        // Get all king attacks from the attack table
        let attacks = ATTACK_TABLE.king_attacks(from);

        // Filter out squares with friendly pieces
        let friendly = self.color_bitboard[color as usize];
        let valid_targets = attacks & !friendly;

        // Generate normal king moves
        for to in valid_targets {
            let captured = self.piece_at(to);
            let move_type = if captured.is_some() {
                MoveType::Capture
            } else {
                MoveType::Normal
            };

            moves.push(Move {
                from,
                to,
                move_type,
                piece,
                captured,
            });
        }

        // Generate castling moves
        self.generate_castling_moves(from, moves);
    }

    fn generate_castling_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.side_to_move;
        let piece = Piece {
            piece_type: PieceType::King,
            color,
        };

        // Can't castle if in check
        if self.is_square_attacked(from, color.opposite()) {
            return;
        }

        match color {
            Color::White => {
                // White kingside castling
                if self.castling_rights.white_kingside() {
                    let f1 = Square::from_coords(5, 0);
                    let g1 = Square::from_coords(6, 0);

                    // Check squares are empty
                    if !self.all_pieces.is_set(f1) && !self.all_pieces.is_set(g1) {
                        // Check king doesn't pass through or land in check
                        if !self.is_square_attacked(f1, Color::Black)
                            && !self.is_square_attacked(g1, Color::Black)
                        {
                            moves.push(Move {
                                from,
                                to: g1,
                                move_type: MoveType::Castle,
                                piece,
                                captured: None,
                            });
                        }
                    }
                }

                // White queenside castling
                if self.castling_rights.white_queenside() {
                    let d1 = Square::from_coords(3, 0);
                    let c1 = Square::from_coords(2, 0);
                    let b1 = Square::from_coords(1, 0);

                    // Check squares are empty
                    if !self.all_pieces.is_set(d1)
                        && !self.all_pieces.is_set(c1)
                        && !self.all_pieces.is_set(b1)
                    {
                        // Check king doesn't pass through or land in check
                        if !self.is_square_attacked(d1, Color::Black)
                            && !self.is_square_attacked(c1, Color::Black)
                        {
                            moves.push(Move {
                                from,
                                to: c1,
                                move_type: MoveType::Castle,
                                piece,
                                captured: None,
                            });
                        }
                    }
                }
            }
            Color::Black => {
                // Black kingside castling
                if self.castling_rights.black_kingside() {
                    let f8 = Square::from_coords(5, 7);
                    let g8 = Square::from_coords(6, 7);

                    if !self.all_pieces.is_set(f8) && !self.all_pieces.is_set(g8) {
                        if !self.is_square_attacked(f8, Color::White)
                            && !self.is_square_attacked(g8, Color::White)
                        {
                            moves.push(Move {
                                from,
                                to: g8,
                                move_type: MoveType::Castle,
                                piece,
                                captured: None,
                            });
                        }
                    }
                }

                // Black queenside castling
                if self.castling_rights.black_queenside() {
                    let d8 = Square::from_coords(3, 7);
                    let c8 = Square::from_coords(2, 7);
                    let b8 = Square::from_coords(1, 7);

                    if !self.all_pieces.is_set(d8)
                        && !self.all_pieces.is_set(c8)
                        && !self.all_pieces.is_set(b8)
                    {
                        if !self.is_square_attacked(d8, Color::White)
                            && !self.is_square_attacked(c8, Color::White)
                        {
                            moves.push(Move {
                                from,
                                to: c8,
                                move_type: MoveType::Castle,
                                piece,
                                captured: None,
                            });
                        }
                    }
                }
            }
        }
    }

    fn generate_pawn_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.side_to_move;
        let piece = Piece {
            piece_type: PieceType::Pawn,
            color,
        };

        let rank = from.rank();
        let file = from.file();

        // Determine direction and special ranks based on color
        let (direction, start_rank, promotion_rank) = match color {
            Color::White => (1i8, 1u8, 7u8),
            Color::Black => (-1i8, 6u8, 1u8),
        };

        // Pawn captures (including normal captures and en passant)
        let attacks = ATTACK_TABLE.pawn_attacks(from, color);
        let enemy = self.color_bitboard[color.opposite() as usize];
        let capture_targets = attacks & enemy;

        for to in capture_targets {
            let captured = self.piece_at(to);

            // Check if this is a promotion capture
            if to.rank() == promotion_rank {
                // Generate all 4 promotion moves
                for promo_piece in [
                    PieceType::Queen,
                    PieceType::Rook,
                    PieceType::Bishop,
                    PieceType::Knight,
                ] {
                    moves.push(Move {
                        from,
                        to,
                        move_type: MoveType::Promotion(promo_piece),
                        piece,
                        captured,
                    });
                }
            } else {
                moves.push(Move {
                    from,
                    to,
                    move_type: MoveType::Capture,
                    piece,
                    captured,
                });
            }
        }

        // En passant capture
        if let Some(ep_square) = self.en_passant_square {
            if attacks.is_set(ep_square) {
                let captured_pawn = Piece {
                    piece_type: PieceType::Pawn,
                    color: color.opposite(),
                };
                moves.push(Move {
                    from,
                    to: ep_square,
                    move_type: MoveType::EnPassant,
                    piece,
                    captured: Some(captured_pawn),
                });
            }
        }

        // Single pawn push
        let new_rank = (rank as i8 + direction) as u8;
        if new_rank < 8 {
            let to = Square::from_coords(file, new_rank);

            if !self.all_pieces.is_set(to) {
                // Check if this is a promotion
                if new_rank == promotion_rank {
                    // Generate all 4 promotion moves
                    for promo_piece in [
                        PieceType::Queen,
                        PieceType::Rook,
                        PieceType::Bishop,
                        PieceType::Knight,
                    ] {
                        moves.push(Move {
                            from,
                            to,
                            move_type: MoveType::Promotion(promo_piece),
                            piece,
                            captured: None,
                        });
                    }
                } else {
                    moves.push(Move {
                        from,
                        to,
                        move_type: MoveType::Normal,
                        piece,
                        captured: None,
                    });
                }
            }
        }

        // Double pawn push from starting position
        if rank == start_rank {
            let single_push_rank = (rank as i8 + direction) as u8;
            let double_push_rank = (rank as i8 + 2 * direction) as u8;

            let single_push_sq = Square::from_coords(file, single_push_rank);
            let double_push_sq = Square::from_coords(file, double_push_rank);

            // Both squares must be empty
            if !self.all_pieces.is_set(single_push_sq) && !self.all_pieces.is_set(double_push_sq) {
                moves.push(Move {
                    from,
                    to: double_push_sq,
                    move_type: MoveType::Normal,
                    piece,
                    captured: None,
                });
            }
        }
    }

    /// Check if a square is attacked by a given color
    pub fn is_square_attacked(&self, square: Square, by_color: Color) -> bool {
        // Check for pawn attacks
        let pawn_attacks = ATTACK_TABLE.pawn_attacks(square, by_color.opposite());
        let enemy_pawns = self.piece_bitboards[by_color as usize][PieceType::Pawn as usize];
        if !(pawn_attacks & enemy_pawns).is_empty() {
            return true;
        }

        // Check for knight attacks
        let knight_attacks = ATTACK_TABLE.knight_attacks(square);
        let enemy_knights = self.piece_bitboards[by_color as usize][PieceType::Knight as usize];
        if !(knight_attacks & enemy_knights).is_empty() {
            return true;
        }

        // Check for bishop/queen attacks (diagonal)
        let bishop_attacks = ATTACK_TABLE.bishop_attacks(square, self.all_pieces);
        let enemy_bishops = self.piece_bitboards[by_color as usize][PieceType::Bishop as usize];
        let enemy_queens = self.piece_bitboards[by_color as usize][PieceType::Queen as usize];
        if !(bishop_attacks & (enemy_bishops | enemy_queens)).is_empty() {
            return true;
        }

        // Check for rook/queen attacks (straight)
        let rook_attacks = ATTACK_TABLE.rook_attacks(square, self.all_pieces);
        let enemy_rooks = self.piece_bitboards[by_color as usize][PieceType::Rook as usize];
        if !(rook_attacks & (enemy_rooks | enemy_queens)).is_empty() {
            return true;
        }

        // Check for king attacks
        let king_attacks = ATTACK_TABLE.king_attacks(square);
        let enemy_king = self.piece_bitboards[by_color as usize][PieceType::King as usize];
        if !(king_attacks & enemy_king).is_empty() {
            return true;
        }

        false
    }

    /// Check if a pseudo-legal move is actually legal (doesn't leave king in check)
    pub fn is_legal_move(&self, mv: Move) -> bool {
        // Make a copy of the board and apply the move
        let mut board_copy = self.clone();
        board_copy.make_move_unchecked(mv);

        // Find our king
        let our_color = self.side_to_move;
        let king_bb = board_copy.piece_bitboards[our_color as usize][PieceType::King as usize];

        // King should exist
        if king_bb.is_empty() {
            return false;
        }

        let king_square = king_bb.into_iter().next().unwrap();

        // Check if king is in check
        !board_copy.is_square_attacked(king_square, our_color.opposite())
    }

    /// Make a move without checking legality (used internally for testing)
    fn make_move_unchecked(&mut self, mv: Move) {
        let color = mv.piece.color;
        let piece_type = mv.piece.piece_type;

        // Remove piece from origin square
        self.piece_bitboards[color as usize][piece_type as usize].clear(mv.from);
        self.color_bitboard[color as usize].clear(mv.from);
        self.all_pieces.clear(mv.from);
        self.mailbox[mv.from.index()] = None;

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

            // Remove captured piece
            self.piece_bitboards[captured.color as usize][captured.piece_type as usize]
                .clear(capture_square);
            self.color_bitboard[captured.color as usize].clear(capture_square);
            self.all_pieces.clear(capture_square);
            self.mailbox[capture_square.index()] = None;
        }

        // Place piece on destination square
        let final_piece_type = match mv.move_type {
            MoveType::Promotion(promo_type) => promo_type,
            _ => piece_type,
        };

        self.piece_bitboards[color as usize][final_piece_type as usize].set(mv.to);
        self.color_bitboard[color as usize].set(mv.to);
        self.all_pieces.set(mv.to);
        self.mailbox[mv.to.index()] = Some(Piece {
            piece_type: final_piece_type,
            color,
        });

        // Handle castling - move the rook
        if mv.move_type == MoveType::Castle {
            let (rook_from, rook_to) = match (color, mv.to.file()) {
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
            };

            // Move rook
            self.piece_bitboards[color as usize][PieceType::Rook as usize].clear(rook_from);
            self.color_bitboard[color as usize].clear(rook_from);
            self.all_pieces.clear(rook_from);
            self.mailbox[rook_from.index()] = None;

            self.piece_bitboards[color as usize][PieceType::Rook as usize].set(rook_to);
            self.color_bitboard[color as usize].set(rook_to);
            self.all_pieces.set(rook_to);
            self.mailbox[rook_to.index()] = Some(Piece {
                piece_type: PieceType::Rook,
                color,
            });
        }
    }
}
