use crate::async_engine::AsyncEngine;
use crate::board::*;
use crate::eval::*;
use crate::fen;
use crate::move_gen;
use crate::search;
use crate::search::SearchInfo;
use crate::time_manager::TimeState;
use crate::transpos;
use std::cmp::PartialEq;
// Refs:
// - https://gist.github.com/DOBRO/2592c6dad754ba67e6dcaec8c90165bf
// - https://github.com/ZealanL/BoardMouse/blob/4d3b6c608a3cb82a1299580a90dcb3c831fc02f8/src/UCI/UCI.cpp

#[derive(Debug, Copy, Clone, PartialEq)]
enum UCIOptionType {
    Int,
    Bool,
    Button,
}

#[derive(Debug, Copy, Clone)]
struct UCIOption {
    option_type: UCIOptionType,
    name: &'static str,
    value: i64,
    value_min: i64,
    value_max: i64,
    change_callback: Option<fn(&mut UCIState, i64)>,
}

impl UCIOption {
    const TYPE_NAMES: [&'static str; 3] = ["spin", "check", "button"];

    pub fn new_int(
        name: &'static str,
        default: i64,
        value_min: i64,
        value_max: i64,
        change_callback: Option<fn(&mut UCIState, i64)>,
    ) -> UCIOption {
        UCIOption {
            option_type: UCIOptionType::Int,
            name,
            value: default,
            value_min,
            value_max,
            change_callback,
        }
    }

    pub fn new_button(name: &'static str, change_callback: fn(&mut UCIState, i64)) -> UCIOption {
        UCIOption {
            option_type: UCIOptionType::Button,
            name,
            value: 0,
            value_min: 0,
            value_max: 0,
            change_callback: Some(change_callback),
        }
    }
}

pub struct UCIState {
    engine: AsyncEngine,
    options: Vec<UCIOption>,
}

impl UCIState {
    pub fn new() -> UCIState {
        const DEFAULT_TABLE_SIZE_MBS: usize = 100;
        let options = [
            UCIOption::new_int("Threads", 8, 1, 256, None),
            UCIOption::new_int(
                "Hash",
                DEFAULT_TABLE_SIZE_MBS as i64,
                1,
                65536,
                Some(|state: &mut UCIState, new_value: i64| {
                    state.engine.maybe_update_table_size(new_value as usize);
                }),
            ),
            UCIOption::new_button("Clear Hash", |state: &mut UCIState, new_value: i64| {
                state.engine.reset_table();
            }),
        ];

        let mut result = UCIState {
            engine: AsyncEngine::new(DEFAULT_TABLE_SIZE_MBS),
            options: Vec::new(),
        };

        for option in options.iter() {
            result.options.push(option.clone());
        }

        result
    }

    pub fn get_option_val(&self, name: &str) -> i64 {
        for option in &self.options {
            if option.name == name {
                return option.value;
            }
        }

        panic!("UCI Option {} not found", name);
    }
}

//////////////////////////

pub fn print_search_results(
    board: &Board,
    table: &transpos::Table,
    depth: u8,
    eval: Value,
    search_info: &SearchInfo,
    elapsed_time: f64,
) {
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

pub fn print_best_move(best_move: Move) {
    println!("bestmove {}", best_move);
}

// Just returns an Option<String> of the error
macro_rules! cmd_err {
    ($($x:expr),*) => {
        Some(format!($($x),*))
    };
}

inventory::collect!(Command);
pub struct Command {
    name: &'static str,
    function: fn(&Vec<String>, &mut UCIState) -> Option<String>,
}
impl Command {
    pub const fn new(
        name: &'static str,
        function: fn(&Vec<String>, &mut UCIState) -> Option<String>,
    ) -> Self {
        Command { name, function }
    }
}

inventory::submit! {
    Command::new("uci", cmd_uci)
}
fn cmd_uci(_parts: &Vec<String>, state: &mut UCIState) -> Option<String> {
    println!("id name BoardCrab v{}", env!("CARGO_PKG_VERSION"));
    println!("id author ZealanL");

    for option in &state.options {
        print!(
            "option name {} type {}",
            option.name,
            UCIOption::TYPE_NAMES[option.option_type as usize]
        );

        match option.option_type {
            UCIOptionType::Int => {
                print!(
                    " default {} min {} max {}",
                    option.value, option.value_min, option.value_max
                );
            }
            UCIOptionType::Bool => {
                print!(" default {}", option.value > 0);
            }
            UCIOptionType::Button => {}
        }

        println!();
    }

    println!("uciok");
    None
}

inventory::submit! {
    Command::new("isready", cmd_isready)
}
fn cmd_isready(_parts: &Vec<String>, _state: &mut UCIState) -> Option<String> {
    println!("readyok");
    None
}

inventory::submit! {
    Command::new("setoption", cmd_setoption)
}
fn cmd_setoption(parts: &Vec<String>, state: &mut UCIState) -> Option<String> {
    if parts.len() <= 3 || parts[1] != "name" {
        return cmd_err!("Invalid syntax, format: \"setoption name <name> value <value>\"");
    }

    // Collect the option name
    let mut new_value_start_idx = 3;
    let mut option_name = String::new();
    for i in 2..parts.len() {
        if parts[i] != "value" {
            if !option_name.is_empty() {
                option_name += " ";
            }
            option_name += parts[i].as_str();
            new_value_start_idx += 1;
        } else {
            break;
        }
    }

    let mut new_value_str = String::new();
    for i in new_value_start_idx..parts.len() {
        if !new_value_str.is_empty() {
            new_value_str += " ";
        }
        new_value_str += parts[i].as_str();
    }

    for option in &mut state.options {
        if option.name.eq_ignore_ascii_case(&option_name) {
            let is_button = option.option_type == UCIOptionType::Button;
            if !is_button && new_value_str.is_empty() {
                return cmd_err!("Value missing");
            }

            let new_value: i64 = match new_value_str.to_lowercase().as_str() {
                "false" => 0,
                "true" => 1,
                _ => {
                    let parsed = new_value_str.parse::<i64>();
                    if parsed.is_ok() {
                        parsed.ok().unwrap()
                    } else {
                        if is_button {
                            // We don't need a value
                            0
                        } else {
                            return cmd_err!("Invalid number value: \"{}\"", new_value_str);
                        }
                    }
                }
            };

            // Invalid value handling
            match option.option_type {
                UCIOptionType::Int => {
                    if new_value < option.value_min || new_value > option.value_max {
                        return cmd_err!(
                            "Invalid number value \"{}\", valid range is [{}-{}]",
                            new_value,
                            option.value_min,
                            option.value_max
                        );
                    }
                }
                UCIOptionType::Bool => {
                    if new_value < 0 || new_value > 1 {
                        return cmd_err!("Invalid bool value: \"{}\", expected \"false\", \"true\", \"0\", or \"1\"", new_value_str);
                    }
                }
                UCIOptionType::Button => {
                    // Don't care
                }
            }

            option.value = new_value;
            if is_button {
                println!("info string \"{}\" triggered", option.name);
            } else {
                println!("info string \"{}\" -> {}", option.name, new_value_str);
            }
            if option.change_callback.is_some() {
                option.change_callback.unwrap()(state, new_value);
            }
            return None;
        }
    }

    cmd_err!("No option named \"{}\"", option_name)
}

inventory::submit! {
    Command::new("quit", cmd_quit)
}
fn cmd_quit(_parts: &Vec<String>, _state: &mut UCIState) -> Option<String> {
    std::process::exit(0)
}

inventory::submit! {
    Command::new("position", cmd_position)
}
fn cmd_position(parts: &Vec<String>, state: &mut UCIState) -> Option<String> {
    if parts.len() < 2 {
        cmd_err!("Too few arguments");
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
            return cmd_err!("FEN missing");
        }

        let new_board_result = fen::load_fen_from_parts(&parts[2..(2 + fen_part_amount)].to_vec());
        if new_board_result.is_err() {
            return cmd_err!("Invalid FEN: {}", new_board_result.err().unwrap());
        } else {
            board = new_board_result.unwrap();
        }
    } else if parts[1] == "startpos" {
        board = Board::start_pos();
    } else {
        return cmd_err!("Unknown position type \"{}\"", parts[1]);
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
                    return cmd_err!(
                        "Invalid move \"{}\" for position \"{}\"",
                        move_str,
                        fen::make_fen(&board)
                    );
                }
            }
        } else {
            return cmd_err!("Unknown position suffix \"{}\"", parts[cur_part_idx]);
        }
    }

    state.engine.set_board(&board);
    None
}

inventory::submit! {
    Command::new("go", cmd_go)
}
fn cmd_go(parts: &Vec<String>, state: &mut UCIState) -> Option<String> {
    let board = state.engine.get_board();

    let mut pairs = Vec::new();
    let mut singles = Vec::new();
    let mut i: usize = 1;
    while i < parts.len() {
        let parse_result = parts[usize::min(i + 1, parts.len() - 1)].parse::<i64>();
        if parse_result.is_ok() {
            // Argument with number val
            pairs.push((parts[i].clone(), parse_result.unwrap()));
            i += 2;
        } else {
            // Alone argument
            singles.push(parts[i].clone());
            i += 1;
        }
    }

    let mut max_depth: u8 = u8::MAX;
    let mut time_state: TimeState = TimeState::new();

    let remaining_time_str = if board.turn_idx == 0 {
        "wtime"
    } else {
        "btime"
    };
    let time_inc_str = if board.turn_idx == 0 { "winc" } else { "binc" };

    for pair in pairs {
        let first_arg = pair.0.as_str();
        match first_arg {
            "depth" => {
                max_depth = pair.1 as u8;
            }
            "movetime" => {
                time_state.max_time = Some(pair.1 as f64 / 1000.0);
            }
            "movestogo" => {
                time_state.moves_till_time_control = Some(pair.1 as u64);
            }
            "perft" => {
                search::perft(state.engine.get_board(), pair.1 as u8, true);
                return None;
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

    state
        .engine
        .maybe_update_table_size(state.get_option_val("Hash") as usize);
    state.engine.start_search(
        max_depth,
        Some(time_state),
        state.get_option_val("Threads") as usize,
    );
    None
}

inventory::submit! {
    Command::new("stop", cmd_stop)
}
fn cmd_stop(_parts: &Vec<String>, state: &mut UCIState) -> Option<String> {
    state.engine.stop_search();
    None
}

inventory::submit! {
    Command::new("eval", cmd_eval)
}
fn cmd_eval(_parts: &Vec<String>, state: &mut UCIState) -> Option<String> {
    print_eval(state.engine.get_board());
    None
}

inventory::submit! {
    Command::new("ratemoves", cmd_ratemoves)
}
fn cmd_ratemoves(_parts: &Vec<String>, state: &mut UCIState) -> Option<String> {
    let mut moves_buf = move_gen::MoveBuffer::new();
    move_gen::generate_moves(state.engine.get_board(), &mut moves_buf);

    let mut moves = Vec::new();
    for mv in moves_buf.iter() {
        moves.push(mv);
    }
    moves.sort_by(|a, b| {
        eval_move(state.engine.get_board(), b).total_cmp(&eval_move(state.engine.get_board(), a))
    });

    println!("Moves:");
    for i in 0..moves.len() {
        let mv = moves[i];
        println!("\t{}: {}", mv, eval_move(state.engine.get_board(), &mv));
    }

    None
}

inventory::submit! {
    Command::new("d", cmd_d)
}
fn cmd_d(_parts: &Vec<String>, state: &mut UCIState) -> Option<String> {
    println!("{}", state.engine.get_board());
    None
}

// Returns true if the command was understood and processed correctly
pub fn process_cmd(line_str: String, state: &mut UCIState) -> bool {
    let parts: Vec<String> = line_str
        .trim()
        .split_whitespace()
        .map(|v| v.to_string())
        .collect();
    if parts.is_empty() {
        return false;
    }

    for Command { name, function } in inventory::iter::<Command> {
        if parts[0].eq_ignore_ascii_case(name) {
            let cmd_err = function(&parts, state);
            if cmd_err.is_some() {
                println!("info string Error: {}", cmd_err.unwrap());
                return false;
            } else {
                return true;
            }
        }
    }

    println!("info string Unknown command \"{}\"", parts[0]);
    false
}
