use crate::board::*;
use crate::eval::*;
use crate::{eval, move_gen};

fn _perft(board: &Board, depth: usize, depth_elapsed: usize, print: bool) -> usize {
    let moves = move_gen::generate_moves(board);
    if depth > 1 {
        let mut total: usize = 0;
        for mv in moves {
            let mut next_board: Board = *board;
            next_board.do_move(&mv);
            let sub_total = _perft(&next_board, depth - 1, depth_elapsed + 1, print);
            if depth_elapsed == 0 && print {
                println!("{}: {}", mv, sub_total);
            }
            total += sub_total;
        }
        if depth_elapsed == 0 && print {
            println!("\nNodes Searched: {}", total);
        }
        total
    } else if depth == 1 {
        if depth_elapsed == 0 && print {
            for mv in &moves {
                println!("{}: {}", mv, 1);
            }
            println!("\nNodes Searched: {}", moves.len());
        }
        moves.len()
    } else {
        if depth_elapsed == 0 && print {
            println!("\nNodes Searched: 1");
        }
        return 1;
    }
}

pub fn perft(board: &Board, depth: usize, print: bool) -> usize { _perft(board, depth, 0, print) }

//////////////////////////////////////////////////////////////////////////

pub struct SearchInfo {
    pub total_nodes: u64
}

fn is_extending_move(board: &Board, mv: &Move) -> bool{
    match mv.move_type {
        MoveType::EnPassantCapture | MoveType::Promotion => true,
        _ => {
            (mv.to & board.occupancy[1 - board.turn_idx]) != 0
        }
    }
}

// Maximum depth to extend searches to
const MAX_EXTENSION_DEPTH: usize = 8;

// Searches only extending moves
fn extension_search(board: &Board, search_info: &mut SearchInfo, mut lower_bound: Value, upper_bound: Value, depth_remaining: usize) -> Value {
    search_info.total_nodes += 1;

    // Ref: https://www.chessprogramming.org/Quiescence_Search
    let mut best_eval = eval_board(board);

    if best_eval >= upper_bound {
        return best_eval;
    } else if best_eval > lower_bound {
        lower_bound = best_eval;
    }

    if depth_remaining == 0 {
        return best_eval;
    }

    let moves = &move_gen::generate_moves(board);
    for mv in moves {
        if !is_extending_move(board, mv) {
            continue
        }

        let mut next_board = board.clone();
        next_board.do_move(mv);
        let next_eval = -extension_search(&next_board, search_info, -upper_bound, -lower_bound, depth_remaining - 1);

        if next_eval > best_eval {
            best_eval = next_eval;

            if next_eval > lower_bound {
                lower_bound = best_eval
            }
            if next_eval >= upper_bound {
                break;
            }
        }
    }

    best_eval
}

impl SearchInfo {
    pub fn new() -> SearchInfo {
        SearchInfo {
            total_nodes: 0
        }
    }
}

pub struct SearchResult {
    pub eval: Value,
    pub best_move_idx: Option<usize> // May not exist if at depth 0
}

// NOTE: depth_remaining can go negative when extending
pub fn search(board: &Board, search_info: &mut SearchInfo, mut lower_bound: Value, upper_bound: Value, depth_remaining: i64, depth_elapsed: usize) -> SearchResult {
    search_info.total_nodes += 1;

    let moves = &move_gen::generate_moves(board);

    if depth_elapsed > 10 {
        return SearchResult{ eval: 0.0, best_move_idx: None };
    }

    if moves.is_empty() {
        return SearchResult {
            eval: if board.checkers != 0 { -VALUE_CHECKMATE } else { 0.0 },
            best_move_idx: None
        }
    }

    if depth_remaining > 0 {
        let mut best_eval = -VALUE_INF;
        let mut best_move_idx: usize = 0;
        for i in 0..moves.len() {
            let mv = &moves[i];

            let mut next_board: Board = board.clone();
            next_board.do_move(mv);

            let next_result =
                search(&next_board, search_info, -upper_bound, -lower_bound, depth_remaining - 1, depth_elapsed + 1);
            let next_eval = -next_result.eval;

            if next_eval > best_eval {
                best_eval = next_eval;
                best_move_idx = i;
                if next_eval > lower_bound {
                    lower_bound = next_eval
                }

                if next_eval >= upper_bound {
                    // Beta cut-off
                    break
                }
            }
        }

        SearchResult {
            eval: best_eval,
            best_move_idx: Some(best_move_idx)
        }


    } else {
        SearchResult {
            eval: extension_search(board, search_info, lower_bound, upper_bound, MAX_EXTENSION_DEPTH),
            best_move_idx: None
        }
    }
}