use crate::dataset::LabeledPosition;
use lemonate::{EvalWeights, Evaluator};

/// Formats every tunable value in `weights` as ready-to-paste Rust
/// `pub const` blocks, using the same names and shapes as the
/// originals in `pst.rs`/`pawn_structure.rs`/`king_safety.rs`/
/// `mobility.rs`.
pub fn format_weights(weights: &EvalWeights) -> String {
    let mut out = String::new();

    let pst_tables: [(&str, &[i32; 64]); 12] = [
        ("MG_PAWN_TABLE", &weights.pst_mg[0]),
        ("MG_KNIGHT_TABLE", &weights.pst_mg[1]),
        ("MG_BISHOP_TABLE", &weights.pst_mg[2]),
        ("MG_ROOK_TABLE", &weights.pst_mg[3]),
        ("MG_QUEEN_TABLE", &weights.pst_mg[4]),
        ("MG_KING_TABLE", &weights.pst_mg[5]),
        ("EG_PAWN_TABLE", &weights.pst_eg[0]),
        ("EG_KNIGHT_TABLE", &weights.pst_eg[1]),
        ("EG_BISHOP_TABLE", &weights.pst_eg[2]),
        ("EG_ROOK_TABLE", &weights.pst_eg[3]),
        ("EG_QUEEN_TABLE", &weights.pst_eg[4]),
        ("EG_KING_TABLE", &weights.pst_eg[5]),
    ];
    for (name, table) in pst_tables {
        out.push_str(&format_array(name, table));
    }

    out.push_str(&format_scalar("DOUBLED_PAWN_MG", weights.doubled_pawn_mg));
    out.push_str(&format_scalar("DOUBLED_PAWN_EG", weights.doubled_pawn_eg));
    out.push_str(&format_scalar("ISOLATED_PAWN_MG", weights.isolated_pawn_mg));
    out.push_str(&format_scalar("ISOLATED_PAWN_EG", weights.isolated_pawn_eg));
    out.push_str(&format_scalar("BACKWARD_PAWN_MG", weights.backward_pawn_mg));
    out.push_str(&format_scalar("BACKWARD_PAWN_EG", weights.backward_pawn_eg));
    out.push_str(&format_array("PASSED_PAWN_MG", &weights.passed_pawn_mg));
    out.push_str(&format_array("PASSED_PAWN_EG", &weights.passed_pawn_eg));
    out.push_str(&format_scalar("CONNECTED_PAWN_MG", weights.connected_pawn_mg));
    out.push_str(&format_scalar("CONNECTED_PAWN_EG", weights.connected_pawn_eg));

    out.push_str(&format_scalar("PAWN_SHIELD_CLOSE_MG", weights.pawn_shield_close_mg));
    out.push_str(&format_scalar("PAWN_SHIELD_CLOSE_EG", weights.pawn_shield_close_eg));
    out.push_str(&format_scalar("PAWN_SHIELD_FAR_MG", weights.pawn_shield_far_mg));
    out.push_str(&format_scalar("PAWN_SHIELD_FAR_EG", weights.pawn_shield_far_eg));
    out.push_str(&format_scalar(
        "OPEN_FILE_NEAR_KING_MG",
        weights.open_file_near_king_mg,
    ));
    out.push_str(&format_scalar(
        "OPEN_FILE_NEAR_KING_EG",
        weights.open_file_near_king_eg,
    ));
    out.push_str(&format_scalar(
        "SEMI_OPEN_FILE_NEAR_KING_MG",
        weights.semi_open_file_near_king_mg,
    ));
    out.push_str(&format_scalar(
        "SEMI_OPEN_FILE_NEAR_KING_EG",
        weights.semi_open_file_near_king_eg,
    ));

    out.push_str(&format_array("KNIGHT_MOBILITY_MG", &weights.knight_mobility_mg));
    out.push_str(&format_array("KNIGHT_MOBILITY_EG", &weights.knight_mobility_eg));
    out.push_str(&format_array("BISHOP_MOBILITY_MG", &weights.bishop_mobility_mg));
    out.push_str(&format_array("BISHOP_MOBILITY_EG", &weights.bishop_mobility_eg));
    out.push_str(&format_array("ROOK_MOBILITY_MG", &weights.rook_mobility_mg));
    out.push_str(&format_array("ROOK_MOBILITY_EG", &weights.rook_mobility_eg));
    out.push_str(&format_array("QUEEN_MOBILITY_MG", &weights.queen_mobility_mg));
    out.push_str(&format_array("QUEEN_MOBILITY_EG", &weights.queen_mobility_eg));

    out
}

fn format_scalar(name: &str, value: i32) -> String {
    format!("pub const {name}: i32 = {value};\n")
}

fn format_array(name: &str, values: &[i32]) -> String {
    let body = values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("pub const {name}: [i32; {}] = [{body}];\n", values.len())
}

pub fn sigmoid(eval: f64, k: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-k * eval / 400.0))
}

/// Mean squared error between `sigmoid(k * eval)` and each position's
/// game result, scoring every position with `weights`. Splits the
/// dataset across the available CPU cores using `std::thread::scope`
/// (no external dependency) since this runs on every optimizer trial.
pub fn mean_squared_error(positions: &[LabeledPosition], weights: &EvalWeights, k: f64) -> f64 {
    if positions.is_empty() {
        return 0.0;
    }

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(positions.len());

    if num_threads <= 1 {
        return mean_squared_error_chunk(positions, weights, k) / positions.len() as f64;
    }

    let chunk_size = positions.len().div_ceil(num_threads);
    let total: f64 = std::thread::scope(|scope| {
        let handles: Vec<_> = positions
            .chunks(chunk_size)
            .map(|chunk| scope.spawn(|| mean_squared_error_chunk(chunk, weights, k)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    });

    total / positions.len() as f64
}

fn mean_squared_error_chunk(positions: &[LabeledPosition], weights: &EvalWeights, k: f64) -> f64 {
    let evaluator = Evaluator::new();
    let mut total = 0.0;
    for pos in positions {
        let eval = evaluator.static_eval_white_pov(&pos.board, weights) as f64;
        let predicted = sigmoid(eval, k);
        let diff = pos.result - predicted;
        total += diff * diff;
    }
    total
}

/// Coarse-to-fine grid search for the sigmoid scaling constant `K`
/// that best fits `weights` to the dataset's game results.
pub fn fit_k(positions: &[LabeledPosition], weights: &EvalWeights) -> f64 {
    let mut best_k = 1.0;
    let mut best_error = mean_squared_error(positions, weights, best_k);

    let mut low = 0.1;
    let mut high = 2.0;
    for _ in 0..6 {
        let step = (high - low) / 20.0;
        let mut k = low;
        while k <= high {
            let error = mean_squared_error(positions, weights, k);
            if error < best_error {
                best_error = error;
                best_k = k;
            }
            k += step;
        }
        low = (best_k - step * 2.0).max(0.001);
        high = best_k + step * 2.0;
    }

    best_k
}

/// Classic Texel-style local search: for each tunable scalar, try +1
/// then -1, keeping whichever (if any) reduces the dataset's mean
/// squared error. Repeats full sweeps over every parameter until a
/// sweep makes no improvement, or `max_sweeps` is reached.
pub fn coordinate_descent(
    positions: &[LabeledPosition],
    initial: &EvalWeights,
    k: f64,
    max_sweeps: usize,
) -> (EvalWeights, f64) {
    let mut params = initial.to_vec();
    let mut best_error = mean_squared_error(positions, &EvalWeights::from_vec(&params), k);

    for sweep in 0..max_sweeps {
        let mut improved_this_sweep = false;

        for i in 0..params.len() {
            let original = params[i];

            params[i] = original + 1;
            let mut error = mean_squared_error(positions, &EvalWeights::from_vec(&params), k);
            if error < best_error {
                best_error = error;
                improved_this_sweep = true;
                continue;
            }

            params[i] = original - 1;
            error = mean_squared_error(positions, &EvalWeights::from_vec(&params), k);
            if error < best_error {
                best_error = error;
                improved_this_sweep = true;
                continue;
            }

            params[i] = original;
        }

        eprintln!("sweep {}: error = {:.6}", sweep + 1, best_error);
        if !improved_this_sweep {
            break;
        }
    }

    (EvalWeights::from_vec(&params), best_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lemonate::Board;

    #[test]
    fn mse_is_near_zero_for_drawn_symmetric_position() {
        // Starting position evaluates to 0 (symmetric), so
        // sigmoid(0, k) = 0.5 for any k - matching a labeled draw
        // (result = 0.5) exactly.
        let board = Board::starting_position();
        let positions = vec![
            LabeledPosition {
                board: board.clone(),
                result: 0.5,
            },
            LabeledPosition { board, result: 0.5 },
        ];
        let error = mean_squared_error(&positions, &EvalWeights::DEFAULT, 1.0);
        assert!(error < 1e-9, "expected ~0 error, got {error}");
    }

    #[test]
    fn mse_is_positive_for_mismatched_predictions() {
        // Starting position (eval 0, predicted 0.5) labeled as a White
        // win (result 1.0) - a deliberate mismatch, so error must be
        // clearly positive.
        let board = Board::starting_position();
        let positions = vec![LabeledPosition { board, result: 1.0 }];
        let error = mean_squared_error(&positions, &EvalWeights::DEFAULT, 1.0);
        assert!(error > 0.2, "expected clearly positive error, got {error}");
    }

    #[test]
    fn fit_k_stays_in_search_range() {
        let board = Board::starting_position();
        let positions = vec![LabeledPosition { board, result: 0.5 }];
        let k = fit_k(&positions, &EvalWeights::DEFAULT);
        assert!(k > 0.0 && k < 3.0, "k out of expected range: {k}");
    }

    #[test]
    fn coordinate_descent_never_increases_error() {
        let board = Board::starting_position();
        let mut positions = Vec::new();
        for i in 0..10 {
            let result = if i % 2 == 0 { 0.5 } else { 0.6 };
            positions.push(LabeledPosition {
                board: board.clone(),
                result,
            });
        }

        let k = fit_k(&positions, &EvalWeights::DEFAULT);
        let initial_error = mean_squared_error(&positions, &EvalWeights::DEFAULT, k);
        let (_tuned, final_error) = coordinate_descent(&positions, &EvalWeights::DEFAULT, k, 3);

        assert!(
            final_error <= initial_error,
            "error increased: {initial_error} -> {final_error}"
        );
    }

    #[test]
    fn coordinate_descent_with_zero_sweeps_is_a_no_op() {
        let board = Board::starting_position();
        let positions = vec![LabeledPosition { board, result: 0.5 }];
        let k = 1.0;
        let initial_error = mean_squared_error(&positions, &EvalWeights::DEFAULT, k);

        let (tuned, final_error) = coordinate_descent(&positions, &EvalWeights::DEFAULT, k, 0);

        assert_eq!(final_error, initial_error);
        assert_eq!(tuned.to_vec(), EvalWeights::DEFAULT.to_vec());
    }

    #[test]
    fn parallel_mse_matches_sequential_reference() {
        let board = Board::starting_position();
        let mut positions = Vec::new();
        for i in 0..50 {
            let result = match i % 3 {
                0 => 1.0,
                1 => 0.0,
                _ => 0.5,
            };
            positions.push(LabeledPosition {
                board: board.clone(),
                result,
            });
        }

        let evaluator = Evaluator::new();
        let mut sequential_total = 0.0;
        for pos in &positions {
            let eval = evaluator.static_eval_white_pov(&pos.board, &EvalWeights::DEFAULT) as f64;
            let predicted = sigmoid(eval, 1.0);
            let diff = pos.result - predicted;
            sequential_total += diff * diff;
        }
        let sequential_mse = sequential_total / positions.len() as f64;

        let parallel_mse = mean_squared_error(&positions, &EvalWeights::DEFAULT, 1.0);
        assert!(
            (sequential_mse - parallel_mse).abs() < 1e-12,
            "sequential {sequential_mse} vs parallel {parallel_mse}"
        );
    }

    #[test]
    fn format_weights_contains_all_const_names() {
        let output = format_weights(&EvalWeights::DEFAULT);
        for name in [
            "MG_PAWN_TABLE",
            "EG_KING_TABLE",
            "DOUBLED_PAWN_MG",
            "PASSED_PAWN_EG",
            "PAWN_SHIELD_CLOSE_MG",
            "SEMI_OPEN_FILE_NEAR_KING_EG",
            "KNIGHT_MOBILITY_MG",
            "QUEEN_MOBILITY_EG",
        ] {
            assert!(output.contains(name), "missing {name} in output:\n{output}");
        }
    }

    #[test]
    fn format_weights_has_expected_line_count() {
        // 12 PST arrays + 10 pawn-structure consts + 8 king-safety
        // scalars + 8 mobility arrays = 38 lines.
        let output = format_weights(&EvalWeights::DEFAULT);
        assert_eq!(output.lines().count(), 38);
    }

    #[test]
    fn format_array_round_trips_through_rust_syntax() {
        let formatted = format_array("TEST_ARRAY", &[1, -2, 3]);
        assert_eq!(formatted, "pub const TEST_ARRAY: [i32; 3] = [1, -2, 3];\n");
    }
}
