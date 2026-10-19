//! Random-walk perft for differential testing against another engine.
//!
//! Reads FENs on stdin, plays a pseudo-random number of random legal moves from each, then
//! prints `FEN<TAB>moves<TAB>perft(depth)`, where `moves` is the walk in UCI notation. Feed the
//! output to `tests/differential/diff_perft.py`, which asks Stockfish for the same count.
//!
//!     cargo run --release --example walk_perft -- 4 < fens.txt | python3 tests/differential/diff_perft.py 4
use lemonate::board::MoveType;
use lemonate::{Board, PieceType, Square};
use std::io::BufRead;

fn square(s: Square) -> String {
    format!("{}{}", (b'a' + s.file()) as char, s.rank() + 1)
}

fn uci(m: &lemonate::board::Move) -> String {
    let mut out = format!("{}{}", square(m.from), square(m.to));
    if let MoveType::Promotion(p) = m.move_type {
        out.push(match p {
            PieceType::Queen => 'q',
            PieceType::Rook => 'r',
            PieceType::Bishop => 'b',
            _ => 'n',
        });
    }
    out
}

fn perft(board: &mut Board, depth: u8) -> u64 {
    let mut moves = Vec::new();
    board.generate_legal_moves_into(&mut moves);
    if depth == 1 {
        return moves.len() as u64;
    }
    let mut nodes = 0;
    for mv in moves {
        let undo = board.do_move(mv);
        nodes += perft(board, depth - 1);
        board.undo_move(mv, undo);
    }
    nodes
}

fn main() {
    let depth: u8 = std::env::args()
        .nth(1)
        .and_then(|d| d.parse().ok())
        .expect("usage: walk_perft <depth> < fens.txt");
    // Fixed seed: the same input always produces the same positions.
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let fen = line.trim().to_string();
        if fen.is_empty() {
            continue;
        }
        let Ok(mut board) = Board::from_fen(&fen) else {
            eprintln!("skipping unparseable FEN: {fen}");
            continue;
        };
        let mut walk = Vec::new();
        for _ in 0..(next() % 60) {
            let legal = board.generate_legal_moves();
            if legal.is_empty() {
                break;
            }
            let mv = legal[(next() % legal.len() as u64) as usize];
            walk.push(uci(&mv));
            assert!(board.make_move(mv));
        }
        if board.generate_legal_moves().is_empty() {
            continue;
        }
        println!("{}\t{}\t{}", fen, walk.join(" "), perft(&mut board, depth));
    }
}
