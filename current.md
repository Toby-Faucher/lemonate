# Recommended Attack Lookup Functions for Chess Engine

Your magic bitboard implementation is complete for sliding pieces. Here are additional attack lookup functions you should add to create a comprehensive attack system:

## 1. Non-Sliding Piece Attack Functions

### Knight Attacks (Precomputed Table)

```rust
// Add to AttackTable struct
pub knight_attacks: [Bitboard; 64],

// Initialization in AttackTable::new()
let knight_attacks = init_knight_attacks();

// Implementation
pub fn init_knight_attacks() -> [Bitboard; 64] {
    let mut attacks = [Bitboard::EMPTY; 64];
    const KNIGHT_MOVES: [(i8, i8); 8] = [
        (2, 1), (2, -1), (-2, 1), (-2, -1),
        (1, 2), (1, -2), (-1, 2), (-1, -2)
    ];

    for square_idx in 0..64 {
        let square = Square::from_index(square_idx);
        let (rank, file) = (square.rank() as i8, square.file() as i8);

        for &(dr, df) in &KNIGHT_MOVES {
            let new_rank = rank + dr;
            let new_file = file + df;

            if (0..8).contains(&new_rank) && (0..8).contains(&new_file) {
                let target = Square::from_coords(new_rank as u8, new_file as u8);
                attacks[square_idx].set(target);
            }
        }
    }
    attacks
}

pub fn knight_attacks(&self, square: Square) -> Bitboard {
    self.knight_attacks[square.index()]
}
```

### King Attacks (Precomputed Table)

```rust
// Add to AttackTable struct
pub king_attacks: [Bitboard; 64],

// Implementation
pub fn init_king_attacks() -> [Bitboard; 64] {
    let mut attacks = [Bitboard::EMPTY; 64];
    const KING_MOVES: [(i8, i8); 8] = [
        (1, 0), (-1, 0), (0, 1), (0, -1),
        (1, 1), (1, -1), (-1, 1), (-1, -1)
    ];

    for square_idx in 0..64 {
        let square = Square::from_index(square_idx);
        let (rank, file) = (square.rank() as i8, square.file() as i8);

        for &(dr, df) in &KING_MOVES {
            let new_rank = rank + dr;
            let new_file = file + df;

            if (0..8).contains(&new_rank) && (0..8).contains(&new_file) {
                let target = Square::from_coords(new_rank as u8, new_file as u8);
                attacks[square_idx].set(target);
            }
        }
    }
    attacks
}

pub fn king_attacks(&self, square: Square) -> Bitboard {
    self.king_attacks[square.index()]
}
```

### Pawn Attacks (Color-Dependent)

```rust
// Add to AttackTable struct
pub white_pawn_attacks: [Bitboard; 64],
pub black_pawn_attacks: [Bitboard; 64],

// Implementation
pub fn init_pawn_attacks() -> ([Bitboard; 64], [Bitboard; 64]) {
    let mut white_attacks = [Bitboard::EMPTY; 64];
    let mut black_attacks = [Bitboard::EMPTY; 64];

    for square_idx in 0..64 {
        let square = Square::from_index(square_idx);
        let (rank, file) = (square.rank() as i8, square.file() as i8);

        // White pawn attacks (moving up)
        if rank < 7 {
            if file > 0 {
                let target = Square::from_coords((rank + 1) as u8, (file - 1) as u8);
                white_attacks[square_idx].set(target);
            }
            if file < 7 {
                let target = Square::from_coords((rank + 1) as u8, (file + 1) as u8);
                white_attacks[square_idx].set(target);
            }
        }

        // Black pawn attacks (moving down)
        if rank > 0 {
            if file > 0 {
                let target = Square::from_coords((rank - 1) as u8, (file - 1) as u8);
                black_attacks[square_idx].set(target);
            }
            if file < 7 {
                let target = Square::from_coords((rank - 1) as u8, (file + 1) as u8);
                black_attacks[square_idx].set(target);
            }
        }
    }

    (white_attacks, black_attacks)
}

pub fn pawn_attacks(&self, square: Square, color: Color) -> Bitboard {
    match color {
        Color::White => self.white_pawn_attacks[square.index()],
        Color::Black => self.black_pawn_attacks[square.index()],
    }
}
```

## 2. Advanced Attack Detection Functions

### Square Attacked By

```rust
pub fn square_attacked_by(&self, square: Square, attackers: Bitboard, color: Color) -> bool {
    // Check pawn attacks
    let pawn_attackers = self.pawn_attacks(square, color.opposite()) & attackers;
    if !pawn_attackers.is_empty() {
        return true;
    }

    // Check knight attacks
    let knight_attackers = self.knight_attacks(square) & attackers;
    if !knight_attackers.is_empty() {
        return true;
    }

    // Check king attacks
    let king_attackers = self.king_attacks(square) & attackers;
    if !king_attackers.is_empty() {
        return true;
    }

    // Check sliding piece attacks
    let all_pieces = attackers; // You'll need to pass this from board state

    let rook_attackers = self.rook_attacks(square, all_pieces) & attackers;
    let bishop_attackers = self.bishop_attacks(square, all_pieces) & attackers;

    !rook_attackers.is_empty() || !bishop_attackers.is_empty()
}
```

### All Attackers of Square

```rust
pub fn attackers_of(&self, square: Square, all_pieces: Bitboard, white_pieces: Bitboard, black_pieces: Bitboard) -> Bitboard {
    let mut attackers = Bitboard::EMPTY;

    // Pawn attackers
    attackers |= self.white_pawn_attacks[square.index()] & black_pieces; // Black pawns that can attack this square
    attackers |= self.black_pawn_attacks[square.index()] & white_pieces; // White pawns that can attack this square

    // Knight attackers
    attackers |= self.knight_attacks(square) & all_pieces;

    // King attackers
    attackers |= self.king_attacks(square) & all_pieces;

    // Sliding piece attackers
    attackers |= self.rook_attacks(square, all_pieces) & all_pieces;
    attackers |= self.bishop_attacks(square, all_pieces) & all_pieces;

    attackers
}
```

## 3. Optimized Lookup Functions

### Pin Detection

```rust
pub fn pinned_pieces(&self, king_square: Square, own_pieces: Bitboard, enemy_pieces: Bitboard) -> Bitboard {
    let mut pinned = Bitboard::EMPTY;

    // Check diagonal pins (bishops/queens)
    let diagonal_attackers = self.bishop_attacks(king_square, enemy_pieces) & enemy_pieces;
    for attacker_square in diagonal_attackers {
        let between = self.bishop_attacks(king_square, Bitboard::EMPTY)
                    & self.bishop_attacks(attacker_square, Bitboard::EMPTY);
        let pieces_between = between & own_pieces;
        if pieces_between.count_pieces() == 1 {
            pinned |= pieces_between;
        }
    }

    // Check straight pins (rooks/queens)
    let straight_attackers = self.rook_attacks(king_square, enemy_pieces) & enemy_pieces;
    for attacker_square in straight_attackers {
        let between = self.rook_attacks(king_square, Bitboard::EMPTY)
                    & self.rook_attacks(attacker_square, Bitboard::EMPTY);
        let pieces_between = between & own_pieces;
        if pieces_between.count_pieces() == 1 {
            pinned |= pieces_between;
        }
    }

    pinned
}
```

### Check Detection

```rust
pub fn in_check(&self, king_square: Square, all_pieces: Bitboard, enemy_pieces: Bitboard) -> bool {
    self.square_attacked_by(king_square, enemy_pieces, Color::White) // Adjust color as needed
}

pub fn check_rays(&self, king_square: Square, all_pieces: Bitboard, enemy_pieces: Bitboard) -> Bitboard {
    let mut check_rays = Bitboard::EMPTY;

    // Find all pieces giving check
    let attackers = self.attackers_of(king_square, all_pieces, Bitboard::EMPTY, enemy_pieces) & enemy_pieces;

    for attacker_square in attackers {
        // Add ray from attacker to king (for sliding pieces)
        check_rays |= self.ray_between(attacker_square, king_square);
        check_rays.set(attacker_square); // Include the attacker itself
    }

    check_rays
}
```

## Logic Explanation

**Why these functions:**

1. **Non-sliding pieces** need precomputed tables since their attacks don't depend on board state
2. **Pawn attacks** are color-dependent and different from pawn moves
3. **Advanced functions** enable move generation, check detection, and pin analysis
4. **Optimized lookups** reduce computation in critical game tree search

**Performance benefits:**

- Precomputed tables: O(1) lookup vs O(8) calculation for each query
- Magic bitboards: O(1) sliding piece attacks vs O(7) ray casting
- Combined functions: Reduce multiple bitboard operations into single calls

**Memory usage:**

- Knight attacks: 512 bytes (64 squares × 8 bytes)
- King attacks: 512 bytes
- Pawn attacks: 1024 bytes (2 colors × 64 squares × 8 bytes)
- Total additional: ~2KB for massive speed improvement

