use lemonate::bitboard::{Bitboard, Square};
use lemonate::magic::*;
use lemonate::types::magic::{Magic, init_bishop_magics};

fn main() {
    println!("=== Attack Table Tests ===\n");

    // Test attack tables for different squares
    println!("Testing rook attacks from various squares:");
    
    let squares = [
        (Square::from_coords(0, 0), "a1"),
        (Square::from_coords(4, 3), "e4"),
        (Square::from_coords(7, 7), "h8"),
        (Square::from_coords(3, 3), "d4"),
    ];

    for (square, name) in squares {
        println!("\nRook attacks from {}:", name);
        let empty_attacks = calculate_rook_attacks(square, Bitboard::EMPTY);
        println!("{}", empty_attacks);
        
        println!("Bishop attacks from {}:", name);
        let bishop_attacks = calculate_bishop_attacks(square, Bitboard::EMPTY);
        println!("{}", bishop_attacks);
    }

    println!("\n=== Testing with blockers ===");
    
    let center_square = Square::from_coords(4, 4); // e5
    let mut blockers = Bitboard::EMPTY;
    blockers.set(Square::from_coords(4, 6)); // e7
    blockers.set(Square::from_coords(6, 4)); // g5
    blockers.set(Square::from_coords(2, 2)); // c3
    
    println!("Blockers pattern:");
    println!("{}", blockers);
    
    println!("\nRook attacks from e5 with blockers:");
    let rook_blocked = calculate_rook_attacks(center_square, blockers);
    println!("{}", rook_blocked);
    
    println!("Bishop attacks from e5 with blockers:");
    let bishop_blocked = calculate_bishop_attacks(center_square, blockers);
    println!("{}", bishop_blocked);

    println!("\n=== Magic table initialization ===");
    let bishop_magics = init_bishop_magics();
    println!("Successfully initialized {} bishop magic entries", bishop_magics.len());
    
    for i in [0, 9, 14, 18, 27, 36, 45, 54, 63] {
        let square_name = format!("{}{}", 
            (b'a' + (i % 8) as u8) as char,
            (i / 8) + 1
        );
        println!("Bishop magic for {}: 0x{:016x}", square_name, bishop_magics[i].magic);
    }
}
