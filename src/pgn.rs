use std::fmt::Write;
use crate::bitmask::{bm_make_column, bm_make_row, bm_to_coord, bm_to_xy, BitMask};
use crate::board::{Board, Move, NUM_PIECES, PIECE_CHARS, PIECE_NAMES, PIECE_PAWN};
use crate::move_gen;
use crate::fen;

type Result<T> = std::result::Result<T, PgnError>;

#[derive(Debug, Clone)]
pub struct PgnError(String);

impl std::fmt::Display for PgnError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "PgnError: {}", self.0)
    }
}

pub fn move_to_algebraic_str(board: &Board, mv: &Move) -> Result<String> {
    let mut move_buffer = move_gen::MoveBuffer::new();
    move_gen::generate_moves(board, &mut move_buffer);

    if move_buffer.is_empty() {
        return Err(PgnError("No valid moves".to_string()));
    }

    if mv.has_flag(Move::FLAG_CASTLE) {
        let castle_str = if mv.to > mv.from { "O-O" } else { "O-O-O" };
        return Ok(castle_str.to_string());
    }

    let mut move_str;
    if mv.from_piece_idx == PIECE_PAWN {
        if mv.has_flag(Move::FLAG_CAPTURE) {
            move_str = format!("{}x{}", bm_to_coord(mv.from).chars().next().unwrap(), bm_to_coord(mv.to));
        } else {
            move_str = bm_to_coord(mv.to);
        }

        if mv.to_piece_idx != PIECE_PAWN {
            move_str += format!("={}", PIECE_CHARS[mv.to_piece_idx]).as_str();
        }
    } else {
        move_str = PIECE_CHARS[mv.from_piece_idx].to_string();

        let mut possible_froms: BitMask = 0;
        for omv in move_buffer.iter() {
            if omv.from_piece_idx == mv.from_piece_idx && omv.to == mv.to {
                possible_froms |= omv.from;
            }
        }

        if (possible_froms & mv.from) == 0 {
            return Err(PgnError("Piece move does not exist in the position".to_string()));
        }

        let (from_x, from_y) = bm_to_xy(mv.from);
        let other_possible_froms = possible_froms & !mv.from;
        if other_possible_froms != 0 {
            // Whether the move is ambiguous on [file, rank]
            let ambiguities = [
                (other_possible_froms & bm_make_column(from_x)) != 0,
                (other_possible_froms & bm_make_row(from_y)) != 0
            ];

            if !ambiguities[0] || !ambiguities[1] {
                // Either rank or file is non-ambiguous, append the corresponding character
                for i in 0..2 {
                    if !ambiguities[i] {
                        move_str.push(bm_to_coord(mv.from).chars().nth(i).unwrap());
                        break;
                    }
                }
            } else {
                // Both rank and file is ambiguous, append the full coordinate
                move_str += bm_to_coord(mv.from).as_str();
            }
        }

        if mv.has_flag(Move::FLAG_CAPTURE) {
            move_str += "x";
        }

        move_str += bm_to_coord(mv.to).as_str();
    }

    // Determine if this move is a check
    {
        let mut next_board = board.clone();
        next_board.do_move(mv);

        if next_board.checkers != 0 {
            // Determine if checkmate
            let mut next_move_buffer = move_gen::MoveBuffer::new();
            move_gen::generate_moves(&next_board, &mut next_move_buffer);

            if next_move_buffer.is_empty() {
                // Checkmate
                move_str += "#";
            } else {
                // Just a check
                move_str += "+";
            }
        }

    }

    Ok(move_str)
}

pub fn make_pgn(start_board: &Board, moves: &Vec<Move>) -> Result<String> {
    let mut stream: String = String::new();
    let start_fen = fen::make_fen(&start_board);
    if start_fen != fen::FEN_START_POS {
        // Add fen to PGN
        // This is the Lichess format (which also works on chess.com)
        writeln!(stream, "[Variant \"From Position\"]").unwrap();
        writeln!(stream, "[FEN \"{}\"]", start_fen).unwrap();
    }

    if !stream.is_empty() {
        // Create space
        writeln!(stream).unwrap();
    }

    let mut cur_board = start_board.clone();
    let mut cur_move_number = 1;

    if cur_board.turn_idx == 1 {
        // We start with black's turn, so we need to add the appropriate prefix
        write!(stream, "{}... ", cur_move_number).unwrap();
    }

    for mv in moves {
        let turn = cur_board.turn_idx;
        if turn == 0 {
            // White turn, declare move number
            write!(stream, "{}. ", cur_move_number).unwrap();
        }

        write!(stream, "{} ", move_to_algebraic_str(&cur_board, mv)?).unwrap();

        if turn == 1 {
            // Black just moved, increase turn number
            cur_move_number += 1;
        }

        cur_board.do_move(mv);
    }

    Ok(stream.trim().to_string())
}