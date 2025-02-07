use crate::bitmask::*;
use crate::board::*;
use crate::lookup_gen;

// Special check if an en passant capture would unpin a horizontal slider
// Because en passant removes 2 pawns from the rank at the same time, it can bypass our pin checks
fn is_en_passant_pinned_horizontal(pawn_from: BitMask, board: &Board, turn_idx: usize, pawn_advance_dy: i64) -> bool{
    let rank_mask = bm_make_row(bm_to_xy(pawn_from).1);

    let opp_horizontal_sliders = board.pieces[1 - turn_idx][PIECE_ROOK] | board.pieces[1 - turn_idx][PIECE_QUEEN];
    if rank_mask & opp_horizontal_sliders != 0 {
        // There are horizontal sliders on the rank
        // It's important to do this check branched first, because the next checks will be much more expensive

        // Where the pawn we are actually capturing is
        let target_pawn_pos = bm_shift(board.en_passant_mask, 0, -pawn_advance_dy);

        // Separate the capturing pawn and the en passant square into left and right masks
        let left_pos = BitMask::min(pawn_from, target_pawn_pos);
        let right_pos = BitMask::max(pawn_from, target_pawn_pos);
        let occ_combined = board.combined_occupancy();

        // Walk left and right from the positions
        let left_walk_mask = lookup_gen::walk_in_dir::<-1>(left_pos, !occ_combined);
        let right_walk_mask = lookup_gen::walk_in_dir::<1>(right_pos, !occ_combined);
        let combined_walk_mask = left_walk_mask | right_walk_mask;

        let king = board.pieces[turn_idx][PIECE_KING];

        // If the path has an opponent's horizontal slider on one side, and our king on the other, it's a pin
        ((opp_horizontal_sliders & combined_walk_mask) != 0) && ((king & combined_walk_mask) != 0)
    } else {
        // No horizontal sliders
        false
    }
}

fn generate_pawn_attacks_side<const SIDE: usize>(pawns: BitMask, pawn_advance_dy: i64) -> BitMask {
    // The full board excluding the furthest column
    // Order is left,right
    const CAPTURE_MASK: [BitMask; 2] = [!bm_make_column(0), !bm_make_column(7)];

    bm_shift(pawns & CAPTURE_MASK[SIDE], if SIDE == 0 { -1 } else { 1 }, pawn_advance_dy)
}

pub fn generate_attacks(board: &Board, team_idx: usize, piece_idx: usize, from: BitMask) -> BitMask {
    let opp_king = board.pieces[1 - team_idx][PIECE_KING];
    let occ_for_attack = board.combined_occupancy() & !opp_king; // We want slider attacks to go through the king

    let mut attacks: BitMask = 0;

    if piece_idx == PIECE_PAWN {
        // Just bitmask it
        let pawn_advance_dy = if team_idx == 0 { 1 } else { -1 };
        let pawn_attacks = generate_pawn_attacks_side::<0>(from, pawn_advance_dy) |  generate_pawn_attacks_side::<1>(from, pawn_advance_dy);
        attacks |= pawn_attacks;
    } else {
        // TODO: We don't need to have lookup gen mask out our own team
        let idx = bm_to_idx(from);
        let tos = lookup_gen::get_piece_tos(piece_idx, from, idx, occ_for_attack);

        attacks |= tos;
    }

    attacks
}

pub fn can_castle(side: usize, board: &Board, team_idx: usize, is_in_check: bool) -> bool {
    // From: https://github.com/ZealanL/BoardMouse/blob/4d3b6c608a3cb82a1299580a90dcb3c831fc02f8/src/Engine/MoveGen/MoveGen.cpp#L13
    // Ordering is [Left/Queen-side, Right/King-side]
    const CASTLE_EMPTY_MASKS: [[BitMask; 2]; 2] = [
        [ // White
            bm_from_coord("B1") | bm_from_coord("C1") | bm_from_coord("D1"),
            bm_from_coord("F1") | bm_from_coord("G1")
        ],

        [ // Black
            bm_from_coord("B8") | bm_from_coord("C8") | bm_from_coord("D8"),
            bm_from_coord("F8") | bm_from_coord("G8")
        ]
    ];

    // These squares cannot be in attack from the enemy in order to castle
    const CASTLE_SAFETY_MASKS: [[BitMask; 2]; 2] = [
        [ // White
            bm_from_coord("C1") | bm_from_coord("D1"),
            bm_from_coord("F1") | bm_from_coord("G1"),
        ],

        [ // Black
            bm_from_coord("C8") | bm_from_coord("D8"), // Far
            bm_from_coord("F8") | bm_from_coord("G8"), // Near
        ]
    ];

    if !board.castle_rights[team_idx][side] { return false; }
    if is_in_check { return false; }
    if (board.combined_occupancy() & CASTLE_EMPTY_MASKS[team_idx][side]) != 0 { return false; }
    if (board.attacks[1 - team_idx] & CASTLE_SAFETY_MASKS[team_idx][side]) != 0 { return false; }

    true
}

pub fn generate_moves(board: &Board) -> Vec<Move> {
    let mut moves: Vec<Move> = Vec::with_capacity(50);

    let occ_team = board.occupancy[board.turn_idx];
    let occ_opp = board.occupancy[1 - board.turn_idx];
    let occ_combined = occ_team | occ_opp;
    let king = board.pieces[board.turn_idx][PIECE_KING];
    let num_checkers = board.checkers.count_ones(); // TODO: Don't need a full popcount, just >1 check

    let move_mask: BitMask;
    if num_checkers == 1 {
        // Must block the check or capture the checker
        move_mask = lookup_gen::get_between_mask_inclusive(bm_to_idx(king), bm_to_idx(board.checkers));
    } else {
        // No restrictions
        // (Double checks will be handled separately)
        move_mask = !0;
    }

    let pawn_advance_dy = if board.turn_idx == 0 { 1 } else { -1 };

    for piece_idx in 0..NUM_PIECES {
        if (num_checkers > 1) && (piece_idx != PIECE_KING) {
            // Multiple checks, king must move
            continue;
        }

        for from in bm_itr_bits(board.pieces[board.turn_idx][piece_idx]) {
            let idx = bm_to_idx(from);
            let mut tos: BitMask;
            if piece_idx == PIECE_PAWN {

                // Single-move
                tos = bm_shift(from, 0, pawn_advance_dy) & !occ_combined;

                // Double-move
                const STARTING_PAWNS_MASK: [BitMask; 2] = [bm_make_row(1), bm_make_row(6)];
                if (from & STARTING_PAWNS_MASK[board.turn_idx]) != 0 {
                    tos |= bm_shift(tos, 0, pawn_advance_dy) & !occ_combined;
                }

                // Pawn attacks
                let attack_tos =
                    (generate_pawn_attacks_side::<0>(from, pawn_advance_dy) | generate_pawn_attacks_side::<1>(from, pawn_advance_dy))
                        & (occ_opp | board.en_passant_mask);

                tos |= attack_tos;

                if attack_tos & board.en_passant_mask != 0 {
                    // Check annoying edge case to make sure en passant is actually legal
                    if is_en_passant_pinned_horizontal(from, board, board.turn_idx, pawn_advance_dy) {
                        // En passant isn't legal!
                        tos &= !board.en_passant_mask;
                    }
                }
            } else {
                // Use lookup
                tos = lookup_gen::get_piece_tos(piece_idx, from, idx, occ_combined);
            }

            // Ban capturing our own pieces
            tos &= !occ_team;

            if piece_idx == PIECE_KING {
                // King cannot move into attacked areas
                tos &= !board.attacks[1 - board.turn_idx];

                for castle_side in 0..2 {
                    if can_castle(castle_side, board, board.turn_idx, num_checkers != 0) {
                        moves.push(Move {
                            from: king,
                            to: if castle_side == 0 { bm_shift(king, -2, 0) } else { bm_shift(king, 2, 0) },
                            from_piece_idx: PIECE_KING,
                            to_piece_idx: PIECE_KING,
                            move_type: MoveType::Castle
                        });
                    }
                }

            } else {
                if (board.pinned[board.turn_idx] & from) != 0 {
                    // Restrict to the path following the inverse direction of the pin from the king
                    // Thankfully no piece can jump over a square without leaving a pin, otherwise this would break
                    tos &= lookup_gen::get_ray_mask(bm_to_idx(king), idx);
                }

                tos &= move_mask;
            }

            for to in bm_itr_bits(tos) {
                let move_type: MoveType;
                if piece_idx == PIECE_PAWN {
                    const PROMOTE_MASK: [BitMask; 2] = [bm_make_row(7), bm_make_row(0)];
                    if (to & PROMOTE_MASK[board.turn_idx]) != 0 {
                        // Promotion
                        for to_piece_idx in 1..NUM_PIECES {
                            if to_piece_idx == PIECE_KING {
                                continue; // Can't promote to king lol
                            }

                            moves.push(Move {
                                from,
                                to,
                                from_piece_idx: PIECE_PAWN,
                                to_piece_idx,
                                move_type: MoveType::Promotion
                            });
                        }
                        continue
                    } else if (to & board.en_passant_mask) != 0 {
                        move_type = MoveType::EnPassantCapture;
                    } else if to == bm_shift(from, 0, pawn_advance_dy * 2) {
                        move_type = MoveType::DoublePawnMove;
                    } else {
                        move_type = MoveType::Normal;
                    }
                } else {
                    move_type = MoveType::Normal;
                }

                moves.push(Move {
                    from,
                    to,
                    from_piece_idx: piece_idx,
                    to_piece_idx: piece_idx,
                    move_type
                });
            }
        }
    }

    moves
}