use crate::bitmask::*;
use crate::board::*;
use crate::lookup_gen;

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

const PIECE_BASE_VALUES: [Value; NUM_PIECES] = [1.0, 3.2, 3.5, 5.2, 10.0, 1.0];

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

const CENTER_26: BitMask = 0x183c7e7e3c1800; // Center-most 26 squares
const CENTER_12: BitMask = 0x183c3c180000; // Center-most 12 squares
const CENTER_4: BitMask = 0x1818000000; // Center-most 4 squares
const EDGES: BitMask = 0xff818181818181ff;
const CORNER_MASK: BitMask = 0x8100000000000081;
const ELEVATED_2: BitMask = 0xffff000000000000;
const ELEVATED_4: BitMask = 0xffffffff00000000;
const LIGHT_SQUARES: BitMask = 0x55aa55aa55aa55aa;
const DARK_SQUARES: BitMask = !LIGHT_SQUARES;
const COLOR_MASKS: [BitMask; 2] = [LIGHT_SQUARES, DARK_SQUARES];

fn mask_eval(team_idx: usize, mut a: BitMask, b: BitMask, scale: Value) -> Value {
    if team_idx == 1 {
        a = bm_flip_vertical(a);
    }

    ((a & b).count_ones() as Value) * scale
}

fn get_pawn_attack_mask(board: &Board, team_idx: usize) -> BitMask {
    let pawns = board.pieces[team_idx][PIECE_PAWN];

    let mut capture_mask: BitMask = 0;
    for side in 0..2 {
        capture_mask |= bm_shift(pawns & [!bm_make_column(0), !bm_make_column(7)][side], [-1, 1][side], [1, -1][team_idx])
    }

    capture_mask
}

fn eval_piece_type(board: &Board, team_idx: usize, piece_idx: usize, piece_mask: BitMask, opp_attack_power: Value) -> Value {
    let mut value: Value = 0.0;

    let team_pawns = board.pieces[team_idx][PIECE_PAWN];
    let opp_pawns = board.pieces[1 - team_idx][PIECE_PAWN];
    let pawn_attacks = get_pawn_attack_mask(board, team_idx);

    match piece_idx {
        PIECE_PAWN => {
            for pawn in bm_iter_bits(piece_mask) {
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

                let is_passed = (pass_prev & opp_pawns) == 0;
                let promote_threat_value;
                {
                    let promote_ratio = ((pawn_rel_y - 1) as Value) / 6.0;
                    let promote_ratio_sq = promote_ratio * promote_ratio;
                    let promote_threat_scale = 1.0 - (opp_attack_power * 0.7);

                    if is_passed {
                        promote_threat_value = promote_ratio_sq * 3.0 * promote_threat_scale;
                    } else {
                        promote_threat_value = promote_ratio_sq * 1.2 * promote_threat_scale;
                    }
                }

                // TODO: Scale with distance between the pawns
                let pawns_in_file = (piece_mask & column).count_ones();
                let stacked_penalty = (((pawns_in_file - 1) as Value) / 2.0) * -0.3 * (1.0 - opp_attack_power*0.5);

                value += promote_threat_value + stacked_penalty;

                let is_isolated = ((piece_mask & !pawn) & columns) == 0;
                if is_isolated {
                    value += -0.1 * opp_attack_power;
                }

                let connected = (pawn_attacks & pawn) != 0;
                if connected {
                    value += 0.1;
                }

                let in_center = (pawn & CENTER_4) != 0;
                if in_center {
                    value += 0.4 * opp_attack_power;
                }
            }
        },
        PIECE_KNIGHT => {
            // Bonus for central knights
            value += mask_eval(team_idx, piece_mask, CENTER_12, 0.2);

            // Squares that are pushed up onto the opponent's side
            // Knights are very strong on these squares
            const PUSHED_UP_CENTER_MASK: BitMask = 0x247e3c18000000;
            value += mask_eval(team_idx, piece_mask, PUSHED_UP_CENTER_MASK, 0.2 * opp_attack_power);

            // Extra bonus for central knights defended by pawns
            value += mask_eval(team_idx, piece_mask, CENTER_12, 0.1);

            // Penalty for knights on the edge of the board, and again for corner
            value += mask_eval(team_idx, piece_mask, EDGES, -0.4);
            value += mask_eval(team_idx, piece_mask, CORNER_MASK, -0.3);
        },
        PIECE_BISHOP => {
            // In middle games, have the bishops positioned in this mask
            const GOOD_BISHOP_MASK_MG: BitMask = 0x422418183c7e00;
            const BAD_BISHOP_MASK_MG: BitMask = 0xc381810000000000; // Top wing edges of the board
            value += mask_eval(team_idx, piece_mask, GOOD_BISHOP_MASK_MG, 0.3 * opp_attack_power);
            value += mask_eval(team_idx, piece_mask, BAD_BISHOP_MASK_MG, -0.5 * opp_attack_power);

            // In end games, have the bishops towards the middle of the board
            value += mask_eval(team_idx, piece_mask, CENTER_12, 0.3 * (1.0 - opp_attack_power));

            // Give penalties for bishops on the same square as pawns
            let team_pawns = board.pieces[team_idx][PIECE_PAWN];
            let opp_pawns = board.pieces[1 - team_idx][PIECE_PAWN];
            for color_mask in COLOR_MASKS {
                let bishops_of_color = (piece_mask & color_mask).count_ones();
                let team_pawns_of_color = (team_pawns & color_mask).count_ones();

                // Small penalty for having our own pawns on the same color as our bishop
                value += ((bishops_of_color * team_pawns_of_color) as Value) * -0.05;
            }
        },
        PIECE_ROOK => {
            // Bonus for more central rooks in the middlegame
            value += mask_eval(team_idx, piece_mask, CENTER_26, 0.1 * opp_attack_power);
            value += mask_eval(team_idx, piece_mask, CENTER_12, 0.1 * opp_attack_power);

            // Bonus for elevated rooks in the middlegame
            value += mask_eval(team_idx, piece_mask, ELEVATED_2, 0.4 * opp_attack_power);
            value += mask_eval(team_idx, piece_mask, ELEVATED_4, 0.2 * opp_attack_power);

            let all_pawns = board.pieces[0][PIECE_PAWN] | board.pieces[1][PIECE_PAWN];
            for x in 0..8 {
                let file = bm_make_column(x);

                let pawns_in_file = (all_pawns & file).count_ones();
                let rooks_in_file = (piece_mask & file).count_ones();

                if pawns_in_file == 0 {
                    // Open file
                    value += (rooks_in_file as Value) * 0.4;
                } else if pawns_in_file == 1 {
                    // Half-open
                    value += (rooks_in_file as Value) * 0.2;
                }
            }
        },
        PIECE_QUEEN => {
            // Slight bonus for having a central queen
            value += mask_eval(team_idx, piece_mask, CENTER_26, 0.15);

            // Slight bonus for having an elevated queen in the middlegame
            value += mask_eval(team_idx, piece_mask, ELEVATED_2, 0.12 * opp_attack_power);
            value += mask_eval(team_idx, piece_mask, ELEVATED_4, 0.12 * opp_attack_power);
        },
        PIECE_KING => { // King
            // Centralize the king in endgames
            value += mask_eval(team_idx, piece_mask, CENTER_12, 0.2 * (1.0 - opp_attack_power));
            value += mask_eval(team_idx, piece_mask, CENTER_26, 0.2 * (1.0 - opp_attack_power));

            let king_attacks = lookup_gen::get_piece_base_tos(PIECE_KING, bm_to_idx(piece_mask));
            // Slight bonus for defending our pawns with our king in endgames
            value += mask_eval(team_idx, king_attacks, team_pawns, 0.1 * (1.0 - opp_attack_power));
        }
        _ => {}
    }

    value
}

fn eval_center_control(board: &Board, team_idx: usize) -> Value {
    const CENTER_12: BitMask = 0x183c3c180000; // Center-most 12 squares

    let attack_center_12_count = (board.attacks[team_idx] & CENTER_12).count_ones();
    let attack_center_4_count = (board.attacks[team_idx] & CENTER_4).count_ones();

    (attack_center_12_count as Value) * 0.01
        + (attack_center_4_count as Value) * 0.02
}

fn eval_mobility(board: &Board, team_idx: usize) -> Value {
    let attacks = board.attacks[team_idx];
    (attacks.count_ones() as Value) * 0.04 // Per square-attacked
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
            pawn_coverage_frac *= 0.65;
        }
    }

    // Calculate distance from our back rank, and from the center of the board
    let king_height_frac = ((king_y - back_rank_y).abs() as Value) / 8.0;
    let king_off_center_frac = (((king_x as Value) - 3.5).abs() / 3.5).min(0.8); // We don't care about the very corner

    // Pretending the king is a queen to measure accessibility
    let accessible_squares = lookup_gen::get_piece_tos(PIECE_QUEEN, king, king_pos_idx, board.occupancy[team_idx]).count_ones();
    let accessibility_penalty  = (accessible_squares as Value) * -0.05;

    ((pawn_coverage_frac * 0.5)
        + (king_height_frac * -2.5)
        + (king_off_center_frac * 0.75)
        + accessibility_penalty
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

    let mut value = eval_material(board, team_idx);
    for piece_idx in 0..NUM_PIECES {
        value += eval_piece_type(board, team_idx, piece_idx, board.pieces[team_idx][piece_idx], opp_attack_power);
    }

    value
        + eval_center_control(board, team_idx)
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
        entries[team_idx].push(("Material".to_string(), eval_material(board, team_idx)));
        for piece_idx in 0..NUM_PIECES {
            let piece_type_eval = eval_piece_type(
                board, team_idx, piece_idx, board.pieces[team_idx][piece_idx], attack_power[1 -team_idx]
            );
            entries[team_idx].push((PIECE_NAMES[piece_idx].to_string() + "s", piece_type_eval));
        }
        entries[team_idx].push(("Center Control".to_string(), eval_center_control(board, team_idx)));
        entries[team_idx].push(("Mobility".to_string(), eval_mobility(board, team_idx)));
        entries[team_idx].push(("King Safety".to_string(), eval_king_safety(board, team_idx, attack_power[1 -team_idx])));

        entries[team_idx].push(("TOTAL ADV".to_string(), team_vals[team_idx] - team_vals[1 - team_idx]));
    }

    assert_eq!(entries[0].len(), entries[1].len());
    let num_entries = entries[0].len();
    for i in 0..num_entries {
        if i == (num_entries - 1) {
            println!("{}", "-".to_string().repeat(33));
        }

        let name = &entries[0][i].0;
        let vals = [entries[0][i].1, entries[1][i].1];
        println!("{:>14} | {:>+0width$.prec$} | {:>+0width$.prec$}", name, vals[0], vals[1], width=6, prec=2);
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
        eval -= PIECE_BASE_VALUES[mv.from_piece_idx];
    }

    eval += TURN_BONUS;

    eval
}

pub fn to_centipawns(value: Value) -> i64 {
    (value * 100.0).round() as i64
}