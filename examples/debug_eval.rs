use lemonate::board::Board;
use lemonate::eval::Evaluator;

fn main() {
    let evaluator = Evaluator::new();
    
    // Position after 1.e4 e6 2.d4 Qg5?? (White to move)
    let before_fen = "rnb1kbnr/pppp1ppp/4p3/6q1/3PP3/8/PPP2PPP/RNBQKBNR w KQkq - 1 3";
    let before = Board::from_fen(before_fen).expect("Failed to parse FEN");
    
    // Position after Bxg5 (Black to move)
    let after_fen = "rnb1kbnr/pppp1ppp/4p3/6B1/3PP3/8/PPP2PPP/RN1QKBNR b KQkq - 0 3";
    let after = Board::from_fen(after_fen).expect("Failed to parse FEN");
    
    println!("Before Bxg5 (White to move):");
    println!("  FEN: {}", before_fen);
    println!("  Eval (from White's perspective): {}", evaluator.evaluate(&before));
    
    println!("\nAfter Bxg5 (Black to move):");
    println!("  FEN: {}", after_fen);
    let eval_after = evaluator.evaluate(&after);
    println!("  Eval (from Black's perspective): {}", eval_after);
    println!("  Eval (from White's perspective): {}", -eval_after);
    
    // Also evaluate with f4 instead
    let f4_fen = "rnb1kbnr/pppp1ppp/4p3/6q1/3PPP2/8/PPP3PP/RNBQKBNR b KQkq f3 0 3";
    let f4_board = Board::from_fen(f4_fen).expect("Failed to parse FEN");
    let eval_f4 = evaluator.evaluate(&f4_board);
    println!("\nAfter f2f4 instead (Black to move):");
    println!("  FEN: {}", f4_fen);
    println!("  Eval (from Black's perspective): {}", eval_f4);
    println!("  Eval (from White's perspective): {}", -eval_f4);
    
    // Test detailed evaluation
    println!("\n=== Detailed evaluation after Bxg5 ===");
    let details = evaluator.evaluate_detailed(&after);
    println!("  PST:          {} cp", details.pst);
    println!("  Pawn struct:  {} cp", details.pawn_structure);
    println!("  King safety:  {} cp", details.king_safety);
    println!("  Mobility:     {} cp", details.mobility);
    println!("  Total:        {} cp", details.total());
    println!("  Phase:        {}/24", details.phase);
}
