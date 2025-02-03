use crate::board::*;
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