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
        // xorshift has a fixed point at 0.
        Self { state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed } }
    }

    fn next(&mut self) -> u64 {
        // xorshift64*: consecutive outputs are uncorrelated, which matters because
        // `sparse` ANDs several of them. (A plain LCG returning its raw state gave
        // candidates so poor that rook a1 took minutes and rook c1 never succeeded.)
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn sparse(&mut self) -> u64 {
        self.next() & self.next() & self.next()
    }

    // Less sparse than sparse(), more sparse than next()
    fn semi_sparse(&mut self) -> u64 {
        self.next() & self.next()
    }
}

/// Search for a magic multiplier for `square` with the given occupancy `mask`.
///
/// Candidates are always sparse (an AND of three random words): dense random words almost
/// never work for masks of 10+ bits, so falling back to them (as an earlier version did
/// after 30k attempts) turns a millisecond search into one that never finishes.
pub fn find_magic(square: Square, mask: Bitboard, is_rook: bool) -> u64 {
    let n_bits = mask.count_pieces();
    assert!(n_bits > 0, "cannot find a magic for an empty mask");
    let shift = 64 - n_bits;
    let num_patterns = 1usize << n_bits;

    let mut blockers = Vec::with_capacity(num_patterns);
    let mut attacks = Vec::with_capacity(num_patterns);
    for i in 0..num_patterns {
        let blocker_board = generate_blocker_board(i, mask);
        blockers.push(blocker_board);
        attacks.push(if is_rook {
            calculate_rook_attacks(square, blocker_board)
        } else {
            calculate_bishop_attacks(square, blocker_board)
        });
    }

    let seed = (square.index() as u64)
        .wrapping_mul(1103515245)
        .wrapping_add(mask.0);
    let mut rng = MagicRng::new(seed);
    let mut used: Vec<Option<Bitboard>> = vec![None; num_patterns];

    const MAX_ATTEMPTS: u64 = 1_000_000_000;
    for _ in 0..MAX_ATTEMPTS {
        let magic = rng.sparse();

        // A useful magic spreads the mask into the top byte (Tord Romstad's filter).
        if (mask.0.wrapping_mul(magic) & 0xFF00_0000_0000_0000).count_ones() < 6 {
            continue;
        }

        used.fill(None);
        let mut collision = false;
        for i in 0..num_patterns {
            let index = (blockers[i].0.wrapping_mul(magic) >> shift) as usize;
            match used[index] {
                None => used[index] = Some(attacks[i]),
                // Two blocker sets may share a slot only if their attacks are identical.
                Some(stored) if stored == attacks[i] => {}
                Some(_) => {
                    collision = true;
                    break;
                }
            }
        }
        if !collision {
            return magic;
        }
    }

    panic!(
        "no magic found for {} square {} ({} mask bits) after {} attempts",
        if is_rook { "rook" } else { "bishop" },
        square.index(),
        n_bits,
        MAX_ATTEMPTS
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A magic is valid if no two blocker sets that need different attack sets share a slot.
    fn is_valid_magic(square: Square, mask: Bitboard, magic: u64, is_rook: bool) -> bool {
        let n_bits = mask.count_pieces();
        let shift = 64 - n_bits;
        let mut table: Vec<Option<Bitboard>> = vec![None; 1usize << n_bits];
        for i in 0..(1usize << n_bits) {
            let blockers = generate_blocker_board(i, mask);
            let attacks = if is_rook {
                calculate_rook_attacks(square, blockers)
            } else {
                calculate_bishop_attacks(square, blockers)
            };
            let slot = (blockers.0.wrapping_mul(magic) >> shift) as usize;
            match table[slot] {
                None => table[slot] = Some(attacks),
                Some(stored) if stored == attacks => {}
                Some(_) => return false,
            }
        }
        true
    }

    #[test]
    fn rook_masks_are_never_empty_even_in_corners() {
        // init_rook_magics has a branch for "empty" corner masks; a rook always sees along
        // its rank and file, so that branch must be unreachable.
        for idx in 0..64 {
            let mask = generate_rook_mask(Square::from_index(idx));
            assert!(mask.count_pieces() >= 10, "square {idx}: {} bits", mask.count_pieces());
        }
    }

    #[test]
    fn hardcoded_rook_magics_are_collision_free() {
        let magics = init_rook_magics();
        for idx in 0..64 {
            let sq = Square::from_index(idx);
            assert!(
                is_valid_magic(sq, magics[idx].mask, magics[idx].magic, true),
                "rook magic for square {idx} collides"
            );
        }
    }

    #[test]
    fn hardcoded_bishop_magics_are_collision_free() {
        let magics = init_bishop_magics();
        for idx in 0..64 {
            let sq = Square::from_index(idx);
            assert!(
                is_valid_magic(sq, magics[idx].mask, magics[idx].magic, false),
                "bishop magic for square {idx} collides"
            );
        }
    }

    #[test]
    fn finder_returns_valid_magics_for_hard_and_easy_squares() {
        // a1 and h8 have the largest rook masks (12 bits) and used to take minutes or never
        // finish; e4 is a middle-of-the-board bishop; c1 is the square that used to panic.
        for (idx, is_rook) in [(0, true), (63, true), (2, true), (28, false), (0, false)] {
            let sq = Square::from_index(idx);
            let mask = if is_rook { generate_rook_mask(sq) } else { generate_bishop_mask(sq) };
            let magic = find_magic(sq, mask, is_rook);
            assert!(is_valid_magic(sq, mask, magic, is_rook), "square {idx} rook={is_rook}");
        }
    }

    /// Regenerates all 128 magics (~2 s in release, much slower in debug).
    /// Run with: cargo test --release -- --ignored finder_regenerates_every_magic
    #[test]
    #[ignore]
    fn finder_regenerates_every_magic() {
        for is_rook in [true, false] {
            for idx in 0..64 {
                let sq = Square::from_index(idx);
                let mask = if is_rook { generate_rook_mask(sq) } else { generate_bishop_mask(sq) };
                let magic = find_magic(sq, mask, is_rook);
                assert!(is_valid_magic(sq, mask, magic, is_rook), "square {idx} rook={is_rook}");
            }
        }
    }
}
