use board_crab_lib::fen;
use board_crab_lib::search;

fn do_test(name: &str, position_fen: &str, depth: usize, target_node_count: usize) {
    board_crab_lib::init();
    let board = fen::load_fen(position_fen).unwrap();
    let perft_count = search::perft(&board, depth as u8, false);

    if perft_count != target_node_count {
        // Test failed
        panic!(
            "Failed position \"{}\" at depth {} (got: {}, target: {}), fen: \"{}\"",
            name, depth, perft_count, target_node_count, position_fen
        );
    }
}

#[test]
fn depth_1_perft_test() {
    let test_entries = [
        (
            "starting position",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            20,
        ),
        (
            "basic captures",
            "1Q6/5k2/3q1pp1/4p3/3P3p/1pP2N2/1KB5/2n2R2 w - - 0 1",
            41,
        ),
        (
            "crazy captures",
            "4N3/3k2b1/1b3r2/2rbr1B1/1PQKBP2/3bN3/2r5/8 w - - 0 1",
            37,
        ),
        ("basic pin", "8/4k3/4r3/8/8/4N3/4K3/8 w - - 0 1", 7),
        ("free pin", "8/3k4/3q4/8/8/3R4/3K4/8 w - - 0 1", 10),
        ("en passant", "5k2/8/8/3Pp3/2K5/8/8/8 w - e6 0 2", 8),
        (
            "bishop-pin passant (legal)",
            "8/2b5/1k6/3pP3/5K2/8/8/8 w - d6 0 2",
            7,
        ),
        (
            "bishop-pin passant (illegal)",
            "8/2k5/5K2/3pP3/8/2b5/8/8 w - d6 0 2",
            7,
        ),
        (
            "rook-pin passant (illegal)",
            "8/2k5/8/1r1pPK2/8/8/8/8 w - d6 0 2",
            7,
        ),
        (
            "almost-rook-pin passant (legal)",
            "8/2k5/8/rn1pPK2/8/8/8/8 w - d6 0 2",
            8,
        ),
        (
            "en passant check capture",
            "8/6k1/6p1/4NpPp/3PK2P/1r2P3/1br5/4RR2 w - f6 0 33",
            5,
        ),
        (
            "single check 1",
            "1k6/ppp3r1/8/8/4P1K1/1P1P1N2/PBP5/8 w - - 0 1",
            7,
        ),
        ("double check", "8/8/6k1/8/5q2/5n2/3KN3/8 w - - 0 1", 4),
        ("basic both-castle", "4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", 26),
        ("basic no-castle", "4k3/8/8/8/8/8/8/R3K2R w - - 0 1", 24),
        ("funky one-castle", "8/8/4k3/5q2/8/8/8/R3K2R w KQ - 0 1", 23),
        ("basic promotion", "8/6P1/8/5K2/1k6/8/8/8 w - - 0 1", 12),
        (
            "hell",
            "3Nk1n1/1q2p2P/3p1p2/2r2Pp1/rb3nPq/2BB1b2/P4Q2/R3K2R w - g6 0 2",
            36,
        ),
        (
            "crazy mate in 1",
            "4b3/BP1P1P1P/q5Q1/1R1NkN1R/4B1n1/1n1P1Pb1/1r5r/3K4 w - - 0 1",
            71,
        ),
        // ^ is from https://www.reddit.com/r/chess/comments/13v0elu/a_complicated_mate_in_1/
    ];

    for pair in test_entries {
        do_test(pair.0, pair.1, 1, pair.2);
    }
}

#[test]
fn depth_2_perft_test() {
    let test_entries = [
        ("castling", "r3k2r/3N4/8/8/4B3/4K3/4R3/8 w kq - 0 1", 724),
        (
            "many checks",
            "1NR1n3/1k1n4/1q6/PB6/8/6Q1/6K1/8 w - - 0 1",
            1162,
        ),
    ];

    for pair in test_entries {
        do_test(pair.0, pair.1, 2, pair.2);
    }
}

// This one features very difficult positions from https://www.chessprogramming.org/Perft_Results and other places
#[test]
fn super_perft_test() {
    let test_entries = [
        (
            "Complex 1 (Kiwipete)",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            vec![48, 2039, 97862],
        ),
        (
            "Complex 2 (Talkchess)",
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            vec![44, 1486, 62379],
        ),
        (
            "Complex 3 (Steven Edwards)",
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            vec![46, 2079, 89890],
        ),
        (
            "Rooks and En Passant",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -",
            vec![14, 191, 2812, 43238],
        ),
    ];

    for pair in test_entries {
        let name = pair.0;
        let fen_str = pair.1;
        let target_perft_results = pair.2;

        for i in 0..target_perft_results.len() {
            let depth = i + 1;
            do_test(name, fen_str, depth, target_perft_results[i]);
        }
    }
}
