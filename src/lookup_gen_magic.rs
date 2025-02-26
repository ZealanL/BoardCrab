use crate::bitmask::*;
use crate::board::*;
use crate::lookup_gen;
use rand::RngCore;

#[derive(Debug, Copy, Clone)]
struct MagicEntry {
    mask: BitMask,
    magic_factor: u64,
    shift: u8,
    table_offset: usize,
}

impl MagicEntry {
    const fn new() -> MagicEntry {
        MagicEntry {
            mask: 0,
            magic_factor: 0,
            shift: 0,
            table_offset: !0, // Causes a crash if this entry is used
        }
    }

    fn index(&self, occupy: BitMask) -> usize {
        let hash = (occupy & self.mask) * self.magic_factor;
        (hash >> self.shift) as usize + self.table_offset
    }
}

static mut LT_MAGICS_BISHOP: [MagicEntry; 64] = [MagicEntry::new(); 64];
static mut LT_MAGICS_ROOK: [MagicEntry; 64] = [MagicEntry::new(); 64];

static mut LT_ALL_MOVES: Vec<BitMask> = Vec::new();

pub fn get_bishop_moves(pos_idx: usize, occupy: BitMask) -> BitMask {
    unsafe {
        let idx = LT_MAGICS_BISHOP[pos_idx].index(occupy);
        LT_ALL_MOVES[idx]
    }
}

pub fn get_rook_moves(pos_idx: usize, occupy: BitMask) -> BitMask {
    unsafe {
        let idx = LT_MAGICS_ROOK[pos_idx].index(occupy);
        LT_ALL_MOVES[idx]
    }
}

pub fn init() {
    println!("Generating magic bitboards...");

    let mut rng = rand::rngs::ThreadRng::default();

    let mut total_table_size = 0;

    for pos_idx in 0..64 {
        let mask = bm_from_idx(pos_idx);
        let (x, y) = bm_to_xy(mask);
        const BOARD_EDGES_X: BitMask = bm_make_column(0) | bm_make_column(7);
        const BOARD_EDGES_Y: BitMask = bm_make_row(0) | bm_make_row(7);

        let base_moves_bishop =
            lookup_gen::get_piece_base_tos(PIECE_BISHOP, pos_idx) & !BOARD_EDGES_X & !BOARD_EDGES_Y;
        let base_moves_rook =
            (bm_make_column(x) & !BOARD_EDGES_Y) | (bm_make_row(y) & !BOARD_EDGES_X) & !mask;

        for i in 0..2 {
            let is_bishop = i == 0;
            let piece_idx = if is_bishop { PIECE_BISHOP } else { PIECE_ROOK };
            let base_moves = if is_bishop {
                base_moves_bishop
            } else {
                base_moves_rook
            };

            let mut occ_subsets = Vec::new();

            // https://www.chessprogramming.org/Traversing_Subsets_of_a_Set
            let mut occ_subset = 0;
            loop {
                occ_subsets.push(occ_subset);

                occ_subset = (occ_subset - base_moves) & base_moves;
                if occ_subset == 0 {
                    break;
                }
            }

            let num_bits = base_moves.count_ones();
            let mut magic_entry = MagicEntry {
                mask: base_moves,
                magic_factor: 0, // To be determined
                shift: (64 - num_bits) as u8,
                table_offset: 0, // Set after
            };

            let cur_table_size = (1 << num_bits) as usize;

            let mut test_table = Vec::new();
            test_table.resize(cur_table_size, false);

            loop {
                // Reset the table
                test_table.fill(false);

                magic_entry.magic_factor = rng.next_u64() & rng.next_u64() & rng.next_u64();

                let mut has_duplicates = false;
                for subset in &occ_subsets {
                    let idx = magic_entry.index(*subset);
                    let table_bool = &mut test_table[idx];
                    if *table_bool {
                        // Duplicate
                        has_duplicates = true;
                        break;
                    } else {
                        *table_bool = true;
                    }
                }

                if !has_duplicates {
                    // Valid hash found, stop searching
                    magic_entry.table_offset = total_table_size;
                    if is_bishop {
                        unsafe {
                            LT_MAGICS_BISHOP[pos_idx] = magic_entry;
                        }
                    } else {
                        unsafe {
                            LT_MAGICS_ROOK[pos_idx] = magic_entry;
                        }
                    }
                    break;
                }
            }

            if total_table_size != unsafe { LT_ALL_MOVES.len() } {
                panic!("Total table size doesn't match expected size");
            }
            unsafe {
                LT_ALL_MOVES.resize(total_table_size + cur_table_size, 0);
            }

            // Populate
            for occ_subset in occ_subsets {
                let idx = magic_entry.index(occ_subset);

                if idx < total_table_size || idx >= total_table_size + cur_table_size {
                    panic!(
                        "Bad table index while populating tables: {} (this should never happen)",
                        idx
                    );
                }

                let valid_moves = lookup_gen::get_slider_tos_slow(piece_idx, pos_idx, occ_subset);

                if unsafe { LT_ALL_MOVES[idx] != 0 } {
                    panic!("Hash collision while populating table (this should never happen)");
                }
                unsafe {
                    LT_ALL_MOVES[idx] = valid_moves;
                }
            }

            total_table_size += cur_table_size;
        }
    }

    println!(" > Total table size: {}", unsafe { LT_ALL_MOVES.len() });
}
