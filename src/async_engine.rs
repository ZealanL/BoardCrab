use std::thread;
use std::sync::Arc;
use crate::board::*;
use crate::move_gen;
use crate::search;
use crate::eval::*;
use crate::transpos;
use crate::thread_flag::ThreadFlag;
use crate::uci;
use crate::time_manager;

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

    pub fn start_search(&mut self, max_depth: u8, time_state: Option<time_manager::TimeState>, num_threads: usize) {

        self.stop_search();

        let start_time = std::time::Instant::now();

        let mut max_time_to_use: Option<f64> = None;
        let mut stop_time: Option<std::time::Instant> = None;
        if time_state.is_some() {
            max_time_to_use = time_manager::get_max_time_to_use(self.get_board(), time_state.unwrap());
            if max_time_to_use.is_some() {
                stop_time = Some(start_time + std::time::Duration::from_secs_f64(max_time_to_use.unwrap()));
            }
        }

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

                    let mut node_counts = Vec::new();
                    let mut best_moves = Vec::new();
                    let mut guessed_next_eval: Option<Value> = None;
                    for depth_minus_one in 0..max_depth {
                        let depth = depth_minus_one + 1;

                        {
                            let (search_eval, search_info) = search::search(
                                &board, table, depth,
                                guessed_next_eval,
                                Some(&stop_flag), stop_time
                            );

                            if search_eval.is_infinite() {
                                // Search aborted
                                break;
                            }

                            node_counts.push(search_info.total_nodes);

                            guessed_next_eval = Some(search_eval);
                            {
                                best_moves.push(search_info.root_best_move_idx);

                                if is_leader_thread && !search_eval.is_infinite() {
                                    // TODO: Somewhat lame to be calling UCI stuff from async_engine
                                    let elapsed_time = std::time::Instant::now() - start_time;
                                    uci::print_search_results(&board, table, depth, search_eval, &search_info, elapsed_time.as_secs_f64());
                                }

                                if stop_time.is_some() {
                                    let remaining_time = stop_time.unwrap() - std::time::Instant::now();
                                    if time_manager::should_exit_early(max_time_to_use.unwrap(), remaining_time.as_secs_f64(), &best_moves) {
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    if is_leader_thread {
                        if best_moves.len() > 0 {
                            let mut moves = move_gen::MoveBuffer::new();
                            move_gen::generate_moves(&board, &mut moves);
                            uci::print_best_move(moves[*best_moves.last().unwrap() as usize]);
                        } else {
                            panic!("No best move found in time")
                        }
                    }

                    if best_moves.len() > 0 {
                        Some(*best_moves.last().unwrap())
                    } else {
                        None
                    }
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