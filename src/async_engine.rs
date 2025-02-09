use std::thread;
use std::sync::Arc;
use std::sync::Mutex;
use crate::board::*;
use crate::move_gen;
use crate::search;
use crate::eval::*;
use crate::move_gen::MoveBuffer;
use crate::search::SearchInfo;
use crate::transpos;
use crate::thread_flag::ThreadFlag;
use crate::uci;

pub struct AsyncEngine {
    board: Board,
    arc_table: Arc<Mutex<transpos::Table>>,
    stop_flag: ThreadFlag,
    thread_join_handle: Option<thread::JoinHandle<Option<usize>>>, // Outputs best move idx
}

impl AsyncEngine {
    pub fn new(table_size_mbs: usize) -> AsyncEngine {
        AsyncEngine {
            board: Board::start_pos(),
            arc_table: Arc::new(Mutex::new(transpos::Table::new(table_size_mbs))),
            stop_flag: ThreadFlag::new(),
            thread_join_handle: None
        }
    }

    pub fn start_search(&mut self, max_depth: u8, max_time_ms: Option<u64>) {
        self.stop_search();

        let start_time = std::time::Instant::now();
        let stop_time: Option<std::time::Instant>;
        if max_time_ms.is_some() {
            stop_time = Some(std::time::Instant::now() + std::time::Duration::from_millis(max_time_ms.unwrap()));
        } else {
            stop_time = None;
        }

        let board = self.board.clone();
        let stop_flag = self.stop_flag.clone();
        let arc_table = self.arc_table.clone();

        self.thread_join_handle = Some(
            thread::spawn(move || {
                let mut search_info = SearchInfo::new();
                let mut latest_best_move_idx = None;
                for depth_minus_one in 0..max_depth {
                    let depth = depth_minus_one + 1;
                    let mut table = arc_table.lock().unwrap();
                    let search_result = search::search(
                        &board, &mut table, &mut search_info,
                        -VALUE_CHECKMATE, VALUE_CHECKMATE,
                        depth, 0, &stop_flag, stop_time
                    );

                    if search_result.best_move_idx.is_some() {
                        // TODO: Somewhat lame to be calling UCI stuff from async_engine
                        let elapsed_time = std::time::Instant::now() - start_time;
                        uci::print_search_results(&board, &table, depth, search_result.eval, &search_info, elapsed_time.as_secs_f64());

                        latest_best_move_idx = search_result.best_move_idx;
                    }

                    if latest_best_move_idx.is_some() {
                        // If we've found a best move and are out of time, break
                        if stop_time.is_some() && std::time::Instant::now() >= stop_time.unwrap() {
                            break;
                        }
                    }
                }

                if latest_best_move_idx.is_some() {
                    let mut moves = MoveBuffer::new();
                    move_gen::generate_moves(&board, &mut moves);
                    // TODO: Lame UCI code here, should be in uci::*
                    //  Ideally we should have a callback or something? Not sure
                    println!("bestmove {}", moves[latest_best_move_idx.unwrap()]);
                } else {
                    panic!("No best move found in time")
                }
                latest_best_move_idx
            })
        );
    }

    // Returns the best move index
    pub fn stop_search(&mut self) -> Option<usize> {
        self.stop_flag.trigger();
        let result: Option<usize>;
        if let Some(handle) = self.thread_join_handle.take() {
            result = handle.join().unwrap()
        } else {
            result = None
        }
        self.stop_flag.reset();
        result
    }

    pub fn get_board(&self) -> &Board {
        &self.board
    }

    pub fn set_board(&mut self, new_board: &Board) {
        self.board = new_board.clone();
    }
}