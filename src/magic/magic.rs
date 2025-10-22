use crate::types::Square;
use crate::{
    calculate_bishop_attacks, calculate_rook_attacks, generate_bishop_mask, generate_blocker_board,
    generate_rook_mask, Bitboard,
};

#[derive(Clone, Copy, Default)]
pub struct Magic {
    pub mask: Bitboard,
    pub magic: u64,
    pub shift: u32,
    pub offset: u32,
}

impl Magic {
    pub fn new(mask: Bitboard, magic: u64, offset: u32) -> Self {
        let shift = 64 - mask.count_pieces();
        Self {
            mask,
            magic,
            shift,
            offset,
        }
    }

    pub fn hash(&self, blockers: Bitboard) -> usize {
        let relevant = blockers & self.mask;

        if self.shift >= 64 {
            return 0;
        }

        let hash = relevant.0.wrapping_mul(self.magic) >> self.shift;

        hash as usize
    }

    pub fn table_size(&self) -> usize {
        1 << self.mask.count_pieces()
    }
}

#[derive(Debug)]
struct MagicRng {
    state: u64,
}

impl MagicRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);

        self.state
    }

    fn sparse(&mut self) -> u64 {
        self.next() & self.next() & self.next()
    }
}

pub fn find_magic(square: Square, mask: Bitboard, is_rook: bool) -> u64 {
    let n_bits = mask.count_pieces();
    let shift = 64 - n_bits;
    let num_patterns = 1 << n_bits;

    let mut blockers = Vec::new();
    let mut attacks = Vec::new();

    for i in 0..num_patterns {
        let blocker_board = generate_blocker_board(i, mask);
        blockers.push(blocker_board);

        let attack_board = if is_rook {
            calculate_rook_attacks(square, blocker_board)
        } else {
            calculate_bishop_attacks(square, blocker_board)
        };
        attacks.push(attack_board);
    }

    let mut rng = MagicRng::new(square.index() as u64 + 12345);
    let mut used = vec![None; num_patterns];

    const MAX_ATTEMPTS: usize = 100_000_000;
    let mut attempts = 0;

    'search: loop {
        attempts += 1;
        if attempts > MAX_ATTEMPTS {
            panic!(
                "Failed to find magic number for {} square {} after {} attempts. Mask has {} bits.",
                if is_rook { "rook" } else { "bishop" },
                square.index(),
                MAX_ATTEMPTS,
                n_bits
            );
        }

        let magic = rng.sparse();

        if ((mask.0.wrapping_mul(magic)) >> 56).count_ones() < 6 {
            continue;
        }

        used.fill(None);

        for i in 0..blockers.len() {
            let index = ((blockers[i].0 & mask.0).wrapping_mul(magic) >> shift) as usize;

            match used[index] {
                None => used[index] = Some(attacks[i]),
                Some(stored) if stored == attacks[i] => continue,
                Some(_) => continue 'search,
            }
        }

        if attempts > 1000 {
            println!(
                "Found magic for {} square {} after {} attempts",
                if is_rook { "rook" } else { "bishop" },
                square.index(),
                attempts
            );
        }

        return magic;
    }
}

pub fn init_bishop_magics() -> [Magic; 64] {
    let mut magics = [Magic {
        mask: Bitboard::EMPTY,
        magic: 0,
        shift: 0,
        offset: 0,
    }; 64];

    let mut offset = 0;

    #[allow(clippy::needless_range_loop)]
    for sq_idx in 0..64 {
        let square = Square::from_index(sq_idx);
        let mask = generate_bishop_mask(square);
        let magic_number = find_magic(square, mask, false); // false = bishop

        magics[sq_idx] = Magic {
            mask,
            magic: magic_number,
            shift: 64 - mask.count_pieces(),
            offset,
        };

        offset += magics[sq_idx].table_size() as u32;
    }

    magics
}

pub fn init_rook_magics() -> [Magic; 64] {
    let mut magics = [Magic {
        mask: Bitboard::EMPTY,
        magic: 0,
        shift: 0,
        offset: 0,
    }; 64];

    let mut offset = 0;

    #[allow(clippy::needless_range_loop)]
    for sq_idx in 0..64 {
        let square = Square::from_index(sq_idx);
        let mask = generate_rook_mask(square);

        if mask.count_pieces() == 0 {
            // Corner squares have empty masks - they don't need magic numbers
            magics[sq_idx] = Magic {
                mask: Bitboard::EMPTY,
                magic: 0,
                shift: 64,
                offset,
            };
            continue;
        }

        let magic_number = find_magic(square, mask, true); // true = rook

        magics[sq_idx] = Magic {
            mask,
            magic: magic_number,
            shift: 64 - mask.count_pieces(),
            offset,
        };

        offset += magics[sq_idx].table_size() as u32;
    }

    magics
}
