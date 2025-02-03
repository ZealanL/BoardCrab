use std::cmp::PartialEq;
use std::fmt::Write;
use crate::bitmask::*;
use crate::{lookup_gen, move_gen};

pub const PIECE_PAWN: usize = 0;
pub const PIECE_KNIGHT: usize = 1;
pub const PIECE_BISHOP: usize = 2;
pub const PIECE_ROOK: usize = 3;
pub const PIECE_QUEEN: usize = 4;
pub const PIECE_KING: usize = 5;

pub const NUM_PIECES: usize = 6;
pub const PIECE_CHARS: [char; NUM_PIECES] = ['P', 'N', 'B', 'R', 'Q', 'K'];

////////////////////////////////////////////////////////////////////////////

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum MoveType {
    Normal,
    EnPassantCapture,
    DoublePawnMove,
    Promotion,
    Castle
}

#[derive(Debug, Copy, Clone)]
pub struct Move {
    pub from: BitMask,
    pub to: BitMask,
    pub from_piece_idx: usize,
    pub to_piece_idx: usize,

    pub move_type: MoveType
}

impl Move {
    pub fn new() -> Move {
        Move {
            from: 0,
            to: 0,
            from_piece_idx: 0,
            to_piece_idx: 0,
            move_type: MoveType::Normal
        }
    }
}

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut stream: String = String::new();
        write!(stream, "{}{}", bm_to_coord(self.from), bm_to_coord(self.to))?;
        if self.to_piece_idx != self.from_piece_idx {
            // Promotion
            write!(stream, "{}", PIECE_CHARS[self.to_piece_idx].to_ascii_lowercase())?;
        }
        write!(f, "{}", stream)
    }
}

////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Copy, Clone)]
pub struct Board {

    pub turn_idx: usize,

    // Occupancy (where a piece is for each player)
    pub occupancy: [BitMask; 2],

    // Attacks (where each player attacks)
    pub attacks: [BitMask; 2],

    // Pieces that currently check the player who's turn it is
    pub checkers: BitMask,

    // Pinned pieces
    pub pinned: [BitMask; 2],

    // Bitboard for each piece, for each team, except the king
    pub pieces: [[BitMask; NUM_PIECES]; 2],

    // If a double pawn move was played last move, this is the mask of the en passant capture square
    pub en_passant_mask: BitMask,

    // Castle rights for each side, for each player
    // Order is [left,right] (left being queenside)
    pub castle_rights: [[bool; 2]; 2],

    pub half_move_counter: u8
}

impl Board {

    // Board will be empty
    pub const fn new() -> Board {
        Board {
            occupancy: [0; 2],
            attacks: [0; 2],
            checkers: 0,
            pinned: [0; 2],
            pieces: [[0; NUM_PIECES]; 2],
            turn_idx: 0,
            en_passant_mask: 0,
            castle_rights: [[false; 2]; 2],
            half_move_counter: 0
        }
    }

    pub fn start_pos() -> Board {
        let mut board = Board::new();

        // Set white's pieces
        board.pieces[0][PIECE_PAWN] = bm_make_row(1);
        bm_set(&mut board.pieces[0][PIECE_ROOK], 0, 0, true);
        bm_set(&mut board.pieces[0][PIECE_ROOK], 7, 0, true);
        bm_set(&mut board.pieces[0][PIECE_KNIGHT], 1, 0, true);
        bm_set(&mut board.pieces[0][PIECE_KNIGHT], 6, 0, true);
        bm_set(&mut board.pieces[0][PIECE_BISHOP], 2, 0, true);
        bm_set(&mut board.pieces[0][PIECE_BISHOP], 5, 0, true);
        bm_set(&mut board.pieces[0][PIECE_QUEEN], 3, 0, true);
        bm_set(&mut board.pieces[0][PIECE_KING], 4, 0, true);

        // Copy to black but flipped vertically
        for i in 0..NUM_PIECES {
            board.pieces[1][i] = bm_flip_vertical(board.pieces[0][i]);
        }

        // Enable castling
        board.castle_rights = [[true; 2]; 2];

        board.full_update();

        board
    }

    /////////////////////////////////////////////////

    pub fn combined_occupancy(&self) -> u64 {
        self.occupancy[0] | self.occupancy[1]
    }

    // Updates everything persistent, for after you set up the board
    // Only to be used infrequently
    pub fn full_update(&mut self) {
        // Full-update occupancy
        self.occupancy = [0; 2];
        for team_idx in 0..2 {
            for i in 0..NUM_PIECES {
                self.occupancy[team_idx] |= self.pieces[team_idx][i];
            }
        }

        self.update_attacks(self.turn_idx);
        self.update_attacks(1 - self.turn_idx);
    }

    fn update_attacks(&mut self, team_idx: usize) -> BitMask {
        self.attacks[team_idx] = 0;
        self.pinned[1 - team_idx] = 0;
        self.checkers = 0;

        let occ_opp = self.occupancy[1 - team_idx];
        let occ_combined = self.combined_occupancy();

        let opp_king = self.pieces[1 - team_idx][PIECE_KING];

        for piece_idx in 0..NUM_PIECES {
            for from in bm_itr_bits(self.pieces[team_idx][piece_idx]) {
                let piece_attacks = move_gen::generate_attacks(self, team_idx, piece_idx, from);

                match piece_idx {
                    PIECE_BISHOP | PIECE_ROOK | PIECE_QUEEN => {
                        // Slider, check for pins
                        let from_idx = bm_to_idx(from);
                        let piece_base_attacks = lookup_gen::get_piece_base_tos(piece_idx, from_idx);
                        if piece_base_attacks & opp_king != 0 {
                            { // Update pin
                                let between_mask = lookup_gen::get_between_mask_exclusive(from_idx, bm_to_idx(opp_king));
                                let pinned_by_us = between_mask & occ_combined;
                                if (pinned_by_us.count_ones() == 1) && ((pinned_by_us & occ_opp) != 0) { // TODO: Don't need a full popcount, just >1 check
                                    self.pinned[1 - team_idx] |= pinned_by_us;
                                    debug_assert!((pinned_by_us & occ_opp) == pinned_by_us);
                                }
                            }
                        }
                    }
                    _ => {
                        // Normal piece
                    }
                }

                self.attacks[team_idx] |= piece_attacks;
                if piece_attacks & opp_king != 0 {
                    self.checkers |= from;
                }
            }
        }

        self.attacks[team_idx]
    }

    pub fn do_move(&mut self, mv: Move) {

        // From: https://github.com/ZealanL/BoardMouse/blob/4d3b6c608a3cb82a1299580a90dcb3c831fc02f8/src/Engine/BoardState/BoardState.cpp
        // Order: Left/Queen-side, Right/King-side
        const CASTLING_ROOK_FROM_MASKS: [[BitMask; 2]; 2] = [
            [ // White
                bm_from_coord("A1"), bm_from_coord("H1")
            ],

            [ // Black
                bm_from_coord("A8"), bm_from_coord("H8")
            ]
        ];

        let inv_from = !mv.from;
        let inv_to = !mv.to;

        // Update pieces
        self.pieces[self.turn_idx][mv.from_piece_idx] &= inv_from;
        self.pieces[self.turn_idx][mv.to_piece_idx] |= mv.to;
        for i in 0..NUM_PIECES {
            // TODO: Maybe not required?
            self.pieces[1 - self.turn_idx][i] &= inv_to;
        }

        // Update occupancy
        self.occupancy[self.turn_idx] |= mv.to;
        self.occupancy[self.turn_idx] &= inv_from;
        self.occupancy[1 - self.turn_idx] &= inv_to;

        self.en_passant_mask = 0; // Reset en passant mask (we will set it only if it is a double pawn move)
        match mv.move_type {
            MoveType::DoublePawnMove => {
                self.en_passant_mask = bm_shift(mv.to, 0, if self.turn_idx == 0 { -1 } else { 1 });
            },

            MoveType::EnPassantCapture => {
                let inv_en_passant_pos = !bm_shift(mv.to, 0, if self.turn_idx == 0 { -1 } else { 1 });

                for i in 0..NUM_PIECES {
                    // TODO: Maybe not required?
                    self.pieces[1 - self.turn_idx][i] &= inv_en_passant_pos;
                }
                self.occupancy[1 - self.turn_idx] &= inv_en_passant_pos;
            },

            MoveType::Castle => {
                // We are castling, find and move the rook

                // This works because we cant castle with a vertical king move
                let castle_left: bool = mv.to < mv.from;
                let rook_from = CASTLING_ROOK_FROM_MASKS[self.turn_idx][castle_left as usize];
                let rook_to = if castle_left { bm_shift(mv.to, 1, 0) } else { bm_shift(mv.to, -1, 0) };
                debug_assert!(self.pieces[self.turn_idx][PIECE_ROOK] & rook_from == rook_from);
                debug_assert!(self.combined_occupancy() & rook_to == 0);

                let rook_flip = rook_from | rook_to;
                self.pieces[self.turn_idx][PIECE_ROOK] ^= rook_flip;
                self.occupancy[self.turn_idx] ^= rook_flip;

                // Don't need to update castle rights as the king move clause will handle it after
            }

            _ => {}
        }

        if mv.from_piece_idx == PIECE_KING {
            // Castling is now banned
            self.castle_rights[self.turn_idx] = [false; 2];
        } else {
            // Detect rook move
            for i in 0..2 {
                if mv.from == CASTLING_ROOK_FROM_MASKS[self.turn_idx][i] {
                    self.castle_rights[self.turn_idx][i] = false;
                }
            }
        }

        let is_capture_or_pawn_move = (
            self.occupancy[1 - self.turn_idx] & mv.to != 0)
            || (mv.from_piece_idx == PIECE_PAWN);
        if is_capture_or_pawn_move {
            self.half_move_counter = 0;
        } else {
            self.half_move_counter += 1;
        }

        self.update_attacks(self.turn_idx);
        self.turn_idx = 1 - self.turn_idx;
    }
}

impl std::fmt::Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Based on: https://github.com/official-stockfish/Stockfish/blob/d46c0b6f492bc00fa0a91d91f18e474c14541330/src/bitboard.cpp#L58

        const DIVIDER: &str = "+---+---+---+---+---+---+---+---+";

        let mut stream: String = String::new();
        writeln!(stream, "Board {{")?;
        writeln!(stream, "\tTurn: {}", self.turn_idx)?;
        writeln!(stream, "\tOccupancy[0/1]: {}/{}", self.occupancy[0], self.occupancy[1])?;
        writeln!(stream, "\tCheckers: {}", self.checkers)?;
        writeln!(stream, "\tPinned[0/1]: {}/{}", self.pinned[0], self.pinned[1])?;
        writeln!(stream, "\tAttacks[0/1]: {}/{}", self.attacks[0], self.attacks[1])?;
        writeln!(stream, "\t{DIVIDER}")?;

        for i in 0..8 {
            let y = 8 - i - 1;

            write!(stream, "\t")?;

            for j in 0..8 {
                let x = j;

                let mut piece_char = ' ';
                for piece_type in 0..NUM_PIECES {
                    if bm_get(self.pieces[0][piece_type], x, y) {
                        piece_char = PIECE_CHARS[piece_type].to_ascii_uppercase();
                    } else if bm_get(self.pieces[1][piece_type], x, y) {
                        piece_char = PIECE_CHARS[piece_type].to_ascii_lowercase();
                    }
                }

                let decoration_chars: [char; 2];
                if bm_get(self.checkers, x, y) {
                    decoration_chars = ['+', '+']; // Show checker
                } else if bm_get(self.pinned[self.turn_idx], x, y) {
                    decoration_chars = ['>', '<']; // Show pinned
                } else if bm_get(self.pieces[self.turn_idx][PIECE_KING], x, y) && (self.checkers != 0) {
                    decoration_chars = ['!', '!']; // Show checked
                } else {
                    decoration_chars = [' ', ' '];
                }

                write!(stream, "|{}{piece_char}{}", decoration_chars[0], decoration_chars[1])?;
            }

            writeln!(stream, "| {}\n\t{DIVIDER}", 1 + y)?;
        }

        writeln!(stream, "\t  a   b   c   d   e   f   g   h")?;
        write!(stream, "}}")?;

        write!(f, "{}", stream)
    }
}