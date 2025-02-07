use crate::bitmask::*;
use crate::board::*;
use crate::lookup_gen;

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
    for piece_idx in 0..NUM_PIECES_NO_KING {
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

// Evaluates a move
pub fn eval_move(board: &Board, mv: &Move) -> Value {
    const CAPTURE_BASE_BONUS: Value = 10.0; // Always give captures a big bonus
    const CHECK_BONUS: Value = 2.0;
    const PIN_BONUS: Value = 0.5;

    let mut eval: Value = 0.0;

    let to_defended = (board.attacks[1 - board.turn_idx] & mv.to) != 0;

    if mv.has_flag(Move::FLAG_PROMOTION) {
        eval += PIECE_BASE_VALUES[mv.to_piece_idx]
    }

    if mv.has_flag(Move::FLAG_CAPTURE) {
        let mut capture_val: Value = CAPTURE_BASE_BONUS;

        if mv.has_flag(Move::FLAG_EN_PASSANT) {
            capture_val += PIECE_BASE_VALUES[PIECE_PAWN];
        } else {
            for i in 0..NUM_PIECES_NO_KING {
                if (board.pieces[1 - board.turn_idx][i] & mv.to) != 0 {
                    capture_val += PIECE_BASE_VALUES[i];
                    break;
                }
            }
        }

        eval += CAPTURE_BASE_BONUS;
    }

    let to_idx = bm_to_idx(mv.to);
    let opp_king_pos = board.pieces[1 - board.turn_idx][PIECE_KING];
    let next_base_moves = lookup_gen::get_piece_base_tos(mv.to_piece_idx, to_idx);
    if (next_base_moves & opp_king_pos) != 0 {
        let is_check: bool;
        let is_pin: bool;
        match mv.to_piece_idx {
            PIECE_BISHOP | PIECE_ROOK | PIECE_QUEEN => {
                let between_mask = lookup_gen::get_between_mask_exclusive(bm_to_idx(opp_king_pos), to_idx);
                let num_blockers = (board.combined_occupancy() & between_mask).count_ones(); // Only need to count to 2
                is_check = (num_blockers == 0);
                is_pin = (num_blockers == 1);
            },
            _ => {
                is_check = true;
                is_pin = false;
            }
        }

        if is_check {
            eval += CHECK_BONUS;
        } else if is_pin {
            eval += PIN_BONUS;
        }
    }

    if to_defended {
        eval -= PIECE_BASE_VALUES[mv.from_piece_idx];
    }

    eval
}