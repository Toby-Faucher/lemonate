use lemonate::bitboard::{Bitboard, Square};
use lemonate::magic::attacks::AttackTable;

fn main() {
    println!("=== AttackTable Tests ===\n");

    let attack_table = AttackTable::new();

    println!("\n=== Basic Attack Tests ===");

    let test_squares = [
        (Square::from_coords(0, 0), "a1"),
        (Square::from_coords(4, 3), "e4"),
        (Square::from_coords(7, 7), "h8"),
        (Square::from_coords(3, 3), "d4"),
    ];

    for (square, name) in test_squares {
        println!("\nTesting attacks from {}:", name);
        
        let rook_attacks = attack_table.rook_attacks(square, Bitboard::EMPTY);
        println!("Rook attacks (empty board):");
        println!("{}", rook_attacks);

        let bishop_attacks = attack_table.bishop_attacks(square, Bitboard::EMPTY);
        println!("Bishop attacks (empty board):");
        println!("{}", bishop_attacks);

        let queen_attacks = attack_table.queen_attacks(square, Bitboard::EMPTY);
        println!("Queen attacks (empty board):");
        println!("{}", queen_attacks);
    }

    println!("\n=== Blocker Tests ===");

    let center_square = Square::from_coords(4, 4); // e5
    let mut blockers = Bitboard::EMPTY;
    blockers.set(Square::from_coords(4, 6)); // e7
    blockers.set(Square::from_coords(6, 4)); // g5
    blockers.set(Square::from_coords(2, 2)); // c3

    println!("Testing from e5 with blockers:");
    println!("Blockers pattern:");
    println!("{}", blockers);

    let rook_blocked = attack_table.rook_attacks(center_square, blockers);
    println!("\nRook attacks with blockers:");
    println!("{}", rook_blocked);

    let bishop_blocked = attack_table.bishop_attacks(center_square, blockers);
    println!("Bishop attacks with blockers:");
    println!("{}", bishop_blocked);

    let queen_blocked = attack_table.queen_attacks(center_square, blockers);
    println!("Queen attacks with blockers:");
    println!("{}", queen_blocked);

    println!("\n=== Performance Test ===");

    let start = std::time::Instant::now();
    let mut total_attacks = 0u64;

    for square_idx in 0..64 {
        let square = Square::from_index(square_idx);
        let rook = attack_table.rook_attacks(square, blockers);
        let bishop = attack_table.bishop_attacks(square, blockers);
        total_attacks += rook.count_pieces() as u64;
        total_attacks += bishop.count_pieces() as u64;
    }

    let duration = start.elapsed();
    println!("Calculated attacks for all 64 squares in {:?}", duration);
    println!("Total attack squares: {}", total_attacks);

    println!("\nAttackTable tests completed successfully!");
}
