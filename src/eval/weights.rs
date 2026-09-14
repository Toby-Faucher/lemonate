use super::king_safety::{
    OPEN_FILE_NEAR_KING_EG, OPEN_FILE_NEAR_KING_MG, PAWN_SHIELD_CLOSE_EG, PAWN_SHIELD_CLOSE_MG,
    PAWN_SHIELD_FAR_EG, PAWN_SHIELD_FAR_MG, SEMI_OPEN_FILE_NEAR_KING_EG,
    SEMI_OPEN_FILE_NEAR_KING_MG,
};
use super::mobility::{
    BISHOP_MOBILITY_EG, BISHOP_MOBILITY_MG, KNIGHT_MOBILITY_EG, KNIGHT_MOBILITY_MG,
    QUEEN_MOBILITY_EG, QUEEN_MOBILITY_MG, ROOK_MOBILITY_EG, ROOK_MOBILITY_MG,
};
use super::pawn_structure::{
    BACKWARD_PAWN_EG, BACKWARD_PAWN_MG, CONNECTED_PAWN_EG, CONNECTED_PAWN_MG, DOUBLED_PAWN_EG,
    DOUBLED_PAWN_MG, ISOLATED_PAWN_EG, ISOLATED_PAWN_MG, PASSED_PAWN_EG, PASSED_PAWN_MG,
};
use super::pst::{
    EG_BISHOP_TABLE, EG_KING_TABLE, EG_KNIGHT_TABLE, EG_PAWN_TABLE, EG_QUEEN_TABLE, EG_ROOK_TABLE,
    MG_BISHOP_TABLE, MG_KING_TABLE, MG_KNIGHT_TABLE, MG_PAWN_TABLE, MG_QUEEN_TABLE, MG_ROOK_TABLE,
};

/// Every tunable evaluation constant, grouped into one struct so the
/// tuner binary can construct alternative candidate weight sets at
/// runtime without touching the engine's hardcoded consts.
///
/// `DEFAULT` mirrors today's values exactly by referencing the same
/// per-module consts the engine has always used, so pasting tuner
/// output into those consts (in `pst.rs`, `pawn_structure.rs`,
/// `king_safety.rs`, `mobility.rs`) updates `DEFAULT` automatically on
/// the next build - there is no separate copy to keep in sync.
///
/// Material values (`MG_VALUE`/`EG_VALUE` in `pst.rs`) are
/// deliberately not included here - they stay fixed.
#[derive(Clone, Debug, PartialEq)]
pub struct EvalWeights {
    pub pst_mg: [[i32; 64]; 6],
    pub pst_eg: [[i32; 64]; 6],

    pub doubled_pawn_mg: i32,
    pub doubled_pawn_eg: i32,
    pub isolated_pawn_mg: i32,
    pub isolated_pawn_eg: i32,
    pub backward_pawn_mg: i32,
    pub backward_pawn_eg: i32,
    pub passed_pawn_mg: [i32; 8],
    pub passed_pawn_eg: [i32; 8],
    pub connected_pawn_mg: i32,
    pub connected_pawn_eg: i32,

    pub pawn_shield_close_mg: i32,
    pub pawn_shield_close_eg: i32,
    pub pawn_shield_far_mg: i32,
    pub pawn_shield_far_eg: i32,
    pub open_file_near_king_mg: i32,
    pub open_file_near_king_eg: i32,
    pub semi_open_file_near_king_mg: i32,
    pub semi_open_file_near_king_eg: i32,

    pub knight_mobility_mg: [i32; 9],
    pub knight_mobility_eg: [i32; 9],
    pub bishop_mobility_mg: [i32; 14],
    pub bishop_mobility_eg: [i32; 14],
    pub rook_mobility_mg: [i32; 15],
    pub rook_mobility_eg: [i32; 15],
    pub queen_mobility_mg: [i32; 28],
    pub queen_mobility_eg: [i32; 28],
}

impl EvalWeights {
    pub const DEFAULT: EvalWeights = EvalWeights {
        pst_mg: [
            MG_PAWN_TABLE,
            MG_KNIGHT_TABLE,
            MG_BISHOP_TABLE,
            MG_ROOK_TABLE,
            MG_QUEEN_TABLE,
            MG_KING_TABLE,
        ],
        pst_eg: [
            EG_PAWN_TABLE,
            EG_KNIGHT_TABLE,
            EG_BISHOP_TABLE,
            EG_ROOK_TABLE,
            EG_QUEEN_TABLE,
            EG_KING_TABLE,
        ],
        doubled_pawn_mg: DOUBLED_PAWN_MG,
        doubled_pawn_eg: DOUBLED_PAWN_EG,
        isolated_pawn_mg: ISOLATED_PAWN_MG,
        isolated_pawn_eg: ISOLATED_PAWN_EG,
        backward_pawn_mg: BACKWARD_PAWN_MG,
        backward_pawn_eg: BACKWARD_PAWN_EG,
        passed_pawn_mg: PASSED_PAWN_MG,
        passed_pawn_eg: PASSED_PAWN_EG,
        connected_pawn_mg: CONNECTED_PAWN_MG,
        connected_pawn_eg: CONNECTED_PAWN_EG,
        pawn_shield_close_mg: PAWN_SHIELD_CLOSE_MG,
        pawn_shield_close_eg: PAWN_SHIELD_CLOSE_EG,
        pawn_shield_far_mg: PAWN_SHIELD_FAR_MG,
        pawn_shield_far_eg: PAWN_SHIELD_FAR_EG,
        open_file_near_king_mg: OPEN_FILE_NEAR_KING_MG,
        open_file_near_king_eg: OPEN_FILE_NEAR_KING_EG,
        semi_open_file_near_king_mg: SEMI_OPEN_FILE_NEAR_KING_MG,
        semi_open_file_near_king_eg: SEMI_OPEN_FILE_NEAR_KING_EG,
        knight_mobility_mg: KNIGHT_MOBILITY_MG,
        knight_mobility_eg: KNIGHT_MOBILITY_EG,
        bishop_mobility_mg: BISHOP_MOBILITY_MG,
        bishop_mobility_eg: BISHOP_MOBILITY_EG,
        rook_mobility_mg: ROOK_MOBILITY_MG,
        rook_mobility_eg: ROOK_MOBILITY_EG,
        queen_mobility_mg: QUEEN_MOBILITY_MG,
        queen_mobility_eg: QUEEN_MOBILITY_EG,
    };

    /// Flattens every tunable scalar into one vector, in a fixed order,
    /// for the coordinate-descent optimizer to iterate over. Paired
    /// with `from_vec`.
    pub fn to_vec(&self) -> Vec<i32> {
        let mut v = Vec::with_capacity(768 + 24 + 8 + 132);
        for table in &self.pst_mg {
            v.extend_from_slice(table);
        }
        for table in &self.pst_eg {
            v.extend_from_slice(table);
        }
        v.push(self.doubled_pawn_mg);
        v.push(self.doubled_pawn_eg);
        v.push(self.isolated_pawn_mg);
        v.push(self.isolated_pawn_eg);
        v.push(self.backward_pawn_mg);
        v.push(self.backward_pawn_eg);
        v.extend_from_slice(&self.passed_pawn_mg);
        v.extend_from_slice(&self.passed_pawn_eg);
        v.push(self.connected_pawn_mg);
        v.push(self.connected_pawn_eg);
        v.push(self.pawn_shield_close_mg);
        v.push(self.pawn_shield_close_eg);
        v.push(self.pawn_shield_far_mg);
        v.push(self.pawn_shield_far_eg);
        v.push(self.open_file_near_king_mg);
        v.push(self.open_file_near_king_eg);
        v.push(self.semi_open_file_near_king_mg);
        v.push(self.semi_open_file_near_king_eg);
        v.extend_from_slice(&self.knight_mobility_mg);
        v.extend_from_slice(&self.knight_mobility_eg);
        v.extend_from_slice(&self.bishop_mobility_mg);
        v.extend_from_slice(&self.bishop_mobility_eg);
        v.extend_from_slice(&self.rook_mobility_mg);
        v.extend_from_slice(&self.rook_mobility_eg);
        v.extend_from_slice(&self.queen_mobility_mg);
        v.extend_from_slice(&self.queen_mobility_eg);
        v
    }

    /// Rebuilds an `EvalWeights` from a vector produced by `to_vec`.
    /// Panics if `v.len()` doesn't match the expected total (768 + 24
    /// + 8 + 132 = 932).
    pub fn from_vec(v: &[i32]) -> Self {
        assert_eq!(v.len(), 768 + 24 + 8 + 132, "unexpected parameter vector length");

        let mut i = 0;
        let mut take = |n: usize| {
            let slice = &v[i..i + n];
            i += n;
            slice
        };

        let mut pst_mg = [[0i32; 64]; 6];
        for table in pst_mg.iter_mut() {
            table.copy_from_slice(take(64));
        }
        let mut pst_eg = [[0i32; 64]; 6];
        for table in pst_eg.iter_mut() {
            table.copy_from_slice(take(64));
        }

        let doubled_pawn_mg = take(1)[0];
        let doubled_pawn_eg = take(1)[0];
        let isolated_pawn_mg = take(1)[0];
        let isolated_pawn_eg = take(1)[0];
        let backward_pawn_mg = take(1)[0];
        let backward_pawn_eg = take(1)[0];

        let mut passed_pawn_mg = [0i32; 8];
        passed_pawn_mg.copy_from_slice(take(8));
        let mut passed_pawn_eg = [0i32; 8];
        passed_pawn_eg.copy_from_slice(take(8));

        let connected_pawn_mg = take(1)[0];
        let connected_pawn_eg = take(1)[0];
        let pawn_shield_close_mg = take(1)[0];
        let pawn_shield_close_eg = take(1)[0];
        let pawn_shield_far_mg = take(1)[0];
        let pawn_shield_far_eg = take(1)[0];
        let open_file_near_king_mg = take(1)[0];
        let open_file_near_king_eg = take(1)[0];
        let semi_open_file_near_king_mg = take(1)[0];
        let semi_open_file_near_king_eg = take(1)[0];

        let mut knight_mobility_mg = [0i32; 9];
        knight_mobility_mg.copy_from_slice(take(9));
        let mut knight_mobility_eg = [0i32; 9];
        knight_mobility_eg.copy_from_slice(take(9));
        let mut bishop_mobility_mg = [0i32; 14];
        bishop_mobility_mg.copy_from_slice(take(14));
        let mut bishop_mobility_eg = [0i32; 14];
        bishop_mobility_eg.copy_from_slice(take(14));
        let mut rook_mobility_mg = [0i32; 15];
        rook_mobility_mg.copy_from_slice(take(15));
        let mut rook_mobility_eg = [0i32; 15];
        rook_mobility_eg.copy_from_slice(take(15));
        let mut queen_mobility_mg = [0i32; 28];
        queen_mobility_mg.copy_from_slice(take(28));
        let mut queen_mobility_eg = [0i32; 28];
        queen_mobility_eg.copy_from_slice(take(28));

        EvalWeights {
            pst_mg,
            pst_eg,
            doubled_pawn_mg,
            doubled_pawn_eg,
            isolated_pawn_mg,
            isolated_pawn_eg,
            backward_pawn_mg,
            backward_pawn_eg,
            passed_pawn_mg,
            passed_pawn_eg,
            connected_pawn_mg,
            connected_pawn_eg,
            pawn_shield_close_mg,
            pawn_shield_close_eg,
            pawn_shield_far_mg,
            pawn_shield_far_eg,
            open_file_near_king_mg,
            open_file_near_king_eg,
            semi_open_file_near_king_mg,
            semi_open_file_near_king_eg,
            knight_mobility_mg,
            knight_mobility_eg,
            bishop_mobility_mg,
            bishop_mobility_eg,
            rook_mobility_mg,
            rook_mobility_eg,
            queen_mobility_mg,
            queen_mobility_eg,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::piece::PieceType;

    #[test]
    fn default_pst_matches_original_tables() {
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Pawn as usize], MG_PAWN_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Knight as usize], MG_KNIGHT_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Bishop as usize], MG_BISHOP_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Rook as usize], MG_ROOK_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::Queen as usize], MG_QUEEN_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_mg[PieceType::King as usize], MG_KING_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Pawn as usize], EG_PAWN_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Knight as usize], EG_KNIGHT_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Bishop as usize], EG_BISHOP_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Rook as usize], EG_ROOK_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::Queen as usize], EG_QUEEN_TABLE);
        assert_eq!(EvalWeights::DEFAULT.pst_eg[PieceType::King as usize], EG_KING_TABLE);
    }

    #[test]
    fn default_pawn_structure_matches_originals() {
        assert_eq!(EvalWeights::DEFAULT.doubled_pawn_mg, DOUBLED_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.doubled_pawn_eg, DOUBLED_PAWN_EG);
        assert_eq!(EvalWeights::DEFAULT.isolated_pawn_mg, ISOLATED_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.isolated_pawn_eg, ISOLATED_PAWN_EG);
        assert_eq!(EvalWeights::DEFAULT.backward_pawn_mg, BACKWARD_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.backward_pawn_eg, BACKWARD_PAWN_EG);
        assert_eq!(EvalWeights::DEFAULT.passed_pawn_mg, PASSED_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.passed_pawn_eg, PASSED_PAWN_EG);
        assert_eq!(EvalWeights::DEFAULT.connected_pawn_mg, CONNECTED_PAWN_MG);
        assert_eq!(EvalWeights::DEFAULT.connected_pawn_eg, CONNECTED_PAWN_EG);
    }

    #[test]
    fn default_king_safety_matches_originals() {
        assert_eq!(EvalWeights::DEFAULT.pawn_shield_close_mg, PAWN_SHIELD_CLOSE_MG);
        assert_eq!(EvalWeights::DEFAULT.pawn_shield_close_eg, PAWN_SHIELD_CLOSE_EG);
        assert_eq!(EvalWeights::DEFAULT.pawn_shield_far_mg, PAWN_SHIELD_FAR_MG);
        assert_eq!(EvalWeights::DEFAULT.pawn_shield_far_eg, PAWN_SHIELD_FAR_EG);
        assert_eq!(EvalWeights::DEFAULT.open_file_near_king_mg, OPEN_FILE_NEAR_KING_MG);
        assert_eq!(EvalWeights::DEFAULT.open_file_near_king_eg, OPEN_FILE_NEAR_KING_EG);
        assert_eq!(
            EvalWeights::DEFAULT.semi_open_file_near_king_mg,
            SEMI_OPEN_FILE_NEAR_KING_MG
        );
        assert_eq!(
            EvalWeights::DEFAULT.semi_open_file_near_king_eg,
            SEMI_OPEN_FILE_NEAR_KING_EG
        );
    }

    #[test]
    fn default_mobility_matches_originals() {
        assert_eq!(EvalWeights::DEFAULT.knight_mobility_mg, KNIGHT_MOBILITY_MG);
        assert_eq!(EvalWeights::DEFAULT.knight_mobility_eg, KNIGHT_MOBILITY_EG);
        assert_eq!(EvalWeights::DEFAULT.bishop_mobility_mg, BISHOP_MOBILITY_MG);
        assert_eq!(EvalWeights::DEFAULT.bishop_mobility_eg, BISHOP_MOBILITY_EG);
        assert_eq!(EvalWeights::DEFAULT.rook_mobility_mg, ROOK_MOBILITY_MG);
        assert_eq!(EvalWeights::DEFAULT.rook_mobility_eg, ROOK_MOBILITY_EG);
        assert_eq!(EvalWeights::DEFAULT.queen_mobility_mg, QUEEN_MOBILITY_MG);
        assert_eq!(EvalWeights::DEFAULT.queen_mobility_eg, QUEEN_MOBILITY_EG);
    }

    #[test]
    fn to_vec_from_vec_round_trips() {
        let original = EvalWeights::DEFAULT.clone();
        let flat = original.to_vec();
        let rebuilt = EvalWeights::from_vec(&flat);

        assert_eq!(rebuilt, original);
    }

    #[test]
    fn to_vec_has_expected_length() {
        // 6 pieces * 64 squares * 2 (mg/eg) = 768 PST values, plus
        // 6 pawn scalars + 8+8 passed-pawn arrays + 2 connected scalars = 24,
        // plus 8 king-safety scalars,
        // plus (9+9+14+14+15+15+28+28) = 132 mobility values.
        assert_eq!(EvalWeights::DEFAULT.to_vec().len(), 768 + 24 + 8 + 132);
    }
}
