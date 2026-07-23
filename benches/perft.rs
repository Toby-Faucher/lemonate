use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lemonate::search::{SearchEngine, SearchLimits};
use lemonate::{evaluate, Board};

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

fn startpos() -> Board {
    Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
}

// A busy middlegame position (Kiwipete), good for stressing move gen/eval
// on a board with lots of captures and special moves available.
fn middlegame() -> Board {
    Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap()
}

fn bench_perft(c: &mut Criterion) {
    let board = startpos();

    c.bench_function("perft depth 4", |b| b.iter(|| perft(black_box(&board), 4)));
    c.bench_function("perft depth 5", |b| b.iter(|| perft(black_box(&board), 5)));

    // Depth 6 is ~35x depth 5's node count, so it needs its own group with a
    // smaller sample size and longer measurement time or the full suite
    // would take many minutes.
    let mut group = c.benchmark_group("perft-deep");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(20));
    group.bench_function("perft depth 6", |b| b.iter(|| perft(black_box(&board), 6)));
    group.finish();
}

fn bench_eval(c: &mut Criterion) {
    let start = startpos();
    let middle = middlegame();

    c.bench_function("evaluate startpos", |b| {
        b.iter(|| evaluate(black_box(&start)))
    });
    c.bench_function("evaluate middlegame", |b| {
        b.iter(|| evaluate(black_box(&middle)))
    });
}

fn bench_search(c: &mut Criterion) {
    let start = startpos();
    let middle = middlegame();

    // Depth 10 searches take much longer than depth 6, so use a smaller
    // sample size and a longer measurement window to keep total bench time
    // reasonable while still getting a stable estimate.
    let mut group = c.benchmark_group("search-deep");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(30));

    group.bench_function("search depth 10 startpos", |b| {
        b.iter(|| {
            let mut engine = SearchEngine::new();
            engine.search(black_box(&start), SearchLimits::depth(10))
        })
    });
    group.bench_function("search depth 10 middlegame", |b| {
        b.iter(|| {
            let mut engine = SearchEngine::new();
            engine.search(black_box(&middle), SearchLimits::depth(10))
        })
    });
    group.finish();
}

criterion_group!(benches, bench_perft, bench_eval, bench_search);
criterion_main!(benches);
