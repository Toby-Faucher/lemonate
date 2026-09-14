//! Lazy SMP parallel search.
//!
//! N worker threads each run the existing iterative-deepening search on
//! the same position at staggered starting depths, sharing one
//! lock-free transposition table ([`SharedTT`]) and one stop signal.
//! Workers hold no shared mutable state besides the TT: each owns its
//! killers, history, PV tables, and move buffers.
//!
//! Threading model: threads are spawned per search and joined before
//! returning, so no worker outlives its search and no threads leak
//! across games. A single-threaded call behaves exactly like
//! [`SearchEngine::search`](super::SearchEngine::search) on a private
//! table (the UCI handler keeps using that path for `Threads 1`).

use super::{SearchEngine, SearchLimits, SearchResult, SharedTT};
use crate::Board;

use std::sync::Arc;

/// Maximum worker threads (matches the UCI `Threads` option ceiling).
pub const MAX_THREADS: usize = 256;

/// Stack size for search worker threads (matches the UCI search thread).
const WORKER_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Run a Lazy SMP parallel search.
///
/// # Arguments
/// * `board` - The position to search.
/// * `limits` - Search limits. The stop signal inside is shared by all
///   workers, so `stop`/timeout wakes every thread promptly.
/// * `num_threads` - Worker count (clamped to `1..=MAX_THREADS`).
/// * `shared` - Table shared by all workers. Its age is bumped exactly
///   once here, keeping replacement equivalent to a single-threaded
///   search.
/// * `main_engine` - Recycled as worker 0 so its history/killers persist
///   across moves; returned afterwards. Helper engines are fresh per
///   search and dropped (their heuristics are ordering-only hints).
///
/// # Returns
/// The recycled main engine plus the combined result: best move/score/PV
/// from the deepest completed worker, with node statistics summed across
/// all workers.
pub fn parallel_search(
    board: &Board,
    limits: SearchLimits,
    num_threads: usize,
    shared: &Arc<SharedTT>,
    main_engine: SearchEngine,
) -> (SearchEngine, SearchResult) {
    let num_threads = num_threads.clamp(1, MAX_THREADS);

    // Single worker: no threads, same staggered start as worker 0.
    if num_threads == 1 {
        let mut engine = main_engine;
        engine.set_shared_tt(Some(Arc::clone(shared)));
        shared.new_search();
        let result = engine.search_worker(board, limits, 1);
        engine.set_shared_tt(None);
        return (engine, result);
    }

    shared.new_search();

    let mut main_engine = Some(main_engine);
    let mut handles = Vec::with_capacity(num_threads);
    for thread_id in 0..num_threads {
        let board = board.clone();
        let limits = limits.clone();
        let shared = Arc::clone(shared);
        // Stagger starting depths for diversity (worker i starts at
        // depth 1+i, clamped to the depth cap inside the engine).
        let start_depth = (1 + thread_id).min(u8::MAX as usize) as u8;

        if thread_id == 0 {
            let mut engine = main_engine.take().unwrap_or_default();
            handles.push(
                std::thread::Builder::new()
                    .name("lemonate-smp-0".to_string())
                    .stack_size(WORKER_STACK_SIZE)
                    .spawn(move || {
                        engine.set_shared_tt(Some(shared));
                        let result = engine.search_worker(&board, limits, start_depth);
                        engine.set_shared_tt(None);
                        (0usize, engine, result)
                    })
                    .expect("failed to spawn SMP worker"),
            );
        } else {
            handles.push(
                std::thread::Builder::new()
                    .name(format!("lemonate-smp-{thread_id}"))
                    .stack_size(WORKER_STACK_SIZE)
                    .spawn(move || {
                        // Helpers use a minimal private table; all TT
                        // traffic goes to the shared table.
                        let mut engine = SearchEngine::with_hash_size(1);
                        engine.set_shared_tt(Some(shared));
                        let result = engine.search_worker(&board, limits, start_depth);
                        (thread_id, engine, result)
                    })
                    .expect("failed to spawn SMP worker"),
            );
        }
    }

    // Collect. Thread 0's engine is recycled; helpers are dropped.
    // A panicking worker simply contributes nothing.
    let mut main_engine: Option<SearchEngine> = None;
    let mut collected: Vec<(usize, SearchResult)> = Vec::with_capacity(num_threads);
    for handle in handles {
        if let Ok((id, engine, result)) = handle.join() {
            if id == 0 {
                main_engine = Some(engine);
            }
            // Helper engines are dropped here with `engine`.
            collected.push((id, result));
        }
    }

    let main_engine = main_engine.unwrap_or_default();

    if collected.is_empty() {
        return (main_engine, SearchResult::default());
    }

    // Deepest completed iteration wins; ties go to the lowest thread
    // index (worker 0) for determinism.
    collected.sort_by(|a, b| b.1.depth.cmp(&a.1.depth).then(a.0.cmp(&b.0)));
    let winner = &collected[0].1;

    let mut combined = SearchResult {
        best_move: winner.best_move,
        score: winner.score,
        depth: winner.depth,
        pv: winner.pv.clone(),
        stats: winner.stats.clone(),
    };
    // Aggregate node statistics across workers; seldepth is the max.
    for (_, r) in collected.iter().skip(1) {
        combined.stats.nodes += r.stats.nodes;
        combined.stats.qnodes += r.stats.qnodes;
        combined.stats.tt_hits += r.stats.tt_hits;
        combined.stats.tt_cutoffs += r.stats.tt_cutoffs;
        combined.stats.null_cutoffs += r.stats.null_cutoffs;
        combined.stats.beta_cutoffs += r.stats.beta_cutoffs;
        combined.stats.alpha_improvements += r.stats.alpha_improvements;
        combined.stats.seldepth = combined.stats.seldepth.max(r.stats.seldepth);
    }

    (main_engine, combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARITY_FENS: [&str; 3] = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3",
        "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
    ];

    /// libtest threads default to a small stack while production runs
    /// searches on 8 MB threads (see the UCI handler), so run search
    /// workloads the same way here.
    fn on_search_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .name("lemonate-test-search".to_string())
            .stack_size(16 * 1024 * 1024)
            .spawn(f)
            .expect("failed to spawn test search thread")
            .join()
            .expect("test search thread panicked")
    }

    #[test]
    fn test_two_threads_terminates_with_legal_move() {
        let result = on_search_stack(move || {
            let board = Board::starting_position();
            let shared = Arc::new(SharedTT::new(1));
            let (_engine, result) = parallel_search(
                &board,
                SearchLimits::depth(4),
                2,
                &shared,
                SearchEngine::new(),
            );
            result
        });

        let best = result.best_move.expect("should find a move");
        let legal = Board::starting_position().generate_legal_moves();
        assert!(
            legal.contains(&best),
            "best move must be legal, got {:?}",
            best
        );
        assert_eq!(result.depth, 4);
        assert!(result.stats.nodes > 0);
    }

    #[test]
    fn test_single_thread_parity_with_direct_search() {
        // Threads == 1 through the SMP entry point must match the plain
        // single-threaded path exactly: nodes, qnodes, score, best move.
        for fen in PARITY_FENS {
            for depth in [5u8, 6, 7] {
                let expected = on_search_stack(move || {
                    let board = Board::from_fen(fen).unwrap();
                    let mut direct = SearchEngine::new();
                    direct.search(&board, SearchLimits::depth(depth))
                });

                let actual = on_search_stack(move || {
                    let board = Board::from_fen(fen).unwrap();
                    let shared = Arc::new(SharedTT::new(64));
                    let (_, result) = parallel_search(
                        &board,
                        SearchLimits::depth(depth),
                        1,
                        &shared,
                        SearchEngine::new(),
                    );
                    result
                });

                assert_eq!(actual.depth, expected.depth, "depth {depth} {fen}");
                assert_eq!(actual.stats.nodes, expected.stats.nodes, "nodes {depth} {fen}");
                assert_eq!(actual.stats.qnodes, expected.stats.qnodes, "qnodes {depth} {fen}");
                assert_eq!(actual.score, expected.score, "score {depth} {fen}");
                assert_eq!(actual.best_move, expected.best_move, "move {depth} {fen}");
            }
        }
    }

    #[test]
    fn test_shared_table_single_worker_matches_private() {
        // One worker on the shared table follows the same probe/store
        // sequence as the private table, so counts must match exactly.
        for fen in PARITY_FENS {
            for depth in [5u8, 6] {
                let expected = on_search_stack(move || {
                    let board = Board::from_fen(fen).unwrap();
                    let mut direct = SearchEngine::new();
                    direct.search(&board, SearchLimits::depth(depth))
                });

                let actual = on_search_stack(move || {
                    let board = Board::from_fen(fen).unwrap();
                    let shared = Arc::new(SharedTT::new(64));
                    let mut worker = SearchEngine::new();
                    worker.set_shared_tt(Some(Arc::clone(&shared)));
                    worker.search(&board, SearchLimits::depth(depth))
                });

                assert_eq!(actual.stats.nodes, expected.stats.nodes, "nodes {depth} {fen}");
                assert_eq!(actual.stats.qnodes, expected.stats.qnodes, "qnodes {depth} {fen}");
                assert_eq!(actual.score, expected.score, "score {depth} {fen}");
                assert_eq!(actual.best_move, expected.best_move, "move {depth} {fen}");
            }
        }
    }

    #[test]
    fn test_four_threads_fixed_depth_completes() {
        let result = on_search_stack(move || {
            let board = Board::from_fen(PARITY_FENS[1]).unwrap();
            let shared = Arc::new(SharedTT::new(1));
            let (_, result) = parallel_search(
                &board,
                SearchLimits::depth(5),
                4,
                &shared,
                SearchEngine::new(),
            );
            result
        });

        assert_eq!(result.depth, 5);
        let best = result.best_move.expect("should find a move");
        let board = Board::from_fen(PARITY_FENS[1]).unwrap();
        assert!(board.generate_legal_moves().contains(&best));
    }
}
