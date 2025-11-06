use lemonate::*;

fn main() {
    println!("Testing move generation...\n");

    // Test starting position
    let board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
        .expect("Failed to parse FEN");

    let moves = board.generate_legal_moves();
    println!("Starting position: {} legal moves", moves.len());
    println!("Expected: 20 (16 pawn moves + 4 knight moves)");

    if moves.len() == 20 {
        println!("✓ PASS");
    } else {
        println!("✗ FAIL");
    }
}
