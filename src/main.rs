use board_crab_lib::uci;
use board_crab_lib::async_engine::AsyncEngine;

fn main() {
    board_crab_lib::init();

    // Set panic to print in release mode
    #[cfg(not(debug_assertions))]
    std::panic::set_hook(Box::new(|panic_info| {
        if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            eprintln!("Fatal error: {:?}", s);
        } else {
            eprintln!("Fatal error (no further info)");
        }
    }));

    let mut engine = AsyncEngine::new(100);

    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        uci::process_cmd(input, &mut engine);
    }
}
