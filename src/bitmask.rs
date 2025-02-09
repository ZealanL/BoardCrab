pub type BitMask = u64;

pub const fn bm_to_idx(mask: BitMask) -> usize {
    debug_assert!(mask.count_ones() == 1);
    mask.trailing_zeros() as usize
}

pub const fn bm_from_idx(idx: usize) -> BitMask {
    debug_assert!(idx < 64);
    1u64 << idx
}

pub const fn bm_from_xy(x: i64, y: i64) -> BitMask {
    debug_assert!(x < 8 && y < 8);
    1 << (x + y * 8)
}

pub const fn bm_to_xy(mask: BitMask) -> (i64, i64) {
    let idx = bm_to_idx(mask);
    ((idx % 8) as i64, (idx / 8) as i64)
}

pub const fn bm_from_coord(coord_name: &str) -> BitMask {
    let coord_name_bytes = coord_name.as_bytes();
    let first_char = coord_name_bytes[0];
    let second_char = coord_name_bytes[1];

    let x: u8;
    if first_char <= ('Z' as u8) {
        x = first_char - ('A' as u8);
    } else {
        x = first_char - ('a' as u8);
    }
    let y = second_char - ('1' as u8);

    bm_from_xy(x as i64, y as i64)
}

pub fn bm_to_coord(mask: BitMask) -> String {
    let (x, y) = bm_to_xy(mask);
    [(x + ('a' as i64)) as u8 as char, (y + ('1' as i64)) as u8 as char].iter().collect()
}

pub const fn bm_get(mask: BitMask, x: i64, y: i64) -> bool {
    debug_assert!(x < 8 && y < 8);

    let pos_mask: BitMask = 1 << (x + y * 8);
    (mask & pos_mask) != 0
}

pub fn bm_set(mask: &mut BitMask, x: i64, y: i64, val: bool) {
    debug_assert!(x < 8 && y < 8);

    let pos_mask: BitMask = 1 << (x + y * 8);
    if val {
        *mask |= pos_mask;
    } else {
        *mask &= !pos_mask;
    }
}

pub const fn bm_shift(mask: BitMask, x: i64, y: i64) -> BitMask {
    let shift_amount = x + y * 8;
    if shift_amount >= 0 {
        mask << shift_amount
    } else {
        mask >> -shift_amount
    }
}

pub const fn bm_make_row(y: i64) -> BitMask {
    bm_shift(0xFF, 0, y)
}

pub const fn bm_make_column(x: i64) -> BitMask {
    bm_shift(0x101010101010101, x, 0)
}

pub const fn bm_flip_vertical(mask: BitMask) -> BitMask {
    mask.swap_bytes()
}

//////////////////////////

// Iterate over the bits in a mask
pub struct ItrBits {
    remaining_mask: BitMask
}

pub fn bm_iter_bits(mask: BitMask) -> ItrBits {
    ItrBits{ remaining_mask: mask }
}

impl Iterator for ItrBits {
    type Item = BitMask;

    fn next(&mut self) -> Option<Self::Item> {
        let cur_mask_signed = self.remaining_mask as i64;
        let next_bit = (cur_mask_signed & -cur_mask_signed) as BitMask;
        self.remaining_mask &= !next_bit;

        if next_bit != 0 {
            Some(next_bit)
        } else {
            None
        }
    }
}