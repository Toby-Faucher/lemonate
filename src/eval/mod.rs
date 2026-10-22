use crate::types::Color;
use crate::Board;

mod king_safety;
mod material;
mod mobility;
mod pawn_structure;
mod phase;
mod pst;
mod weights;

pub use king_safety::KingSafetyEval;
pub use material::MaterialEvaluator;
pub use mobility::MobilityEval;
pub use pawn_structure::PawnStructureEval;
pub use phase::GamePhase;
pub use pst::PieceSquareTableEval;
pub use weights::EvalWeights;

pub struct Evaluator {
    pst: PieceSquareTableEval,
    phase: GamePhase,
    pawn_structure: PawnStructureEval,
    king_safety: KingSafetyEval,
    mobility: MobilityEval,
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            pst: PieceSquareTableEval::new(),
            phase: GamePhase::new(),
            pawn_structure: PawnStructureEval::new(),
            king_safety: KingSafetyEval::new(),
            mobility: MobilityEval::new(),
        }
    }

    pub fn evaluate(&self, board: &Board) -> i32 {
        // Material that cannot force mate is a draw whatever the piece-square tables say.
        // (Only here, not in static_eval_white_pov, so the tuner's training scores are
        // untouched.)
        if board.is_drawn_material() {
            return 0;
        }
        let score = self.static_eval_white_pov(board, &EvalWeights::DEFAULT);

        // Return score from side-to-move's perspective for negamax
        if board.side_to_move() == Color::White {
            score
        } else {
            -score
        }
    }

    /// The tapered evaluation score from White's perspective (no
    /// side-to-move flip), computed with an explicit `weights` set.
    /// Used by the tuner to score training positions consistently
    /// regardless of which side is to move.
    pub fn static_eval_white_pov(&self, board: &Board, weights: &EvalWeights) -> i32 {
        let phase = self.phase.calculate(board);
        self.pst.evaluate_with_phase_and_weights(board, phase, weights)
            + self
                .pawn_structure
                .evaluate_with_phase_and_weights(board, phase, weights)
            + self
                .king_safety
                .evaluate_with_phase_and_weights(board, phase, weights)
            + self
                .mobility
                .evaluate_with_phase_and_weights(board, phase, weights)
    }

    pub fn evaluate_detailed(&self, board: &Board) -> EvalDetails {
        EvalDetails {
            pst: self.pst.evaluate(board),
            pawn_structure: self.pawn_structure.evaluate(board),
            king_safety: self.king_safety.evaluate(board),
            mobility: self.mobility.evaluate(board),
            phase: self.phase.calculate(board),
        }
    }
}

pub struct EvalDetails {
    pub pst: i32,
    pub pawn_structure: i32,
    pub king_safety: i32,
    pub mobility: i32,
    pub phase: i32,
}

impl EvalDetails {
    pub fn total(&self) -> i32 {
        self.pst + self.pawn_structure + self.king_safety + self.mobility
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

pub fn evaluate(board: &Board) -> i32 {
    let evaluator = Evaluator::new();
    evaluator.evaluate(board)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Board;

    const KNOWN_CASES: [(&str, i32); 4] = [
        ("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 0),
        (
            "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1",
            449,
        ),
        ("r3k2r/ppp2ppp/8/8/8/8/PPP2PPP/R3K2R w KQkq - 0 1", 0),
        (
            "r1bqk2r/pp1nbppp/2p2n2/3p4/3P4/2N1PN2/PPP1BPPP/R1BQK2R w KQ - 0 1",
            245,
        ),
    ];

    #[test]
    fn evaluate_matches_pre_refactor_values() {
        let evaluator = Evaluator::new();
        for (fen, expected) in KNOWN_CASES {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(evaluator.evaluate(&board), expected, "mismatch for {}", fen);
        }
    }

    #[test]
    fn static_eval_white_pov_matches_known_values() {
        // All KNOWN_CASES FENs have White to move, so evaluate() and
        // static_eval_white_pov() coincide (no side-to-move flip applies).
        let evaluator = Evaluator::new();
        for (fen, expected) in KNOWN_CASES {
            let board = Board::from_fen(fen).unwrap();
            let score = evaluator.static_eval_white_pov(&board, &EvalWeights::DEFAULT);
            assert_eq!(score, expected, "white-POV mismatch for {}", fen);
        }
    }

    #[test]
    fn drawn_material_evaluates_to_zero() {
        let evaluator = Evaluator::new();
        let drawn = [
            "8/8/8/4k3/8/8/8/K7 w - - 0 1",         // K v K
            "8/8/8/4k3/8/8/8/K6B w - - 0 1",        // K+B v K
            "8/8/8/4k3/8/8/8/K6N b - - 0 1",        // K+N v K
            "8/8/8/4k3/8/8/8/KNN5 w - - 0 1",       // K+N+N v K
            "4k3/8/8/8/8/8/8/KB5b w - - 0 1",       // K+B v K+B, bishops on b1 and h1 (both light squares)
            "4kb2/8/8/8/8/8/8/KB6 w - - 0 1",       // K+B v K+B, opposite colours, no pawns
        ];
        for fen in drawn {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(evaluator.evaluate(&board), 0, "should be a draw: {}", fen);
        }
    }

    #[test]
    fn mating_material_is_not_scored_as_a_draw() {
        let evaluator = Evaluator::new();
        let winning = [
            "8/8/8/4k3/8/8/8/K6R w - - 0 1",        // K+R v K
            "4k3/8/8/8/8/8/P7/K7 w - - 0 1",        // K+P v K
            "8/8/8/4k3/8/8/8/KBN5 w - - 0 1",       // K+B+N v K
            "4k3/8/8/8/8/8/8/KN5n w - - 0 1",       // K+N v K+N: mate is possible
            "4kb2/8/8/8/8/8/P7/KB6 w - - 0 1",      // opposite bishops WITH a pawn
        ];
        for fen in winning {
            let board = Board::from_fen(fen).unwrap();
            assert_ne!(evaluator.evaluate(&board), 0, "should not be a draw: {}", fen);
        }
    }

    #[test]
    fn evaluate_flips_sign_for_black_to_move() {
        // Same position as one of KNOWN_CASES but with Black to move —
        // static_eval_white_pov must be unchanged (side-to-move never
        // enters that computation), while evaluate() must return its
        // negation. Values captured directly from the compiled engine.
        let fen = "r1bqkbnr/pppppppp/8/8/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 0 1";
        let board = Board::from_fen(fen).unwrap();
        let evaluator = Evaluator::new();

        assert_eq!(evaluator.evaluate(&board), -449);
        assert_eq!(
            evaluator.static_eval_white_pov(&board, &EvalWeights::DEFAULT),
            449
        );
    }

    /// Colour-mirror a FEN: flip ranks, swap piece case, swap side to move.
    fn mirror_fen(fen: &str) -> String {
        let mut parts = fen.split(' ');
        let board: Vec<String> = parts
            .next()
            .unwrap()
            .split('/')
            .rev()
            .map(|r| {
                r.chars()
                    .map(|c| {
                        if c.is_ascii_uppercase() {
                            c.to_ascii_lowercase()
                        } else {
                            c.to_ascii_uppercase()
                        }
                    })
                    .collect()
            })
            .collect();
        let stm = if parts.next().unwrap() == "w" { "b" } else { "w" };
        format!("{} {} - - 0 1", board.join("/"), stm)
    }

    #[test]
    fn eval_is_colour_symmetric() {
        let evaluator = Evaluator::new();
        for fen in [
            "r1bqk2r/pp1nbppp/2p2n2/3p4/3P4/2N1PN2/PPP1BPPP/R1BQK2R w - - 0 1",
            "r2qk2r/pp1n1ppp/2p2n2/3p4/3P4/2N1PN2/PPP1BPPP/R1BQK2R w - - 0 1",
            "4k3/8/8/3b4/8/2B1B3/8/4K3 b - - 0 1",
        ] {
            let a = evaluator.evaluate(&Board::from_fen(fen).unwrap());
            let b = evaluator.evaluate(&Board::from_fen(&mirror_fen(fen)).unwrap());
            assert_eq!(a, b, "asymmetric eval for {}", fen);
        }
    }

    #[test]
    fn bishop_pair_bonus_fires_only_with_two_bishops() {
        let evaluator = Evaluator::new();
        let w = &EvalWeights::DEFAULT;
        let pov = |fen: &str| evaluator.static_eval_white_pov(&Board::from_fen(fen).unwrap(), w);
        // White has two bishops vs one bishop + one knight; neutral case swaps the second
        // bishop for a knight on a square of similar value.
        let pair = pov("4k3/8/8/8/8/8/8/2B1KB2 w - - 0 1");
        let no_pair = pov("4k3/8/8/8/8/8/8/2B1KN2 w - - 0 1");
        assert!(pair - no_pair > 20, "pair {} vs no pair {}", pair, no_pair);
        // One bishop each: term must not fire for either side.
        assert_eq!(pov("4kb2/8/8/8/8/8/8/4KB2 w - - 0 1"), 0);
        // Both sides with the pair cancel.
        assert_eq!(pov("2b1kb2/8/8/8/8/8/8/2B1KB2 w - - 0 1"), 0);
    }
}
