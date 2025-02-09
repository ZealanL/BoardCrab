use crate::bitmask::{bm_from_coord, bm_from_xy, bm_get, bm_to_coord};
use crate::board::*;

type Result<T> = std::result::Result<T, FenError>;

#[derive(Debug, Clone)]
pub struct FenError(String);

impl std::fmt::Display for FenError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "FenError: {}", self.0)
    }
}

// Based off of https://github.com/ZealanL/BoardMouse/blob/main/src/FEN/FEN.cpp
pub fn load_fen_from_parts(fen_parts: &Vec<String>) -> Result<Board> {
    let throw_err = |msg: &str| ->Result<Board>{
        let fen = fen_parts.join(" ");
        Err(FenError(format!("Invalid fen: \"{}\", {}", fen, msg)))?
    };

    let mut board = Board::new();

    if fen_parts.len() < 2 {
        throw_err("fen needs at least 2 parts (position and turn)")?;
    }

    for part in fen_parts {
        if !part.is_ascii() {
            throw_err("fen is not ascii")?;
        }
    }

    // Parse pieces
    {
        let mut x: i64 = 0;
        let mut y: i64 = 7;
        for ch in fen_parts[0].chars() {
            if ch.is_ascii_alphabetic() {
                let team_idx = if ch.is_ascii_uppercase() { 0 } else { 1 };
                let mut piece_type: usize = 0;
                let mut piece_type_found = false;
                for i in 0..NUM_PIECES {
                    if ch.eq_ignore_ascii_case(&PIECE_CHARS[i]) {
                        piece_type = i;
                        piece_type_found = true;
                        break;
                    }
                }

                if !piece_type_found {
                    throw_err(format!("invalid piece type char \'{ch}\'").as_str())?;
                }

                if piece_type == PIECE_KING && board.pieces[team_idx][PIECE_KING] != 0 {
                    throw_err(format!("team idx {team_idx} has multiple kings").as_str())?;
                }

                board.pieces[team_idx][piece_type] |= bm_from_xy(x, y);

                x += 1;
            } else if ch.is_ascii_digit() {
                let num = (ch as i64) - ('0' as i64);
                if num < 1 || num > 8 {
                    throw_err(format!("bad padding digit '{ch}', expected 1-8").as_str())?;
                }
                x += num;
                if x > 8 {
                    throw_err("padding puts x out of bounds")?;
                }
            } else if ch == '/' {
                if y <= 0 {
                    throw_err("too many rank separators")?;
                } else if x != 8 {
                    throw_err("bad rank separator, each separator must occur at x=8")?;
                }

                y -= 1;
                x = 0;
            } else {
                throw_err(format!("invalid fen character \'{ch}\'").as_str())?;
            }

            if x > 8 {
                throw_err("row goes out of bounds on x")?;
            }
        }

        for team_idx in 0..2 {
            if board.pieces[team_idx as usize][PIECE_KING] == 0 {
                throw_err(format!("team idx {team_idx} has no king").as_str())?;
            }
        }

        board.full_update();
    }

    // Parse turn
    {
        let turn_str = &fen_parts[1];
        if turn_str.len() != 1 {
            throw_err(format!("invalid turn token \"{turn_str}\", bad length").as_str())?;
        }

        let turn_char: char = turn_str.chars().nth(0).unwrap();
        if turn_char.eq_ignore_ascii_case(&'W') {
            board.turn_idx = 0;
        } else if turn_char.eq_ignore_ascii_case(&'B') {
            board.turn_idx = 1;
        } else {
            throw_err(format!("invalid turn token \"{turn_str}\", expected 'w' or 'b'").as_str())?;
        }
    }

    // Read castle rights
    if fen_parts.len() >= 3 {
        let castle_str = &fen_parts[2];
        if castle_str == "-" {
            // No castling
        } else {
            if castle_str.len() > 4 {
                throw_err(format!("invalid castle string \"{castle_str}\", bad length").as_str())?;
            }

            for ch in castle_str.chars() {
                let team_idx = if ch.is_ascii_uppercase() { 0 } else { 1 };
                if ch.eq_ignore_ascii_case(&'K') {
                    board.castle_rights[team_idx][1] = true;
                } else if ch.eq_ignore_ascii_case(&'Q') {
                    board.castle_rights[team_idx][0] = true;
                } else {
                    throw_err(format!("invalid castle string \"{castle_str}\", bad char \'{ch}\'").as_str())?;
                }
            }
        }
    }

    // Read en passant coordinate
    if fen_parts.len() >= 4 {
        let en_passant_str = &fen_parts[3];
        if en_passant_str == "-" {
            // No en passant
        } else {
            if en_passant_str.len() != 2 {
                throw_err(format!("invalid castle string \"{en_passant_str}\", bad length").as_str())?;
            }

            let pos = bm_from_coord(en_passant_str);
            if board.combined_occupancy() & pos != 0 {
                throw_err(format!("invalid en passant coordinate \"{en_passant_str}\", a pawn/piece resides there").as_str())?;
            }

            board.en_passant_mask = pos;
        }
    }

    // Read half-move counter
    if fen_parts.len() >= 5 {
        let half_move_counter = &fen_parts[4];
        match half_move_counter.parse::<u8>() {
            Ok(x) => { board.half_move_counter = x },
            _ => { throw_err(format!("invalid half-move counter \"{half_move_counter}\"").as_str())?; }
        }
    }

    // We don't need the move number

    // Full update again
    board.full_update();

    Ok(board)
}

pub fn load_fen(fen: &str) -> Result<Board> {
    let fen_parts = fen.split(" ").map(|v| v.to_string()).collect::<Vec<String>>();
    load_fen_from_parts(&fen_parts)
}

pub fn make_fen(board: &Board) -> String {
    // TODO: This code is pretty messy and generally lame

    use std::fmt::Write;
    let mut position_stream: String = String::new();

    // Write position
    for y in (0..8).rev() {

        let mut empty_counter = 0;

        for x in 0..8 {
            let mask = bm_from_xy(x, y);
            if board.combined_occupancy() & mask != 0 {

                // Piece on the square
                if empty_counter > 0 {
                    write!(position_stream, "{empty_counter}").unwrap();
                    empty_counter = 0;
                }

                let mut team_idx = 0;
                let mut piece_char = 0 as char;
                for piece_type in 0..NUM_PIECES {
                    if bm_get(board.pieces[0][piece_type], x, y) {
                        piece_char = PIECE_CHARS[piece_type];
                    } else if bm_get(board.pieces[1][piece_type], x, y) {
                        piece_char = PIECE_CHARS[piece_type];
                        team_idx = 1;
                    }
                }
                debug_assert!(piece_char != (0 as char));

                let piece_char_c;
                if team_idx == 0 {
                    piece_char_c = piece_char.to_ascii_uppercase();
                } else {
                    piece_char_c = piece_char.to_ascii_lowercase();
                }

                write!(position_stream, "{piece_char_c}").unwrap();
            } else {
                // Empty square

                empty_counter += 1;
            }
        }

        if empty_counter > 0 {
            write!(position_stream, "{empty_counter}").unwrap();
        }

        if y > 0 {
            write!(position_stream, "/").unwrap();
        }
    }

    // Write castle rights
    let mut castle_rights_stream: String = String::new();
    for team_idx in 0..2 {
        for side in (0..2).rev() {
            if board.castle_rights[team_idx][side] {
                let side_char = if side == 0 { 'Q' } else { 'K' };
                write!(
                    castle_rights_stream, "{}",
                    if team_idx == 0 { side_char.to_ascii_uppercase() } else { side_char.to_ascii_lowercase() }
                ).unwrap();
            }
        }
    }

    // Write en passant square
    let mut en_passant_stream: String = String::new();
    if board.en_passant_mask != 0 {
        write!(en_passant_stream, "{}", bm_to_coord(board.en_passant_mask)).unwrap();
    }

    // Combine all streams
    let mut result: String = String::new();
    write!(result, "{position_stream}").unwrap();
    write!(result, " {}", if board.turn_idx == 0 { 'w' } else { 'b' }).unwrap(); // Write team
    for sub_stream in [castle_rights_stream, en_passant_stream] {
        if !sub_stream.is_empty() {
            write!(result, " {}", sub_stream).unwrap();
        } else {
            write!(result, " -").unwrap();
        }
    }

    write!(result, " {} {}", board.half_move_counter, 1).unwrap(); // Write half move and normal move counter
    // TODO: Add actual move counter

    result
}

pub const FEN_START_POS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";