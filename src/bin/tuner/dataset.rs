use lemonate::Board;

/// One labeled training position: a board plus its game result from
/// White's perspective (1.0 = White won, 0.5 = draw, 0.0 = Black won).
pub struct LabeledPosition {
    pub board: Board,
    pub result: f64,
}

/// Parses a quiet-labeled EPD dataset (zurichess-tuner format:
/// `<FEN> c9 "<result>";` per line, e.g.
/// `... w - - 0 1 c9 "1-0";`). Blank lines and lines starting with `#`
/// are skipped; lines that fail to parse are skipped with a warning
/// rather than aborting the whole load.
pub fn load_dataset(path: &str) -> Vec<LabeledPosition> {
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read dataset {path}: {e}"));

    let mut positions = Vec::new();
    for (line_no, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse_line(line) {
            Some(pos) => positions.push(pos),
            None => eprintln!("skipping unparseable line {}: {line}", line_no + 1),
        }
    }
    positions
}

fn parse_line(line: &str) -> Option<LabeledPosition> {
    let marker = "c9 \"";
    let marker_idx = line.find(marker)?;
    let fen = line[..marker_idx].trim();

    let after_marker = &line[marker_idx + marker.len()..];
    let end_quote = after_marker.find('"')?;
    let result_str = &after_marker[..end_quote];

    let result = match result_str {
        "1-0" => 1.0,
        "0-1" => 0.0,
        "1/2-1/2" => 0.5,
        _ => return None,
    };

    let board = Board::from_fen(fen).ok()?;
    Some(LabeledPosition { board, result })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_lines() {
        let sample = concat!(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"1-0\";\n",
            "8/8/8/8/8/4k3/8/4K3 w - - 0 1 c9 \"1/2-1/2\";\n",
            "8/8/8/8/8/4k3/8/4K3 b - - 0 1 c9 \"0-1\";\n",
        );
        let dir = std::env::temp_dir();
        let path = dir.join("lemonate_tuner_dataset_test_valid.epd");
        std::fs::write(&path, sample).unwrap();

        let positions = load_dataset(path.to_str().unwrap());

        std::fs::remove_file(&path).unwrap();

        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0].result, 1.0);
        assert_eq!(positions[1].result, 0.5);
        assert_eq!(positions[2].result, 0.0);
    }

    #[test]
    fn skips_blank_and_comment_lines() {
        let sample = concat!(
            "\n",
            "# comment\n",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"1-0\";\n",
        );
        let dir = std::env::temp_dir();
        let path = dir.join("lemonate_tuner_dataset_test_blanks.epd");
        std::fs::write(&path, sample).unwrap();

        let positions = load_dataset(path.to_str().unwrap());

        std::fs::remove_file(&path).unwrap();

        assert_eq!(positions.len(), 1);
    }

    #[test]
    fn skips_unparseable_lines_without_panicking() {
        let sample = concat!(
            "not a real epd line\n",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"1-0\";\n",
        );
        let dir = std::env::temp_dir();
        let path = dir.join("lemonate_tuner_dataset_test_garbage.epd");
        std::fs::write(&path, sample).unwrap();

        let positions = load_dataset(path.to_str().unwrap());

        std::fs::remove_file(&path).unwrap();

        assert_eq!(positions.len(), 1);
    }
}
