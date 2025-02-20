use crate::bitmask::*;
use crate::board::*;
use crate::lookup_gen;
use crate::eval_lookup;

pub type Value = f32; // Note: MUST be a float type
pub const VALUE_INF: Value = Value::INFINITY;
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

const LIGHT_SQUARES: BitMask = 0x55aa55aa55aa55aa;
const DARK_SQUARES: BitMask = !LIGHT_SQUARES;

// Returns the "attacking power" of a team from 0-1
// This is meant to represent how capable the player is of making a deadly attack on the king
pub fn calc_attacking_power(board: &Board, team_idx: usize) -> Value {
    let rook_count = board.pieces[team_idx][PIECE_ROOK].count_ones();
    if board.pieces[team_idx][PIECE_QUEEN] != 0 {
        let bishop_count = board.pieces[team_idx][PIECE_BISHOP].count_ones();
        let knight_count = board.pieces[team_idx][PIECE_BISHOP].count_ones();
        Value::min(1.0, 0.8 + (rook_count as Value) * 0.15 + (bishop_count as Value) * 0.05 + (knight_count as Value) * 0.03)
    } else {
        if rook_count >= 2 {
            0.4
        } else {
            // Can't really create much of an attack
            0.0
        }
    }
}

fn dual_weight(weights: [Value; 2], scale: Value) -> Value {
    weights[0] * scale + weights[1] * (1.0 - scale)
}

fn get_pawn_attack_mask(board: &Board, team_idx: usize) -> BitMask {
    let pawns = board.pieces[team_idx][PIECE_PAWN];

    let mut capture_mask: BitMask = 0;
    for side in 0..2 {
        capture_mask |= bm_shift(pawns & [!bm_make_column(0), !bm_make_column(7)][side], [-1, 1][side], [1, -1][team_idx])
    }

    capture_mask
}

fn eval_material(board: &Board, team_idx: usize, opp_attack_power: Value) -> Value {
    let mut value: Value = 0.0;
    for piece_idx in 0..NUM_PIECES_NO_KING {
        value +=
            (board.pieces[team_idx][piece_idx].count_ones() as Value)
                * dual_weight(eval_lookup::PIECE_BASE_VALUE[piece_idx], opp_attack_power);
    }

    value
}

fn eval_piece_type(board: &Board, team_idx: usize, piece_idx: usize, piece_mask: BitMask, opp_attack_power: Value) -> Value {
    let mut value: Value = 0.0;

    let opp_pawns = board.pieces[1 - team_idx][PIECE_PAWN];
    let pawn_attacks = get_pawn_attack_mask(board, team_idx);

    for pos_mask in bm_iter_bits(piece_mask) {
        let (x,y) = bm_to_xy(pos_mask);
        let rel_y = [y, 7-y][team_idx];
        let rel_pos_idx = x + rel_y*8;
        value +=
            dual_weight(eval_lookup::PIECE_TB[piece_idx][rel_pos_idx as usize], opp_attack_power);

        if piece_idx == PIECE_PAWN {
            let (pawn_x, pawn_y) = bm_to_xy(pos_mask);
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

            let is_passed = (pass_prev & opp_pawns) == 0;
            if is_passed {
                let rel_pos_idx = pawn_x + pawn_rel_y * 8;
                value += dual_weight(eval_lookup::PASSED_PAWN_TB[rel_pos_idx as usize], opp_attack_power);
            }

            // TODO: Scale with distance between the pawns
            let pawns_in_file = (piece_mask & column).count_ones();
            if pawns_in_file > 1 {
                value += dual_weight(eval_lookup::DOUBLED_PAWNS, opp_attack_power);
            }

            if (pawn_attacks & pos_mask) != 0 {
                value += dual_weight(eval_lookup::CONNECTED_PAWNS, opp_attack_power);
            }

            let color_mask = if (pos_mask & LIGHT_SQUARES) != 0 { LIGHT_SQUARES } else { DARK_SQUARES };
            if (board.pieces[team_idx][PIECE_BISHOP] & color_mask) != 0 {
                value += dual_weight(eval_lookup::BLOCKING_PAWNS, opp_attack_power);
            }
        } else if piece_idx == PIECE_ROOK {
            let is_open_file = (bm_make_column(x) & (board.pieces[0][PIECE_PAWN] | board.pieces[1][PIECE_PAWN])) == 0;
            if is_open_file {
                value += dual_weight(eval_lookup::OPEN_ROOKS, opp_attack_power);
            }
        }
    }

    value
}

fn eval_mobility(board: &Board, team_idx: usize) -> Value {
    let attacks = board.attacks[team_idx];
    (attacks.count_ones() as Value) * 0.02 // Per square-attacked
}

fn eval_king_safety(board: &Board, team_idx: usize, opp_attack_power: Value) -> Value {
    if opp_attack_power <= 0.0 {
        return 0.0;
    }

    let king = board.pieces[team_idx][PIECE_KING];
    let king_pos_idx = bm_to_idx(king);
    let (_king_x, king_y) = bm_to_xy(king);

    let top_rank_y: i64 = [7, 0][team_idx];
    let up_dir: i64 = [1, -1][team_idx];

    let pawns = board.pieces[team_idx][PIECE_PAWN];

    let squares_around_king = lookup_gen::get_piece_base_tos(PIECE_KING, king_pos_idx);
    let squares_above_king_1 = squares_around_king &
        bm_shift(bm_make_row(king_y), 0, up_dir); // NOTE: Totally fine if the shift wraps over
    let squares_above_king_2 = bm_shift(squares_above_king_1 & !bm_make_row(top_rank_y), 0, up_dir);

    let covering_pawns = (pawns & (squares_above_king_1 | squares_above_king_2)).count_ones();

    // Pretending the king is a queen to measure accessibility
    let accessibility =
        lookup_gen::get_piece_tos(PIECE_QUEEN, king, king_pos_idx, board.occupancy[team_idx]).count_ones();

    dual_weight(eval_lookup::KING_PAWN_COVER, opp_attack_power) * (covering_pawns as Value) +
    dual_weight(eval_lookup::KING_ACCESSIBILITY, opp_attack_power) * (accessibility as Value)
}

fn eval_team(board: &Board, team_idx: usize) -> Value {
    let opp_attack_power = calc_attacking_power(board, 1 - team_idx);

    let mut value: Value = eval_material(board, team_idx, opp_attack_power);
    for piece_idx in 0..NUM_PIECES {
        value += eval_piece_type(board, team_idx, piece_idx, board.pieces[team_idx][piece_idx], opp_attack_power);
    }

    if board.turn_idx == team_idx {
        value += dual_weight(eval_lookup::TURN_BONUS, opp_attack_power);
    }

    value
        + eval_mobility(board, team_idx)
        + eval_king_safety(board, team_idx, opp_attack_power)
}

// Returns true if the player can possibly checkmate the other
fn is_checkmate_possible(board: &Board, team_idx: usize) -> bool {
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
        "{:<14}   {:<6}   {:<6}",
        "", "White", "Black"
    );

    let team_vals = [eval_team(board, 0), eval_team(board, 1)];
    let mut entries = [Vec::new(), Vec::new()];
    for team_idx in 0..2 {
        entries[team_idx].push(("Material".to_string(), eval_material(board, team_idx, attack_power[1 - team_idx])));
        for piece_idx in 0..NUM_PIECES {
            let piece_type_eval = eval_piece_type(
                board, team_idx, piece_idx, board.pieces[team_idx][piece_idx], attack_power[1 - team_idx]
            );
            entries[team_idx].push((PIECE_NAMES[piece_idx].to_string() + "s", piece_type_eval));
        }
        entries[team_idx].push(("Mobility".to_string(), eval_mobility(board, team_idx)));
        entries[team_idx].push(("King Safety".to_string(), eval_king_safety(board, team_idx, attack_power[1 - team_idx])));

        entries[team_idx].push(("TOTAL".to_string(), team_vals[team_idx]));
    }

    assert_eq!(entries[0].len(), entries[1].len());
    let num_entries = entries[0].len();
    for i in 0..num_entries {
        if i == (num_entries - 1) {
            println!("{}", "-".to_string().repeat(33));
        }

        let name = &entries[0][i].0;
        let vals = [entries[0][i].1, entries[1][i].1];
        println!("{:>14} | {:>+0width$.prec$} | {:>+0width$.prec$} | {:>+0width$.prec$}", name, vals[0], vals[1], vals[0]-vals[1], width = 6, prec = 2);
    }
}

// Evaluates a move
pub fn eval_move(board: &Board, mv: &Move) -> Value {
    const CAPTURE_BASE_BONUS: Value = 1.0; // Always give captures a bit of a bonus
    const CHECK_BONUS: Value = 1.0;
    const PIN_BONUS: Value = 0.0;
    const TURN_BONUS: Value = 0.1;

    let mut eval: Value = 0.0;

    let to_defended = (board.attacks[1 - board.turn_idx] & mv.to) != 0;

    if mv.has_flag(Move::FLAG_PROMOTION) {
        if mv.to_piece_idx == PIECE_QUEEN {
            eval += 50.0; // Very important move to look at
        } else {
            eval -= 10.0; // Very rarely do we want to promote to something other than a queen
        }
    }

    if mv.has_flag(Move::FLAG_CAPTURE) {
        let mut capture_val: Value = CAPTURE_BASE_BONUS;

        if mv.has_flag(Move::FLAG_EN_PASSANT) {
            capture_val += eval_lookup::PIECE_BASE_VALUE[PIECE_PAWN][0];
        } else {
            for piece_idx in 0..NUM_PIECES_NO_KING {
                if (board.pieces[1 - board.turn_idx][piece_idx] & mv.to) != 0 {
                    capture_val += eval_lookup::PIECE_BASE_VALUE[piece_idx][0];
                    break;
                }
            }
        }

        eval += capture_val;
    }

    // Determine if the move is a check or pin
    let to_idx = bm_to_idx(mv.to);
    let opp_king_pos = board.pieces[1 - board.turn_idx][PIECE_KING];
    let next_base_moves = lookup_gen::get_piece_base_tos(mv.to_piece_idx, to_idx);
    if (next_base_moves & opp_king_pos) != 0 {
        let is_check: bool;
        let is_pin: bool;
        match mv.to_piece_idx {
            PIECE_BISHOP | PIECE_ROOK | PIECE_QUEEN => {
                let between_mask = lookup_gen::get_between_mask_exclusive(bm_to_idx(opp_king_pos), to_idx);
                let num_blockers = (board.combined_occupancy() & between_mask).count_ones(); // TODO: Only need to count to 2
                is_check = num_blockers == 0;
                is_pin = num_blockers == 1;
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
        eval -= eval_lookup::PIECE_BASE_VALUE[mv.from_piece_idx][0];
    }

    eval += TURN_BONUS;

    eval
}

pub fn to_centipawns(value: Value) -> i64 {
    (value * 100.0).round() as i64
}