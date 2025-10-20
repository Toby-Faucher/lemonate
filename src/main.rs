use lemonate::bitboard::{Bitboard, Square};
use lemonate::magic::attacks::AttackTable;
use lemonate::types::piece::Color;

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

        let knight_attacks = attack_table.knight_attacks(square);
        println!("Knight attacks:");
        println!("{}", knight_attacks);

        let king_attacks = attack_table.king_attacks(square);
        println!("King attacks:");
        println!("{}", king_attacks);
    }

    println!("\n=== Knight and King Pattern Tests ===");

    let special_squares = [
        (Square::from_coords(1, 1), "b2 (near corner)"),
        (Square::from_coords(4, 4), "e5 (center)"),
        (Square::from_coords(0, 7), "a8 (corner)"),
        (Square::from_coords(7, 0), "h1 (corner)"),
    ];

    for (square, name) in special_squares {
        println!("\nTesting {} patterns:", name);

        let knight_attacks = attack_table.knight_attacks(square);
        println!("Knight attacks from {}:", name);
        println!("{}", knight_attacks);
        println!(
            "Knight can attack {} squares",
            knight_attacks.count_pieces()
        );

        let king_attacks = attack_table.king_attacks(square);
        println!("King attacks from {}:", name);
        println!("{}", king_attacks);
        println!("King can attack {} squares", king_attacks.count_pieces());
    }

    println!("\n=== Performance Test ===");

    let start = std::time::Instant::now();
    let mut total_attacks = 0u64;

    for square_idx in 0..64 {
        let square = Square::from_index(square_idx);
        let knight = attack_table.knight_attacks(square);
        let king = attack_table.king_attacks(square);
        total_attacks += knight.count_pieces() as u64;
        total_attacks += king.count_pieces() as u64;
    }

    let duration = start.elapsed();
    println!(
        "Calculated knight and king attacks for all 64 squares in {:?}",
        duration
    );
    println!("Total attack squares: {}", total_attacks);

    println!("\n=== Pawn Attack Tests ===");

    let pawn_test_squares = [
        (Square::from_coords(0, 1), "a2 (white starting)"),
        (Square::from_coords(4, 3), "e4 (center)"),
        (Square::from_coords(7, 6), "h7 (black starting)"),
        (Square::from_coords(0, 0), "a1 (corner)"),
        (Square::from_coords(7, 7), "h8 (corner)"),
        (Square::from_coords(3, 6), "d7 (near promotion)"),
    ];

    for (square, name) in pawn_test_squares {
        println!("\nTesting pawn attacks from {}:", name);

        let white_pawn_attacks = attack_table.pawn_attacks(square, Color::White);
        println!("White pawn attacks:");
        println!("{}", white_pawn_attacks);
        println!(
            "White pawn can attack {} squares",
            white_pawn_attacks.count_pieces()
        );

        let black_pawn_attacks = attack_table.pawn_attacks(square, Color::Black);
        println!("Black pawn attacks:");
        println!("{}", black_pawn_attacks);
        println!(
            "Black pawn can attack {} squares",
            black_pawn_attacks.count_pieces()
        );
    }

    println!("\nAttackTable tests completed successfully!");
}
