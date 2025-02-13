use std::io::Write;
use crate::board::*;
use crate::move_gen;
use crate::search;
use crate::eval::*;
use crate::fen;
use crate::search::SearchInfo;
use crate::transpos;
use crate::async_engine::AsyncEngine;
use crate::time_manager::TimeState;
// Refs:
// - https://gist.github.com/DOBRO/2592c6dad754ba67e6dcaec8c90165bf
// - https://github.com/ZealanL/BoardMouse/blob/4d3b6c608a3cb82a1299580a90dcb3c831fc02f8/src/UCI/UCI.cpp

pub fn print_search_results(board: &Board, table: &transpos::Table, depth: u8, eval: Value, search_info: &SearchInfo, elapsed_time: f64) {
    let mut moves = move_gen::MoveBuffer::new();
    move_gen::generate_moves(board, &mut moves);

    let pv_moves = search::determine_pv(*board, table);
    let mut pv_str = String::new();
    for i in 0..pv_moves.len() {
        if i > 0 {
            pv_str.push(' ');
        }

        pv_str += format!("{}", &pv_moves[i]).as_str();
    }

    let eval_str;
    if eval.abs() >= VALUE_CHECKMATE_MIN {
        eval_str = eval_to_str(eval).replace("#", "mate ");
    } else {
        eval_str = format!("cp {}", (eval * 100.0).round() as i64);
    }

    let multipv = 1;
    let total_nodes = search_info.total_nodes;
    let nodes_per_sec = ((search_info.total_nodes as f64) / elapsed_time).round() as i64;
    let elapsed_ms = (elapsed_time * 1000.0).round() as i64;

    println!(
        "info depth {depth} multipv {multipv} score {eval_str} nodes {total_nodes} nps {nodes_per_sec} time {elapsed_ms} pv {pv_str}"
    );
}

pub fn cmd_position(parts: Vec<String>, engine: &mut AsyncEngine) -> bool {
    if parts.len() < 2 {
        panic!("\"position\" called with too few arguments")
    }

    let mut board;

    let mut cur_part_idx: usize = 2;
    if parts[1] == "fen" {
        let mut fen_part_amount: usize = 0;
        while cur_part_idx < parts.len() && parts[cur_part_idx] != "moves" {
            fen_part_amount += 1;
            cur_part_idx += 1;
        }

        if fen_part_amount == 0 {
            panic!("\"position fen\" is missing actual fen");
        }

        let new_board_result = fen::load_fen_from_parts(&parts[2..(2 + fen_part_amount)].to_vec());
        if new_board_result.is_err() {
            panic!("{}", new_board_result.unwrap_err());
        } else {
            board = new_board_result.unwrap();
        }
    } else if parts[1] == "startpos" {
        board = Board::start_pos();
    } else {
        panic!("Unknown position type \"{}\"", parts[1]);
    }

    if cur_part_idx < parts.len() {
        if parts[cur_part_idx] == "moves" {
            for i in (cur_part_idx + 1)..parts.len() {
                let move_str = &parts[i];

                let mut moves = move_gen::MoveBuffer::new();
                move_gen::generate_moves(&board, &mut moves);

                let mut move_found = false;
                for mv in moves.iter() {
                    if format!("{mv}").eq(move_str) {
                        board.do_move(mv);
                        move_found = true;
                        break;
                    }
                }

                if !move_found {
                    panic!("Invalid move \"{}\" for position \"{}\"", move_str, fen::make_fen(&board));
                }
            }
        } else {
            panic!("Unknown position suffix \"{}\"", parts[cur_part_idx]);
        }
    }

    engine.set_board(&board);
    true
}

pub fn cmd_go(parts: Vec<String>, engine: &mut AsyncEngine) -> bool {
    let board = engine.get_board();

    let mut pairs = Vec::new();
    let mut singles = Vec::new();
    let mut i: usize = 1;
    while i < parts.len() - 1 {
        let parse_result = parts[i + 1].parse::<i64>();
        if parse_result.is_ok() {
            // Argument with number val
            pairs.push(
                (parts[i].clone(), parse_result.unwrap())
            );
            i += 2;
        } else {
            // Alone argument
            singles.push(parts[i].clone());
            i += 1;
        }
    }

    let mut max_depth: u8 = u8::MAX;
    let mut time_state: TimeState = TimeState::new();

    let remaining_time_str = if board.turn_idx == 0 { "wtime" } else { "btime" };
    let time_inc_str = if board.turn_idx == 0 { "winc" } else { "binc" };

    for pair in pairs {
        let first_arg = pair.0.as_str();
        match first_arg {
            "depth" => {
                max_depth = pair.1 as u8;
            },
            "movetime" => {
                time_state.max_time = Some(pair.1 as f64 / 1000.0);
            }
            "movestogo" => {
                time_state.moves_till_time_control = Some(pair.1 as u64);
            }
            _ => {
                if first_arg == remaining_time_str {
                    time_state.remaining_time = Some(pair.1 as f64 / 1000.0);
                } else if first_arg == time_inc_str {
                    time_state.time_inc = Some(pair.1 as f64 / 1000.0);
                }

            }
        }

    }

    engine.start_search(max_depth, Some(time_state));
    true
}

pub fn cmd_ratemoves(engine: &mut AsyncEngine) -> bool {
    let mut moves_buf = move_gen::MoveBuffer::new();
    move_gen::generate_moves(engine.get_board(), &mut moves_buf);

    let mut moves = Vec::new();
    for mv in moves_buf.iter() {
        moves.push(mv);
    }
    moves.sort_by(|a, b| {
        eval_move(engine.get_board(), b).total_cmp(&eval_move(engine.get_board(), a))
    });

    println!("Moves:");
    for i in 0..moves.len() {
        let mv = moves[i];
        println!("\t{}: {}", mv, eval_move(engine.get_board(), &mv));
    }

    true
}

// Returns true if the command was understood and processed correctly
pub fn process_cmd(parts: Vec<String>, engine: &mut AsyncEngine) -> bool {
    if parts.is_empty() {
        return false;
    }

    match parts[0].as_str() {

        "uci" => {
            println!("id name BoardCrab v{}", env!("CARGO_PKG_VERSION"));
            println!("id author ZealanL");
            println!("uciok");
            true
        },
        "isready" => {
            println!("readyok");
            true
        }
        "quit" => {
            std::process::exit(0);
        }
        "position" => cmd_position(parts, engine),
        "go" => cmd_go(parts, engine),
        "eval" => {
            print_eval(engine.get_board());
            true
        },
        "ratemoves" => cmd_ratemoves(engine),
        "stop" => {
            engine.stop_search();
            true
        }
        "d" => {
            // Display
            println!("{}", engine.get_board());
            true
        }

        _ => {
            println!("info string Unknown command \"{}\"", parts[0]);
            false
        }
    }
}