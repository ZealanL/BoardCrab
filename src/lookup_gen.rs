use crate::bitmask::*;
use crate::board::*;
use crate::lookup_gen_magic;

// Basic lookup tables for possible moves
static mut LT_KNIGHT_MOVE: [BitMask; 64] = [0; 64];
static mut LT_ROOK_MOVE: [BitMask; 64] = [0; 64];
static mut LT_BISHOP_MOVE: [BitMask; 64] = [0; 64];
static mut LT_QUEEN_MOVE: [BitMask; 64] = [0; 64];
static mut LT_KING_MOVE: [BitMask; 64] = [0; 64];

// Complex lookup tables for occluded slider moves
// TODO: Implement
//const SLIDER_OCCLUSION_LOOKUP_COUNT: usize = usize::pow(128, 2);
//static mut LT_ROOK_OCCLUDE: [[BitMask; 64]; SLIDER_OCCLUSION_LOOKUP_COUNT] = [[0; 64]; SLIDER_OCCLUSION_LOOKUP_COUNT];
//static mut LT_BISHOP_OCCLUDE: [[BitMask; 64]; SLIDER_OCCLUSION_LOOKUP_COUNT] = [[0; 64]; SLIDER_OCCLUSION_LOOKUP_COUNT];

// Masks from one square to another
// These only work for straight lines and perfect diagonals
static mut LT_BETWEEN_INCLUSIVE: [[BitMask; 64]; 64] = [[0; 64]; 64]; // The path between two points, including the two points
static mut LT_BETWEEN_EXCLUSIVE: [[BitMask; 64]; 64] = [[0; 64]; 64]; // The path between two points, NOT including the two points

static mut LT_RAY: [[BitMask; 64]; 64] = [[0; 64]; 64]; // A full ray that starts at the first pos and continues until the edge of the board

fn is_inside_board(x: i64, y: i64) -> bool {
    (x >= 0) && (y >= 0) && (x < 8) && (y < 8)
}

// Starts at the specified position, continues until the end of the board or until it hits the occlusion mask
fn make_ray(start_x: i64, start_y: i64, dx: i64, dy: i64, occlusion_mask: BitMask) -> BitMask {
    let mut mask: BitMask = 0;
    let mut x = start_x;
    let mut y = start_y;
    while is_inside_board(x, y) && !bm_get(occlusion_mask, x, y) {
        bm_set(&mut mask, x, y, true);
        x += dx;
        y += dy;
    }
    mask
}

fn init_at_pos(x: i64, y: i64) {
    let idx = (x + y * 8) as usize;

    // Make knight moves
    for cx in x-2..x+3 {
        for cy in y-2..y+3 {
            if !is_inside_board(cx, cy) {
                continue
            }

            if cx == x || cy == y {
                continue; // Knight must move on both dimensions
            }

            if (cx - x).abs() == (cy - y).abs() {
                continue; // Knight must move a different amount on both dimensions
            }

            unsafe {
                bm_set(&mut LT_KNIGHT_MOVE[idx], cx, cy, true);
            }
        }
    }

    // Make rook moves
    unsafe {
        LT_ROOK_MOVE[idx] = (bm_make_column(x) | bm_make_row(y)) & !bm_from_xy(x, y);
    }

    // Make bishop moves
    let mut bishop_moves: BitMask = 0;
    bishop_moves |= make_ray(x, y, -1, -1, 0);
    bishop_moves |= make_ray(x, y, -1,  1, 0);
    bishop_moves |= make_ray(x, y,  1, -1, 0);
    bishop_moves |= make_ray(x, y,  1,  1, 0);
    unsafe {
        LT_BISHOP_MOVE[idx] = bishop_moves & !bm_from_xy(x, y);
    }

    // Make queen moves
    unsafe {
        LT_QUEEN_MOVE[idx] = LT_ROOK_MOVE[idx] | LT_BISHOP_MOVE[idx];
    }

    // Make king moves
    for cx in x-1..x+2 {
        for cy in y-1..y+2 {
            if !is_inside_board(cx, cy) {
                continue
            }

            if cx == x && cy == y {
                continue; // King must move
            }

            unsafe {
                bm_set(&mut LT_KING_MOVE[idx], cx, cy, true);
            }
        }
    }

    /////////////////

    // Make between paths
    for ex in 0..8 {
        for ey in 0..8 {
            let eidx = (ex + ey * 8) as usize;

            // Make sure these masks always give the start and end points properly
            unsafe{ LT_BETWEEN_INCLUSIVE[idx][eidx] |= bm_from_xy(x, y) | bm_from_xy(ex, ey) };
            unsafe{ LT_RAY[idx][eidx] |= bm_from_xy(x, y) };

            let dx = ex - x;
            let dy = ey - y;
            if dx == 0 && dy == 0 {
                // No direction
                continue;
            } else if (dx.abs() != dy.abs()) && (dx != 0 && dy != 0) {
                // Mismatched direction
                continue;
            }

            let mag = i64::max(dx.abs(), dy.abs());
            let dir_x = dx / mag;
            let dir_y = dy / mag;

            let mut mask: BitMask = 0;
            mask |= make_ray(x, y, dir_x, dir_y, 0);
            mask &= !make_ray(ex, ey, dir_x, dir_y, 0); // Remove bits starting from the endpoint

            unsafe{ LT_BETWEEN_INCLUSIVE[idx][eidx] |= mask };
            unsafe{ LT_BETWEEN_EXCLUSIVE[idx][eidx] = mask & !bm_from_xy(x, y) & !bm_from_xy(ex, ey); };

            unsafe{ LT_RAY[idx][eidx] = make_ray(x, y, dir_x, dir_y, 0); };
        }
    }
}

pub fn walk_in_dir<const SHIFT: i64>(start: BitMask, inv_occ: BitMask) -> BitMask {
    let mut result = start;

    let mask: BitMask;
    if (SHIFT.abs() % 8) != 0 {
        // Needs clamping since we are moving horizontally
        let clamp_area: BitMask = match SHIFT {
            1 | 9 | -7 => bm_make_column(7),
            _ => bm_make_column(0)
        };

        mask = (inv_occ | start) & !clamp_area;
    } else {
        mask = inv_occ | start;
    }

    for _i in 0..7 {
        if SHIFT > 0 {
            result |= (result & mask) << SHIFT;
        } else {
            result |= (result & mask) >> -SHIFT;
        }
    }

    result
}

// TODO: Move this stuff to move_gen maybe?
fn generate_slider_tos_slow<const ROOK: bool, const BISHOP: bool>(piece_pos: BitMask, occupy: BitMask) -> BitMask {
    // TODO: This is very slow
    let inv_occ = !occupy;

    let mut attack: BitMask = 0;

    if ROOK {
        attack |= walk_in_dir::<-1>(piece_pos, inv_occ);
        attack |= walk_in_dir::< 1>(piece_pos, inv_occ);
        attack |= walk_in_dir::<-8>(piece_pos, inv_occ);
        attack |= walk_in_dir::< 8>(piece_pos, inv_occ);
    }

    if BISHOP {
        attack |= walk_in_dir::<-7>(piece_pos, inv_occ);
        attack |= walk_in_dir::< 7>(piece_pos, inv_occ);
        attack |= walk_in_dir::<-9>(piece_pos, inv_occ);
        attack |= walk_in_dir::< 9>(piece_pos, inv_occ);
    }

    attack & !piece_pos
}


pub fn get_piece_base_tos(piece_idx: usize, pos_idx: usize) -> BitMask {
    match piece_idx {
        PIECE_KNIGHT => unsafe { LT_KNIGHT_MOVE[pos_idx] },
        PIECE_BISHOP => unsafe { LT_BISHOP_MOVE[pos_idx] },
        PIECE_ROOK => unsafe { LT_ROOK_MOVE[pos_idx] },
        PIECE_QUEEN => unsafe { LT_QUEEN_MOVE[pos_idx] },
        PIECE_KING => unsafe { LT_KING_MOVE[pos_idx] },
        _ => 0
    }
}

pub fn get_slider_tos_slow(piece_idx: usize, piece_pos_idx: usize, occupy: BitMask) -> BitMask {
    match piece_idx {
        PIECE_BISHOP => generate_slider_tos_slow::<false, true>(bm_from_idx(piece_pos_idx), occupy),
        PIECE_ROOK => generate_slider_tos_slow::<true, false>(bm_from_idx(piece_pos_idx), occupy),
        PIECE_QUEEN => generate_slider_tos_slow::<true, true>(bm_from_idx(piece_pos_idx), occupy),
        _ => {
            panic!("Piece is not a slider")
        }
    }
}

pub fn get_slider_tos_fast(piece_idx: usize, piece_pos_idx: usize, occupy: BitMask) -> BitMask {
    match piece_idx {
        PIECE_BISHOP => lookup_gen_magic::get_bishop_moves(piece_pos_idx, occupy),
        PIECE_ROOK => lookup_gen_magic::get_rook_moves(piece_pos_idx, occupy),
        PIECE_QUEEN => lookup_gen_magic::get_bishop_moves(piece_pos_idx, occupy) | lookup_gen_magic::get_rook_moves(piece_pos_idx, occupy),
        _ => {
            panic!("Piece is not a slider")
        }
    }
}

pub fn get_piece_tos(piece_idx: usize, piece_pos: BitMask, piece_pos_idx: usize, occupy: BitMask) -> BitMask {
    let occupy = occupy & !piece_pos;

    match piece_idx {
        PIECE_BISHOP | PIECE_ROOK | PIECE_QUEEN => {
            #[cfg(not(debug_assertions))]
            return get_slider_tos_fast(piece_idx, piece_pos_idx, occupy);
            #[cfg(debug_assertions)]
            get_slider_tos_slow(piece_idx, piece_pos_idx, occupy)
        },
        _ => { // Non-sliding
            get_piece_base_tos(piece_idx, piece_pos_idx)
        },
    }

}
pub fn get_between_mask_exclusive(idx_a: usize, idx_b: usize) -> BitMask {
    unsafe { LT_BETWEEN_EXCLUSIVE[idx_a][idx_b] }
}

pub fn get_between_mask_inclusive(idx_a: usize, idx_b: usize) -> BitMask {
    unsafe { LT_BETWEEN_INCLUSIVE[idx_a][idx_b] }
}

pub fn get_ray_mask(idx_from: usize, idx_towards: usize) -> BitMask {
    unsafe { LT_RAY[idx_from][idx_towards] }
}

pub fn init() {
    println!("Initializing move lookup tables...");
    for x in 0..8 {
        for y in 0..8 {
            init_at_pos(x, y);
        }
    }

    println!("Initializing lookup tables...");
    println!(" > Done!");
}