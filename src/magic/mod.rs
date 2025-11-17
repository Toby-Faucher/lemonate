pub mod attacks;
pub mod masks;

use crate::Bitboard;
use crate::types::Square;

pub use attacks::*;
pub use masks::*;

// Pre-computed magic numbers from Pradyumna Kannan's research
// These are known-good magic numbers that work for magic bitboard move generation

const ROOK_MAGICS: [u64; 64] = [
    0x0080001020400080, 0x0040001000200040, 0x0080081000200080, 0x0080040800100080,
    0x0080020400080080, 0x0080010200040080, 0x0080008001000200, 0x0080002040800100,
    0x0000800020400080, 0x0000400020005000, 0x0000801000200080, 0x0000800800100080,
    0x0000800400080080, 0x0000800200040080, 0x0000800100020080, 0x0000800040800100,
    0x0000208000400080, 0x0000404000201000, 0x0000808010002000, 0x0000808008001000,
    0x0000808004000800, 0x0000808002000400, 0x0000010100020004, 0x0000020000408104,
    0x0000208080004000, 0x0000200040005000, 0x0000100080200080, 0x0000080080100080,
    0x0000040080080080, 0x0000020080040080, 0x0000010080800200, 0x0000800080004100,
    0x0000204000800080, 0x0000200040401000, 0x0000100080802000, 0x0000080080801000,
    0x0000040080800800, 0x0000020080800400, 0x0000020001010004, 0x0000800040800100,
    0x0000204000808000, 0x0000200040008080, 0x0000100020008080, 0x0000080010008080,
    0x0000040008008080, 0x0000020004008080, 0x0000010002008080, 0x0000004081020004,
    0x0000204000800080, 0x0000200040008080, 0x0000100020008080, 0x0000080010008080,
    0x0000040008008080, 0x0000020004008080, 0x0000800100020080, 0x0000800041000080,
    0x0000102040800101, 0x0000102040008101, 0x0000081020004101, 0x0000040810002101,
    0x0001000204080011, 0x0001000204000801, 0x0001000082000401, 0x0000002040810402,
];

const BISHOP_MAGICS: [u64; 64] = [
    0x0002020202020200, 0x0002020202020000, 0x0004010202000000, 0x0004040080000000,
    0x0001104000000000, 0x0000821040000000, 0x0000410410400000, 0x0000104104104000,
    0x0000040404040400, 0x0000020202020200, 0x0000040102020000, 0x0000040400800000,
    0x0000011040000000, 0x0000008210400000, 0x0000004104104000, 0x0000002082082000,
    0x0004000808080800, 0x0002000404040400, 0x0001000202020200, 0x0000800802004000,
    0x0000800400A00000, 0x0000200100884000, 0x0000400082082000, 0x0000200041041000,
    0x0002080010101000, 0x0001040008080800, 0x0000208004010400, 0x0000404004010200,
    0x0000840000802000, 0x0000404002011000, 0x0000808001041000, 0x0000404000820800,
    0x0001041000202000, 0x0000820800101000, 0x0000104400080800, 0x0000020080080080,
    0x0000404040040100, 0x0000808100020100, 0x0001010100020800, 0x0000808080010400,
    0x0000820820004000, 0x0000410410002000, 0x0000082088001000, 0x0000002011000800,
    0x0000080100400400, 0x0001010101000200, 0x0002020202000400, 0x0001010101000200,
    0x0000410410400000, 0x0000208208200000, 0x0000002084100000, 0x0000000020880000,
    0x0000001002020000, 0x0000040408020000, 0x0004040404040000, 0x0002020202020000,
    0x0000104104104000, 0x0000002082082000, 0x0000000020841000, 0x0000000000208800,
    0x0000000010020200, 0x0000000404080200, 0x0000040404040400, 0x0002020202020200,
];

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

    // Less sparse than sparse(), more sparse than next()
    fn semi_sparse(&mut self) -> u64 {
        self.next() & self.next()
    }
}

pub fn find_magic(square: Square, mask: Bitboard, is_rook: bool) -> u64 {
    let n_bits = mask.count_pieces();
    let shift = 64 - n_bits;
    let num_patterns = 1 << n_bits;

    if square.index() <= 5 {
        eprintln!(
            "Finding magic for {} square {} ({}x{}, {} bits in mask, {} patterns)",
            if is_rook { "rook" } else { "bishop" },
            square.index(),
            square.file(),
            square.rank(),
            n_bits,
            num_patterns
        );
    }

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

    // Use a better seed based on square index and mask pattern
    let seed = (square.index() as u64).wrapping_mul(1103515245).wrapping_add(mask.0);
    let mut rng = MagicRng::new(seed);
    let mut used = vec![None; num_patterns];

    const MAX_ATTEMPTS: usize = 100_000_000;
    let mut attempts = 0;

    // Progressive strategy:
    // 0-10k: very sparse (3-way AND)
    // 10k-30k: semi-sparse (2-way AND)
    // 30k-60k: regular random
    // 60k+: restart with new seed

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

        // Progressive fallback strategy
        if attempts == 10_000 {
            eprintln!(
                "Switching to semi-sparse random numbers for {} square {}",
                if is_rook { "rook" } else { "bishop" },
                square.index()
            );
        } else if attempts == 30_000 {
            eprintln!(
                "Switching to non-sparse random numbers for {} square {}",
                if is_rook { "rook" } else { "bishop" },
                square.index()
            );
        } else if attempts % 60_000 == 0 && attempts > 0 {
            // Reset with new seed every 60k attempts
            let new_seed = rng.next().wrapping_mul(square.index() as u64 + attempts as u64);
            rng = MagicRng::new(new_seed);
            eprintln!(
                "Restarting with new seed for {} square {} at attempt {}",
                if is_rook { "rook" } else { "bishop" },
                square.index(),
                attempts
            );
        }

        let magic = if attempts < 10_000 {
            rng.sparse()
        } else if attempts < 30_000 {
            rng.semi_sparse()
        } else {
            rng.next()
        };

        // Skip obviously bad candidates
        if magic == 0 {
            continue;
        }

        // For sparse strategies, ensure the magic has enough bits set in the upper portion
        // This is important because we're using the upper bits after multiplication
        if attempts < 30_000 {
            let upper_bits = magic >> 32;
            let bit_count = upper_bits.count_ones();
            // Need at least a few bits set in upper half for good hashing
            if bit_count < 3 {
                continue;
            }
        }

        // Quick quality check: the product of mask and magic should have good bit distribution
        // in the upper bits that we'll be using
        let test_product = mask.0.wrapping_mul(magic);
        let upper_bits = test_product >> shift;
        if upper_bits.count_ones() < 3 {
            continue;
        }

        used.fill(None);

        let mut failed_at = None;
        for i in 0..blockers.len() {
            let index = (blockers[i].0.wrapping_mul(magic) >> shift) as usize;

            if index >= num_patterns {
                eprintln!(
                    "ERROR: index {} >= num_patterns {} for square {}, shift={}, n_bits={}",
                    index, num_patterns, square.index(), shift, n_bits
                );
                panic!("Index out of bounds in magic search!");
            }

            match used[index] {
                None => used[index] = Some(attacks[i]),
                Some(stored) if stored == attacks[i] => {
                    // Constructive collision - OK
                    continue;
                }
                Some(_) => {
                    // Destructive collision - bad
                    failed_at = Some(i);
                    break;
                }
            }
        }

        if let Some(fail_idx) = failed_at {
            if square.index() <= 5 && attempts % 10000 == 0 {
                eprintln!(
                    "  Attempt {}: magic=0x{:X}, failed at blocker {}/{}",
                    attempts,
                    magic,
                    fail_idx,
                    blockers.len()
                );
            }
            continue 'search;
        }

        if attempts > 1000 {
            eprintln!(
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

        magics[sq_idx] = Magic {
            mask,
            magic: BISHOP_MAGICS[sq_idx],
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

        magics[sq_idx] = Magic {
            mask,
            magic: ROOK_MAGICS[sq_idx],
            shift: 64 - mask.count_pieces(),
            offset,
        };

        offset += magics[sq_idx].table_size() as u32;
    }

    magics
}
