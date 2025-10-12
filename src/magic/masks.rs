use crate::bitboard::Bitboard;
use crate::types::PieceType;
use crate::bitboard::Square;

fn generate_ray(square: Square, file_delta: i8, rank_delta: i8, mask: &mut Bitboard) {
    let mut cf = square.file() as i8;
    let mut cr = square.rank() as i8;
    loop {
        cf += file_delta;
        cr += rank_delta;

        if matches!(cf, 0 | 7) || matches!(cr, 0 | 7) {
            break;
        }

        let new_square = Square::from_coords(cf as u8, cr as u8);
        mask.set(new_square);
    }
}

fn generate_occupancy_mask(square: Square, piece_type: PieceType) -> Bitboard {
    let mut mask = Bitboard::EMPTY;

    match piece_type {
        PieceType::Rook => {
            let dirs = [(0, 1), (0, -1), (1, 0), (-1, 0)];
            for (df, dr) in dirs {
                generate_ray(square, df, dr, &mut mask);
            }
        }
        PieceType::Bishop => {
            let dirs = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
            for (df, dr) in dirs {
                generate_ray(square, df, dr, &mut mask);
            }
        }
        _ => {
            // Other pieces don't need occupany masks
        }
    }
    mask
}
