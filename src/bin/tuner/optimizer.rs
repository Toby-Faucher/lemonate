use crate::dataset::LabeledPosition;
use lemonate::{EvalWeights, Evaluator};

pub fn sigmoid(eval: f64, k: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-k * eval / 400.0))
}

/// Mean squared error between `sigmoid(k * eval)` and each position's
/// game result, scoring every position with `weights`.
pub fn mean_squared_error(positions: &[LabeledPosition], weights: &EvalWeights, k: f64) -> f64 {
    let evaluator = Evaluator::new();
    let mut total = 0.0;
    for pos in positions {
        let eval = evaluator.static_eval_white_pov(&pos.board, weights) as f64;
        let predicted = sigmoid(eval, k);
        let diff = pos.result - predicted;
        total += diff * diff;
    }
    total / positions.len() as f64
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
}
