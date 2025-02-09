use board_crab_lib::eval::Value;
use board_crab_lib::fen;
use board_crab_lib::search;
use board_crab_lib::transpos;
use board_crab_lib::thread_flag::ThreadFlag;
extern crate rand;

// Measures how consistent the search result is at increasing depths
#[test]
fn search_consistency_test() {
    board_crab_lib::init();

    const MAX_DEPTH: u8 = 2;

    let fens= include_str!("../data/gm_fen_positions.txt").split('\n').collect::<Vec<&str>>();

    let mut table = transpos::Table::new(4); // Small for low depth

    let mut total_move_matches: usize = 0;
    let mut total_positions: usize = 0;
    for i in 0..fens.len() {
        let cur_fen = fens[i];
        if cur_fen.trim().is_empty() {
            continue;
        }

        let board = fen::load_fen(cur_fen).unwrap();
        let stop_flag = ThreadFlag::new();
        let best_move_a = search::search(&board, &mut table, MAX_DEPTH - 1, None, &stop_flag, None).0.best_move_idx.unwrap();
        let best_move_b = search::search(&board, &mut table, MAX_DEPTH, None, &stop_flag, None).0.best_move_idx.unwrap();

        if best_move_a == best_move_b {
            total_move_matches += 1;
        }
        total_positions += 1;
    }

    let consistent_frac = (total_move_matches as Value) / (total_positions as Value);
    println!("Search consistency: {}%", consistent_frac * 100.0);

    if consistent_frac < 0.3 {
        panic!("Very low search consistency of {}%", consistent_frac * 100.0);
    }
}