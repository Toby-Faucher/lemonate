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
    while seen < bestmoves {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(l) => {
                if l.starts_with("bestmove") {
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
