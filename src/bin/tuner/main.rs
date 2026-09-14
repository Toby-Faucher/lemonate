mod dataset;
mod optimizer;

use dataset::load_dataset;
use lemonate::EvalWeights;
use optimizer::{coordinate_descent, fit_k, format_weights};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: tuner <dataset.epd> [max_sweeps]");
        std::process::exit(1);
    }
    let dataset_path = &args[1];
    let max_sweeps: usize = args
        .get(2)
        .map(|s| s.parse().expect("max_sweeps must be a non-negative integer"))
        .unwrap_or(50);

    println!("loading dataset from {dataset_path}...");
    let positions = load_dataset(dataset_path);
    println!("loaded {} positions", positions.len());
    if positions.is_empty() {
        eprintln!("no usable positions loaded, aborting");
        std::process::exit(1);
    }

    println!("fitting K...");
    let k = fit_k(&positions, &EvalWeights::DEFAULT);
    println!("K = {k:.4}");

    println!("running coordinate descent (max {max_sweeps} sweeps)...");
    let (tuned, final_error) =
        coordinate_descent(&positions, &EvalWeights::DEFAULT, k, max_sweeps);
    println!("final error: {final_error:.6}");

    println!("\n--- paste the following into pst.rs / pawn_structure.rs / king_safety.rs / mobility.rs ---\n");
    print!("{}", format_weights(&tuned));
}

#[cfg(test)]
mod tests {
    use super::*;
    use dataset::LabeledPosition;
    use lemonate::Board;

    #[test]
    fn pipeline_reduces_or_maintains_error_on_synthetic_dataset() {
        let fens_and_results = [
            ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 0.5),
            (
                "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
                1.0,
            ),
            (
                "rnbqkb1r/pppp1ppp/5n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
                0.5,
            ),
            ("8/8/8/8/8/4k3/8/4K3 w - - 0 1", 0.5),
            ("r3k2r/ppp2ppp/8/8/8/8/PPP2PPP/R3K2R w KQkq - 0 1", 0.5),
        ];

        let mut positions = Vec::new();
        for (fen, result) in fens_and_results {
            let board = Board::from_fen(fen).unwrap();
            positions.push(LabeledPosition { board, result });
        }

        let k = fit_k(&positions, &EvalWeights::DEFAULT);
        let initial_error = optimizer::mean_squared_error(&positions, &EvalWeights::DEFAULT, k);
        let (tuned, final_error) = coordinate_descent(&positions, &EvalWeights::DEFAULT, k, 3);

        assert!(
            final_error <= initial_error,
            "error increased: {initial_error} -> {final_error}"
        );

        let output = format_weights(&tuned);
        assert!(output.contains("MG_PAWN_TABLE"));
        assert!(output.contains("QUEEN_MOBILITY_EG"));
        assert_eq!(output.lines().count(), 38);
    }

    #[test]
    fn pipeline_round_trips_through_a_real_epd_file() {
        let sample = concat!(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"1-0\";\n",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 c9 \"0-1\";\n",
            "8/8/8/8/8/4k3/8/4K3 w - - 0 1 c9 \"1/2-1/2\";\n",
        );
        let dir = std::env::temp_dir();
        let path = dir.join("lemonate_tuner_pipeline_test.epd");
        std::fs::write(&path, sample).unwrap();

        let positions = load_dataset(path.to_str().unwrap());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(positions.len(), 3);

        let k = fit_k(&positions, &EvalWeights::DEFAULT);
        let (_tuned, final_error) = coordinate_descent(&positions, &EvalWeights::DEFAULT, k, 1);
        assert!(final_error.is_finite());
    }
}
