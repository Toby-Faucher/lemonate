// Debug the queen blunder at different depths
use lemonate::board::Board;
use lemonate::search::{SearchEngine, SearchLimits};

fn main() {
    // Position after 1.e4 e6 2.d4 (black to move)
    let fen = "rnbqkbnr/pppp1ppp/4p3/8/3PP3/8/PPP2PPP/RNBQKBNR b KQkq - 0 2";
    let mut board = Board::from_fen(fen).expect("Failed to parse FEN");
    board.enable_history();

    // Test at various depths with a FRESH engine each time
    for depth in 1..=8 {
        let mut engine = SearchEngine::new();
        let result = engine.search(&board, SearchLimits::depth(depth));

        if let Some(best_move) = result.best_move {
            let from = format!("{}{}", (b'a' + best_move.from.file()) as char, (b'1' + best_move.from.rank()) as char);
            let to = format!("{}{}", (b'a' + best_move.to.file()) as char, (b'1' + best_move.to.rank()) as char);
            let move_str = format!("{}{}", from, to);

            let is_qg5 = move_str == "d8g5";
            let marker = if is_qg5 { " <-- BLUNDER!" } else { "" };

            println!("Depth {}: {} (score: {}){}", depth, move_str, result.score, marker);
        }
    }
}
