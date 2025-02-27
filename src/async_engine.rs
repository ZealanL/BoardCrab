use std::thread;
use std::sync::Arc;
use std::time::{Instant, Duration};
use crate::board::*;
use crate::move_gen;
use crate::search;
use crate::eval::*;
use crate::transpos;
use crate::thread_flag::ThreadFlag;
use crate::uci;
use crate::time_manager;

pub struct AsyncSearchConfig<'a> {
    pub max_depth: Option<u8>,
    pub stop_flag: Option<&'a ThreadFlag>,
    pub start_time: Instant,
    pub time_state: Option<time_manager::TimeState>,

    pub print_uci: bool
}

pub fn do_search_thread(board: &Board, table: &mut transpos::Table, search_cfg: &AsyncSearchConfig) -> Option<u8> {

    // Only exists if we have a soft time limit
    let mut max_time_to_use: Option<f64> = None;

    // The time at which we should stop searching, either due to a soft or hard limit
    let mut stop_time: Option<Instant> = None;

    if search_cfg.time_state.is_some() {
        // Possibly determine the maximum time to use (this will be our soft time limit)
        let time_state = search_cfg.time_state.clone().unwrap();
        max_time_to_use = time_manager::get_max_time_to_use(board, &time_state);

        let mut soft_stop_time: Option<Instant> = None;
        if max_time_to_use.is_some() {
            soft_stop_time = Some(search_cfg.start_time + Duration::from_secs_f64(max_time_to_use.unwrap()));
        }

        let mut hard_stop_time: Option<Instant> = None;
        let hard_max_time = time_state.hard_max_time.clone();
        if hard_max_time.is_some() {
            hard_stop_time = Some(search_cfg.start_time + Duration::from_secs_f64(hard_max_time.unwrap()));
        }

        if soft_stop_time.is_some() && hard_stop_time.is_some() {
            // We have both a soft and a hard time limit
            // Take the minimum of both as our stop time
            stop_time = Some(Instant::min(soft_stop_time.unwrap(), hard_stop_time.unwrap()));
        } else if soft_stop_time.is_some() {
            stop_time = soft_stop_time;
        } else if hard_stop_time.is_some() {
            stop_time = hard_stop_time;
        }
    }

    let mut best_moves = Vec::new();
    let mut guessed_next_eval: Option<Value> = None;
    let max_depth = if search_cfg.max_depth.is_some() {
        search_cfg.max_depth.unwrap()
    } else {
        u8::MAX
    };
    for depth_minus_one in 0..max_depth {
        let depth = depth_minus_one + 1;

        {
            let (search_eval, search_info) = search::search(
                &board, table, depth,
                guessed_next_eval,
                search_cfg.stop_flag, stop_time
            );

            if search_eval.is_infinite() {
                // Search aborted
                break;
            }

            if search_info.root_best_move.is_some() {
                best_moves.push(search_info.root_best_move.unwrap());
            }

            guessed_next_eval = Some(search_eval);

            let cur_time = Instant::now();
            let elapsed_time_f64 = (cur_time - search_cfg.start_time).as_secs_f64();
            if search_cfg.print_uci {
                // TODO: Somewhat lame to be calling UCI stuff from async_engine

                uci::print_search_results(&board, table, depth, search_eval, &search_info, elapsed_time_f64);
            }

            if max_time_to_use.is_some() {
                if time_manager::should_exit_early(max_time_to_use.unwrap(), elapsed_time_f64, &best_moves) {
                    break;
                }
            }
        }
    }

    if best_moves.len() > 0 {
        Some(*best_moves.last().unwrap())
    } else {
        println!("No best moves from depth {}", max_depth);
        None
    }
}

pub struct AsyncEngine {
    board: Board,
    arc_table: Arc<transpos::Table>,
    stop_flag: ThreadFlag,
    thread_join_handles: Vec<thread::JoinHandle<Option<u8>>> // Outputs best move idx
}

impl AsyncEngine {
    pub fn new(table_size_mbs: usize) -> AsyncEngine {
        AsyncEngine {
            board: Board::start_pos(),
            arc_table: Arc::new(transpos::Table::new(table_size_mbs)),
            stop_flag: ThreadFlag::new(),
            thread_join_handles: Vec::new()
        }
    }

    pub fn start_search(&mut self, max_depth: Option<u8>, time_state: Option<time_manager::TimeState>, num_threads: usize) {

        self.stop_search();

        let start_time = Instant::now();

        for thread_idx in 0..num_threads {
            let board = self.board.clone();
            let stop_flag = self.stop_flag.clone();
            let table_ref = Arc::clone(&self.arc_table);
            self.thread_join_handles.push(
                thread::spawn(move || {

                    // Unsafe deference the table
                    let table_ptr = Arc::as_ptr(&table_ref);
                    let table = unsafe { &mut *(table_ptr as *mut transpos::Table) };
                    let is_leader_thread = thread_idx == 0;

                    let search_config = AsyncSearchConfig {
                        max_depth,
                        stop_flag: Some(&stop_flag),
                        start_time,
                        time_state,

                        print_uci: is_leader_thread
                    };

                    let mut best_move = do_search_thread(&board, table, &search_config);

                    if is_leader_thread {
                        if best_move.is_some() {
                            let mut moves = move_gen::MoveBuffer::new();
                            move_gen::generate_moves(&board, &mut moves);
                            uci::print_best_move(moves[best_move.unwrap() as usize]);
                        } else {
                            panic!("No best move found in time")
                        }
                    }

                    best_move
                })
            );
        }
    }

    // Returns the best move index
    pub fn stop_search(&mut self) -> Option<u8> {
        self.stop_flag.trigger();
        let mut best_move_idx: Option<u8> = None;
        for handle in self.thread_join_handles.drain(..) {
            let handle_result = handle.join();
            if handle_result.is_ok() {
                best_move_idx = handle_result.unwrap();
            } else {
                panic!("Search thread crashed");
            }

        }
        self.stop_flag.reset();
        best_move_idx
    }

    pub fn get_board(&self) -> &Board {
        &self.board
    }

    pub fn set_board(&mut self, new_board: &Board) {
        self.board = new_board.clone();
    }

    // NOTE: Doesn't reset the table if the size matches
    pub fn maybe_update_table_size(&mut self, new_size_mbs: usize) {
        self.stop_search();
        if self.arc_table.get_size_mbs() != new_size_mbs {
            self.arc_table = Arc::new(transpos::Table::new(new_size_mbs));
        }
    }

    pub fn reset_table(&mut self) {
        self.stop_search();
        self.arc_table = Arc::new(transpos::Table::new(self.arc_table.get_size_mbs()));
    }
}