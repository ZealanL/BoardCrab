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
        let cmd_parts: Vec<String> = input.trim().split_whitespace().map(|v| v.to_string()).collect();
        uci::process_cmd(cmd_parts, &mut engine);
    }
}
