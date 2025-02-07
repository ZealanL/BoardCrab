use crate::board::*;
use crate::eval::*;
use crate::move_gen;

fn _perft(board: &Board, depth: usize, depth_elapsed: usize, print: bool) -> usize {
    let moves = move_gen::generate_moves(board);
    if depth > 1 {
        let mut total: usize = 0;
        for mv in moves {
            let mut next_board: Board = *board;
            next_board.do_move(mv);
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

// Maximum depth to extend searches to
const MAX_EXTENSION_DEPTH: i64 = 8;

pub struct SearchResult {
    pub eval: Value,
    pub best_move_idx: Option<usize> // May not exist if at depth 0
}
pub fn search(board: &Board, mut lower_bound: Value, upper_bound: Value, depth_remaining: i64, depth_elapsed: i64) -> SearchResult {
    let moves = move_gen::generate_moves(board);

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
            let mv = moves[i];

            let mut next_board: Board = board.clone();
            next_board.do_move(mv);

            let next_result =
                search(&next_board, -upper_bound, -lower_bound, depth_remaining - 1, depth_elapsed + 1);
            let next_eval = -next_result.eval;

            if next_eval > best_eval {
                best_eval = next_eval;
                best_move_idx = i;
                if next_eval > lower_bound {
                    lower_bound = next_eval
                }

                if next_eval > upper_bound {
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
            eval: eval_board(board),
            best_move_idx: None
        }
    }
}