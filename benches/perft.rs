use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lemonate::Board;

fn perft(board: &mut Board, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }

    // Reusable scratch buffer: no allocation beyond retained capacity.
    let mut buf = Vec::new();
    board.generate_legal_moves_into(&mut buf);
    if depth == 1 {
        return buf.len() as u64;
    }

    let mut nodes = 0;
    for i in 0..buf.len() {
        let mv = buf[i];
        let undo = board.do_move(mv);
        nodes += perft(board, depth - 1);
        board.undo_move(mv, undo);
    }
    nodes
}

fn bench_perft(c: &mut Criterion) {
    let mut board =
        Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();

    c.bench_function("perft depth 4", |b| {
        b.iter(|| perft(black_box(&mut board), black_box(4)))
    });
}

criterion_group!(benches, bench_perft);
criterion_main!(benches);
