use crate::bitboard::Bitboard;
use crate::masks::*;
use crate::types::Square;
use crate::types::{Color, PieceType};
use crate::Magic;
use crate::{init_bishop_magics, init_rook_magics};

pub struct AttackTable {
    pub rook_attacks: Box<[Bitboard]>,
    pub bishop_attacks: Box<[Bitboard]>,
    pub knight_attacks: [Bitboard; 64],
    pub king_attacks: [Bitboard; 64],
    pub white_pawn_attacks: [Bitboard; 64],
    pub black_pawn_attacks: [Bitboard; 64],
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
        let knight_attacks = init_knight_attacks();
        let king_attacks = init_king_attacks();

        let pawn_attacks = init_pawn_attacks();

        println!("Attck tables initialized");

        Self {
            rook_attacks,
            bishop_attacks,
            knight_attacks,
            king_attacks,
            rook_magics,
            bishop_magics,
            white_pawn_attacks: pawn_attacks.0,
            black_pawn_attacks: pawn_attacks.1,
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

    pub fn knight_attacks(&self, square: Square) -> Bitboard {
        self.knight_attacks[square.index()]
    }

    pub fn king_attacks(&self, square: Square) -> Bitboard {
        self.king_attacks[square.index()]
    }

    pub fn queen_attacks(&self, square: Square, blockers: Bitboard) -> Bitboard {
        self.rook_attacks(square, blockers) | self.bishop_attacks(square, blockers)
    }

    pub fn pawn_attacks(&self, square: Square, color: Color) -> Bitboard {
        match color {
            Color::White => self.white_pawn_attacks[square.index()],
            Color::Black => self.black_pawn_attacks[square.index()],
        }
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

fn init_knight_attacks() -> [Bitboard; 64] {
    let mut attacks = [Bitboard::EMPTY; 64];

    const KNIGHT_MOVES: [(i8, i8); 8] = [
        (2, 1),
        (2, -1),
        (-2, 1),
        (-2, -1),
        (1, 2),
        (1, -2),
        (-1, 2),
        (-1, -2),
    ];

    for (sq_idx, attack) in attacks.iter_mut().enumerate() {
        let square = Square::from_index(sq_idx);
        let (rank, file) = (square.rank() as i8, square.file() as i8);

        for &(dr, df) in &KNIGHT_MOVES {
            let new_rank = rank + dr;
            let new_file = file + df;

            if (0..8).contains(&new_rank) && (0..8).contains(&new_file) {
                let target = Square::from_coords(new_file as u8, new_rank as u8);
                attack.set(target);
            }
        }
    }
    attacks
}

pub fn init_king_attacks() -> [Bitboard; 64] {
    let mut attacks = [Bitboard::EMPTY; 64];
    const KING_MOVES: [(i8, i8); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];

    for (sq_idx, attack) in attacks.iter_mut().enumerate() {
        let square = Square::from_index(sq_idx);
        let (rank, file) = (square.rank() as i8, square.file() as i8);

        for &(dr, df) in &KING_MOVES {
            let new_rank = rank + dr;
            let new_file = file + df;

            if (0..8).contains(&new_rank) && (0..8).contains(&new_file) {
                let target = Square::from_coords(new_file as u8, new_rank as u8);
                attack.set(target);
            }
        }
    }
    attacks
}

pub fn init_pawn_attacks() -> ([Bitboard; 64], [Bitboard; 64]) {
    let mut white_attacks = [Bitboard::EMPTY; 64];
    let mut black_attacks = [Bitboard::EMPTY; 64];

    for square_idx in 0..64 {
        let square = Square::from_index(square_idx);
        let (rank, file) = (square.rank() as i8, square.file() as i8);

        // White pawn attacks (moving up)
        if rank < 7 {
            if file > 0 {
                let target = Square::from_coords((file - 1) as u8, (rank + 1) as u8);
                white_attacks[square_idx].set(target);
            }
            if file < 7 {
                let target = Square::from_coords((file + 1) as u8, (rank + 1) as u8);
                white_attacks[square_idx].set(target);
            }
        }

        // Black pawn attacks (moving down)
        if rank > 0 {
            if file > 0 {
                let target = Square::from_coords((file - 1) as u8, (rank - 1) as u8);
                black_attacks[square_idx].set(target);
            }
            if file < 7 {
                let target = Square::from_coords((file + 1) as u8, (rank - 1) as u8);
                black_attacks[square_idx].set(target);
            }
        }
    }

    (white_attacks, black_attacks)
}
