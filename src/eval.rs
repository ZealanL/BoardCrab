use crate::bitmask::bm_itr_bits;
use crate::board::*;

pub type Value = f32;
pub const VALUE_INF: f32 = f32::MAX;
pub const VALUE_CHECKMATE: f32 = 1_000.0;
pub const VALUE_CHECKMATE_MIN: f32 = 500.0;

pub fn eval_to_str(eval: Value) -> String {
    if eval.abs() >= VALUE_CHECKMATE_MIN {
        let mate_move_count = VALUE_CHECKMATE - eval.abs();
        let ply_till_mate = (mate_move_count * eval.signum()) as i64;
        format!("mate {}", (ply_till_mate + 1) / 2)
    } else {
        eval.to_string()
    }
}

pub fn decay_eval(eval: Value) -> Value {
    if eval.abs() >= VALUE_CHECKMATE_MIN {
        eval - eval.signum()
    } else {
        eval
    }
}

//////////////////////////////////////////////////////////

// From the AlphaZero paper: https://arxiv.org/pdf/2009.04374
const PIECE_BASE_VALUES: [Value; NUM_PIECES-1] = [1.0, 3.05, 3.33, 5.63, 9.5];

fn eval_team(board: &Board, team_idx: usize) -> Value {
    let mut value: Value = 0.0;
    for piece_idx in 0..(NUM_PIECES-1) {
        for piece_pos in bm_itr_bits(board.pieces[team_idx][piece_idx]) {
            value += PIECE_BASE_VALUES[piece_idx];
        }
    }

    value
}

// Evaluates the position from the perspective of the current turn
pub fn eval_board(board: &Board) -> Value {
    eval_team(board, board.turn_idx) - eval_team(board, 1 - board.turn_idx)
}