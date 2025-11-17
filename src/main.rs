use lemonate::board::{Board, MoveType};
use lemonate::types::{PieceType, Square};

fn main() {
    println!("=== Board Tests ===\n");

    // Test 1: Create board from starting position FEN
    println!("\n=== Test 1: FEN Parsing - Starting Position ===");
    let starting_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    match Board::from_fen(starting_fen) {
        Ok(board) => {
            println!("Successfully parsed starting position FEN");
            println!("All pieces bitboard:");
            println!("{}", board.all_pieces());

            // Check some key squares
            let e2 = Square::from_coords(4, 1);
            let e7 = Square::from_coords(4, 6);
            println!("Piece at e2: {:?}", board.peice_at(e2));
            println!("Piece at e7: {:?}", board.peice_at(e7));
        }
        Err(e) => println!("Failed to parse FEN: {:?}", e),
    }

    // Test 2: Custom position with knights
    println!("\n=== Test 2: FEN Parsing - Custom Position ===");
    let custom_fen = "rnbqkb1r/pppppppp/5n2/8/8/5N2/PPPPPPPP/RNBQKB1R w KQkq - 0 1";
    match Board::from_fen(custom_fen) {
        Ok(board) => {
            println!("Successfully parsed custom position");
            let f3 = Square::from_coords(5, 2);
            let f6 = Square::from_coords(5, 5);
            println!("Piece at f3: {:?}", board.peice_at(f3));
            println!("Piece at f6: {:?}", board.peice_at(f6));
        }
        Err(e) => println!("Failed to parse custom FEN: {:?}", e),
    }

    // Test 3: Move generation from starting position
    println!("\n=== Test 3: Move Generation - Starting Position ===");
    if let Ok(board) = Board::from_fen(starting_fen) {
        let pseudo_legal = board.generate_pseudo_legal_moves();
        let legal_moves = board.generate_legal_moves();

        println!("Pseudo-legal moves: {}", pseudo_legal.len());
        println!("Legal moves: {}", legal_moves.len());

        // Count moves by piece type
        let pawn_moves = legal_moves.iter().filter(|m| m.piece.piece_type == PieceType::Pawn).count();
        let knight_moves = legal_moves.iter().filter(|m| m.piece.piece_type == PieceType::Knight).count();

        println!("Pawn moves: {}", pawn_moves);
        println!("Knight moves: {}", knight_moves);

        // Show first few moves
        println!("\nFirst 10 legal moves:");
        for (i, mv) in legal_moves.iter().take(10).enumerate() {
            println!("  {}. {:?} {} -> {}",
                i + 1,
                mv.piece.piece_type,
                square_to_algebraic(mv.from),
                square_to_algebraic(mv.to)
            );
        }
    }

    // Test 4: Knight moves from center
    println!("\n=== Test 4: Knight Move Generation ===");
    let knight_fen = "4k3/8/8/3N4/8/8/8/4K3 w - - 0 1";
    if let Ok(board) = Board::from_fen(knight_fen) {
        let moves = board.generate_legal_moves();
        println!("Knight on d5 can move to {} squares", moves.len());
        println!("Knight moves:");
        for mv in moves.iter() {
            println!("  {} -> {}", square_to_algebraic(mv.from), square_to_algebraic(mv.to));
        }
    }

    // Test 5: Bishop sliding moves
    println!("\n=== Test 5: Bishop Move Generation ===");
    let bishop_fen = "4k3/8/8/3B4/8/8/8/4K3 w - - 0 1";
    if let Ok(board) = Board::from_fen(bishop_fen) {
        let moves = board.generate_legal_moves();
        println!("Bishop on d5 can move to {} squares", moves.len());
        println!("First 10 bishop moves:");
        for (i, mv) in moves.iter().take(10).enumerate() {
            println!("  {}. {} -> {}", i + 1, square_to_algebraic(mv.from), square_to_algebraic(mv.to));
        }
    }

    // Test 6: Rook sliding moves
    println!("\n=== Test 6: Rook Move Generation ===");
    let rook_fen = "4k3/8/8/3R4/8/8/8/4K3 w - - 0 1";
    if let Ok(board) = Board::from_fen(rook_fen) {
        let moves = board.generate_legal_moves();
        println!("Rook on d5 can move to {} squares", moves.len());
    }

    // Test 7: Queen moves
    println!("\n=== Test 7: Queen Move Generation ===");
    let queen_fen = "4k3/8/8/3Q4/8/8/8/4K3 w - - 0 1";
    if let Ok(board) = Board::from_fen(queen_fen) {
        let moves = board.generate_legal_moves();
        println!("Queen on d5 can move to {} squares", moves.len());
    }

    // Test 8: Pawn moves and double push
    println!("\n=== Test 8: Pawn Move Generation ===");
    let pawn_fen = "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1";
    if let Ok(board) = Board::from_fen(pawn_fen) {
        let moves = board.generate_legal_moves();
        println!("White pawn on e2 can make {} moves", moves.len());
        for mv in moves.iter() {
            let move_desc = match mv.move_type {
                MoveType::Normal => {
                    if mv.to.rank() as i8 - mv.from.rank() as i8 == 2 {
                        "double push"
                    } else {
                        "single push"
                    }
                }
                _ => "other",
            };
            println!("  {} -> {} ({})", square_to_algebraic(mv.from), square_to_algebraic(mv.to), move_desc);
        }
    }

    // Test 9: Pawn captures
    println!("\n=== Test 9: Pawn Capture Generation ===");
    let pawn_capture_fen = "4k3/8/8/3p1p2/4P3/8/8/4K3 w - - 0 1";
    if let Ok(board) = Board::from_fen(pawn_capture_fen) {
        let moves = board.generate_legal_moves();
        println!("White pawn on e4 with black pawns on d5 and f5:");
        for mv in moves.iter() {
            let move_type = match mv.move_type {
                MoveType::Capture => "capture",
                MoveType::Normal => "push",
                _ => "other",
            };
            println!("  {} -> {} ({})", square_to_algebraic(mv.from), square_to_algebraic(mv.to), move_type);
        }
    }

    // Test 10: Castling rights
    println!("\n=== Test 10: Castling ===");
    let castling_fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
    if let Ok(board) = Board::from_fen(castling_fen) {
        let moves = board.generate_legal_moves();
        let castling_moves: Vec<_> = moves.iter()
            .filter(|m| m.move_type == MoveType::Castle)
            .collect();
        println!("Available castling moves: {}", castling_moves.len());
        for mv in castling_moves {
            println!("  {} -> {}", square_to_algebraic(mv.from), square_to_algebraic(mv.to));
        }
    }

    // Test 11: Promotion
    println!("\n=== Test 11: Pawn Promotion ===");
    let promotion_fen = "4k3/4P3/8/8/8/8/8/4K3 w - - 0 1";
    if let Ok(board) = Board::from_fen(promotion_fen) {
        let moves = board.generate_legal_moves();
        println!("White pawn on e7 (ready to promote): {} moves", moves.len());
        for mv in moves.iter() {
            if let MoveType::Promotion(piece_type) = mv.move_type {
                println!("  {} -> {} promotes to {:?}",
                    square_to_algebraic(mv.from),
                    square_to_algebraic(mv.to),
                    piece_type
                );
            }
        }
    }

    // Test 12: En passant
    println!("\n=== Test 12: En Passant ===");
    let en_passant_fen = "4k3/8/8/3Pp3/8/8/8/4K3 w - e6 0 1";
    if let Ok(board) = Board::from_fen(en_passant_fen) {
        let moves = board.generate_legal_moves();
        let ep_moves: Vec<_> = moves.iter()
            .filter(|m| m.move_type == MoveType::EnPassant)
            .collect();
        println!("En passant captures available: {}", ep_moves.len());
        for mv in ep_moves {
            println!("  {} -> {} (en passant)", square_to_algebraic(mv.from), square_to_algebraic(mv.to));
        }
    }

    // Test 13: Complex position (Italian Game)
    println!("\n=== Test 13: Complex Position - Italian Game ===");
    let italian_fen = "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 0 1";
    if let Ok(board) = Board::from_fen(italian_fen) {
        let moves = board.generate_legal_moves();
        println!("Legal moves in Italian Game position: {}", moves.len());

        // Group by piece type
        for piece_type in [PieceType::Pawn, PieceType::Knight, PieceType::Bishop,
                          PieceType::Rook, PieceType::Queen, PieceType::King] {
            let count = moves.iter()
                .filter(|m| m.piece.piece_type == piece_type)
                .count();
            if count > 0 {
                println!("  {:?} moves: {}", piece_type, count);
            }
        }
    }

    // Test 14: Performance test
    println!("\n=== Test 14: Performance Test ===");
    if let Ok(board) = Board::from_fen(starting_fen) {
        let start = std::time::Instant::now();
        let iterations = 1000;

        for _ in 0..iterations {
            let _ = board.generate_legal_moves();
        }

        let duration = start.elapsed();
        println!("Generated legal moves {} times in {:?}", iterations, duration);
        println!("Average time per generation: {:?}", duration / iterations);
    }

    println!("\n=== Board Tests Completed ===");
}

fn square_to_algebraic(square: Square) -> String {
    let file = (b'a' + square.file()) as char;
    let rank = (b'1' + square.rank()) as char;
    format!("{}{}", file, rank)
}
