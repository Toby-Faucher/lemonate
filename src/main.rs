use lemonate::board::{Board, Move, MoveType};
use lemonate::types::{Color, PieceType, Square};
use std::io::{self, Write};

fn main() {
    println!("=== Lemonate Chess Engine ===\n");

    // Start from the initial position
    let starting_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let mut board = Board::from_fen(starting_fen).expect("Failed to parse starting FEN");

    println!("Starting new game. You are White, engine is Black.");
    println!("Enter moves in algebraic notation (e.g., 'e2e4' or 'e7e8q' for promotion)");
    println!("Type 'quit' to exit, 'fen' to see current FEN, 'moves' to see legal moves\n");

    loop {
        // Display the board
        display_board(&board);

        // Check for checkmate or stalemate
        let legal_moves = board.generate_legal_moves();
        if legal_moves.is_empty() {
            if is_in_check(&board) {
                println!("\nCheckmate! {} wins!",
                    if board.side_to_move() == Color::White { "Black" } else { "White" });
            } else {
                println!("\nStalemate! Draw.");
            }
            break;
        }

        // Check whose turn it is
        if board.side_to_move() == Color::White {
            // Player's turn
            println!("\nYour move (White): ");

            let user_move = loop {
                print!("> ");
                io::stdout().flush().unwrap();

                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                let input = input.trim().to_lowercase();

                if input == "quit" {
                    println!("Thanks for playing!");
                    return;
                }

                if input == "fen" {
                    println!("Current position hash: {:#x}", board.position_hash());
                    continue;
                }

                if input == "moves" {
                    println!("\nLegal moves ({}):", legal_moves.len());
                    for (i, mv) in legal_moves.iter().enumerate() {
                        print!("{}  ", move_to_string(mv));
                        if (i + 1) % 8 == 0 {
                            println!();
                        }
                    }
                    println!();
                    continue;
                }

                // Try to parse the move
                match parse_move(&input, &legal_moves) {
                    Some(mv) => break mv,
                    None => {
                        println!("Invalid move! Try again (e.g., 'e2e4' or type 'moves' to see legal moves)");
                        continue;
                    }
                }
            };

            // Make the player's move
            board.make_move(user_move);
            println!("You played: {}", move_to_string(&user_move));

        } else {
            // Engine's turn (random move)
            println!("\nEngine is thinking...");

            use rand::prelude::IndexedRandom;
            let mut rng = rand::rng();

            if let Some(engine_move) = legal_moves.choose(&mut rng) {
                board.make_move(*engine_move);
                println!("Engine played: {}", move_to_string(engine_move));
            }
        }

        println!();
    }
}

fn display_board(board: &Board) {
    println!("\n  +---+---+---+---+---+---+---+---+");

    for rank in (0..8).rev() {
        print!("{} |", rank + 1);

        for file in 0..8 {
            let square = Square::from_coords(file, rank);
            let piece_char = match board.peice_at(square) {
                Some(piece) => piece_to_char(piece),
                None => ' ',
            };
            print!(" {} |", piece_char);
        }

        println!("\n  +---+---+---+---+---+---+---+---+");
    }

    println!("    a   b   c   d   e   f   g   h");
}

fn piece_to_char(piece: lemonate::Piece) -> char {
    let base = match piece.piece_type {
        PieceType::Pawn => 'p',
        PieceType::Knight => 'n',
        PieceType::Bishop => 'b',
        PieceType::Rook => 'r',
        PieceType::Queen => 'q',
        PieceType::King => 'k',
    };

    if piece.color == Color::White {
        base.to_uppercase().next().unwrap()
    } else {
        base
    }
}

fn square_to_algebraic(square: Square) -> String {
    let file = (b'a' + square.file()) as char;
    let rank = (b'1' + square.rank()) as char;
    format!("{}{}", file, rank)
}

fn move_to_string(mv: &Move) -> String {
    let from = square_to_algebraic(mv.from);
    let to = square_to_algebraic(mv.to);

    match mv.move_type {
        MoveType::Promotion(piece_type) => {
            let promo = match piece_type {
                PieceType::Queen => 'q',
                PieceType::Rook => 'r',
                PieceType::Bishop => 'b',
                PieceType::Knight => 'n',
                _ => '?',
            };
            format!("{}{}{}", from, to, promo)
        }
        MoveType::Castle => {
            if to.starts_with('g') {
                "O-O".to_string()
            } else {
                "O-O-O".to_string()
            }
        }
        _ => format!("{}{}", from, to),
    }
}

fn parse_move(input: &str, legal_moves: &[Move]) -> Option<Move> {
    // Handle castling notation
    if input == "o-o" || input == "0-0" {
        return legal_moves.iter()
            .find(|m| m.move_type == MoveType::Castle && m.to.file() == 6)
            .copied();
    }

    if input == "o-o-o" || input == "0-0-0" {
        return legal_moves.iter()
            .find(|m| m.move_type == MoveType::Castle && m.to.file() == 2)
            .copied();
    }

    // Parse algebraic notation (e.g., "e2e4" or "e7e8q")
    if input.len() < 4 {
        return None;
    }

    let from_file = (input.chars().nth(0)? as u8).checked_sub(b'a')?;
    let from_rank = (input.chars().nth(1)? as u8).checked_sub(b'1')?;
    let to_file = (input.chars().nth(2)? as u8).checked_sub(b'a')?;
    let to_rank = (input.chars().nth(3)? as u8).checked_sub(b'1')?;

    if from_file > 7 || from_rank > 7 || to_file > 7 || to_rank > 7 {
        return None;
    }

    let from = Square::from_coords(from_file, from_rank);
    let to = Square::from_coords(to_file, to_rank);

    // Check for promotion
    let promotion = if input.len() >= 5 {
        match input.chars().nth(4)? {
            'q' => Some(PieceType::Queen),
            'r' => Some(PieceType::Rook),
            'b' => Some(PieceType::Bishop),
            'n' => Some(PieceType::Knight),
            _ => None,
        }
    } else {
        None
    };

    // Find matching legal move
    legal_moves.iter()
        .find(|m| {
            m.from == from && m.to == to &&
            match (&m.move_type, promotion) {
                (MoveType::Promotion(p1), Some(p2)) => p1 == &p2,
                (MoveType::Promotion(_), None) => false,
                (_, Some(_)) => false,
                _ => true,
            }
        })
        .copied()
}

fn is_in_check(board: &Board) -> bool {
    let color = board.side_to_move();
    let king_bb = board.piece_bitboard(color, PieceType::King);

    if king_bb.is_empty() {
        return false;
    }

    let king_square = king_bb.into_iter().next().unwrap();
    board.is_square_attacked(king_square, color.opposite())
}
