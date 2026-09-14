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
