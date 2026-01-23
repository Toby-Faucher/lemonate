//! UCI Command Handler
//!
//! Manages engine state and executes UCI commands.

use crate::board::Board;
use crate::search::{SearchEngine, SearchLimits};
use crate::types::Color;

use super::protocol::{
    format_bestmove, format_info, format_uci_id, parse_command, parse_uci_move, GoParams,
    UciCommand,
};

use std::io::{self, BufRead, Write};
use std::time::Instant;

/// UCI engine handler.
pub struct UciEngine {
    /// Current board position.
    board: Board,
    /// Search engine.
    engine: SearchEngine,
    /// Debug mode.
    debug: bool,
}

impl UciEngine {
    /// Create a new UCI engine.
    pub fn new() -> Self {
        let mut board = Board::starting_position();
        board.enable_history();

        Self {
            board,
            engine: SearchEngine::new(),
            debug: false,
        }
    }

    /// Run the UCI main loop.
    pub fn run(&mut self) {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            let command = parse_command(&line);

            match command {
                UciCommand::Uci => self.handle_uci(&mut stdout),
                UciCommand::Debug(on) => self.debug = on,
                UciCommand::IsReady => self.handle_isready(&mut stdout),
                UciCommand::SetOption { name, value } => self.handle_setoption(&name, value),
                UciCommand::Register => {} // Not implemented.
                UciCommand::UciNewGame => self.handle_ucinewgame(),
                UciCommand::Position { fen, moves } => self.handle_position(fen.as_deref(), &moves),
                UciCommand::Go(params) => self.handle_go(params, &mut stdout),
                UciCommand::Stop => {} // Stop is handled within search.
                UciCommand::PonderHit => {} // Not implemented.
                UciCommand::Quit => break,
                UciCommand::Unknown(cmd) => {
                    if self.debug && !cmd.is_empty() {
                        eprintln!("Unknown command: {}", cmd);
                    }
                }
            }
        }
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
                        self.engine.set_hash_size(size_mb);
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
        self.engine.new_game();
        self.board = Board::starting_position();
        self.board.enable_history();
    }

    /// Handle "position" command.
    fn handle_position(&mut self, fen: Option<&str>, moves: &[String]) {
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

    /// Handle "go" command.
    fn handle_go(&mut self, params: GoParams, stdout: &mut io::Stdout) {
        let limits = self.go_params_to_limits(&params);
        let start = Instant::now();

        let result = self.engine.search(&self.board, limits);
        let elapsed_ms = start.elapsed().as_millis();

        // Output info string.
        let _ = writeln!(stdout, "{}", format_info(&result, elapsed_ms));

        // Output bestmove.
        if let Some(mv) = result.best_move {
            let _ = writeln!(stdout, "{}", format_bestmove(&mv));
        } else {
            // No legal moves - output a placeholder.
            let _ = writeln!(stdout, "bestmove 0000");
        }

        let _ = stdout.flush();
    }

    /// Convert GoParams to SearchLimits.
    fn go_params_to_limits(&self, params: &GoParams) -> SearchLimits {
        // Infinite search.
        if params.infinite {
            let mut limits = SearchLimits::infinite();
            if let Some(depth) = params.depth {
                limits = limits.with_depth(depth);
            }
            if let Some(nodes) = params.nodes {
                limits = limits.with_nodes(nodes);
            }
            return limits;
        }

        // Fixed depth.
        if let Some(depth) = params.depth {
            let mut limits = SearchLimits::depth(depth);
            if let Some(nodes) = params.nodes {
                limits = limits.with_nodes(nodes);
            }
            return limits;
        }

        // Fixed time.
        if let Some(movetime) = params.movetime {
            let mut limits = SearchLimits::movetime(movetime);
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
            );
            if let Some(depth) = params.depth {
                limits = limits.with_depth(depth);
            }
            if let Some(nodes) = params.nodes {
                limits = limits.with_nodes(nodes);
            }
            return limits;
        }

        // Fallback to infinite.
        SearchLimits::infinite()
    }
}

impl Default for UciEngine {
    fn default() -> Self {
        Self::new()
    }
}
