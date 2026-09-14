mod dataset;
mod optimizer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: tuner <dataset.epd>");
        std::process::exit(1);
    }
    let positions = dataset::load_dataset(&args[1]);
    println!("loaded {} positions", positions.len());
}
