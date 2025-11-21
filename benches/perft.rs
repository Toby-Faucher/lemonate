use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lemonate::Board;

fn perft(board: &Board, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }

    let moves = board.generate_legal_moves();
    if depth == 1 {
        return moves.len() as u64;
    }

    let mut nodes = 0;
    for mv in moves {
        let mut board_copy = board.clone();
        board_copy.make_move(mv);
        nodes += perft(&board_copy, depth - 1);
    }
    nodes
}

fn bench_perft(c: &mut Criterion) {
    let board =
        Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();

    c.bench_function("perft depth 4", |b| b.iter(|| perft(black_box(&board), 4)));
}

criterion_group!(benches, bench_perft);
criterion_main!(benches);
