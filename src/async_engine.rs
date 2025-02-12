use std::thread;
use std::sync::Arc;
use std::sync::RwLock;
use crate::board::*;
use crate::move_gen;
use crate::search;
use crate::eval::*;
use crate::raw_ptr::RawPtr;
use crate::transpos;
use crate::thread_flag::ThreadFlag;
use crate::uci;
use crate::time_manager;

pub struct AsyncEngine {
    board: Board,
    table: transpos::Table,
    stop_flag: ThreadFlag,
    thread_join_handles: Vec<thread::JoinHandle<Option<u8>>>, // Outputs best move idx
}

impl AsyncEngine {
    pub fn new(table_size_mbs: usize) -> AsyncEngine {
        AsyncEngine {
            board: Board::start_pos(),
            table: transpos::Table::new(table_size_mbs),
            stop_flag: ThreadFlag::new(),
            thread_join_handles: Vec::new()
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

        let num_threads = 8; // TODO: Make configurable

        for thread_idx in 0..num_threads {
            let board = self.board.clone();
            let stop_flag = self.stop_flag.clone();
            let table_ptr = RawPtr::<transpos::Table>::new(&mut self.table);

            self.thread_join_handles.push(
                thread::spawn(move || {

                    let is_leader_thread = thread_idx == 0 || true;

                    let mut latest_best_move_idx: Option<u8> = None;
                    let mut guessed_next_eval: Option<Value> = None;
                    for depth_minus_one in 0..max_depth {
                        let depth = depth_minus_one + 1;

                        {
                            let (search_eval, search_info) = search::search(
                                &board, &table_ptr, depth,
                                guessed_next_eval,
                                Some(&stop_flag), stop_time
                            );

                            guessed_next_eval = Some(search_eval);

                            let root_entry = table_ptr.get().get(board.hash);

                            if root_entry.is_valid() && is_leader_thread && !search_eval.is_infinite() {
                                // TODO: Somewhat lame to be calling UCI stuff from async_engine
                                let elapsed_time = std::time::Instant::now() - start_time;
                                uci::print_search_results(&board, table_ptr.get(), depth, search_eval, &search_info, elapsed_time.as_secs_f64());
                                latest_best_move_idx = Some(root_entry.best_move_idx);
                            }
                        }
                    }

                    if is_leader_thread {
                        if latest_best_move_idx.is_some() {
                            let mut moves = move_gen::MoveBuffer::new();
                            move_gen::generate_moves(&board, &mut moves);
                            // TODO: Lame UCI code here, should be in uci::*
                            //  Ideally we should have a callback or something? Not sure
                            println!("bestmove {}", moves[latest_best_move_idx.unwrap() as usize]);
                        } else {
                            panic!("No best move found in time")
                        }
                    }
                    latest_best_move_idx
                })
            );
        }
    }

    // Returns the best move index
    pub fn stop_search(&mut self) -> Option<u8> {
        self.stop_flag.trigger();
        let mut result: Option<u8> = None;
        for handle in self.thread_join_handles.drain(..) {
            result = handle.join().unwrap();
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