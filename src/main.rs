use lemonate::board::{Board, Move, MoveType};
use lemonate::book::{BookManager, BookMoveSelection};
use lemonate::eval::Evaluator;
use lemonate::search::{is_mate_score, mate_in, SearchEngine, SearchLimits};
use lemonate::types::{Color, PieceType, Square};
use std::io::{self, Write};

/// Default search depth for the engine.
const DEFAULT_DEPTH: u8 = 6;

/// Path to the opening book.
const BOOK_PATH: &str = "bins/Perfect2021.bin";

fn main() {
    println!("=== Lemonate Chess Engine ===\n");

    // Start from the initial position
    let starting_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let mut board = Board::from_fen(starting_fen).expect("Failed to parse starting FEN");
    board.enable_history();

    let mut engine = SearchEngine::new();
    let mut search_depth = DEFAULT_DEPTH;

    // Load opening book
    let mut book = BookManager::new();
    book.selection = BookMoveSelection::WeightedRandom;
    match book.load(BOOK_PATH) {
        Ok(()) => println!("Loaded opening book ({} positions)", book.len()),
        Err(e) => println!("Warning: Could not load opening book: {}", e),
    }

    println!("Starting new game. You are White, engine is Black.");
    println!("Enter moves in algebraic notation (e.g., 'e2e4' or 'e7e8q' for promotion)");
    println!("Commands: 'quit', 'fen', 'moves', 'eval', 'depth <n>', 'undo', 'book'\n");

    'game: loop {
        // Display the board
        display_board(&board);

        // Check for checkmate or stalemate
        let legal_moves = board.generate_legal_moves();
        if legal_moves.is_empty() {
            if board.is_in_check() {
                println!(
                    "\nCheckmate! {} wins!",
                    if board.side_to_move() == Color::White {
                        "Black"
                    } else {
                        "White"
                    }
                );
            } else {
                println!("\nStalemate! Draw.");
            }
            break;
        }

        // Check for draws
        if board.is_draw_by_fifty_moves() {
            println!("\nDraw by fifty-move rule!");
            break;
        }

        if board.is_insufficient_material() {
            println!("\nDraw by insufficient material!");
            break;
        }

        // Show check status
        if board.is_in_check() {
            println!("\n** CHECK **");
        }

        // Check whose turn it is
        if board.side_to_move() == Color::White {
            // Player's turn
            println!("\nYour move (White): ");

            let user_move = loop {
                print!("> ");
                io::stdout().flush().unwrap();

                let mut input = String::new();
                let bytes_read = io::stdin().read_line(&mut input).unwrap();

                // Handle EOF (e.g., piped input ended)
                if bytes_read == 0 {
                    println!("\nEnd of input. Goodbye!");
                    return;
                }

                let input = input.trim().to_lowercase();

                if input == "quit" {
                    println!("Thanks for playing!");
                    return;
                }

                if input == "fen" {
                    println!("Position hash: {:#x}", board.position_hash());
                    continue;
                }

                if input == "moves" {
                    println!("\nLegal moves ({}):", legal_moves.len());
                    for (i, mv) in legal_moves.iter().enumerate() {
                        print!("{:<7}", move_to_string(mv));
                        if (i + 1) % 8 == 0 {
                            println!();
                        }
                    }
                    println!();
                    continue;
                }

                if input == "eval" {
                    print_evaluation(&board);
                    continue;
                }

                if input == "book" {
                    let book_moves = book.probe_all(&board);
                    if book_moves.is_empty() {
                        println!("No book moves for this position.");
                    } else {
                        println!("\nBook moves ({}):", book_moves.len());
                        for (mv, weight) in &book_moves {
                            println!("  {} (weight: {})", move_to_string(mv), weight);
                        }
                    }
                    continue;
                }

                if input.starts_with("depth ") {
                    if let Ok(d) = input[6..].trim().parse::<u8>() {
                        if d >= 1 && d <= 20 {
                            search_depth = d;
                            println!("Search depth set to {}", search_depth);
                        } else {
                            println!("Depth must be between 1 and 20");
                        }
                    } else {
                        println!("Invalid depth. Usage: depth <n>");
                    }
                    continue;
                }

                if input == "undo" {
                    // Undo both player and engine move
                    if board.unmake_move() {
                        board.unmake_move(); // Try to undo engine's move too
                        println!("Move undone.");
                        continue 'game; // Re-display board
                    } else {
                        println!("Nothing to undo.");
                    }
                    continue;
                }

                // Try to parse the move
                match parse_move(&input, &legal_moves) {
                    Some(mv) => break mv,
                    None => {
                        println!("Invalid move '{}'. Format: <from><to>[promotion]", input);
                        println!("  Examples: e2e4, g1f3, e7e8q (pawn promotion to queen)");
                        println!("  Castling: o-o (kingside), o-o-o (queenside)");
                        println!("  Type 'moves' to see all legal moves, or 'quit' to exit.");
                        continue;
                    }
                }
            };

            // Make the player's move
            board.make_move(user_move);
            println!("You played: {}", move_to_string(&user_move));
        } else {
            // Engine's turn
            // First, check the opening book
            if let Some(book_move) = book.probe(&board) {
                board.make_move(book_move);
                println!("\nEngine played: {} (book)", move_to_string(&book_move));
                print_material_count(&board);
            } else {
                // No book move, search normally
                println!("\nEngine thinking (depth {})...", search_depth);

                let start = std::time::Instant::now();
                let result = engine.search(&board, SearchLimits::depth(search_depth));
                let elapsed = start.elapsed();

                if let Some(engine_move) = result.best_move {
                    board.make_move(engine_move);

                    // Format the score
                    let score_str = if is_mate_score(result.score) {
                        if let Some(moves) = mate_in(result.score) {
                            if result.score > 0 {
                                format!("M{}", moves)
                            } else {
                                format!("-M{}", moves)
                            }
                        } else {
                            format!("{:+}", result.score)
                        }
                    } else {
                        format!("{:+.2}", result.score as f64 / 100.0)
                    };

                    println!("Engine played: {}", move_to_string(&engine_move));

                    // Show search info
                    println!("\n--- Search Info ---");
                    println!("  Score:      {} pawns", score_str);
                    println!("  Depth:      {}/{}", result.depth, result.stats.seldepth);
                    println!("  Nodes:      {}", format_nodes(result.stats.nodes));
                    println!(
                        "  Time:       {:.2}s",
                        elapsed.as_secs_f64()
                    );
                    println!(
                        "  NPS:        {}",
                        format_nodes(result.stats.nps(elapsed.as_millis()))
                    );

                    // Show principal variation
                    if !result.pv.is_empty() {
                        print!("  PV:         ");
                        for (i, mv) in result.pv.iter().take(6).enumerate() {
                            if i > 0 {
                                print!(" ");
                            }
                            print!("{}", move_to_string(mv));
                        }
                        if result.pv.len() > 6 {
                            print!(" ...");
                        }
                        println!();
                    }

                    print_material_count(&board);
                } else {
                    println!("Engine has no legal moves!");
                }
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
            let piece_char = match board.piece_at(square) {
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
        return legal_moves
            .iter()
            .find(|m| m.move_type == MoveType::Castle && m.to.file() == 6)
            .copied();
    }

    if input == "o-o-o" || input == "0-0-0" {
        return legal_moves
            .iter()
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
    legal_moves
        .iter()
        .find(|m| {
            m.from == from
                && m.to == to
                && match (&m.move_type, promotion) {
                    (MoveType::Promotion(p1), Some(p2)) => p1 == &p2,
                    (MoveType::Promotion(_), None) => false,
                    (_, Some(_)) => false,
                    _ => true,
                }
        })
        .copied()
}

fn print_evaluation(board: &Board) {
    let evaluator = Evaluator::new();
    let details = evaluator.evaluate_detailed(board);

    println!("\n--- Position Evaluation ---");
    println!("  PST:          {:+.2} pawns", details.pst as f64 / 100.0);
    println!(
        "  Pawn struct:  {:+.2} pawns",
        details.pawn_structure as f64 / 100.0
    );
    println!(
        "  King safety:  {:+.2} pawns",
        details.king_safety as f64 / 100.0
    );
    println!(
        "  Mobility:     {:+.2} pawns",
        details.mobility as f64 / 100.0
    );
    println!("  -------------------------");
    println!("  Total:        {:+.2} pawns", details.total() as f64 / 100.0);
    println!(
        "  Phase:        {}/24 (0=endgame, 24=opening)",
        details.phase
    );
}

fn print_material_count(board: &Board) {
    let pieces = [
        (PieceType::Queen, "Q"),
        (PieceType::Rook, "R"),
        (PieceType::Bishop, "B"),
        (PieceType::Knight, "N"),
        (PieceType::Pawn, "P"),
    ];

    print!("\n--- Material ---\n  White: ");
    for (pt, name) in &pieces {
        let count = board.piece_bitboard(Color::White, *pt).count_pieces();
        if count > 0 {
            print!("{}{} ", count, name);
        }
    }

    print!("\n  Black: ");
    for (pt, name) in &pieces {
        let count = board.piece_bitboard(Color::Black, *pt).count_pieces();
        if count > 0 {
            print!("{}{} ", count, name);
        }
    }
    println!();
}

fn format_nodes(nodes: u64) -> String {
    if nodes >= 1_000_000 {
        format!("{:.2}M", nodes as f64 / 1_000_000.0)
    } else if nodes >= 1_000 {
        format!("{:.1}K", nodes as f64 / 1_000.0)
    } else {
        format!("{}", nodes)
    }
}
