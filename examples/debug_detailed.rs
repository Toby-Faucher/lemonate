// Detailed debug of the queen blunder
use lemonate::board::Board;
use lemonate::search::{SearchEngine, SearchLimits};

fn main() {
    // Position after 1.e4 e6 2.d4 (black to move)
    let fen = "rnbqkbnr/pppp1ppp/4p3/8/3PP3/8/PPP2PPP/RNBQKBNR b KQkq - 0 2";
    let mut board = Board::from_fen(fen).expect("Failed to parse FEN");
    board.enable_history();

    println!("Testing position after 1.e4 e6 2.d4 (Black to move)");
    println!("{}", "=".repeat(60));
    
    // Test at depth 8 with fresh engine
    let mut engine = SearchEngine::new();
    
    // First, let's search at depth 7 where it finds Qh4
    println!("\nDepth 7 search:");
    let result7 = engine.search(&board, SearchLimits::depth(7));
    if let Some(mv) = result7.best_move {
        let from = format!("{}{}", (b'a' + mv.from.file()) as char, (b'1' + mv.from.rank()) as char);
        let to = format!("{}{}", (b'a' + mv.to.file()) as char, (b'1' + mv.to.rank()) as char);
        println!("  Best: {}{}, Score: {}", from, to, result7.score);
        println!("  PV: {:?}", result7.pv.iter().map(|m| {
            format!("{}{}{}{}", 
                (b'a' + m.from.file()) as char, (b'1' + m.from.rank()) as char,
                (b'a' + m.to.file()) as char, (b'1' + m.to.rank()) as char)
        }).collect::<Vec<_>>());
    }
    
    // Now with a FRESH engine, search at depth 8
    println!("\nDepth 8 search (fresh engine):");
    let mut engine2 = SearchEngine::new();
    let result8 = engine2.search(&board, SearchLimits::depth(8));
    if let Some(mv) = result8.best_move {
        let from = format!("{}{}", (b'a' + mv.from.file()) as char, (b'1' + mv.from.rank()) as char);
        let to = format!("{}{}", (b'a' + mv.to.file()) as char, (b'1' + mv.to.rank()) as char);
        println!("  Best: {}{}, Score: {}", from, to, result8.score);
        println!("  PV: {:?}", result8.pv.iter().map(|m| {
            format!("{}{}{}{}", 
                (b'a' + m.from.file()) as char, (b'1' + m.from.rank()) as char,
                (b'a' + m.to.file()) as char, (b'1' + m.to.rank()) as char)
        }).collect::<Vec<_>>());
        println!("  Stats: nodes={}, tt_hits={}, tt_cutoffs={}", 
            result8.stats.nodes, result8.stats.tt_hits, result8.stats.tt_cutoffs);
    }
    
    // Now let's manually play Qg5 and see what the engine thinks White should do
    println!("\n{}", "=".repeat(60));
    println!("After Black plays Qg5 (the blunder):");
    
    // Find and play Qg5
    let moves = board.generate_legal_moves();
    let qg5 = moves.iter().find(|m| {
        m.from.file() == 3 && m.from.rank() == 7  // d8
        && m.to.file() == 6 && m.to.rank() == 4   // g5
    }).copied();
    
    if let Some(qg5_move) = qg5 {
        board.make_move(qg5_move);
        
        // Now search for White's response
        let mut engine3 = SearchEngine::new();
        let white_result = engine3.search(&board, SearchLimits::depth(6));
        
        if let Some(mv) = white_result.best_move {
            let from = format!("{}{}", (b'a' + mv.from.file()) as char, (b'1' + mv.from.rank()) as char);
            let to = format!("{}{}", (b'a' + mv.to.file()) as char, (b'1' + mv.to.rank()) as char);
            println!("  White's best response: {}{}, Score: {} (for White)", from, to, white_result.score);
            
            if mv.captured.is_some() {
                println!("  This captures the queen!");
            }
        }
        
        // This means Black's score after Qg5 should be approximately -white_result.score
        println!("  => Black's actual score after Qg5 should be approximately: {}", -white_result.score);
        println!("  => But the engine reported Qg5 with score: {}", result8.score);
    }
}
