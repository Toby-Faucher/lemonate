//! UCI Command Handler
//!
//! Manages engine state and executes UCI commands.
//!
//! The search runs on a worker thread so that `stop`/`quit` received on
//! stdin take effect while a search is in progress. A shared atomic flag
//! (see [`SearchLimits::with_stop_signal`]) halts the worker promptly.

use crate::board::Board;
use crate::search::{parallel_search, SearchEngine, SearchLimits, SearchResult, SharedTT, MAX_DEPTH};
use crate::types::Color;

use super::protocol::{
    format_bestmove, format_info, format_uci_id, parse_command, parse_uci_move, GoParams,
    UciCommand,
};

use std::io::{self, BufRead, Write};
use std::sync::{
    Arc, atomic::{AtomicBool, Ordering}, mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

/// Result sent back by the search worker thread.
struct WorkerResult {
    engine: SearchEngine,
    result: SearchResult,
    elapsed_ms: u128,
}

/// A search currently running on the worker thread.
struct ActiveSearch {
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
    /// Transposition table shared by SMP workers. Persists across moves
    /// like the main engine; recreated on `Hash` changes, cleared on
    /// `ucinewgame`. Unused while `num_threads == 1`.
    shared_tt: Arc<SharedTT>,
}

/// Default TT size in MB (matches the engine default).
const DEFAULT_HASH_MB: usize = 64;

impl UciEngine {
    /// Create a new UCI engine.
    pub fn new() -> Self {
        let mut board = Board::starting_position();
        board.enable_history();

        Self {
            board,
            engine: Some(SearchEngine::new()),
            active: None,
            debug: false,
            num_threads: 1,
            shared_tt: Arc::new(SharedTT::new(DEFAULT_HASH_MB)),
        }
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
        self.stop_search();
        self.engine.take().unwrap_or_default()
    }

    /// Run the UCI main loop.
    pub fn run(&mut self) {
        // Forward stdin lines to the main loop through a channel so the
        // loop can also poll for search completion.
        let (line_tx, line_rx) = mpsc::channel::<String>();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(l) => {
                        if line_tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
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
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
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
            UciCommand::SetOption { name, value } => self.handle_setoption(&name, value),
            UciCommand::Register => {} // Not implemented.
            UciCommand::UciNewGame => self.handle_ucinewgame(),
            UciCommand::Position { fen, moves } => self.handle_position(fen.as_deref(), &moves),
            UciCommand::Go(params) => self.handle_go(params),
            UciCommand::Stop => self.handle_stop(stdout),
            UciCommand::PonderHit => {} // Not implemented.
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
    fn handle_setoption(&mut self, name: &str, value: Option<String>) {
        match name.to_lowercase().as_str() {
            "hash" => {
                if let Some(v) = value {
                    if let Ok(size_mb) = v.parse::<usize>() {
                        let mut engine = self.take_engine();
                        engine.set_hash_size(size_mb);
                        self.engine = Some(engine);
                        // Keep the shared table in sync (recreated while
                        // no search is running, so no worker holds it).
                        self.shared_tt = Arc::new(SharedTT::new(size_mb));
                    }
                }
            }
            "threads" => {
                if let Some(v) = value {
                    if let Ok(n) = v.parse::<usize>() {
                        self.stop_search();
                        self.num_threads = n.clamp(1, 256);
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
        self.shared_tt.clear();
        self.board = Board::starting_position();
        self.board.enable_history();
    }

    /// Handle "position" command.
    fn handle_position(&mut self, fen: Option<&str>, moves: &[String]) {
        // Changing the position while searching is meaningless; stop first.
        self.stop_search();

        // Set up the position.
        self.board = match fen {
            Some(f) => match Board::from_fen(f) {
                Ok(b) => b,
                Err(_) => {
                    if self.debug {
                        eprintln!("Invalid FEN: {}", f);
                    }
                    return;
                }
            },
            None => Board::starting_position(),
        };

        self.board.enable_history();

        // Apply moves.
        for move_str in moves {
            let legal_moves = self.board.generate_legal_moves();
            if let Some(mv) = parse_uci_move(move_str, &legal_moves) {
                self.board.make_move(mv);
            } else if self.debug {
                eprintln!("Invalid move: {}", move_str);
            }
        }
    }

    /// Handle "go" command: start a search on the worker thread.
    fn handle_go(&mut self, params: GoParams) {
        // Only one search at a time; stop (and discard) any active one.
        self.stop_search();

        let stop = Arc::new(AtomicBool::new(false));
        let mut limits = self.go_params_to_limits(&params, Arc::clone(&stop));

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
        let shared_tt = Arc::clone(&self.shared_tt);
        let (result_tx, result_rx) = mpsc::channel::<WorkerResult>();
        // Generous stack: deep search frames hold fixed-size move buffers.
        let handle = thread::Builder::new()
            .name("lemonate-search".to_string())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let start = Instant::now();
                // Threads == 1 keeps the exact historical path
                // (private TT, no SMP machinery).
                let (engine, result) = if num_threads <= 1 {
                    let mut engine = engine;
                    let result = engine.search(&board, limits);
                    (engine, result)
                } else {
                    parallel_search(&board, limits, num_threads, &shared_tt, engine)
                };
                let elapsed_ms = start.elapsed().as_millis();
                let _ = result_tx.send(WorkerResult {
                    engine,
                    result,
                    elapsed_ms,
                });
            })
            .expect("failed to spawn search thread");

        self.active = Some(ActiveSearch {
            stop,
            result_rx,
            handle: Some(handle),
        });
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

        // Mate search: mate in N needs up to 2*N - 1 plies to confirm.
        if let Some(mate) = params.mate {
            let depth = mate
                .saturating_mul(2)
                .saturating_sub(1)
                .max(1)
                .min(MAX_DEPTH as u32) as u8;
            let mut limits = SearchLimits::depth(depth).with_stop_signal(stop);
            if let Some(nodes) = params.nodes {
                limits = limits.with_nodes(nodes);
            }
            return limits;
        }

        // Fixed depth.
        if let Some(depth) = params.depth {
            let mut limits = SearchLimits::depth(depth).with_stop_signal(stop);
            if let Some(nodes) = params.nodes {
                limits = limits.with_nodes(nodes);
            }
            return limits;
        }

        // Fixed time.
        if let Some(movetime) = params.movetime {
            let mut limits = SearchLimits::movetime(movetime).with_stop_signal(stop);
            if let Some(nodes) = params.nodes {
                limits = limits.with_nodes(nodes);
            }
            return limits;
        }

        // Game clock.
        let side = self.board.side_to_move();
        let (remaining, increment) = match side {
            Color::White => (params.wtime, params.winc),
            Color::Black => (params.btime, params.binc),
        };

        if let Some(time) = remaining {
            let mut limits = SearchLimits::game_clock(
                time,
                increment.unwrap_or(0),
                params.movestogo,
            )
            .with_stop_signal(stop);
            if let Some(depth) = params.depth {
                limits = limits.with_depth(depth);
            }
            if let Some(nodes) = params.nodes {
                limits = limits.with_nodes(nodes);
            }
            return limits;
        }

        // Fallback to infinite (still honoring node/depth caps if given,
        // e.g. bare `go nodes N`).
        let mut limits = SearchLimits::infinite().with_stop_signal(stop);
        if let Some(depth) = params.depth {
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
