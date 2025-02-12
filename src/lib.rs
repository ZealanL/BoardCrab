pub mod board;
pub mod fen;
pub mod search;
pub mod move_gen;
pub mod eval;
pub mod transpos;
pub mod uci;
pub mod async_engine;
pub mod thread_flag;
pub mod zobrist;
pub mod bitmask;
pub mod lookup_gen;
pub mod lookup_gen_magic;
pub mod time_manager;
pub mod raw_ptr;

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