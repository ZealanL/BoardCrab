pub mod board;
pub mod fen;
pub mod search;
pub mod move_gen;

mod bitmask;
mod lookup_gen;

static INIT_ONCE: std::sync::Once = std::sync::Once::new();

fn _init() {
    lookup_gen::init();
}

pub fn init() {
    INIT_ONCE.call_once(_init);
}