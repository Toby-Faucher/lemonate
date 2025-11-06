use lemonate::magic::AttackTable;

fn main() {
    println!("Testing magic number generation...");
    let start = std::time::Instant::now();

    let _table = AttackTable::new();

    let elapsed = start.elapsed();
    println!("\nSuccess! Magic numbers generated in {:.2?}", elapsed);
}
