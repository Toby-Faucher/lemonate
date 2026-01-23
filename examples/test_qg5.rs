use lemonate::board::Board;
use lemonate::search::{SearchEngine, SearchLimits};

fn main() {
    // Position after 1.e4 e6 2.d4 Qg5?? (White to move)
    // White should easily find Bxg5 winning the queen
    let fen = "rnb1kbnr/pppp1ppp/4p3/6q1/3PP3/8/PPP2PPP/RNBQKBNR w KQkq - 1 3";
    let board = Board::from_fen(fen).expect("Failed to parse FEN");
    
    println!("Position after 1.e4 e6 2.d4 Qg5??");
    println!("White to move - should find Bxg5 (bishop takes queen)\n");
    
    // List legal moves
    let moves = board.generate_legal_moves();
    println!("Legal moves for White ({} total):", moves.len());
    for mv in &moves {
        let from = format!("{}{}", (b'a' + mv.from.file()) as char, (b'1' + mv.from.rank()) as char);
        let to = format!("{}{}", (b'a' + mv.to.file()) as char, (b'1' + mv.to.rank()) as char);
        let capture = if mv.captured.is_some() { "x" } else { "" };
        print!("{}{}{} ", from, capture, to);
    }
    println!("\n");
    
    // Check if Bxg5 is in the move list
    let bxg5 = moves.iter().find(|m| {
        m.from.file() == 2 && m.from.rank() == 0  // c1
        && m.to.file() == 6 && m.to.rank() == 4   // g5
    });
    println!("Is Bxg5 (c1xg5) in legal moves? {}", bxg5.is_some());
    if let Some(mv) = bxg5 {
        println!("  Capture: {:?}", mv.captured);
    }
    
    // Search
    println!("\nSearching...");
    let mut engine = SearchEngine::new();
    let result = engine.search(&board, SearchLimits::depth(6));
    
    if let Some(best_move) = result.best_move {
        let from = format!("{}{}", (b'a' + best_move.from.file()) as char, (b'1' + best_move.from.rank()) as char);
        let to = format!("{}{}", (b'a' + best_move.to.file()) as char, (b'1' + best_move.to.rank()) as char);
        println!("Best move: {}{} (score: {})", from, to, result.score);
        if let Some(cap) = best_move.captured {
            println!("  Captures: {:?}", cap.piece_type);
        }
    }
}
