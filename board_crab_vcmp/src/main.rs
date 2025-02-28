use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use rand::Rng;
use statrs::distribution::ContinuousCDF;
use board_crab_lib::{fen, lookup_gen, move_gen, async_engine, transpos, time_manager, pgn};
use board_crab_lib::bitmask::*;
use board_crab_lib::board::*;
use board_crab_lib::eval::*;
use board_crab_lib::search::SearchConfig;

// Time for both player's clocks
const GAME_CLOCK_TIME: f64 = 20.0; // Base time
const GAME_CLOCK_TIME_COMPLEMENT: f64 = 0.0; // Per-move complement

// Eval at which we conclude the game over
// Both engines must agree on the eval
const TRUNCATE_EVAL_THRESH: Value = 4.5;

fn is_game_over(board: &Board) -> bool {
    // Draw by half-move limit
    if board.half_move_counter >= 50 {
        return true;
    }

    // Draw by insufficient material
    if !is_checkmate_possible(board, 0) && !is_checkmate_possible(board, 1) {
        return true;
    }

    // Checkmate
    if board.checkers != 0 {
        let king = board.pieces[board.turn_idx][PIECE_KING];
        let team_occupy = board.occupancy[board.turn_idx];
        let opp_attack = board.attacks[1 - board.turn_idx];
        let base_king_moves = lookup_gen::get_piece_base_tos(PIECE_KING, bm_to_idx(king));
        let king_moves = base_king_moves & !opp_attack & !team_occupy;

        if king_moves == 0 {
            // King can't move, could be checkmate
            // Generate moves to know for sure
            let mut move_buffer = move_gen::MoveBuffer::new();
            move_gen::generate_moves(board, &mut move_buffer);

            if move_buffer.is_empty() {
                // It's checkmate
                return true;
            }
        }
    }

    false
}

const TEAM_NAMES: [&str; 2] = ["WHITE", "BLACK"];

fn simulate_game(tables: &mut [&mut transpos::Table; 2], search_configs: [SearchConfig; 2], starting_fen: &str, print: bool) -> Option<usize> {
    let starting_board = fen::load_fen(starting_fen).unwrap();
    let mut board = starting_board;
    let mut clock_times = [GAME_CLOCK_TIME; 2];
    let mut moves = Vec::new();

    let finish_game = |board: &Board, moves: &Vec<Move>, msg: String| {
        if print {
            println!("Game over, {} (fen: {}), PGN:", msg, fen::make_fen(&board));
            println!("{}", pgn::make_pgn(&starting_board, &moves).unwrap());
        }
    };

    let mut last_eval: Value = 0.0;
    let mut position_counts = std::collections::HashMap::<u64, usize>::new();
    while !is_game_over(&board) {

        let search_config = search_configs[board.turn_idx];

        *position_counts.entry(board.hash).or_insert(0) += 1;
        if position_counts[&board.hash] >= 3 {
            finish_game(&board, &moves, "draw by repetition".to_string());
            return None; // Draw by repetition
        }

        let start_time = Instant::now();
        let clock_time = clock_times[board.turn_idx];

        if clock_time <= 0.0 {
            let opp_wins = is_checkmate_possible(&board, 1 - board.turn_idx);
            finish_game(&board, &moves, format!("player {}'s clock ran out ({})", TEAM_NAMES[board.turn_idx], if opp_wins { "lost" } else { "draw" }));
            if opp_wins {
                return Some(1 - board.turn_idx); // Black wins
            } else {
                return None;
            }
        }

        let mut time_state = time_manager::TimeState::new();
        time_state.remaining_time = Some(clock_time);
        time_state.time_inc = Some(GAME_CLOCK_TIME_COMPLEMENT);

        let async_search_config = async_engine::AsyncSearchConfig {
            max_depth: None,
            stop_flag: None,
            start_time,
            time_state: Some(time_state),
            print_uci: false,

            search_config
        };
        let (best_move_idx, eval) = async_engine::do_search_thread(&board, tables[board.turn_idx], &async_search_config);

        if eval.abs() >= TRUNCATE_EVAL_THRESH && last_eval.abs() >= TRUNCATE_EVAL_THRESH {
            if eval.signum() == -last_eval.signum() {
                // Engines both agree on a very high eval in the same absolute direction
                // Truncate the game
                let winning_team = if eval > 0.0 { board.turn_idx } else { 1 - board.turn_idx };
                finish_game(&board, &moves, format!("truncated at eval {}, player {} wins", eval, TEAM_NAMES[winning_team]));
                return Some(winning_team);
            }
        }

        let mut move_buffer = move_gen::MoveBuffer::new();
        move_gen::generate_moves(&board, &mut move_buffer);

        assert!(best_move_idx.is_some());
        assert!((best_move_idx.unwrap() as usize) < move_buffer.len());

        let best_move = move_buffer[best_move_idx.unwrap() as usize];
        board.do_move(&best_move);
        moves.push(best_move);
        clock_times[board.turn_idx] += GAME_CLOCK_TIME_COMPLEMENT - start_time.elapsed().as_secs_f64();
        last_eval = eval;
    }

    if board.checkers != 0 {
        finish_game(&board, &moves, format!("{} won by checkmate", TEAM_NAMES[1 - board.turn_idx]));
        Some(1 - board.turn_idx)
    } else {
        finish_game(&board, &moves, "draw".to_string());
        None
    }
}

struct GameResults {
    next_fens: Vec<String>,

    new_wins: usize,
    old_wins: usize,
    draws: usize
}

impl GameResults {
    fn new() -> GameResults {
        GameResults {
            next_fens: Vec::new(),

            new_wins: 0,
            old_wins: 0,
            draws: 0
        }
    }

    fn total_games(&self) -> usize {
        self.new_wins + self.old_wins + self.draws
    }

    // Returns the probability (0-1) that the new version is better than the old
    fn calc_new_better_prob(&self) -> f64 {
        let games = self.total_games() as f64;
        let wins = self.new_wins as f64 + self.draws as f64 * 0.5;
        let beta_distribution = statrs::distribution::Beta::new(wins + 1.0, games - wins + 1.0).unwrap();
        1.0 - beta_distribution.cdf(0.5)
    }
}

fn main() {
    board_crab_lib::init();

    let fens = include_str!("../../data/gm_opening_fens.txt").split('\n').collect::<Vec<&str>>();

    let fen_stack_arc = Arc::new(Mutex::new(GameResults::new()));

    for fen in fens {
        if fen.trim().is_empty() {
            continue;
        }

        fen_stack_arc.lock().unwrap().next_fens.push(fen.to_string());
    }

    let mut search_config_new = SearchConfig::new();
    {
        // Here we decide how the new version should play differently by changing the search config
        search_config_new.late_move_reduction_factor = 0.0;
    }
    let search_config_old = SearchConfig::new();

    const NUM_THREADS: usize = 10; // Number of threads to run in parallel
    const TABLE_SIZE_MBS: usize = 25; // Table size (there are two tables per thread)
    let mut handles = Vec::new();
    for thread_idx in 0..NUM_THREADS {
        let fen_stack_arc_clone = Arc::clone(&fen_stack_arc);
        println!("Launching thread {}/{}...", thread_idx + 1, NUM_THREADS);
        let handle = thread::spawn(move || {
            let mut rng = rand::rng();
            let mut tables = [
                &mut transpos::Table::new(TABLE_SIZE_MBS),
                &mut transpos::Table::new(TABLE_SIZE_MBS),
            ];

            loop {
                let mut cur_fen;
                {
                    let mut game_results = fen_stack_arc_clone.lock().unwrap();
                    if !game_results.next_fens.is_empty() {
                        cur_fen = game_results.next_fens.pop().unwrap();
                    } else {
                        break; // We're done
                    }
                }

                let new_team_idx = rng.random_range(0..2) as usize;
                let search_configs = if new_team_idx == 0 {
                    [search_config_new, search_config_old]
                } else {
                    [search_config_old, search_config_new]
                };

                let winning_team = simulate_game(&mut tables, search_configs, &cur_fen, true);
                {
                    let mut game_results = fen_stack_arc_clone.lock().unwrap();

                    if winning_team.is_some() {
                        if new_team_idx == winning_team.unwrap() {
                            game_results.new_wins += 1;
                        } else {
                            game_results.old_wins += 1;
                        }
                    } else {
                        game_results.draws += 1;
                    }

                    println!("Score line: {} - {} - {}", game_results.new_wins, game_results.draws, game_results.old_wins);
                    println!(" > Better prob: {:.2}%", game_results.calc_new_better_prob() * 100.0);
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
    println!("Done!");
}
