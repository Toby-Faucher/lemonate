//! UCI Command Handler
//!
//! Manages engine state and executes UCI commands.
//!
//! The search runs on a worker thread so that `stop`/`quit` received on
//! stdin take effect while a search is in progress. A shared atomic flag
//! (see [`SearchLimits::with_stop_signal`]) halts the worker promptly.

use crate::board::{Board, Move, MoveType};
use crate::book::BookManager;
use crate::search::{
    parallel_search, InfoSink, SearchEngine, SearchLimits, SearchResult, SharedTT, MAX_DEPTH,
};
use crate::types::Color;

use super::protocol::{
    clamp_hash_mb, clamp_overhead_ms, format_bestmove, format_info, format_uci_id, parse_command, parse_uci_move,
    GoParams, UciCommand, HASH_DEFAULT_MB, OVERHEAD_DEFAULT_MS,
};

use std::io::{self, BufRead, Write};
use std::sync::{
    Arc, atomic::{AtomicBool, Ordering}, mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

const BENCH_DEPTH: u8 = 10;

const BENCH_FENS: [&str; 14] = [
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4",
    "2rr3k/pp3pp1/1nnqbN1p/3pN3/2pP4/2P3Q1/PPB4P/R4RK1 w - - 0 1",
    "8/8/4k3/8/2p5/8/B2K4/8 w - - 0 1",
    "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1",
    "8/5pk1/6p1/8/8/6P1/5PK1/3R4 w - - 0 1",
    "r2q1rk1/pp2ppbp/2np1np1/2pP4/4P1b1/2N1BN2/PPP1BPPP/R2Q1RK1 w - - 0 9",
    "3r2k1/pp3ppp/2p5/8/3P4/2P2N2/PP3PPP/6K1 b - - 0 1",
    "r1b2rk1/2q1bppp/p2p1n2/np2p3/3NP3/1BN1BP2/PPP3PP/R2Q1RK1 w - - 0 12",
];

/// Search every bench position to `depth` with a fresh engine. Returns (nodes, elapsed ms).
fn run_bench(depth: u8, hash_mb: usize) -> (u64, u128) {
    let start = Instant::now();
    let mut nodes = 0;
    for fen in BENCH_FENS {
        let mut board = Board::from_fen(fen).expect("bench FEN is valid");
        board.enable_history();
        let mut engine = SearchEngine::with_hash_size(hash_mb);
        nodes += engine.search(&board, SearchLimits::depth(depth)).stats.nodes;
    }
    (nodes, start.elapsed().as_millis())
}

/// Result sent back by the search worker thread.
struct WorkerResult {
    engine: SearchEngine,
    result: SearchResult,
    elapsed_ms: u128,
}

/// A search currently running on the worker thread.
struct ActiveSearch {
    /// True when nothing but `stop` can end this search (`go infinite`, or no limit at all).
    endless: bool,
    stop: Arc<AtomicBool>,
    result_rx: mpsc::Receiver<WorkerResult>,
    handle: Option<thread::JoinHandle<()>>,
}

/// UCI engine handler.
pub struct UciEngine {
    /// Current board position.
    board: Board,
    /// Search engine. Moved to the worker thread while searching
    /// (`None` in that case) and reclaimed afterwards.
    engine: Option<SearchEngine>,
    /// Search currently running on the worker thread, if any.
    active: Option<ActiveSearch>,
    /// Debug mode.
    debug: bool,
    /// Worker thread count for Lazy SMP search (UCI `Threads` option).
    num_threads: usize,
    /// Transposition table shared by SMP workers. Created lazily the first
    /// time a multi-threaded search runs (so the default single-threaded
    /// setup does not pay for a second table), persists across moves like
    /// the main engine, dropped on `Hash` changes and cleared on `ucinewgame`.
    shared_tt: Option<Arc<SharedTT>>,
    /// Current `Hash` option value in MB.
    hash_mb: usize,
    /// Polyglot opening book (`BookFile`), consulted when `OwnBook` is on.
    book: BookManager,
    own_book: bool,
    /// `Move Overhead` option: milliseconds per move reserved for GUI latency.
    move_overhead_ms: u64,
}

impl UciEngine {
    /// Create a new UCI engine.
    pub fn new() -> Self {
        let mut board = Board::starting_position();
        board.enable_history();
        Self::warm_up();

        Self {
            board,
            engine: Some(SearchEngine::new()),
            active: None,
            debug: false,
            num_threads: 1,
            shared_tt: None,
            hash_mb: HASH_DEFAULT_MB,
            book: BookManager::new(),
            own_book: false,
            move_overhead_ms: OVERHEAD_DEFAULT_MS,
        }
    }

    /// Run a tiny search so lazily initialised tables (attack tables, LMR table,
    /// Zobrist keys) are built before the first `go`, outside any game clock.
    fn warm_up() {
        let board = Board::starting_position();
        let mut engine = SearchEngine::with_hash_size(1);
        engine.search(&board, SearchLimits::depth(1));
    }

    /// Signal the worker to stop and wait for it, reclaiming the engine.
    /// Returns the search result if the worker produced one.
    fn stop_search(&mut self) -> Option<(SearchResult, u128)> {
        if self.active.is_none() {
            return None;
        }
        if let Some(active) = &self.active {
            active.stop.store(true, Ordering::Relaxed);
        }
        // The worker polls the flag every 2048 nodes and between
        // iterations, so this blocks only briefly.
        let worker = match &self.active {
            Some(active) => active.result_rx.recv().ok(),
            None => None,
        };
        self.join_worker();
        worker.map(|w| {
            self.engine = Some(w.engine);
            (w.result, w.elapsed_ms)
        })
    }

    /// Stop a running search because another command replaces it, and still answer the `go` that
    /// started it: a GUI that sent `go` and then `position` without `stop` waits for that bestmove.
    fn abort_search(&mut self) {
        if let Some((result, elapsed_ms)) = self.stop_search() {
            let mut stdout = io::stdout();
            let _ = writeln!(stdout, "{}", format_info(&result, elapsed_ms));
            self.write_bestmove(&mut stdout, result.best_move);
        }
    }

    /// Join the worker thread and clear the active search.
    /// If the worker died, install a fresh engine so the handler stays usable.
    fn join_worker(&mut self) {
        if let Some(mut active) = self.active.take() {
            if let Some(handle) = active.handle.take() {
                if handle.join().is_err() && self.engine.is_none() {
                    self.engine = Some(SearchEngine::new());
                }
            }
        }
    }

    /// Take ownership of the search engine, stopping any active search first.
    fn take_engine(&mut self) -> SearchEngine {
        self.abort_search();
        self.engine.take().unwrap_or_default()
    }

    /// Run the UCI main loop.
    pub fn run(&mut self) {
        // Forward stdin lines to the main loop through a channel so the
        // loop can also poll for search completion.
        let (line_tx, line_rx) = mpsc::channel::<String>();
        thread::spawn(move || {
            // Read raw bytes so one line that is not valid UTF-8 (a Latin-1 book path, say)
            // does not end the engine; only EOF or a real I/O error does.
            let mut stdin = io::stdin().lock();
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match stdin.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string();
                        if line_tx.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let mut stdout = io::stdout();

        loop {
            // Reap a finished search and report its result.
            self.poll_search(&mut stdout);

            match line_rx.recv_timeout(Duration::from_millis(5)) {
                Ok(line) => {
                    if !self.handle_line(&line, &mut stdout) {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // EOF (a piped script). A search that has its own limit is allowed to finish
                    // and report, so `printf 'position startpos\ngo depth 12\n' | lemonate`
                    // still prints a bestmove. An endless one can only end by `stop`.
                    while self.active.as_ref().is_some_and(|a| !a.endless) {
                        self.poll_search(&mut stdout);
                        thread::sleep(Duration::from_millis(5));
                    }
                    break;
                }
            }
        }

        // EOF or quit: make sure the worker is stopped before exiting.
        self.stop_search();
    }

    /// Handle a single input line. Returns false when the engine should exit.
    fn handle_line(&mut self, line: &str, stdout: &mut io::Stdout) -> bool {
        let command = parse_command(line);

        match command {
            UciCommand::Uci => self.handle_uci(stdout),
            UciCommand::Debug(on) => self.debug = on,
            UciCommand::IsReady => self.handle_isready(stdout),
            UciCommand::SetOption { name, value } => self.handle_setoption(&name, value, stdout),
            UciCommand::Register => {} // Not implemented.
            UciCommand::UciNewGame => self.handle_ucinewgame(),
            UciCommand::Position { fen, moves } => {
                self.handle_position(fen.as_deref(), &moves, stdout)
            }
            UciCommand::Go(params) => self.handle_go(params, stdout),
            UciCommand::Stop => self.handle_stop(stdout),
            UciCommand::PonderHit => {} // Not implemented.
            UciCommand::Bench(depth) => self.handle_bench(depth, stdout),
            UciCommand::Quit => return false,
            UciCommand::Unknown(cmd) => {
                if self.debug && !cmd.is_empty() {
                    eprintln!("Unknown command: {}", cmd);
                }
            }
        }

        true
    }

    /// Check whether the worker finished; if so, reclaim the engine and
    /// print `info` + `bestmove`.
    fn poll_search(&mut self, stdout: &mut io::Stdout) {
        enum Poll {
            Idle,
            Busy,
            Done(WorkerResult),
            Dead,
        }
        let poll = match &self.active {
            None => Poll::Idle,
            Some(active) => match active.result_rx.try_recv() {
                Ok(worker) => Poll::Done(worker),
                Err(mpsc::TryRecvError::Empty) => Poll::Busy,
                Err(mpsc::TryRecvError::Disconnected) => Poll::Dead,
            },
        };

        match poll {
            Poll::Idle | Poll::Busy => {}
            Poll::Done(worker) => {
                self.join_worker();
                self.engine = Some(worker.engine);
                let _ = writeln!(stdout, "{}", format_info(&worker.result, worker.elapsed_ms));
                self.write_bestmove(stdout, worker.result.best_move);
            }
            Poll::Dead => {
                self.join_worker();
                self.write_bestmove(stdout, None);
            }
        }
    }

    /// Write a bestmove line (with `0000` fallback when there is no move).
    fn write_bestmove(&self, stdout: &mut io::Stdout, mv: Option<crate::board::Move>) {
        if let Some(mv) = mv {
            let _ = writeln!(stdout, "{}", format_bestmove(&mv));
        } else {
            // No legal moves - output a placeholder.
            let _ = writeln!(stdout, "bestmove 0000");
        }
        let _ = stdout.flush();
    }

    /// Handle "uci" command.
    fn handle_uci(&self, stdout: &mut io::Stdout) {
        let _ = writeln!(stdout, "{}", format_uci_id());
        let _ = stdout.flush();
    }

    /// Handle "isready" command.
    fn handle_isready(&self, stdout: &mut io::Stdout) {
        let _ = writeln!(stdout, "readyok");
        let _ = stdout.flush();
    }

    /// Handle "setoption" command.
    fn handle_setoption(&mut self, name: &str, value: Option<String>, stdout: &mut io::Stdout) {
        match name.to_lowercase().as_str() {
            "hash" => {
                if let Some(v) = value {
                    if let Ok(requested) = v.trim().parse::<usize>() {
                        // Stay inside the advertised range: an absurd value
                        // would abort the process on allocation failure.
                        let size_mb = clamp_hash_mb(requested);
                        let mut engine = self.take_engine();
                        engine.set_hash_size(size_mb);
                        self.engine = Some(engine);
                        self.hash_mb = size_mb;
                        // The shared table is rebuilt lazily at the new size
                        // (no search is running, so no worker holds it).
                        self.shared_tt = None;
                    }
                }
            }
            "move overhead" => {
                if let Some(Ok(ms)) = value.map(|v| v.trim().parse::<u64>()) {
                    self.move_overhead_ms = clamp_overhead_ms(ms);
                }
            }
            "clear hash" => {
                let mut engine = self.take_engine();
                engine.clear_hash();
                self.engine = Some(engine);
                if let Some(tt) = &self.shared_tt {
                    tt.clear();
                }
            }
            "ownbook" => {
                if let Some(v) = value {
                    self.own_book = v.trim().eq_ignore_ascii_case("true");
                }
            }
            "bookfile" => {
                // UCI sends "<empty>" (or nothing) to clear a string option.
                let path = value.unwrap_or_default();
                let path = path.trim();
                if path.is_empty() || path == "<empty>" {
                    self.book = BookManager::new();
                } else {
                    let mut book = BookManager::new();
                    match book.load(path) {
                        Ok(()) if book.is_loaded() => self.book = book,
                        Ok(()) => self.report_error(stdout, &format!("book '{}' has no entries", path)),
                        Err(e) => self.report_error(stdout, &format!("cannot load book '{}': {}", path, e)),
                    }
                }
            }
            "threads" => {
                if let Some(v) = value {
                    if let Ok(n) = v.parse::<usize>() {
                        self.abort_search();
                        self.num_threads = n.clamp(1, 256);
                        if self.num_threads == 1 {
                            self.shared_tt = None;
                        }
                    }
                }
            }
            _ => {
                if self.debug {
                    eprintln!("Unknown option: {}", name);
                }
            }
        }
    }

    /// Handle "ucinewgame" command.
    fn handle_ucinewgame(&mut self) {
        let mut engine = self.take_engine();
        engine.new_game();
        self.engine = Some(engine);
        // The shared table is cleared once here, not once per worker.
        if let Some(tt) = &self.shared_tt {
            tt.clear();
        }
        self.board = Board::starting_position();
        self.board.enable_history();
    }

    /// Handle "position" command.
    ///
    /// The new position is built on the side and only installed if the FEN and
    /// every move in the list are valid. Otherwise the previous position is kept
    /// and an `info string` explains why, instead of silently searching a
    /// position the GUI never asked for.
    fn handle_position(&mut self, fen: Option<&str>, moves: &[String], stdout: &mut io::Stdout) {
        // Changing the position while searching is meaningless; stop first.
        self.abort_search();

        let mut board = match fen {
            Some(f) => match Board::from_fen(f) {
                Ok(b) => b,
                Err(_) => {
                    self.report_error(stdout, &format!("invalid FEN '{}', position unchanged", f));
                    return;
                }
            },
            None => Board::starting_position(),
        };
        board.enable_history();

        for move_str in moves {
            let legal_moves = board.generate_legal_moves();
            match parse_uci_move(move_str, &legal_moves) {
                Some(mv) => {
                    board.make_move(mv);
                }
                None => {
                    self.report_error(
                        stdout,
                        &format!("illegal or unparseable move '{}', position unchanged", move_str),
                    );
                    return;
                }
            }
        }

        self.board = board;
    }

    /// The book's choice for the current position, as a legal move of that position.
    ///
    /// Book files come from outside, so the decoded move is only used to pick among the
    /// engine's own legal moves (matched on squares and promotion). An entry that is not
    /// legal here is ignored and the engine searches instead.
    fn book_move(&self) -> Option<Move> {
        if !self.book.is_loaded() {
            return None;
        }
        let wanted = self.book.probe(&self.board)?;
        let promotion = |m: &Move| match m.move_type {
            MoveType::Promotion(p) => Some(p),
            _ => None,
        };
        self.board
            .generate_legal_moves()
            .into_iter()
            .find(|m| m.from == wanted.from && m.to == wanted.to && promotion(m) == promotion(&wanted))
    }

    /// Report a recoverable protocol problem to the GUI.
    fn report_error(&self, stdout: &mut io::Stdout, msg: &str) {
        let _ = writeln!(stdout, "info string error: {}", msg);
        let _ = stdout.flush();
    }

    /// Handle "go" command: start a search on the worker thread.
    fn handle_go(&mut self, params: GoParams, stdout: &mut io::Stdout) {
        // Only one search at a time; stop (and discard) any active one.
        self.abort_search();

        // A book move needs no search. Analysis requests (infinite, ponder, fixed depth or
        // nodes, mate search, searchmoves) are left to the engine.
        let analysis = params.infinite
            || params.ponder
            || params.depth.is_some()
            || params.nodes.is_some()
            || params.mate.is_some()
            || !params.searchmoves.is_empty();
        if self.own_book && !analysis {
            if let Some(mv) = self.book_move() {
                let _ = writeln!(stdout, "info string book move");
                self.write_bestmove(stdout, Some(mv));
                return;
            }
        }

        let stop = Arc::new(AtomicBool::new(false));
        let mut limits = self
            .go_params_to_limits(&params, Arc::clone(&stop))
            .with_overhead(self.move_overhead_ms);

        let side_has_clock = match self.board.side_to_move() {
            Color::White => params.wtime.is_some(),
            Color::Black => params.btime.is_some(),
        };
        let endless = params.infinite
            || (params.depth.is_none()
                && params.mate.is_none()
                && params.movetime.is_none()
                && params.nodes.is_none()
                && !side_has_clock);

        // Stream every finished iteration to the GUI while the search is still running.
        let started = Instant::now();
        limits = limits.with_info_sink(InfoSink::new(move |result: &SearchResult| {
            let line = format_info(result, started.elapsed().as_millis());
            let mut out = io::stdout().lock();
            let _ = writeln!(out, "{}", line);
            let _ = out.flush();
        }));

        // Resolve UCI searchmoves strings to legal moves now; the engine
        // enforces the restriction at the root.
        if !params.searchmoves.is_empty() {
            let legal_moves = self.board.generate_legal_moves();
            let restricted: Vec<crate::board::Move> = params
                .searchmoves
                .iter()
                .filter_map(|s| parse_uci_move(s, &legal_moves))
                .collect();
            if !restricted.is_empty() {
                limits = limits.with_root_moves(restricted);
            } else if self.debug {
                eprintln!("Ignoring searchmoves: none are legal");
            }
        }

        let engine = self.engine.take().unwrap_or_default();
        let board = self.board.clone();
        let num_threads = self.num_threads;
        let hash_mb = self.hash_mb;
        let shared_tt = if num_threads > 1 {
            Some(Arc::clone(
                self.shared_tt
                    .get_or_insert_with(|| Arc::new(SharedTT::new(hash_mb))),
            ))
        } else {
            None
        };
        let (result_tx, result_rx) = mpsc::channel::<WorkerResult>();
        // Generous stack: deep search frames hold fixed-size move buffers.
        let spawned = thread::Builder::new()
            .name("lemonate-search".to_string())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let start = Instant::now();
                // Threads == 1 keeps the exact historical path
                // (private TT, no SMP machinery).
                let (engine, result) = match shared_tt {
                    Some(shared) if num_threads > 1 => {
                        parallel_search(&board, limits, num_threads, &shared, engine)
                    }
                    _ => {
                        let mut engine = engine;
                        let result = engine.search(&board, limits);
                        (engine, result)
                    }
                };
                let elapsed_ms = start.elapsed().as_millis();
                let _ = result_tx.send(WorkerResult {
                    engine,
                    result,
                    elapsed_ms,
                });
            });
        let handle = match spawned {
            Ok(handle) => handle,
            Err(e) => {
                // Out of threads or memory: answer instead of aborting the process mid-game.
                let _ = writeln!(stdout, "info string cannot start the search thread: {e}");
                self.write_bestmove(stdout, self.board.generate_legal_moves().first().copied());
                return;
            }
        };

        self.active = Some(ActiveSearch {
            endless,
            stop,
            result_rx,
            handle: Some(handle),
        });
    }

    /// Handle "bench" command: fixed-depth search of [`BENCH_FENS`], single thread, fresh TT each.
    fn handle_bench(&mut self, depth: Option<u8>, stdout: &mut io::Stdout) {
        self.abort_search();
        let depth = depth.unwrap_or(BENCH_DEPTH).clamp(1, MAX_DEPTH);
        let hash_mb = self.hash_mb;
        // Same generous stack as the search worker.
        let run = thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(move || run_bench(depth, hash_mb))
            .map(|h| h.join());
        match run {
            Ok(Ok((nodes, ms))) => {
                let nps = (nodes as u128 * 1000 / ms.max(1)) as u64;
                let _ = writeln!(stdout, "\n===========================");
                let _ = writeln!(stdout, "Total time (ms) : {ms}");
                let _ = writeln!(stdout, "Nodes searched  : {nodes}");
                let _ = writeln!(stdout, "{nodes} nodes {nps} nps");
            }
            _ => self.report_error(stdout, "bench failed"),
        }
        let _ = stdout.flush();
    }

    /// Handle "stop" command: halt the search and report the best move so far.
    fn handle_stop(&mut self, stdout: &mut io::Stdout) {
        if let Some((result, elapsed_ms)) = self.stop_search() {
            let _ = writeln!(stdout, "{}", format_info(&result, elapsed_ms));
            self.write_bestmove(stdout, result.best_move);
        }
        // No active search: ignore, per UCI spec.
    }

    /// Convert GoParams to SearchLimits.
    fn go_params_to_limits(&self, params: &GoParams, stop: Arc<AtomicBool>) -> SearchLimits {
        // A depth of 0 (or one beyond what the search supports) is not a
        // meaningful request: search at least one ply, at most MAX_DEPTH.
        let params = &GoParams {
            depth: params.depth.map(|d| d.clamp(1, MAX_DEPTH)),
            ..params.clone()
        };
        // Infinite search.
        if params.infinite {
            let mut limits = SearchLimits::infinite().with_stop_signal(stop);
            if let Some(depth) = params.depth {
                limits = limits.with_depth(depth);
            }
            if let Some(nodes) = params.nodes {
                limits = limits.with_nodes(nodes);
            }
            return limits;
        }

        // Depth, mate and node requests are caps on top of whatever clock the GUI also sent:
        // `go depth 30 wtime 5000` must still stop in time.
        let side = self.board.side_to_move();
        let (remaining, increment) = match side {
            Color::White => (params.wtime, params.winc),
            Color::Black => (params.btime, params.binc),
        };
        // Mate in N needs up to 2*N - 1 plies to confirm.
        let mate_depth = params.mate.map(|mate| {
            mate.saturating_mul(2)
                .saturating_sub(1)
                .max(1)
                .min(MAX_DEPTH as u32) as u8
        });
        let depth_cap = match (params.depth, mate_depth) {
            (Some(d), Some(m)) => Some(d.min(m)),
            (d, m) => d.or(m),
        };

        let base = if let Some(movetime) = params.movetime {
            SearchLimits::movetime(movetime)
        } else if let Some(time) = remaining {
            SearchLimits::game_clock(time, increment.unwrap_or(0), params.movestogo)
        } else if let Some(depth) = depth_cap {
            SearchLimits::depth(depth)
        } else {
            SearchLimits::infinite()
        };
        let mut limits = base.with_stop_signal(stop);
        if let Some(depth) = depth_cap {
            limits = limits.with_depth(depth);
        }
        if let Some(nodes) = params.nodes {
            limits = limits.with_nodes(nodes);
        }
        limits
    }
}

impl Default for UciEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_is_deterministic() {
        assert_eq!(run_bench(2, 1).0, run_bench(2, 1).0);
        assert!(run_bench(2, 1).0 > 0);
    }
}
