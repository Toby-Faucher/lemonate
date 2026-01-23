use lemonate::board::Board;
use lemonate::search::{SearchEngine, SearchLimits};

fn main() {
    // Simulate: 1. e4 e6 2. d4
    // This is the position where the engine (Black) should NOT play Qg5
    
    let mut board = Board::starting_position();
    board.enable_history();
    
    let mut engine = SearchEngine::new();
    
    // Move 1: User plays e4
    let e4 = board.generate_legal_moves().into_iter()
        .find(|m| m.from.file() == 4 && m.from.rank() == 1 && m.to.file() == 4 && m.to.rank() == 3)
        .unwrap();
    board.make_move(e4);
    println!("User plays: e4");
    
    // Move 1 response: Engine searches (this populates TT)
    println!("Engine searching for response to e4 at depth 6...");
    let result1 = engine.search(&board, SearchLimits::depth(6));
    let engine_move1 = result1.best_move.unwrap();
    let from1 = format!("{}{}", (b'a' + engine_move1.from.file()) as char, (b'1' + engine_move1.from.rank()) as char);
    let to1 = format!("{}{}", (b'a' + engine_move1.to.file()) as char, (b'1' + engine_move1.to.rank()) as char);
    println!("Engine's response: {}{} (score: {})", from1, to1, result1.score);
    
    // Force e6 instead (to match the bug scenario)
    let e6 = board.generate_legal_moves().into_iter()
        .find(|m| m.from.file() == 4 && m.from.rank() == 6 && m.to.file() == 4 && m.to.rank() == 5)
        .unwrap();
    board.make_move(e6);
    println!("\n(Forcing Black to play e6 to reproduce bug scenario)");
    
    // Move 2: User plays d4
    let d4 = board.generate_legal_moves().into_iter()
        .find(|m| m.from.file() == 3 && m.from.rank() == 1 && m.to.file() == 3 && m.to.rank() == 3)
        .unwrap();
    board.make_move(d4);
    println!("User plays: d4");
    
    // Move 2 response: This is where the bug supposedly occurs
    println!("Engine searching for response to d4 at depth 6...");
    let result2 = engine.search(&board, SearchLimits::depth(6));
    let engine_move2 = result2.best_move.unwrap();
    let from2 = format!("{}{}", (b'a' + engine_move2.from.file()) as char, (b'1' + engine_move2.from.rank()) as char);
    let to2 = format!("{}{}", (b'a' + engine_move2.to.file()) as char, (b'1' + engine_move2.to.rank()) as char);
    println!("Engine's response: {}{} (score: {})", from2, to2, result2.score);
    
    // Check if it's the queen blunder Qg5
    let is_qg5 = engine_move2.from.file() == 3 && engine_move2.from.rank() == 7 
              && engine_move2.to.file() == 6 && engine_move2.to.rank() == 4;
    
    if is_qg5 {
        println!("\n!!! BUG REPRODUCED: Engine blundered with Qg5! !!!");
    } else {
        println!("\nEngine did NOT play Qg5 (good)");
    }
    
    // Print PV
    println!("\nPV: ");
    for mv in &result2.pv {
        let f = format!("{}{}", (b'a' + mv.from.file()) as char, (b'1' + mv.from.rank()) as char);
        let t = format!("{}{}", (b'a' + mv.to.file()) as char, (b'1' + mv.to.rank()) as char);
        print!("{}{} ", f, t);
    }
    println!();
}
