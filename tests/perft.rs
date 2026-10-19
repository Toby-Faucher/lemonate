use lemonate::Board;

fn perft(board: &mut Board, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut buf = Vec::new();
    board.generate_legal_moves_into(&mut buf);
    if depth == 1 {
        return buf.len() as u64;
    }
    let mut nodes = 0;
    for i in 0..buf.len() {
        let mv = buf[i];
        let undo = board.do_move(mv);
        nodes += perft(board, depth - 1);
        board.undo_move(mv, undo);
    }
    nodes
}

/// `expected[i]` is the node count at depth `i + 1`.
fn check(fen: &str, expected: &[u64]) {
    let mut board = Board::from_fen(fen).expect("valid FEN");
    for (i, &want) in expected.iter().enumerate() {
        let depth = (i + 1) as u8;
        assert_eq!(perft(&mut board, depth), want, "perft({depth}) of {fen}");
    }
}

#[test]
fn startpos() {
    check(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        &[20, 400, 8_902, 197_281, 4_865_609],
    );
}

#[test]
fn kiwipete() {
    check(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        &[48, 2_039, 97_862, 4_085_603],
    );
}

#[test]
fn position3_en_passant_pins() {
    check(
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        &[14, 191, 2_812, 43_238, 674_624],
    );
}

#[test]
fn position4_promotions_and_castling() {
    check(
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        &[6, 264, 9_467, 422_333],
    );
}

#[test]
fn position5() {
    check(
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        &[44, 1_486, 62_379, 2_103_487],
    );
}

// The positions below were counted with Stockfish (`go perft`), not copied from memory.

#[test]
fn position6_quiet_middlegame() {
    check(
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        &[46, 2_079, 89_890, 3_894_594],
    );
}

#[test]
fn promotions_and_promotion_captures() {
    check(
        "n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1",
        &[24, 496, 9_483, 182_838],
    );
    check("8/Pk6/8/8/8/8/6Kp/8 w - - 0 1", &[11, 97, 887, 8_048]);
}

#[test]
fn castling_rights_lost_by_captures_and_checks() {
    check(
        "r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1",
        &[26, 1_141, 27_826, 1_274_206],
    );
    check("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", &[26, 568, 13_744, 314_346]);
}

#[test]
fn en_passant_that_exposes_the_king() {
    // Capturing en passant removes two pawns from a rank at once; both positions have a
    // capture that is pseudo-legal but leaves the king in check.
    check("8/8/8/8/k2Pp2Q/8/8/3K4 b - d3 0 1", &[6, 136, 863, 20_471]);
    check("8/8/1k6/2b5/2pP4/8/5K2/8 b - d3 0 1", &[15, 126, 1_928, 13_931]);
}

