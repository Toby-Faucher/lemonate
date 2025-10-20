use std::usize;

use crate::bitboard::Bitboard;
use crate::bitboard::Square;
use crate::masks::*;
use crate::piece::PieceType;
use crate::Magic;
use crate::{init_bishop_magics, init_rook_magics};

pub struct AttackTable {
    pub rook_attacks: Box<[Bitboard]>,
    pub bishop_attacks: Box<[Bitboard]>,
    pub rook_magics: [Magic; 64],
    pub bishop_magics: [Magic; 64],
}

// Helper functions
pub fn generate_blocker_board(index: usize, mask: Bitboard) -> Bitboard {
    let mut blockers = Bitboard::EMPTY;

    let mut mask_copy = mask;
    let mut bit_index = 0;

    while let Some(square) = mask_copy.pop_lsb() {
        if (index & (1 << bit_index)) != 0 {
            blockers.set(square);
        }
        bit_index += 1;
    }

    blockers
}

impl AttackTable {
    pub fn new() -> Self {
        println!("Initialing magic attack tables");

        let rook_magics = init_rook_magics();
        let bishop_magics = init_bishop_magics();

        let rook_attacks = build_rook_table(&rook_magics);
        let bishop_attacks = build_bishop_table(&bishop_magics);

        println!("Attck tables initialized");

        Self {
            rook_attacks,
            bishop_attacks,
            rook_magics,
            bishop_magics,
        }
    }

    pub fn rook_attacks(&self, square: Square, blockers: Bitboard) -> Bitboard {
        let magic = &self.rook_magics[square.index()];
        let hash = magic.hash(blockers);
        self.rook_attacks[magic.offset as usize + hash]
    }

    pub fn bishop_attacks(&self, square: Square, blockers: Bitboard) -> Bitboard {
        let magic = &self.bishop_magics[square.index()];
        let hash = magic.hash(blockers);
        self.bishop_attacks[magic.offset as usize + hash]
    }

    pub fn queen_attacks(&self, square: Square, blockers: Bitboard) -> Bitboard {
        self.rook_attacks(square, blockers) | self.bishop_attacks(square, blockers)
    }
}

impl Default for AttackTable {
    fn default() -> Self {
        Self::new()
    }
}

pub fn generate_rook_mask(square: Square) -> Bitboard {
    generate_occupancy_mask(square, PieceType::Rook)
}

pub fn generate_bishop_mask(square: Square) -> Bitboard {
    generate_occupancy_mask(square, PieceType::Bishop)
}

pub fn calculate_rook_attacks(square: Square, blockers: Bitboard) -> Bitboard {
    const ROOK_DIRS: [(i8, i8); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    generate_sliding_attacks(square, &ROOK_DIRS, blockers)
}

pub fn calculate_bishop_attacks(square: Square, blockers: Bitboard) -> Bitboard {
    const BISHOP_DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    generate_sliding_attacks(square, &BISHOP_DIRS, blockers)
}

fn build_rook_table(magics: &[Magic; 64]) -> Box<[Bitboard]> {
    let total_size: usize = magics.iter().map(|m| m.table_size()).sum();

    println!("Rook table size: {} entries", total_size);

    let mut table = vec![Bitboard::EMPTY; total_size].into_boxed_slice();

    for (sq_idx, magic) in magics.iter().enumerate().take(64) {
        let square = Square::from_index(sq_idx);
        let mask = magic.mask;
        let n_bits = mask.count_pieces();

        for blocker_idx in 0..(1 << n_bits) {
            let blockers = generate_blocker_board(blocker_idx, mask);
            let attacks = calculate_rook_attacks(square, blockers);

            let hash = magic.hash(blockers);
            table[magic.offset as usize + hash] = attacks;
        }
    }

    table
}

fn build_bishop_table(magics: &[Magic; 64]) -> Box<[Bitboard]> {
    let total_size: usize = magics.iter().map(|m| m.table_size()).sum();

    println!("Bishop table size: {} entries", total_size);

    let mut table = vec![Bitboard::EMPTY; total_size].into_boxed_slice();

    for (sq_idx, magic) in magics.iter().enumerate().take(64) {
        let square = Square::from_index(sq_idx);
        let mask = magic.mask;
        let n_bits = mask.count_pieces();

        for blocker_idx in 0..(1 << n_bits) {
            let blockers = generate_blocker_board(blocker_idx, mask);
            let attacks = calculate_bishop_attacks(square, blockers);

            let hash = magic.hash(blockers);
            table[magic.offset as usize + hash] = attacks;
        }
    }

    table
}
