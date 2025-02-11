use std::thread;
use std::sync::Arc;
use std::sync::Mutex;
use crate::board::*;
use crate::move_gen;
use crate::search;
use crate::eval::*;
use crate::move_gen::MoveBuffer;
use crate::transpos;
use crate::thread_flag::ThreadFlag;
use crate::uci;
use crate::time_manager;

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

    pub fn start_search(&mut self, max_depth: u8, max_time: Option<f64>, remaining_time: Option<f64>) {
        self.stop_search();

        let start_time = std::time::Instant::now();
        let mut stop_time: Option<std::time::Instant>;
        if max_time.is_some() {
            stop_time = Some(start_time + std::time::Duration::from_secs_f64(max_time.unwrap()));
        } else {
            stop_time = None;
        }

        if remaining_time.is_some() {
            let allotted_time = time_manager::get_time_to_use(&self.board, remaining_time.unwrap());
            let desired_stop_time = start_time + std::time::Duration::from_secs_f64(allotted_time);

            if stop_time.is_some() {
                // We already have a stop time, take the minimum of the two
                stop_time = Some(stop_time.unwrap().min(desired_stop_time));
            } else {
                stop_time = Some(desired_stop_time);
            }
        }

        let board = self.board.clone();
        let stop_flag = self.stop_flag.clone();
        let arc_table = self.arc_table.clone();

        self.thread_join_handle = Some(
            thread::spawn(move || {
                let mut latest_best_move_idx = None;
                let mut guessed_next_eval: Option<Value> = None;

                for depth_minus_one in 0..max_depth {
                    let depth = depth_minus_one + 1;
                    let mut table = arc_table.lock().unwrap();
                    let (search_result, search_info) = search::search(
                        &board, &mut table, depth,
                        guessed_next_eval,
                        &stop_flag, stop_time
                    );

                    guessed_next_eval = Some(search_result.eval);

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