use crate::bitmask::*;
use crate::board::*;
use crate::lookup_gen;

pub type Value = f32; // Note: MUST be a float type
pub const VALUE_INF: Value = Value::MAX;
pub const VALUE_CHECKMATE: Value = 1_000.0;
pub const VALUE_CHECKMATE_MIN: Value = 500.0;

pub fn eval_to_str(eval: Value) -> String {
    if eval.abs() >= VALUE_CHECKMATE_MIN {
        let mate_move_count = VALUE_CHECKMATE - eval.abs();
        let ply_till_mate = (mate_move_count * eval.signum()) as i64;
        format!("#{}", (ply_till_mate + 1) / 2)
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
// (I also buffed the queen a little bit)
const PIECE_BASE_VALUES: [Value; NUM_PIECES-1] = [1.0, 3.05, 3.33, 5.63, 9.5 + 1.0];

// Returns the "attacking power" of a team from 0-1
// This is meant to represent how capable the player is of making a deadly attack on the king
fn calc_attacking_power(board: &Board, team_idx: usize) -> Value {
    // TODO: Only need to count to 2, >2 rooks and >1 queen is not very helpful to evaluate
    // TODO: Consider minor pieces
    let rook_count = board.pieces[team_idx][PIECE_ROOK].count_ones();
    if board.pieces[team_idx][PIECE_QUEEN] != 0 {
        Value::min(1.0, 0.7 + (rook_count as Value) * 0.15)
    } else {
        if rook_count >= 2 {
            0.25
        } else {
            // Can't really create much of an attack
            0.0
        }
    }
}

fn eval_piece_position(team_idx: usize, piece_idx: usize, piece_mask: BitMask, opp_attack_power: Value) -> Value {
    const EDGE_MASK: BitMask = 0xff818181818181ff;
    const CORNER_MASK: BitMask = 0x8100000000000081;
    const CENTER_3_MASK: BitMask = 0x7e7e7e7e7e7e00;
    const CENTER_2_MASK: BitMask = 0x3c3c3c3c0000;
    const CENTER_1_MASK: BitMask = 0x1818000000;
    const GOOD_BISHOP_MASK: BitMask = 0x422418183c7e00;

    let add_mask_bonus = |piece_mask: BitMask, bonus_mask: BitMask, bonus_scale: Value| -> Value {
        let bonus_mask_absolute = if team_idx == 1 { bm_flip_vertical(bonus_mask) } else { bonus_mask };
        ((piece_mask & bonus_mask_absolute).count_ones() as Value) * bonus_scale
    };

    let mut value: Value = 0.0;

    match piece_idx {
        PIECE_KNIGHT => {
            value += add_mask_bonus(piece_mask, CENTER_1_MASK, 0.2);
            value += add_mask_bonus(piece_mask, CENTER_2_MASK, 0.2);
            value += add_mask_bonus(piece_mask, EDGE_MASK, -0.2);
            value += add_mask_bonus(piece_mask, CORNER_MASK, -0.2);
        },
        PIECE_BISHOP => {
            value += add_mask_bonus(piece_mask, GOOD_BISHOP_MASK, 0.3);
            value += add_mask_bonus(piece_mask, EDGE_MASK, -0.2);
        },
        PIECE_ROOK => {
            value += add_mask_bonus(piece_mask, bm_make_row(7-1), 0.6);
        }
        PIECE_QUEEN => {
            value += add_mask_bonus(piece_mask, CENTER_2_MASK, 0.3);
        }
        PIECE_KING => {
            value += add_mask_bonus(piece_mask, CENTER_1_MASK, 0.1 * (1.0 - opp_attack_power));
            value += add_mask_bonus(piece_mask, CENTER_2_MASK, 0.2 * (1.0 - opp_attack_power));
            value += add_mask_bonus(piece_mask, CENTER_3_MASK, 0.1 * (1.0 - opp_attack_power));
        }
        _ => {}
    }

    value
}

fn eval_piece_masked_positioning(board: &Board, team_idx: usize, opp_attack_power: Value) -> Value {
    // Ref: https://www.chessprogramming.org/Simplified_Evaluation_Function

    // For some simpler evals, we will use bit-masking popcounts instead of lookup tables

    let mut value: Value = 0.0;

    for piece_idx in 0..NUM_PIECES {
        let piece_mask = board.pieces[team_idx][piece_idx];

        value += eval_piece_position(team_idx, piece_idx, piece_mask, opp_attack_power);
    }

    value
}

fn eval_pawns(board: &Board, team_idx: usize, opp_attack_power: Value) -> Value {
    let mut value: Value = 0.0;
    let pawns = board.pieces[team_idx][PIECE_PAWN];

    for pawn in bm_iter_bits(pawns) {
        let (pawn_x, pawn_y) = bm_to_xy(pawn);
        let pawn_rel_y = if team_idx == 0 { pawn_y } else { 7 - pawn_y };

        let mut behind_rows: BitMask = (1u64 << (8 * (pawn_y + 1))) - 1;
        if team_idx == 1 {
            behind_rows = bm_flip_vertical(behind_rows);
        }

        let column: BitMask = bm_make_column(pawn_x);
        let columns =
            column
                | bm_shift(column & !bm_make_column(7),  1, 0)
                | bm_shift(column & !bm_make_column(0), -1, 0);

        let pass_prev = columns & !behind_rows;

        let is_passed = (pass_prev & pawns) == 0;
        let promote_thread_value;
        {
            let promote_ratio = ((pawn_rel_y - 1) as Value) / 6.0;
            let promote_ratio_sq = promote_ratio * promote_ratio;
            let promote_threat_scale = 1.0 - (opp_attack_power * 0.6);

            if is_passed {
                promote_thread_value = promote_ratio_sq * 3.0 * promote_threat_scale;
            } else {
                promote_thread_value = promote_ratio_sq * 0.5 * promote_threat_scale;
            }
        }

        // TODO: Only needs to be able to count to 3
        // TODO: Scale with distance between the pawns
        let pawns_in_file = (pawns & column).count_ones();
        let stacked_penalty = ((pawns_in_file - 1) as Value) * -0.3;

        value += promote_thread_value + stacked_penalty;

        let is_isolated = ((pawns & !pawn) & columns) == 0;
        if is_isolated {
            value += -0.3;
        }

        // TODO: Penalize backwards pawns?
    }

    value
}

fn eval_mobility(board: &Board, team_idx: usize) -> Value {
    // Other pieces are not considered
    let move_options = board.attacks[team_idx] & !board.occupancy[team_idx];
    (move_options.count_ones() as Value) * 0.03 // Per square-attacked
}

fn eval_king_safety(board: &Board, team_idx: usize, opp_attack_power: Value) -> Value {
    if opp_attack_power <= 0.0 {
        return 0.0;
    }

    let king = board.pieces[team_idx][PIECE_KING];
    let king_pos_idx = bm_to_idx(king);
    let (king_x, king_y) = bm_to_xy(king);

    let back_rank_y: i64 = [0, 7][team_idx];
    let top_rank_y: i64 = [7, 0][team_idx];
    let up_dir: i64 = [1, -1][team_idx];

    let squares_around_king = lookup_gen::get_piece_base_tos(PIECE_KING, king_pos_idx);
    let squares_above_king_1 = squares_around_king &
        bm_shift(bm_make_row(king_y), 0, up_dir); // NOTE: Totally fine if the shift wraps over
    let squares_above_king_2 = bm_shift(squares_above_king_1 & !bm_make_row(top_rank_y), 0, up_dir);

    let mut pawn_coverage_frac;
    { // Calculate coverage by pawns

        let pawns = board.pieces[team_idx][PIECE_PAWN];
        pawn_coverage_frac = (
            ((pawns & squares_above_king_1).count_ones() as Value) * 0.33 +
                ((pawns & squares_above_king_2).count_ones() as Value) * 0.2 // Pawns with space above the king provide weaker coverage
        ).min(1.0);

        let directly_above_king = (squares_above_king_1 | squares_above_king_2) & bm_make_row(king_y);
        if (pawns & directly_above_king) == 0 {
            // Penalize the coverage frac if the pawn directly above the king is missing
            pawn_coverage_frac *= 0.6;
        }
    }

    // Calculate distance from our back rank, and from the center of the board
    let king_height_frac = ((king_y - back_rank_y).abs() as Value) / 8.0;
    let king_off_center_frac = (((king_x as Value) - 3.5).abs() / 3.5).min(0.8); // We don't care about the very corner

    let attack_count = (board.attacks[team_idx] & squares_around_king).count_ones();
    let attack_frac = (attack_count as Value) / 4.0;

    // TODO: Implement king accessibility by pretending the king is a queen

    ((pawn_coverage_frac * 0.9)
        + (king_height_frac * -2.5)
        + (king_off_center_frac * 0.7)
        + (attack_frac * -1.0)
    ) * opp_attack_power
}

fn eval_material(board: &Board, team_idx: usize) -> Value {
    let mut material_value: Value = 0.0;

    for piece_idx in 0..NUM_PIECES_NO_KING {
        let piece_mask = board.pieces[team_idx][piece_idx];
        material_value += (piece_mask.count_ones() as Value) * PIECE_BASE_VALUES[piece_idx];
    }

    material_value
}

fn eval_team(board: &Board, team_idx: usize) -> Value {
    let opp_attack_power = calc_attacking_power(board, 1 - team_idx);

    eval_material(board, team_idx)
        + eval_pawns(board, team_idx, opp_attack_power)
        + eval_piece_masked_positioning(board, team_idx, opp_attack_power)
        + eval_mobility(board, team_idx)
        + eval_king_safety(board, team_idx, opp_attack_power)
}

// Returns true if the player can possibly checkmate the other
fn is_checkmate_possible(board: &Board, team_idx: usize) -> bool {
    // TODO: count_ones() is not needed here

    if board.pieces[team_idx][PIECE_PAWN] != 0 {
        return true;
    }

    let occ = board.occupancy[team_idx];
    let piece_count = occ.count_ones() - 1; // Not including the king
    if piece_count >= 3 {
        true
    } else if piece_count == 2 {
        // Not a checkmate if we have two knights
        // (Unless the opponent throws, but we don't care)
        !(board.pieces[team_idx][PIECE_KNIGHT].count_ones() == 2)
    } else if piece_count == 1 {
        // We must have a rook or a queen
        (board.pieces[team_idx][PIECE_ROOK] | board.pieces[team_idx][PIECE_QUEEN]) != 0
    } else {
        // No pieces
        false
    }
}

// Evaluates the position from the perspective of the current turn
pub fn eval_board(board: &Board) -> Value {
    let self_eval = eval_team(board, board.turn_idx);
    let opp_eval = eval_team(board, 1 - board.turn_idx);

    if (self_eval + opp_eval) < 15.0 {
        // Check for insufficient material draw

        let mut checkmate_possible: bool = false;
        for team_idx in 0..2 {
            // TODO: count_ones() is not needed here
            if is_checkmate_possible(board, team_idx) {
                checkmate_possible = true;
                break;
            }
        }

        if !checkmate_possible {
            return 0.0;
        }
    }

    self_eval - opp_eval
}

pub fn print_eval(board: &Board) {
    // Prints a Stockfish-inspired eval table

    let attack_power = [calc_attacking_power(board, 0), calc_attacking_power(board, 1)];
    println!(
        "{:<14} | {:<14} | {:<14} | {:<14} | {:<14} | {:<14}",
        "", "Material", "King Safety", "Pawns", "Pieces", "Mobility"
    );
    for i in 0..2 {
        println!("{:>12} | {:<14} | {:<14} | {:<14} | {:<14} | {:<14}",
            ["White", "Black"][i],
            eval_material(board, i),
            eval_king_safety(board, i, attack_power[1 - i]),
            eval_pawns(board, i, attack_power[1 - i]),
            eval_piece_masked_positioning(board, i, attack_power[1 - i]),
            eval_mobility(board, i)
        );
    }
}

// Evaluates a move
pub fn eval_move(board: &Board, mv: &Move) -> Value {
    const CAPTURE_BASE_BONUS: Value = 1.0; // Always give captures a bit of a bonus
    const CHECK_BONUS: Value = 1.0;
    const PIN_BONUS: Value = 0.25;

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

        eval += capture_val;
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

    // TODO: Temporary attack power placeholder
    eval += eval_piece_position(board.turn_idx, mv.to_piece_idx, mv.to, 1.0);

    eval
}

pub fn to_centipawns(value: Value) -> i64 {
    (value * 100.0).round() as i64
}