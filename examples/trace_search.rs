// Trace the search to find the queen blunder issue
use lemonate::board::Board;
use lemonate::search::{SearchEngine, SearchLimits};

fn main() {
    // Position after 1.e4 e6 2.d4 (black to move)
    let fen = "rnbqkbnr/pppp1ppp/4p3/8/3PP3/8/PPP2PPP/RNBQKBNR b KQkq - 0 2";
    let mut board = Board::from_fen(fen).expect("Failed to parse FEN");
    board.enable_history();

    println!("Testing position after 1.e4 e6 2.d4 (Black to move)");
    println!("Searching at depth 8...\n");
    
    let mut engine = SearchEngine::new();
    let result = engine.search(&board, SearchLimits::depth(8));
    
    if let Some(mv) = result.best_move {
        let from = format!("{}{}", (b'a' + mv.from.file()) as char, (b'1' + mv.from.rank()) as char);
        let to = format!("{}{}", (b'a' + mv.to.file()) as char, (b'1' + mv.to.rank()) as char);
        println!("Best move: {}{}", from, to);
        println!("Score: {}", result.score);
        println!("PV: {:?}", result.pv.iter().map(|m| {
            format!("{}{}{}{}", 
                (b'a' + m.from.file()) as char, (b'1' + m.from.rank()) as char,
                (b'a' + m.to.file()) as char, (b'1' + m.to.rank()) as char)
        }).collect::<Vec<_>>());
    }
    
    // Now manually search after Qg5 to see White's best response
    println!("\n--- After Black plays Qg5 ---");
    
    // Find Qg5 move
    let moves = board.generate_legal_moves();
    let qg5 = moves.iter().find(|m| {
        m.from.file() == 3 && m.from.rank() == 7  // d8
        && m.to.file() == 6 && m.to.rank() == 4   // g5
    }).copied();
    
    if let Some(qg5_move) = qg5 {
        board.make_move(qg5_move);
        
        let mut engine2 = SearchEngine::new();
        let white_result = engine2.search(&board, SearchLimits::depth(7));
        
        if let Some(mv) = white_result.best_move {
            let from = format!("{}{}", (b'a' + mv.from.file()) as char, (b'1' + mv.from.rank()) as char);
            let to = format!("{}{}", (b'a' + mv.to.file()) as char, (b'1' + mv.to.rank()) as char);
            println!("White's best response: {}{}", from, to);
            println!("Score for White: {}", white_result.score);
            println!("PV: {:?}", white_result.pv.iter().map(|m| {
                format!("{}{}{}{}", 
                    (b'a' + m.from.file()) as char, (b'1' + m.from.rank()) as char,
                    (b'a' + m.to.file()) as char, (b'1' + m.to.rank()) as char)
            }).collect::<Vec<_>>());
            
            // What is Bxg5?
            let white_moves = board.generate_legal_moves();
            let bxg5 = white_moves.iter().find(|m| {
                m.from.file() == 2 && m.from.rank() == 0  // c1
                && m.to.file() == 6 && m.to.rank() == 4   // g5
            });
            
            if bxg5.is_some() {
                println!("\nBxg5 IS in White's legal moves");
            } else {
                println!("\nBxg5 is NOT in White's legal moves - BUG!");
            }
        }
    }
}
