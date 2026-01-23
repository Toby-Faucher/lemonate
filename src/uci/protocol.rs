//! UCI Protocol Parsing and Output Formatting
//!
//! Handles parsing UCI commands from stdin and formatting responses.

use crate::board::{Move, MoveType};
use crate::search::{is_mate_score, mate_in, SearchResult};
use crate::types::{PieceType, Square};

/// UCI command parsed from input.
#[derive(Debug, Clone)]
pub enum UciCommand {
    /// Identify as UCI engine.
    Uci,
    /// Debug mode on/off.
    Debug(bool),
    /// Synchronize - respond with readyok.
    IsReady,
    /// Set an option.
    SetOption { name: String, value: Option<String> },
    /// Register (not implemented).
    Register,
    /// New game - clear state.
    UciNewGame,
    /// Set position.
    Position {
        fen: Option<String>,
        moves: Vec<String>,
    },
    /// Start searching.
    Go(GoParams),
    /// Stop searching.
    Stop,
    /// Ponder hit (not implemented).
    PonderHit,
    /// Quit the engine.
    Quit,
    /// Unknown command.
    Unknown(String),
}

/// Parameters for the "go" command.
#[derive(Debug, Clone, Default)]
pub struct GoParams {
    /// Search moves only (not implemented).
    pub searchmoves: Vec<String>,
    /// Ponder mode.
    pub ponder: bool,
    /// White time remaining (ms).
    pub wtime: Option<u64>,
    /// Black time remaining (ms).
    pub btime: Option<u64>,
    /// White increment (ms).
    pub winc: Option<u64>,
    /// Black increment (ms).
    pub binc: Option<u64>,
    /// Moves until next time control.
    pub movestogo: Option<u32>,
    /// Search to this depth only.
    pub depth: Option<u8>,
    /// Search this many nodes only.
    pub nodes: Option<u64>,
    /// Search for mate in N moves.
    pub mate: Option<u32>,
    /// Search for exactly this time (ms).
    pub movetime: Option<u64>,
    /// Search until "stop" command.
    pub infinite: bool,
}

/// Parse a UCI command from a line of input.
pub fn parse_command(line: &str) -> UciCommand {
    let line = line.trim();
    let mut tokens = line.split_whitespace();

    match tokens.next() {
        Some("uci") => UciCommand::Uci,
        Some("debug") => {
            let on = tokens.next() == Some("on");
            UciCommand::Debug(on)
        }
        Some("isready") => UciCommand::IsReady,
        Some("setoption") => parse_setoption(&mut tokens),
        Some("register") => UciCommand::Register,
        Some("ucinewgame") => UciCommand::UciNewGame,
        Some("position") => parse_position(&mut tokens),
        Some("go") => UciCommand::Go(parse_go(&mut tokens)),
        Some("stop") => UciCommand::Stop,
        Some("ponderhit") => UciCommand::PonderHit,
        Some("quit") => UciCommand::Quit,
        _ => UciCommand::Unknown(line.to_string()),
    }
}

/// Parse "setoption name <name> [value <value>]".
fn parse_setoption<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> UciCommand {
    let mut name_parts = Vec::new();
    let mut value_parts = Vec::new();
    let mut in_value = false;

    // Skip "name" token.
    if tokens.next() != Some("name") {
        return UciCommand::Unknown("setoption: missing 'name'".to_string());
    }

    for token in tokens {
        if token == "value" {
            in_value = true;
        } else if in_value {
            value_parts.push(token);
        } else {
            name_parts.push(token);
        }
    }

    let name = name_parts.join(" ");
    let value = if value_parts.is_empty() {
        None
    } else {
        Some(value_parts.join(" "))
    };

    UciCommand::SetOption { name, value }
}

/// Parse "position [startpos | fen <fen>] [moves <move1> <move2> ...]".
fn parse_position<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> UciCommand {
    let mut fen: Option<String> = None;
    let mut moves = Vec::new();
    let mut in_moves = false;
    let mut fen_parts = Vec::new();

    for token in tokens {
        if token == "startpos" {
            fen = None; // Use starting position.
        } else if token == "fen" {
            // Next tokens are FEN parts until "moves".
            continue;
        } else if token == "moves" {
            // If we collected FEN parts, join them.
            if !fen_parts.is_empty() {
                fen = Some(fen_parts.join(" "));
            }
            in_moves = true;
        } else if in_moves {
            moves.push(token.to_string());
        } else {
            // Collecting FEN parts.
            fen_parts.push(token);
        }
    }

    // Handle FEN without moves.
    if !fen_parts.is_empty() && fen.is_none() {
        fen = Some(fen_parts.join(" "));
    }

    UciCommand::Position { fen, moves }
}

/// Parse "go" command parameters.
fn parse_go<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> GoParams {
    let mut params = GoParams::default();

    let tokens: Vec<&str> = tokens.collect();
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i] {
            "searchmoves" => {
                i += 1;
                while i < tokens.len() && !is_go_keyword(tokens[i]) {
                    params.searchmoves.push(tokens[i].to_string());
                    i += 1;
                }
                continue;
            }
            "ponder" => params.ponder = true,
            "wtime" => {
                if i + 1 < tokens.len() {
                    params.wtime = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "btime" => {
                if i + 1 < tokens.len() {
                    params.btime = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "winc" => {
                if i + 1 < tokens.len() {
                    params.winc = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "binc" => {
                if i + 1 < tokens.len() {
                    params.binc = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "movestogo" => {
                if i + 1 < tokens.len() {
                    params.movestogo = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "depth" => {
                if i + 1 < tokens.len() {
                    params.depth = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "nodes" => {
                if i + 1 < tokens.len() {
                    params.nodes = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "mate" => {
                if i + 1 < tokens.len() {
                    params.mate = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "movetime" => {
                if i + 1 < tokens.len() {
                    params.movetime = tokens[i + 1].parse().ok();
                    i += 1;
                }
            }
            "infinite" => params.infinite = true,
            _ => {}
        }
        i += 1;
    }

    params
}

/// Check if a token is a go command keyword.
fn is_go_keyword(token: &str) -> bool {
    matches!(
        token,
        "searchmoves"
            | "ponder"
            | "wtime"
            | "btime"
            | "winc"
            | "binc"
            | "movestogo"
            | "depth"
            | "nodes"
            | "mate"
            | "movetime"
            | "infinite"
    )
}

/// Format UCI engine identification.
pub fn format_uci_id() -> String {
    let mut output = String::new();
    output.push_str("id name Lemonate\n");
    output.push_str("id author Toby\n");
    output.push_str("\n");
    output.push_str("option name Hash type spin default 64 min 1 max 4096\n");
    output.push_str("\n");
    output.push_str("uciok");
    output
}

/// Format UCI info string from search result.
pub fn format_info(result: &SearchResult, elapsed_ms: u128) -> String {
    let mut info = format!(
        "info depth {} seldepth {}",
        result.depth, result.stats.seldepth
    );

    // Format score.
    if is_mate_score(result.score) {
        if let Some(moves) = mate_in(result.score) {
            let mate_moves = if result.score > 0 {
                moves as i32
            } else {
                -(moves as i32)
            };
            info.push_str(&format!(" score mate {}", mate_moves));
        } else {
            info.push_str(&format!(" score cp {}", result.score));
        }
    } else {
        info.push_str(&format!(" score cp {}", result.score));
    }

    // Nodes and NPS.
    info.push_str(&format!(" nodes {}", result.stats.nodes));
    info.push_str(&format!(" nps {}", result.stats.nps(elapsed_ms)));
    info.push_str(&format!(" time {}", elapsed_ms));

    // Principal variation.
    if !result.pv.is_empty() {
        info.push_str(" pv");
        for mv in &result.pv {
            info.push_str(&format!(" {}", move_to_uci(mv)));
        }
    }

    info
}

/// Format bestmove response.
pub fn format_bestmove(mv: &Move) -> String {
    format!("bestmove {}", move_to_uci(mv))
}

/// Convert a Move to UCI notation (e.g., "e2e4", "e7e8q").
pub fn move_to_uci(mv: &Move) -> String {
    let from = square_to_algebraic(mv.from);
    let to = square_to_algebraic(mv.to);

    match mv.move_type {
        MoveType::Promotion(piece_type) => {
            let promo = match piece_type {
                PieceType::Queen => 'q',
                PieceType::Rook => 'r',
                PieceType::Bishop => 'b',
                PieceType::Knight => 'n',
                _ => 'q',
            };
            format!("{}{}{}", from, to, promo)
        }
        _ => format!("{}{}", from, to),
    }
}

/// Convert a Square to algebraic notation (e.g., "e4").
fn square_to_algebraic(square: Square) -> String {
    let file = (b'a' + square.file()) as char;
    let rank = (b'1' + square.rank()) as char;
    format!("{}{}", file, rank)
}

/// Parse a UCI move string and find the matching legal move.
///
/// Returns None if the move string is invalid or not legal.
pub fn parse_uci_move(s: &str, legal_moves: &[Move]) -> Option<Move> {
    let s = s.trim().to_lowercase();

    if s.len() < 4 {
        return None;
    }

    let from_file = s.chars().nth(0)?.to_digit(36)? as u8 - 10; // 'a' = 10 in base 36
    let from_rank = s.chars().nth(1)?.to_digit(10)? as u8 - 1;
    let to_file = s.chars().nth(2)?.to_digit(36)? as u8 - 10;
    let to_rank = s.chars().nth(3)?.to_digit(10)? as u8 - 1;

    if from_file > 7 || from_rank > 7 || to_file > 7 || to_rank > 7 {
        return None;
    }

    let from = Square::from_coords(from_file, from_rank);
    let to = Square::from_coords(to_file, to_rank);

    // Check for promotion.
    let promotion = if s.len() >= 5 {
        match s.chars().nth(4)? {
            'q' => Some(PieceType::Queen),
            'r' => Some(PieceType::Rook),
            'b' => Some(PieceType::Bishop),
            'n' => Some(PieceType::Knight),
            _ => None,
        }
    } else {
        None
    };

    // Find matching legal move.
    legal_moves
        .iter()
        .find(|m| {
            m.from == from
                && m.to == to
                && match (&m.move_type, promotion) {
                    (MoveType::Promotion(p1), Some(p2)) => p1 == &p2,
                    (MoveType::Promotion(_), None) => false,
                    (_, Some(_)) => false,
                    _ => true,
                }
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uci() {
        match parse_command("uci") {
            UciCommand::Uci => {}
            _ => panic!("Expected Uci command"),
        }
    }

    #[test]
    fn test_parse_isready() {
        match parse_command("isready") {
            UciCommand::IsReady => {}
            _ => panic!("Expected IsReady command"),
        }
    }

    #[test]
    fn test_parse_position_startpos() {
        match parse_command("position startpos") {
            UciCommand::Position { fen, moves } => {
                assert!(fen.is_none());
                assert!(moves.is_empty());
            }
            _ => panic!("Expected Position command"),
        }
    }

    #[test]
    fn test_parse_position_startpos_moves() {
        match parse_command("position startpos moves e2e4 e7e5") {
            UciCommand::Position { fen, moves } => {
                assert!(fen.is_none());
                assert_eq!(moves, vec!["e2e4", "e7e5"]);
            }
            _ => panic!("Expected Position command"),
        }
    }

    #[test]
    fn test_parse_position_fen() {
        match parse_command("position fen rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1") {
            UciCommand::Position { fen, moves } => {
                assert_eq!(
                    fen,
                    Some("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string())
                );
                assert!(moves.is_empty());
            }
            _ => panic!("Expected Position command"),
        }
    }

    #[test]
    fn test_parse_go_depth() {
        match parse_command("go depth 10") {
            UciCommand::Go(params) => {
                assert_eq!(params.depth, Some(10));
                assert!(!params.infinite);
            }
            _ => panic!("Expected Go command"),
        }
    }

    #[test]
    fn test_parse_go_time() {
        match parse_command("go wtime 300000 btime 300000 winc 2000 binc 2000") {
            UciCommand::Go(params) => {
                assert_eq!(params.wtime, Some(300000));
                assert_eq!(params.btime, Some(300000));
                assert_eq!(params.winc, Some(2000));
                assert_eq!(params.binc, Some(2000));
            }
            _ => panic!("Expected Go command"),
        }
    }

    #[test]
    fn test_parse_go_infinite() {
        match parse_command("go infinite") {
            UciCommand::Go(params) => {
                assert!(params.infinite);
            }
            _ => panic!("Expected Go command"),
        }
    }

    #[test]
    fn test_parse_setoption() {
        match parse_command("setoption name Hash value 128") {
            UciCommand::SetOption { name, value } => {
                assert_eq!(name, "Hash");
                assert_eq!(value, Some("128".to_string()));
            }
            _ => panic!("Expected SetOption command"),
        }
    }

    #[test]
    fn test_format_uci_id() {
        let id = format_uci_id();
        assert!(id.contains("id name Lemonate"));
        assert!(id.contains("id author Toby"));
        assert!(id.contains("uciok"));
    }
}
