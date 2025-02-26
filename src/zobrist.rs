use crate::bitmask::*;
use crate::board::*;
use rand::Rng;
extern crate rand;

pub type Hash = u64;

static mut LT_HASH_PIECE: [[[Hash; 64]; NUM_PIECES]; 2] = [[[0; 64]; NUM_PIECES]; 2];
static mut LT_HASH_CASTLE_RIGHTS: [[Hash; 2]; 2] = [[0; 2]; 2];
static mut LT_HASH_EN_PASSANT: [Hash; 64] = [0; 64];
static mut LT_HASH_TURN: Hash = 0;

pub fn hash_piece(team_idx: usize, piece_idx: usize, pos_idx: usize) -> Hash {
    unsafe { LT_HASH_PIECE[team_idx][piece_idx][pos_idx] }
}

pub fn hash_castle_rights(castle_rights: [[bool; 2]; 2]) -> Hash {
    let mut result = 0;
    for i in 0..2 {
        for j in 0..2 {
            if castle_rights[i][j] {
                result ^= unsafe { LT_HASH_CASTLE_RIGHTS[i][j] };
            }
        }
    }

    result
}

pub fn hash_en_passant(en_passant_mask: BitMask) -> Hash {
    if en_passant_mask != 0 {
        unsafe { LT_HASH_EN_PASSANT[bm_to_idx(en_passant_mask)] }
    } else {
        0
    }
}

pub fn hash_turn() -> Hash {
    unsafe { LT_HASH_TURN }
}

pub fn init() {
    let mut rng = rand::rng();
    unsafe {
        for i in 0..2 {
            for j in 0..NUM_PIECES {
                for k in 0..64 {
                    LT_HASH_PIECE[i][j][k] = rng.random::<Hash>();
                }
            }

            for j in 0..2 {
                LT_HASH_CASTLE_RIGHTS[i][j] = rng.random::<Hash>();
            }
        }

        for i in 0..64 {
            LT_HASH_EN_PASSANT[i] = rng.random::<Hash>();
        }

        LT_HASH_TURN = rng.random::<Hash>();
    }
}
