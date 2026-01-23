use lemonate::board::Board;
use lemonate::search::{SearchEngine, SearchLimits};

fn main() {
    // Position after 1.e4 e6 2.d4 Qg5?? (White to move)
    // This is the position where White should obviously play Bxg5
    let fen = "rnb1kbnr/pppp1ppp/4p3/6q1/3PP3/8/PPP2PPP/RNBQKBNR w KQkq - 1 3";
    let board = Board::from_fen(fen).expect("Failed to parse FEN");
    
    println!("Position after 1.e4 e6 2.d4 Qg5??");
    println!("FEN: {}", fen);
    println!("White to move - should find Bxg5 (winning the queen)\n");
    
    // List all legal moves with their captures
    let moves = board.generate_legal_moves();
    println!("Legal moves for White ({} total):", moves.len());
    for mv in &moves {
        let from = format!("{}{}", (b'a' + mv.from.file()) as char, (b'1' + mv.from.rank()) as char);
        let to = format!("{}{}", (b'a' + mv.to.file()) as char, (b'1' + mv.to.rank()) as char);
        let capture_str = if let Some(cap) = mv.captured {
            format!(" x {:?}", cap.piece_type)
        } else {
            String::new()
        };
        println!("  {}{}{}", from, to, capture_str);
    }
    
    // Check specifically for Bxg5
    println!("\nLooking for Bxg5 (c1g5)...");
    let bxg5 = moves.iter().find(|m| {
        m.from.file() == 2 && m.from.rank() == 0  // c1
        && m.to.file() == 6 && m.to.rank() == 4   // g5
    });
    
    if let Some(mv) = bxg5 {
        println!("  Found: c1g5, captures {:?}", mv.captured.map(|c| c.piece_type));
    } else {
        println!("  NOT FOUND!");
    }
    
    // Search at different depths
    for depth in 1..=6 {
        println!("\n--- Depth {} search ---", depth);
        let mut engine = SearchEngine::new();
        let result = engine.search(&board, SearchLimits::depth(depth));
        
        if let Some(best) = result.best_move {
            let from = format!("{}{}", (b'a' + best.from.file()) as char, (b'1' + best.from.rank()) as char);
            let to = format!("{}{}", (b'a' + best.to.file()) as char, (b'1' + best.to.rank()) as char);
            let capture_str = if let Some(cap) = best.captured {
                format!(" x {:?}", cap.piece_type)
            } else {
                String::new()
            };
            println!("  Best: {}{}{}, score: {}", from, to, capture_str, result.score);
            print!("  PV: ");
            for mv in &result.pv {
                let f = format!("{}{}", (b'a' + mv.from.file()) as char, (b'1' + mv.from.rank()) as char);
                let t = format!("{}{}", (b'a' + mv.to.file()) as char, (b'1' + mv.to.rank()) as char);
                print!("{}{} ", f, t);
            }
            println!();
        }
    }
}
