//! End-to-end UCI tests: drive the real binary over stdin/stdout.
//! These cover protocol edge cases that used to crash or hang the engine.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Feed `script` to a fresh engine and collect stdout until `bestmoves` "bestmove" lines
/// have been seen (or 20 s pass), then quit it. Returns all output lines.
fn run(script: &str, bestmoves: usize) -> Vec<String> {
    run_until(script, "bestmove", bestmoves)
}

/// Like [`run`], but waits for `count` lines starting with `marker` (e.g. "uciok").
fn run_until(script: &str, marker: &str, count: usize) -> Vec<String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lemonate"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn engine");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    stdin.write_all(script.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let mut lines = Vec::new();
    let mut seen = 0;
    let deadline = Instant::now() + Duration::from_secs(20);
    while seen < count {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(l) => {
                if l.starts_with(marker) {
                    seen += 1;
                }
                lines.push(l);
            }
            Err(_) => break,
        }
    }
    let _ = stdin.write_all(b"quit\n");
    let _ = stdin.flush();
    drop(stdin);
    let _ = child.wait();
    lines
}

fn bestmove(lines: &[String]) -> Option<&str> {
    lines
        .iter()
        .find(|l| l.starts_with("bestmove"))
        .and_then(|l| l.split_whitespace().nth(1))
}

#[test]
fn go_depth_zero_returns_a_move() {
    let out = run("position startpos\ngo depth 0\n", 1);
    let mv = bestmove(&out).expect("engine must answer go depth 0, not crash");
    assert_ne!(mv, "0000");
}

#[test]
fn tiny_clocks_still_return_a_legal_move() {
    for t in [0u64, 1, 5, 30] {
        let out = run(&format!("position startpos\ngo wtime {t} btime {t}\n"), 1);
        let mv = bestmove(&out).unwrap_or_else(|| panic!("no bestmove for wtime {t}"));
        assert_ne!(mv, "0000", "wtime {t} produced bestmove 0000");
    }
}

#[test]
fn movetime_zero_returns_a_legal_move() {
    let out = run("position startpos\ngo movetime 0\n", 1);
    assert_ne!(bestmove(&out).expect("bestmove"), "0000");
}

#[test]
fn negative_clock_does_not_become_an_infinite_search() {
    let out = run("position startpos\ngo wtime -5 btime -5 winc -1\n", 1);
    assert_ne!(bestmove(&out).expect("a flagged clock must still get an answer"), "0000");
}

#[test]
fn illegal_move_keeps_previous_position_and_reports() {
    // After 1.e4 it is Black's turn; the second position command is illegal and must be
    // rejected, leaving the 1.e4 position (so the best move is a Black move).
    let out = run(
        "position startpos moves e2e4\nposition startpos moves e2e5\ngo depth 2\n",
        1,
    );
    assert!(
        out.iter().any(|l| l.starts_with("info string error")),
        "expected an info string error, got {out:?}"
    );
    let mv = bestmove(&out).expect("bestmove");
    let from_rank = mv.as_bytes()[1];
    assert!(from_rank == b'7' || from_rank == b'8', "expected a Black move, got {mv}");
}

#[test]
fn invalid_fen_keeps_previous_position_and_reports() {
    let out = run("position startpos moves e2e4\nposition fen garbage\ngo depth 2\n", 1);
    assert!(out.iter().any(|l| l.starts_with("info string error")), "{out:?}");
    let from_rank = bestmove(&out).expect("bestmove").as_bytes()[1];
    assert!(from_rank == b'7' || from_rank == b'8');
}

#[test]
fn stalemated_root_reports_no_move() {
    let out = run("position fen 7k/5Q2/6K1/8/8/8/8/8 b - - 0 1\ngo depth 3\n", 1);
    assert_eq!(bestmove(&out), Some("0000"));
    assert!(out.iter().any(|l| l.contains("score cp 0")), "{out:?}");
}

#[test]
fn checkmated_root_reports_mate_zero() {
    let out = run("position fen 7k/6Q1/6K1/8/8/8/8/8 b - - 0 1\ngo depth 3\n", 1);
    assert_eq!(bestmove(&out), Some("0000"));
    assert!(out.iter().any(|l| l.contains("score mate 0")), "{out:?}");
}

#[test]
fn single_thread_default_does_not_need_a_shared_table() {
    // Threads 2 builds the shared table lazily and must still work end to end.
    let out = run("setoption name Threads value 2\nposition startpos\ngo depth 4\n", 1);
    assert_ne!(bestmove(&out).expect("bestmove"), "0000");
}

// ---- opening book (OwnBook / BookFile) ----

/// Write a minimal Polyglot book (16-byte big-endian entries, sorted by key) and return its path.
fn temp_book(name: &str, entries: &[(u64, u16, u16)]) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| e.0);
    let mut bytes = Vec::new();
    for (key, raw_move, weight) in sorted {
        bytes.extend_from_slice(&key.to_be_bytes());
        bytes.extend_from_slice(&raw_move.to_be_bytes());
        bytes.extend_from_slice(&weight.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
    }
    let path = std::env::temp_dir().join(format!("lemonate-test-{}-{}.bin", name, std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().into_owned()
}

/// Polyglot move word: to file, to rank, from file, from rank (no promotion).
fn raw_move(from: (u16, u16), to: (u16, u16)) -> u16 {
    to.0 | (to.1 << 3) | (from.0 << 6) | (from.1 << 9)
}

fn start_key() -> u64 {
    lemonate::book::polyglot_hash(&lemonate::Board::starting_position())
}

fn start_book(name: &str) -> String {
    // 1.e4 (weight 10) and 1.d4 (weight 5) from the start position.
    temp_book(
        name,
        &[
            (start_key(), raw_move((4, 1), (4, 3)), 10),
            (start_key(), raw_move((3, 1), (3, 3)), 5),
        ],
    )
}

fn searched(lines: &[String]) -> bool {
    lines.iter().any(|l| l.starts_with("info depth"))
}

#[test]
fn own_book_plays_a_book_move_without_searching() {
    let book = start_book("plays");
    let out = run(
        &format!("setoption name BookFile value {book}\nsetoption name OwnBook value true\nposition startpos\ngo wtime 60000 btime 60000\n"),
        1,
    );
    let mv = bestmove(&out).expect("bestmove");
    assert!(mv == "e2e4" || mv == "d2d4", "not a book move: {mv}");
    assert!(out.iter().any(|l| l == "info string book move"), "{out:?}");
    assert!(!searched(&out), "a book move must not start a search: {out:?}");
}

#[test]
fn own_book_is_off_by_default() {
    let book = start_book("off");
    let out = run(
        &format!("setoption name BookFile value {book}\nposition startpos\ngo depth 3\n"),
        1,
    );
    assert!(searched(&out));
    let out = run(
        &format!("setoption name BookFile value {book}\nposition startpos\ngo wtime 2000 btime 2000\n"),
        1,
    );
    assert!(searched(&out), "OwnBook was never enabled: {out:?}");
}

#[test]
fn analysis_requests_bypass_the_book() {
    let book = start_book("analysis");
    for go in ["go depth 3", "go nodes 2000", "go infinite\nstop"] {
        let out = run(
            &format!("setoption name BookFile value {book}\nsetoption name OwnBook value true\nposition startpos\n{go}\n"),
            1,
        );
        assert!(searched(&out), "'{go}' must search, got {out:?}");
    }
}

#[test]
fn out_of_book_positions_fall_back_to_search() {
    let book = start_book("out");
    let out = run(
        &format!("setoption name BookFile value {book}\nsetoption name OwnBook value true\nposition startpos moves e2e4 e7e5\ngo wtime 2000 btime 2000\n"),
        1,
    );
    assert!(searched(&out), "{out:?}");
    assert_ne!(bestmove(&out), Some("0000"));
}

#[test]
fn an_illegal_book_entry_is_ignored() {
    // a1a8 is not a legal first move; the engine must search instead of playing it.
    let book = temp_book("illegal", &[(start_key(), raw_move((0, 0), (0, 7)), 100)]);
    let out = run(
        &format!("setoption name BookFile value {book}\nsetoption name OwnBook value true\nposition startpos\ngo wtime 2000 btime 2000\n"),
        1,
    );
    assert!(searched(&out), "{out:?}");
    assert_ne!(bestmove(&out), Some("a1a8"));
}

#[test]
fn a_missing_book_file_is_reported_and_play_continues() {
    let out = run(
        "setoption name BookFile value /nonexistent/book.bin\nsetoption name OwnBook value true\nposition startpos\ngo depth 2\n",
        1,
    );
    assert!(out.iter().any(|l| l.starts_with("info string error")), "{out:?}");
    assert!(searched(&out));
}

#[test]
fn the_book_options_are_advertised() {
    let out = run_until("uci\n", "uciok", 1);
    assert!(out.iter().any(|l| l == "option name OwnBook type check default false"), "{out:?}");
    assert!(out.iter().any(|l| l == "option name BookFile type string default <empty>"), "{out:?}");
}

// ---- Move Overhead, Clear Hash, streamed info ----

fn info_depths(lines: &[String]) -> Vec<u32> {
    lines
        .iter()
        .filter(|l| l.starts_with("info depth"))
        .filter_map(|l| l.split_whitespace().nth(2)?.parse().ok())
        .collect()
}

fn info_field(line: &str, key: &str) -> Option<u64> {
    let mut it = line.split_whitespace();
    while let Some(t) = it.next() {
        if t == key {
            return it.next()?.parse().ok();
        }
    }
    None
}

#[test]
fn info_lines_stream_while_the_search_is_still_running() {
    // `go infinite` never ends by itself, so three info lines before any bestmove prove they
    // are sent per iteration, not once at the end.
    let out = run_until("position startpos\ngo infinite\n", "info depth", 3);
    let depths = info_depths(&out);
    assert!(depths.len() >= 3, "{out:?}");
    assert!(depths.windows(2).all(|w| w[0] <= w[1]), "depths should not go backwards: {depths:?}");
    assert!(!out.iter().any(|l| l.starts_with("bestmove")), "{out:?}");
}

#[test]
fn a_fixed_depth_search_reports_every_depth_and_the_hash_fill() {
    let out = run("position startpos\ngo depth 6\n", 1);
    let depths = info_depths(&out);
    for d in 1..=6 {
        assert!(depths.contains(&d), "missing depth {d} in {depths:?}");
    }
    assert!(
        out.iter().filter(|l| l.starts_with("info depth")).all(|l| info_field(l, "hashfull").is_some()),
        "{out:?}"
    );
}

#[test]
fn a_huge_move_overhead_still_answers_with_a_move() {
    let out = run("setoption name Move Overhead value 5000\nposition startpos\ngo movetime 200\n", 1);
    assert_ne!(bestmove(&out).expect("bestmove"), "0000");
}

#[test]
fn move_overhead_is_taken_off_the_search_time() {
    // 300 ms of movetime with 250 ms reserved leaves about 50 ms; without the reserve the
    // search runs for most of the 300 ms.
    let reserved = run("setoption name Move Overhead value 250\nposition startpos\ngo movetime 300\n", 1);
    let free = run("setoption name Move Overhead value 0\nposition startpos\ngo movetime 300\n", 1);
    let time = |lines: &[String]| {
        lines
            .iter()
            .filter(|l| l.starts_with("info depth"))
            .filter_map(|l| info_field(l, "time"))
            .max()
            .expect("an info line with a time")
    };
    assert!(time(&reserved) < 150, "reserved: {}", time(&reserved));
    assert!(time(&free) > 150, "free: {}", time(&free));
}

#[test]
fn clear_hash_is_accepted_and_play_continues() {
    // Options arrive while the engine is idle, so clear between a finished search and the next.
    let out = run(
        "position startpos\nsetoption name Clear Hash\nsetoption name Threads value 2\nsetoption name Clear Hash\ngo depth 4\n",
        1,
    );
    assert!(!out.iter().any(|l| l.starts_with("info string error")), "{out:?}");
    assert_ne!(bestmove(&out).expect("bestmove"), "0000");
}

#[test]
fn the_new_options_are_advertised() {
    let out = run_until("uci\n", "uciok", 1);
    assert!(out.iter().any(|l| l == "option name Move Overhead type spin default 10 min 0 max 5000"), "{out:?}");
    assert!(out.iter().any(|l| l == "option name Clear Hash type button"), "{out:?}");
}
