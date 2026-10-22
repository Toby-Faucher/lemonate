//! UCI Protocol Parsing and Output Formatting
//!
//! Handles parsing UCI commands from stdin and formatting responses.

use crate::board::{Move, MoveType};
use crate::search::{is_mate_score, mate_in, SearchResult};
use crate::types::{PieceType, Square};

/// Advertised `Hash` option range, in MB.
pub const HASH_MIN_MB: usize = 1;
pub const HASH_MAX_MB: usize = 4096;
pub const HASH_DEFAULT_MB: usize = 64;

/// Advertised `Move Overhead` option range, in milliseconds.
pub const OVERHEAD_MIN_MS: u64 = 0;
pub const OVERHEAD_MAX_MS: u64 = 5000;
pub const OVERHEAD_DEFAULT_MS: u64 = 10;

/// Clamp a requested `Move Overhead` to the advertised range.
pub fn clamp_overhead_ms(requested: u64) -> u64 {
    requested.clamp(OVERHEAD_MIN_MS, OVERHEAD_MAX_MS)
}

/// Clamp a requested `Hash` size to the advertised range.
pub fn clamp_hash_mb(requested: usize) -> usize {
    requested.clamp(HASH_MIN_MB, HASH_MAX_MB)
}

/// Parse a UCI integer without dropping the whole parameter on values real GUIs send:
/// a negative number (e.g. the clock of a player already flagged) becomes 0 and a value
/// too large for u64 saturates. Anything that is not an integer is `None`.
fn parse_uci_int(s: &str) -> Option<u64> {
    let (negative, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if negative {
        return Some(0);
    }
    Some(digits.parse::<u64>().unwrap_or(u64::MAX))
}

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
    /// Fixed-depth node-count benchmark (optional depth).
    Bench(Option<u8>),
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
        Some("setoption") => parse_setoption(line),
        Some("register") => UciCommand::Register,
        Some("ucinewgame") => UciCommand::UciNewGame,
        Some("position") => parse_position(&mut tokens),
        Some("go") => UciCommand::Go(parse_go(&mut tokens)),
        Some("stop") => UciCommand::Stop,
        Some("ponderhit") => UciCommand::PonderHit,
        Some("quit") => UciCommand::Quit,
        Some("bench") => UciCommand::Bench(tokens.next().and_then(|d| d.parse().ok())),
        _ => UciCommand::Unknown(line.to_string()),
    }
}

/// Parse "setoption name <name> [value <value>]" from the whole line.
///
/// The value is cut from the original text, not rebuilt from tokens, so a path with repeated
/// spaces, or one containing the word `value`, arrives as typed.
fn parse_setoption(line: &str) -> UciCommand {
    let rest = line.trim_start().strip_prefix("setoption").unwrap_or("").trim_start();
    let body = match rest.strip_prefix("name") {
        Some(b) if b.is_empty() || b.starts_with(char::is_whitespace) => b,
        _ => return UciCommand::Unknown("setoption: missing 'name'".to_string()),
    };

    // The first whole-word `value` ends the name.
    let split = body.split_whitespace().find(|t| *t == "value").map(|t| {
        t.as_ptr() as usize - body.as_ptr() as usize
    });
    let (name_text, value_text) = match split {
        Some(at) => (&body[..at], Some(body[at + "value".len()..].trim())),
        None => (body, None),
    };
    let name = name_text.split_whitespace().collect::<Vec<_>>().join(" ");
    let value = value_text.filter(|v| !v.is_empty()).map(str::to_string);

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
                    params.wtime = parse_uci_int(tokens[i + 1]);
                    i += 1;
                }
            }
            "btime" => {
                if i + 1 < tokens.len() {
                    params.btime = parse_uci_int(tokens[i + 1]);
                    i += 1;
                }
            }
            "winc" => {
                if i + 1 < tokens.len() {
                    params.winc = parse_uci_int(tokens[i + 1]);
                    i += 1;
                }
            }
            "binc" => {
                if i + 1 < tokens.len() {
                    params.binc = parse_uci_int(tokens[i + 1]);
                    i += 1;
                }
            }
            "movestogo" => {
                if i + 1 < tokens.len() {
                    params.movestogo = parse_uci_int(tokens[i + 1]).map(|v| v.min(u32::MAX as u64) as u32);
                    i += 1;
                }
            }
            "depth" => {
                if i + 1 < tokens.len() {
                    params.depth = parse_uci_int(tokens[i + 1]).map(|v| v.min(u8::MAX as u64) as u8);
                    i += 1;
                }
            }
            "nodes" => {
                if i + 1 < tokens.len() {
                    params.nodes = parse_uci_int(tokens[i + 1]);
                    i += 1;
                }
            }
            "mate" => {
                if i + 1 < tokens.len() {
                    params.mate = parse_uci_int(tokens[i + 1]).map(|v| v.min(u32::MAX as u64) as u32);
                    i += 1;
                }
            }
            "movetime" => {
                if i + 1 < tokens.len() {
                    params.movetime = parse_uci_int(tokens[i + 1]);
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
    output.push_str(&format!(
        "option name Hash type spin default {} min {} max {}\n",
        HASH_DEFAULT_MB, HASH_MIN_MB, HASH_MAX_MB
    ));
    output.push_str("option name Threads type spin default 1 min 1 max 256\n");
    output.push_str(&format!(
        "option name Move Overhead type spin default {} min {} max {}\n",
        OVERHEAD_DEFAULT_MS, OVERHEAD_MIN_MS, OVERHEAD_MAX_MS
    ));
    output.push_str("option name Clear Hash type button\n");
    output.push_str("option name OwnBook type check default false\n");
    output.push_str("option name BookFile type string default <empty>\n");
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
    info.push_str(&format!(" hashfull {}", result.stats.hashfull));
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
    let bytes = s.as_bytes();

    if bytes.len() < 4 {
        return None;
    }

    // Strict ranges: files a-h, ranks 1-8. Anything else returns None
    // instead of panicking on arithmetic underflow.
    fn file(c: u8) -> Option<u8> {
        if (b'a'..=b'h').contains(&c) {
            Some(c - b'a')
        } else {
            None
        }
    }
    fn rank(c: u8) -> Option<u8> {
        if (b'1'..=b'8').contains(&c) {
            Some(c - b'1')
        } else {
            None
        }
    }

    let from = Square::from_coords(file(bytes[0])?, rank(bytes[1])?);
    let to = Square::from_coords(file(bytes[2])?, rank(bytes[3])?);

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
    fn parse_uci_int_tolerates_negative_and_overflow() {
        assert_eq!(parse_uci_int("42"), Some(42));
        assert_eq!(parse_uci_int("-5"), Some(0));
        assert_eq!(parse_uci_int("99999999999999999999999"), Some(u64::MAX));
        assert_eq!(parse_uci_int(""), None);
        assert_eq!(parse_uci_int("-"), None);
        assert_eq!(parse_uci_int("12ms"), None);
        assert_eq!(parse_uci_int("+3"), None);
    }

    #[test]
    fn go_with_negative_clock_keeps_the_time_control() {
        match parse_command("go wtime -5 btime -120 winc -1") {
            UciCommand::Go(p) => {
                assert_eq!((p.wtime, p.btime, p.winc), (Some(0), Some(0), Some(0)));
                assert!(!p.infinite);
            }
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn go_depth_saturates_instead_of_being_dropped() {
        match parse_command("go depth 300") {
            UciCommand::Go(p) => assert_eq!(p.depth, Some(u8::MAX)),
            _ => panic!("expected Go"),
        }
    }

    #[test]
    fn move_overhead_is_clamped_to_the_advertised_range() {
        assert_eq!(clamp_overhead_ms(0), 0);
        assert_eq!(clamp_overhead_ms(99_999), OVERHEAD_MAX_MS);
        assert_eq!(clamp_overhead_ms(25), 25);
    }

    #[test]
    fn the_uci_reply_advertises_overhead_and_clear_hash() {
        let id = format_uci_id();
        assert!(id.contains("option name Move Overhead type spin default 10 min 0 max 5000"), "{id}");
        assert!(id.contains("option name Clear Hash type button"), "{id}");
    }

    #[test]
    fn info_lines_carry_hashfull() {
        let mut result = crate::search::SearchResult::default();
        result.depth = 5;
        result.stats.hashfull = 123;
        let line = format_info(&result, 50);
        assert!(line.contains(" hashfull 123 time 50"), "{line}");
    }

    #[test]
    fn hash_is_clamped_to_the_advertised_range() {
        assert_eq!(clamp_hash_mb(0), HASH_MIN_MB);
        assert_eq!(clamp_hash_mb(100_000_000), HASH_MAX_MB);
        assert_eq!(clamp_hash_mb(128), 128);
    }

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
    fn test_parse_setoption_keeps_value_text_verbatim() {
        match parse_command("setoption name BookFile value /my  books/value  one.bin") {
            UciCommand::SetOption { name, value } => {
                assert_eq!(name, "BookFile");
                assert_eq!(value.as_deref(), Some("/my  books/value  one.bin"));
            }
            _ => panic!("Expected SetOption command"),
        }
        match parse_command("setoption name Move   Overhead value 50") {
            UciCommand::SetOption { name, value } => {
                assert_eq!(name, "Move Overhead");
                assert_eq!(value.as_deref(), Some("50"));
            }
            _ => panic!("Expected SetOption command"),
        }
        match parse_command("setoption name Clear Hash") {
            UciCommand::SetOption { name, value } => {
                assert_eq!((name.as_str(), value), ("Clear Hash", None));
            }
            _ => panic!("Expected SetOption command"),
        }
        assert!(matches!(parse_command("setoption nameless"), UciCommand::Unknown(_)));
    }

    #[test]
    fn test_format_uci_id() {
        let id = format_uci_id();
        assert!(id.contains("id name Lemonate"));
        assert!(id.contains("id author Toby"));
        assert!(id.contains("uciok"));
    }

    #[test]
    fn test_format_uci_id_advertises_threads() {
        let id = format_uci_id();
        assert!(
            id.contains("option name Threads type spin default 1 min 1 max 256"),
            "UCI id must advertise the Threads option, got:\n{}",
            id
        );
        // Pre-existing options keep their exact format.
        assert!(id.contains("option name Hash type spin default 64 min 1 max 4096"));
    }

    #[test]
    fn test_parse_uci_move_malformed_no_panic() {
        use crate::Board;
        let board = Board::starting_position();
        let legal = board.generate_legal_moves();

        // Rank 0 / digit files previously caused arithmetic underflow.
        for bad in ["e0e4", "e9e4", "1111", "xxxx", "e2", "", "i2i4", "e2e9"] {
            assert!(
                parse_uci_move(bad, &legal).is_none(),
                "{:?} should not parse",
                bad
            );
        }

        assert!(parse_uci_move("e2e4", &legal).is_some());
    }
}
