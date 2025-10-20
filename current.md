# Phase 3: Board & Moves - Deep Implementation Guide

## File Structure

```
src/
├── board/
│   ├── mod.rs              # Board module exports and re-exports
│   ├── board.rs            # Main Board struct and basic operations
│   ├── fen.rs              # FEN parsing and generation
│   ├── moves.rs            # Move generation and validation
│   └── make_move.rs        # Move making/unmaking logic
├── types/
│   ├── mod.rs              # Type definitions module
│   ├── piece.rs            # Piece, Color, PieceType definitions
│   ├── square.rs           # Square type and operations
│   └── castling.rs         # CastlingRights and related types
├── bitboard/
│   ├── mod.rs              # Bitboard module (already exists)
│   └── board.rs            # Bitboard implementation (already exists)
└── lib.rs                  # Main library file with module declarations
```

## 1. Board Position Representation

### Core Concept
A chess board position contains **complete game state** - not just piece positions, but all information needed to determine legal moves and game status.

### Board Structure Implementation

**File: `src/board/board.rs`**

```rust
use crate::bitboard::Bitboard;
use crate::types::{Piece, Color, PieceType};
use crate::square::Square;

#[derive(Clone, Debug)]
pub struct Board {
    // Piece bitboards - one for each piece type and color
    piece_bitboards: [[Bitboard; 6]; 2], // [color][piece_type]
    
    // Occupancy bitboards for quick lookup
    color_bitboards: [Bitboard; 2], // [color] - all pieces of that color
    all_pieces: Bitboard,           // All pieces on board
    
    // Game state information
    side_to_move: Color,
    castling_rights: CastlingRights,
    en_passant_square: Option<Square>,
    halfmove_clock: u16,    // 50-move rule counter
    fullmove_number: u16,   // Increments after Black's move
    
    // Zobrist hashing for position repetition detection
    position_hash: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}
```

### Why This Design?

**Bitboard Arrays**: Instead of a single `pieces[64]` array, we use bitboards because:
- **Fast set operations**: Union, intersection, XOR operations are single CPU instructions
- **Parallel processing**: Check multiple squares simultaneously
- **Memory efficiency**: 64 bits represents entire board state for one piece type

**Separate Occupancy Bitboards**: 
- `color_bitboards[Color::White]` = union of all white piece bitboards
- `all_pieces` = union of both color bitboards
- **Speed**: Avoid recalculating these frequently-needed values

### Implementation Details

**File: `src/board/board.rs` (continued)**

```rust
impl Board {
    pub fn new() -> Self {
        Self {
            piece_bitboards: [[Bitboard::EMPTY; 6]; 2],
            color_bitboards: [Bitboard::EMPTY; 2],
            all_pieces: Bitboard::EMPTY,
            side_to_move: Color::White,
            castling_rights: CastlingRights::all(),
            en_passant_square: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            position_hash: 0,
        }
    }
    
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        // Check if square is occupied
        if !self.all_pieces.is_set(square) {
            return None;
        }
        
        // Find which piece type and color
        for color in [Color::White, Color::Black] {
            if self.color_bitboards[color as usize].is_set(square) {
                for piece_type in [PieceType::Pawn, PieceType::Knight, 
                                 PieceType::Bishop, PieceType::Rook, 
                                 PieceType::Queen, PieceType::King] {
                    if self.piece_bitboards[color as usize][piece_type as usize].is_set(square) {
                        return Some(Piece { piece_type, color });
                    }
                }
            }
        }
        None
    }
    
    pub fn place_piece(&mut self, square: Square, piece: Piece) {
        // Update piece-specific bitboard
        self.piece_bitboards[piece.color as usize][piece.piece_type as usize].set(square);
        
        // Update color bitboard
        self.color_bitboards[piece.color as usize].set(square);
        
        // Update all pieces bitboard
        self.all_pieces.set(square);
        
        // Update position hash (Zobrist hashing)
        self.position_hash ^= zobrist_piece_hash(square, piece);
    }
}
```

**Key Insight**: We maintain redundant data structures (piece bitboards + color bitboards + all pieces) because the performance gain from not recalculating these frequently-used values outweighs the memory cost.

---

## 2. FEN (Forsyth-Edwards Notation) Parsing

### What is FEN?
FEN is the standard notation for describing a chess position. Example:
```
rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
```

### FEN Format Breakdown
1. **Piece placement**: `rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR`
2. **Active color**: `w` (White to move)
3. **Castling rights**: `KQkq` (K=White kingside, Q=White queenside, k=Black kingside, q=Black queenside)
4. **En passant**: `-` (no en passant square available)
5. **Halfmove clock**: `0` (moves since last pawn move or capture)
6. **Fullmove number**: `1` (increments after Black's move)

### Implementation

**File: `src/board/fen.rs`**

```rust
impl Board {
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            return Err(FenError::InvalidFormat);
        }
        
        let mut board = Board::new();
        
        // 1. Parse piece placement
        board.parse_piece_placement(parts[0])?;
        
        // 2. Parse active color
        board.side_to_move = match parts[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err(FenError::InvalidActiveColor),
        };
        
        // 3. Parse castling rights
        board.castling_rights = CastlingRights::from_fen(parts[2])?;
        
        // 4. Parse en passant square
        board.en_passant_square = if parts[3] == "-" {
            None
        } else {
            Some(Square::from_algebraic(parts[3])?)
        };
        
        // 5. Parse halfmove clock
        board.halfmove_clock = parts[4].parse()
            .map_err(|_| FenError::InvalidHalfmove)?;
            
        // 6. Parse fullmove number
        board.fullmove_number = parts[5].parse()
            .map_err(|_| FenError::InvalidFullmove)?;
        
        // Recalculate derived bitboards
        board.recalculate_occupancy();
        board.recalculate_hash();
        
        Ok(board)
    }
    
    fn parse_piece_placement(&mut self, placement: &str) -> Result<(), FenError> {
        let ranks: Vec<&str> = placement.split('/').collect();
        if ranks.len() != 8 {
            return Err(FenError::InvalidPiecePlacement);
        }
        
        for (rank_idx, rank_str) in ranks.iter().enumerate() {
            let rank = 7 - rank_idx; // FEN starts from rank 8
            let mut file = 0;
            
            for ch in rank_str.chars() {
                if ch.is_ascii_digit() {
                    // Empty squares
                    let empty_count = ch.to_digit(10).unwrap() as u8;
                    file += empty_count;
                } else {
                    // Piece
                    let piece = Piece::from_fen_char(ch)?;
                    let square = Square::from_coords(file, rank);
                    self.place_piece(square, piece);
                    file += 1;
                }
                
                if file > 8 {
                    return Err(FenError::InvalidPiecePlacement);
                }
            }
            
            if file != 8 {
                return Err(FenError::InvalidPiecePlacement);
            }
        }
        
        Ok(())
    }
}

**File: `src/types/piece.rs`**

```rust
impl Piece {
    fn from_fen_char(ch: char) -> Result<Self, FenError> {
        let color = if ch.is_uppercase() { Color::White } else { Color::Black };
        let piece_type = match ch.to_ascii_lowercase() {
            'p' => PieceType::Pawn,
            'n' => PieceType::Knight,
            'b' => PieceType::Bishop,
            'r' => PieceType::Rook,
            'q' => PieceType::Queen,
            'k' => PieceType::King,
            _ => return Err(FenError::InvalidPiece),
        };
        Ok(Piece { piece_type, color })
    }
}
```

### Why FEN Parsing is Complex

**Rank Order**: FEN describes ranks from 8 to 1, but array indices go 0 to 7
**Empty Squares**: Numbers represent consecutive empty squares, not pieces
**Case Sensitivity**: Uppercase = White pieces, lowercase = Black pieces
**Validation**: Must ensure exactly 8 files per rank and valid piece characters

---

## 3. Legal Move Generation

### The Challenge
Legal move generation is the **most complex** part of a chess engine because:
1. **Pseudo-legal vs Legal**: A piece might be able to move somewhere, but the move could leave the king in check
2. **Special moves**: Castling, en passant have unique rules
3. **Performance**: This function is called millions of times during search

### Two-Stage Approach

**File: `src/board/moves.rs`**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub move_type: MoveType,
    pub piece: Piece,
    pub captured: Option<Piece>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveType {
    Normal,
    Capture,
    EnPassant,
    Castle,
    Promotion(PieceType), // Queen, Rook, Bishop, Knight
}

impl Board {
    pub fn generate_legal_moves(&self) -> Vec<Move> {
        let mut legal_moves = Vec::new();
        
        // Step 1: Generate all pseudo-legal moves
        let pseudo_legal = self.generate_pseudo_legal_moves();
        
        // Step 2: Filter out moves that leave king in check
        for mv in pseudo_legal {
            if self.is_legal_move(mv) {
                legal_moves.push(mv);
            }
        }
        
        legal_moves
    }
    
    fn generate_pseudo_legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        let color = self.side_to_move;
        let color_idx = color as usize;
        
        // Generate moves for each piece type
        for piece_type in [PieceType::Pawn, PieceType::Knight, 
                          PieceType::Bishop, PieceType::Rook, 
                          PieceType::Queen, PieceType::King] {
            let piece_bb = self.piece_bitboards[color_idx][piece_type as usize];
            
            for square in piece_bb {
                match piece_type {
                    PieceType::Pawn => self.generate_pawn_moves(square, &mut moves),
                    PieceType::Knight => self.generate_knight_moves(square, &mut moves),
                    PieceType::Bishop => self.generate_bishop_moves(square, &mut moves),
                    PieceType::Rook => self.generate_rook_moves(square, &mut moves),
                    PieceType::Queen => self.generate_queen_moves(square, &mut moves),
                    PieceType::King => self.generate_king_moves(square, &mut moves),
                }
            }
        }
        
        moves
    }
}
```

### Pawn Move Generation (Most Complex Piece)

**File: `src/board/moves.rs` (continued)**

```rust
impl Board {
    fn generate_pawn_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.side_to_move;
        let piece = Piece { piece_type: PieceType::Pawn, color };
        
        let (forward_dir, start_rank, promotion_rank) = match color {
            Color::White => (1, 1, 7),
            Color::Black => (-1, 6, 0),
        };
        
        let from_rank = from.rank() as i8;
        let from_file = from.file() as i8;
        
        // Forward moves
        let one_forward = Square::from_coords(
            from_file as u8, 
            (from_rank + forward_dir) as u8
        );
        
        if !self.all_pieces.is_set(one_forward) {
            if (from_rank + forward_dir) as u8 == promotion_rank {
                // Promotion moves
                for promo_piece in [PieceType::Queen, PieceType::Rook, 
                                   PieceType::Bishop, PieceType::Knight] {
                    moves.push(Move {
                        from,
                        to: one_forward,
                        move_type: MoveType::Promotion(promo_piece),
                        piece,
                        captured: None,
                    });
                }
            } else {
                // Normal forward move
                moves.push(Move {
                    from,
                    to: one_forward,
                    move_type: MoveType::Normal,
                    piece,
                    captured: None,
                });
                
                // Double forward from starting position
                if from_rank == start_rank {
                    let two_forward = Square::from_coords(
                        from_file as u8, 
                        (from_rank + 2 * forward_dir) as u8
                    );
                    if !self.all_pieces.is_set(two_forward) {
                        moves.push(Move {
                            from,
                            to: two_forward,
                            move_type: MoveType::Normal,
                            piece,
                            captured: None,
                        });
                    }
                }
            }
        }
        
        // Capture moves
        for file_offset in [-1, 1] {
            let capture_file = from_file + file_offset;
            let capture_rank = from_rank + forward_dir;
            
            if capture_file >= 0 && capture_file < 8 && 
               capture_rank >= 0 && capture_rank < 8 {
                let capture_square = Square::from_coords(
                    capture_file as u8, 
                    capture_rank as u8
                );
                
                // Normal capture
                if let Some(captured_piece) = self.piece_at(capture_square) {
                    if captured_piece.color != color {
                        if capture_rank as u8 == promotion_rank {
                            // Capture with promotion
                            for promo_piece in [PieceType::Queen, PieceType::Rook, 
                                               PieceType::Bishop, PieceType::Knight] {
                                moves.push(Move {
                                    from,
                                    to: capture_square,
                                    move_type: MoveType::Promotion(promo_piece),
                                    piece,
                                    captured: Some(captured_piece),
                                });
                            }
                        } else {
                            moves.push(Move {
                                from,
                                to: capture_square,
                                move_type: MoveType::Capture,
                                piece,
                                captured: Some(captured_piece),
                            });
                        }
                    }
                }
                
                // En passant capture
                if let Some(ep_square) = self.en_passant_square {
                    if capture_square == ep_square {
                        let captured_pawn_square = Square::from_coords(
                            capture_file as u8,
                            from_rank as u8
                        );
                        let captured_piece = self.piece_at(captured_pawn_square).unwrap();
                        
                        moves.push(Move {
                            from,
                            to: capture_square,
                            move_type: MoveType::EnPassant,
                            piece,
                            captured: Some(captured_piece),
                        });
                    }
                }
            }
        }
    }
}
```

### Sliding Piece Move Generation

**File: `src/board/moves.rs` (continued)**

```rust
impl Board {
    fn generate_rook_moves(&self, from: Square, moves: &mut Vec<Move>) {
        let color = self.side_to_move;
        let piece = Piece { piece_type: PieceType::Rook, color };
        
        // Rook moves in 4 directions: North, South, East, West
        let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];
        
        for (file_dir, rank_dir) in directions {
            let mut current_file = from.file() as i8;
            let mut current_rank = from.rank() as i8;
            
            loop {
                current_file += file_dir;
                current_rank += rank_dir;
                
                // Check bounds
                if current_file < 0 || current_file >= 8 || 
                   current_rank < 0 || current_rank >= 8 {
                    break;
                }
                
                let to_square = Square::from_coords(
                    current_file as u8, 
                    current_rank as u8
                );
                
                if let Some(piece_at_square) = self.piece_at(to_square) {
                    // Square is occupied
                    if piece_at_square.color != color {
                        // Enemy piece - can capture
                        moves.push(Move {
                            from,
                            to: to_square,
                            move_type: MoveType::Capture,
                            piece,
                            captured: Some(piece_at_square),
                        });
                    }
                    // Can't move past any piece
                    break;
                } else {
                    // Empty square - can move here
                    moves.push(Move {
                        from,
                        to: to_square,
                        move_type: MoveType::Normal,
                        piece,
                        captured: None,
                    });
                }
            }
        }
    }
}
```

---

## 4. Basic Move Validation

### Legal vs Pseudo-legal
- **Pseudo-legal**: Move follows piece movement rules but might leave king in check
- **Legal**: Pseudo-legal move that doesn't leave own king in check

### Implementation Strategy

**File: `src/board/moves.rs` (continued)**

```rust
impl Board {
    pub fn is_legal_move(&self, mv: Move) -> bool {
        // Make the move on a copy of the board
        let mut test_board = self.clone();
        test_board.make_move(mv);
        
        // Check if our king is in check after the move
        !test_board.is_king_in_check(self.side_to_move)
    }
    
    pub fn is_king_in_check(&self, color: Color) -> bool {
        let king_square = self.find_king(color);
        self.is_square_attacked(king_square, color.opposite())
    }
    
    fn find_king(&self, color: Color) -> Square {
        let king_bb = self.piece_bitboards[color as usize][PieceType::King as usize];
        // There should always be exactly one king
        king_bb.first_square().expect("King not found")
    }
    
    pub fn is_square_attacked(&self, square: Square, by_color: Color) -> bool {
        // Check if any piece of by_color can attack the square
        
        // Check pawn attacks
        if self.is_square_attacked_by_pawn(square, by_color) {
            return true;
        }
        
        // Check knight attacks
        if self.is_square_attacked_by_knight(square, by_color) {
            return true;
        }
        
        // Check sliding piece attacks (bishop, rook, queen)
        if self.is_square_attacked_by_sliding_piece(square, by_color) {
            return true;
        }
        
        // Check king attacks
        if self.is_square_attacked_by_king(square, by_color) {
            return true;
        }
        
        false
    }
    
    fn is_square_attacked_by_pawn(&self, square: Square, by_color: Color) -> bool {
        let pawn_bb = self.piece_bitboards[by_color as usize][PieceType::Pawn as usize];
        
        let (attack_dir, _) = match by_color {
            Color::White => (-1, 1), // White pawns attack backwards from target square
            Color::Black => (1, 6),
        };
        
        let square_rank = square.rank() as i8;
        let square_file = square.file() as i8;
        
        // Check diagonal squares where attacking pawns could be
        for file_offset in [-1, 1] {
            let pawn_file = square_file + file_offset;
            let pawn_rank = square_rank + attack_dir;
            
            if pawn_file >= 0 && pawn_file < 8 && 
               pawn_rank >= 0 && pawn_rank < 8 {
                let pawn_square = Square::from_coords(
                    pawn_file as u8, 
                    pawn_rank as u8
                );
                
                if pawn_bb.is_set(pawn_square) {
                    return true;
                }
            }
        }
        
        false
    }
}
```

### Make/Unmake Move Pattern

**File: `src/board/make_move.rs`**

```rust
impl Board {
    pub fn make_move(&mut self, mv: Move) -> MoveUndo {
        let undo = MoveUndo {
            captured: mv.captured,
            en_passant_square: self.en_passant_square,
            castling_rights: self.castling_rights,
            halfmove_clock: self.halfmove_clock,
            position_hash: self.position_hash,
        };
        
        // Remove piece from source square
        self.remove_piece(mv.from);
        
        // Handle captures
        if let Some(captured) = mv.captured {
            match mv.move_type {
                MoveType::EnPassant => {
                    // En passant: captured piece is not on destination square
                    let captured_square = match self.side_to_move {
                        Color::White => Square::from_coords(mv.to.file(), mv.to.rank() - 1),
                        Color::Black => Square::from_coords(mv.to.file(), mv.to.rank() + 1),
                    };
                    self.remove_piece(captured_square);
                }
                _ => {
                    // Normal capture: remove piece from destination
                    self.remove_piece(mv.to);
                }
            }
        }
        
        // Place piece on destination square
        let final_piece = match mv.move_type {
            MoveType::Promotion(promo_type) => {
                Piece { piece_type: promo_type, color: mv.piece.color }
            }
            _ => mv.piece,
        };
        self.place_piece(mv.to, final_piece);
        
        // Handle special move types
        match mv.move_type {
            MoveType::Castle => self.handle_castling(mv),
            _ => {}
        }
        
        // Update game state
        self.update_castling_rights(mv);
        self.update_en_passant(mv);
        self.update_clocks(mv);
        self.side_to_move = self.side_to_move.opposite();
        
        undo
    }
    
    pub fn unmake_move(&mut self, mv: Move, undo: MoveUndo) {
        // Restore game state
        self.en_passant_square = undo.en_passant_square;
        self.castling_rights = undo.castling_rights;
        self.halfmove_clock = undo.halfmove_clock;
        self.position_hash = undo.position_hash;
        self.side_to_move = self.side_to_move.opposite();
        
        // Remove piece from destination
        self.remove_piece(mv.to);
        
        // Restore piece to source
        self.place_piece(mv.from, mv.piece);
        
        // Restore captured piece if any
        if let Some(captured) = undo.captured {
            match mv.move_type {
                MoveType::EnPassant => {
                    let captured_square = match mv.piece.color {
                        Color::White => Square::from_coords(mv.to.file(), mv.to.rank() - 1),
                        Color::Black => Square::from_coords(mv.to.file(), mv.to.rank() + 1),
                    };
                    self.place_piece(captured_square, captured);
                }
                _ => {
                    self.place_piece(mv.to, captured);
                }
            }
        }
        
        // Handle special move undos
        match mv.move_type {
            MoveType::Castle => self.undo_castling(mv),
            _ => {}
        }
    }
}

**File: `src/board/make_move.rs` (continued)**

```rust
pub struct MoveUndo {
    captured: Option<Piece>,
    en_passant_square: Option<Square>,
    castling_rights: CastlingRights,
    halfmove_clock: u16,
    position_hash: u64,
}
```

### Performance Optimizations

**Bitboard Operations**: Use bitwise operations for fast piece detection
**Magic Bitboards**: Precomputed attack tables for sliding pieces
**Copy-Make vs Make-Unmake**: 
- Copy-Make: Clone board, make move, test legality (simpler but slower)
- Make-Unmake: Make move, test, unmake (faster but more complex)

**Move Ordering**: Generate captures first, as they're more likely to be good moves

---

## Additional Type Definitions

**File: `src/types/mod.rs`**

```rust
pub mod piece;
pub mod square;
pub mod castling;

pub use piece::*;
pub use square::*;
pub use castling::*;
```

**File: `src/types/piece.rs`**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub piece_type: PieceType,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    White = 0,
    Black = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceType {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl Color {
    pub fn opposite(self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}
```

**File: `src/types/castling.rs`**

```rust
#[derive(Clone, Copy, Debug)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

impl CastlingRights {
    pub fn all() -> Self {
        Self {
            white_kingside: true,
            white_queenside: true,
            black_kingside: true,
            black_queenside: true,
        }
    }
    
    pub fn none() -> Self {
        Self {
            white_kingside: false,
            white_queenside: false,
            black_kingside: false,
            black_queenside: false,
        }
    }
    
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        if fen == "-" {
            return Ok(Self::none());
        }
        
        let mut rights = Self::none();
        for ch in fen.chars() {
            match ch {
                'K' => rights.white_kingside = true,
                'Q' => rights.white_queenside = true,
                'k' => rights.black_kingside = true,
                'q' => rights.black_queenside = true,
                _ => return Err(FenError::InvalidCastlingRights),
            }
        }
        Ok(rights)
    }
}
```

**File: `src/board/mod.rs`**

```rust
pub mod board;
pub mod fen;
pub mod moves;
pub mod make_move;

pub use board::*;
pub use moves::*;
pub use make_move::*;

#[derive(Debug)]
pub enum FenError {
    InvalidFormat,
    InvalidPiecePlacement,
    InvalidActiveColor,
    InvalidCastlingRights,
    InvalidPiece,
    InvalidHalfmove,
    InvalidFullmove,
}
```

This implementation provides a solid foundation for Phase 3. The key insight is balancing **correctness** (handling all chess rules) with **performance** (efficient bitboard operations and minimal copying).