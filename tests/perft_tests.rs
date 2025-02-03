use board_crab_lib::board::*;
use board_crab_lib::fen;
use board_crab_lib::search;
use board_crab_lib::move_gen;

fn do_test(name: &str, position_fen: &str, depth: usize, target_node_count: usize) {
    board_crab_lib::init();
    let mut board = fen::load_fen(position_fen).unwrap();
    let moves = move_gen::generate_moves(&mut board);
    let perft_count = moves.len();

    if perft_count != target_node_count {
        // Test failed
        panic!(
            "Failed position \"{}\" (got: {}, target: {}), fen: \"{}\"",
            name, perft_count, target_node_count, position_fen
        );
    }
}

#[test]
fn depth_1_perft_test() {
    let test_entries = [
        ("starting position", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 20),

        ("basic captures", "1Q6/5k2/3q1pp1/4p3/3P3p/1pP2N2/1KB5/2n2R2 w - - 0 1", 41),
        ("crazy captures", "4N3/3k2b1/1b3r2/2rbr1B1/1PQKBP2/3bN3/2r5/8 w - - 0 1", 37),

        ("basic pin", "8/4k3/4r3/8/8/4N3/4K3/8 w - - 0 1", 7),
        ("free pin", "8/3k4/3q4/8/8/3R4/3K4/8 w - - 0 1", 10),

        ("en passant", "5k2/8/8/3Pp3/2K5/8/8/8 w - e6 0 2", 8),
        ("bishop-pin passant (legal)", "8/2b5/1k6/3pP3/5K2/8/8/8 w - d6 0 2", 7),
        ("bishop-pin passant (illegal)", "8/2k5/5K2/3pP3/8/2b5/8/8 w - d6 0 2", 7),
        ("rook-pin passant (illegal)", "8/2k5/8/1r1pPK2/8/8/8/8 w - d6 0 2", 7),
        ("almost-rook-pin passant (legal)", "8/2k5/8/rn1pPK2/8/8/8/8 w - d6 0 2", 8),

        ("single check 1", "1k6/ppp3r1/8/8/4P1K1/1P1P1N2/PBP5/8 w - - 0 1", 7),

        ("double check", "8/8/6k1/8/5q2/5n2/3KN3/8 w - - 0 1", 4),

        ("basic both-castle", "4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", 26),
        ("basic no-castle", "4k3/8/8/8/8/8/8/R3K2R w - - 0 1", 24),
        ("funky one-castle", "8/8/4k3/5q2/8/8/8/R3K2R w KQ - 0 1", 23),

        ("basic promotion", "8/6P1/8/5K2/1k6/8/8/8 w - - 0 1", 12),

        ("hell", "3Nk1n1/1q2p2P/3p1p2/2r2Pp1/rb3nPq/2BB1b2/P4Q2/R3K2R w - g6 0 2", 36),
        ("crazy mate in 1", "4b3/BP1P1P1P/q5Q1/1R1NkN1R/4B1n1/1n1P1Pb1/1r5r/3K4 w - - 0 1", 71),
        // ^ is from https://www.reddit.com/r/chess/comments/13v0elu/a_complicated_mate_in_1/
    ];

    for pair in test_entries {
        do_test(pair.0, pair.1, 1, pair.2);
    }
}

// This one features very difficult positions from https://www.chessprogramming.org/Perft_Results and other places
#[test]
fn super_perft_test() {
    let test_entries = [
        ("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8", [44, 1486, 62379]),
    ];

    // TODO: Implement
}