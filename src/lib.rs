pub mod async_engine;
pub mod bitmask;
pub mod board;
pub mod eval;
mod eval_lookup;
pub mod fen;
pub mod lookup_gen;
pub mod lookup_gen_magic;
pub mod move_gen;
pub mod search;
pub mod thread_flag;
pub mod time_manager;
pub mod transpos;
pub mod uci;
pub mod zobrist;

static INIT_ONCE: std::sync::Once = std::sync::Once::new();

fn _init() {
    lookup_gen::init();
    #[cfg(not(debug_assertions))]
    lookup_gen_magic::init();
    zobrist::init();
}

pub fn init() {
    INIT_ONCE.call_once(_init);
}
