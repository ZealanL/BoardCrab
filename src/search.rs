use std::collections::HashSet;
use crate::bitmask::*;
use crate::board::*;
use crate::eval::*;
use crate::move_gen;
use crate::zobrist::Hash;
use crate::transpos;
use crate::thread_flag::ThreadFlag;

fn _perft(board: &Board, depth: u8, depth_elapsed: usize, print: bool) -> usize {
    let mut moves = move_gen::MoveBuffer::new();
    move_gen::generate_moves(&board, &mut moves);
    if depth > 1 {
        let mut total: usize = 0;
        for mv in moves.iter() {
            let mut next_board: Board = *board;
            next_board.do_move(&mv);
            let sub_total = _perft(&next_board, depth - 1, depth_elapsed + 1, print);
            if depth_elapsed == 0 && print {
                println!("{}: {}", mv, sub_total);
            }
            total += sub_total;
        }
        if depth_elapsed == 0 && print {
            println!("\nNodes Searched: {}", total);
        }
        total
    } else if depth == 1 {
        if depth_elapsed == 0 && print {
            for mv in moves.iter() {
                println!("{}: {}", mv, 1);
            }
            println!("\nNodes Searched: {}", moves.len());
        }
        moves.len()
    } else {
        if depth_elapsed == 0 && print {
            println!("\nNodes Searched: 1");
        }
        return 1;
    }
}

pub fn perft(board: &Board, depth: u8, print: bool) -> usize { _perft(board, depth, 0, print) }

//////////////////////////////////////////////////////////////////////////

fn get_no_moves_eval(board: &Board) -> Value {
    if board.checkers != 0 { -VALUE_CHECKMATE } else { 0.0 }
}

pub struct SearchInfo {
    pub total_nodes: usize,
    pub depth_hashes: [Hash; 256], // For repetition detection

    // See https://www.chessprogramming.org/History_Heuristic
    pub history_values: [[[Value; 64]; NUM_PIECES]; 2],
    pub root_best_move: Option<u8>
}

impl SearchInfo {
    pub fn new() -> SearchInfo {
        SearchInfo {
            total_nodes: 0,
            depth_hashes: [0; 256],
            history_values: [[[0.0; 64]; NUM_PIECES]; 2],
            root_best_move: None
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SearchConfig {
    pub null_move_pruning: bool,
    pub late_move_reduction_factor: f32
}

impl SearchConfig {
    pub fn new() -> SearchConfig {
        SearchConfig {
            null_move_pruning: true,
            late_move_reduction_factor: 1.0
        }
    }
}

fn _search(
    board: &Board, table: &mut transpos::Table, config: &SearchConfig, search_info: &mut SearchInfo,
    mut lower_bound: Value, upper_bound: Value,
    depth_remaining: u8, depth_elapsed: i64,
    stop_flag: Option<&ThreadFlag>, stop_time: Option<std::time::Instant>) -> Value {

    search_info.total_nodes += 1;

    let in_extension = depth_remaining == 0;

    // Check draw by repetition
    for i in (4..12).step_by(2) {
        if (depth_elapsed >= i) && search_info.depth_hashes[(depth_elapsed - i) as usize] == board.hash {
            // Loop detected
            return 0.0;
        } else {
            break;
        }
    }
    search_info.depth_hashes[depth_elapsed as usize] = board.hash;

    if depth_remaining >= 3 { // No point in checking at a super low depth
        let mut stop = false;

        if stop_flag.is_some() && stop_flag.unwrap().get() {
            stop = true;
        } else if stop_time.is_some() {
            if std::time::Instant::now() >= stop_time.unwrap() {
                stop = true
            }
        }

        if stop {
            return VALUE_INF;
        }
    }

    let mut best_eval = -VALUE_INF;
    let cur_eval = eval_board(board);
    if in_extension {
        // Standing pat eval in extension search
        best_eval = cur_eval;

        if best_eval >= upper_bound {
            return best_eval;
        } else if best_eval > lower_bound {
            lower_bound = best_eval;
        }
    }

    let table_entry;
    if depth_elapsed > 0 {
        table_entry = table.get_fast(board.hash);
    } else {
        // We don't use the tranposition table at depth 0
        // Otherwise we may not populate root_best_move
        table_entry = transpos::Entry::new();
    }

    // Table lookup
    let mut table_best_move: Option<u8> = None;
    if table_entry.is_valid() {
        // NOTE: In extensions, this depth check won't work because depth_remaining is stuck at 0
        if table_entry.depth_remaining >= depth_remaining && !in_extension {
            match table_entry.entry_type {
                transpos::EntryType::FailLow => {
                    // Exceeds our lower bound, do a cutoff
                    if table_entry.eval <= lower_bound {
                        return table_entry.eval;
                    }
                },
                transpos::EntryType::FailHigh => {
                    if table_entry.eval >= upper_bound {
                        // Exceeds our upper bound, do a cutoff
                        return table_entry.eval;
                    }
                },
                transpos::EntryType::Exact => {
                    // Exact node, no further searching is needed
                    return table_entry.eval;
                },
                _ => {
                    panic!("Invalid or unsupported entry type: {}", table_entry.entry_type as usize);
                }
            }
        } else {
            // From a lower depth, so not super useful
        }

        // If we didn't hit a quick return, we can still use the best move from this entry
        table_best_move = Some(table_entry.best_move_idx);
    }

    // Null move pruning
    if config.null_move_pruning &&
        cur_eval >= upper_bound &&
        board.checkers == 0 &&
        depth_remaining >= 1 &&
        depth_elapsed >= 2
    {
        let king_and_pawn = board.pieces[board.turn_idx][PIECE_PAWN] | board.pieces[board.turn_idx][PIECE_KING];
        let is_king_and_pawn = board.occupancy[board.turn_idx] == king_and_pawn;
        if !is_king_and_pawn {

            let mut next_board = board.clone();
            next_board.do_null_move();

            let next_depth = depth_remaining / 2;
            let next_result = _search(
                &next_board, table, config, search_info,
                -upper_bound, -upper_bound + 0.01,
                next_depth, depth_elapsed + 1,
                stop_flag, stop_time
            );

            let next_eval = decay_eval(-next_result);
            if next_eval >= upper_bound {
                return next_eval;
            }
        }
    }

    let mut moves = move_gen::MoveBuffer::new();
    move_gen::generate_moves(&board, &mut moves);
    if moves.is_empty() {
        return get_no_moves_eval(board);
    }

    #[derive(Copy, Clone)]
    struct RatedMove {
        idx: usize,
        eval: Value
    }

    let table_best_move_idx;
    if table_best_move.is_some() {
        if (table_best_move.unwrap() as usize) < moves.len() {
            table_best_move_idx = table_best_move.unwrap() as usize;
        } else {
            // Hash collision
            // Very rare so we'll just ignore it
            debug_assert!(false);
            table_best_move_idx = usize::MAX;
        }
    } else {
        table_best_move_idx = usize::MAX;
    }

    let mut rated_moves: Vec<RatedMove> = Vec::with_capacity(moves.len());
    for i in 0..moves.len() {
        let mv = moves[i];
        let is_quiet = mv.is_quiet();

        if in_extension && is_quiet {
            continue // Only loud moves allowed in extensions
        }

        let mut move_eval = eval_move(board, &mv);

        if is_quiet {
            let history_value = search_info.history_values[board.turn_idx][mv.from_piece_idx][bm_to_idx(mv.to)];
            move_eval += history_value * 0.02;
        }

        if i == table_best_move_idx {
            move_eval = Value::MAX;
        }

        rated_moves.push(
            RatedMove {
                idx: i,
                eval: move_eval
            }
        )
    }

    // Insertion sort
    for i in 1..rated_moves.len() {
        let mut j = i;
        while j > 0 {
            let prev = rated_moves[j - 1];
            let cur = rated_moves[j];

            if cur.eval > prev.eval {
                // Swap
                rated_moves[j - 1] = cur;
                rated_moves[j] = prev;
            } else {
                break
            }

            j -= 1;
        }
    }

    let mut best_move_idx: usize = 0;
    for i in 0..rated_moves.len() {
        let move_idx = rated_moves[i].idx;
        let mv = &moves[move_idx];

        let mut next_board: Board = board.clone();
        next_board.do_move(mv);

        let gives_check = next_board.checkers != 0;
        let mut depth_reduction_f: f32 = 1.0;

        if gives_check {
            depth_reduction_f = 0.0; // Extend after a check
        } else {
            // Late move reductions
            if i >= 1 && depth_elapsed >= 2 {
                let reduction_amount =
                    ((i as f32) * 0.1 + (depth_remaining as f32) * 0.2) * config.late_move_reduction_factor;
                depth_reduction_f += reduction_amount;
            }
        }

        let mut depth_reduction = depth_reduction_f.clamp(0.0, depth_remaining as f32).round() as u8;

        let mut next_eval: Value;
        loop {
            next_eval = _search(
                &next_board, table, config, search_info,
                -upper_bound, -lower_bound,
                depth_remaining - depth_reduction, depth_elapsed + 1,
                stop_flag, stop_time
            );

            if next_eval.is_infinite() {
                return VALUE_INF;
            }

            next_eval = decay_eval(-next_eval);

            if depth_reduction > 1 && next_eval > lower_bound {
                // Exceeded lower bound, we need to do a full search
                depth_reduction = 1;
                continue
            }

            break;
        }

        if next_eval > best_eval {
            best_eval = next_eval;
            best_move_idx = move_idx;
            if next_eval > lower_bound {
                lower_bound = next_eval;
            }

            if next_eval >= upper_bound {
                // Failed high, beta cut-off
                if mv.is_quiet() {
                    // Higher depth means better search and thus better quality info on how good this move is
                    let history_weight = 1.0 / (depth_elapsed as Value);
                    let history = &mut search_info.history_values[board.turn_idx][mv.from_piece_idx][bm_to_idx(mv.to)];
                    *history += history_weight;

                    // Penalize all the moves we already searched
                    for j in 0..i {
                        let omv = &moves[rated_moves[j].idx];
                        if omv.is_quiet() {
                            let other_history = &mut search_info.history_values[board.turn_idx][omv.from_piece_idx][bm_to_idx(omv.to)];
                            *other_history -= history_weight / (i as Value);
                        }
                    }
                }
                break
            }
        }
    }


    table.set(
        board.hash, best_eval, best_move_idx as u8, depth_remaining,
        {
            if best_eval >= upper_bound {
                transpos::EntryType::FailHigh
            } else if best_eval < lower_bound {
                transpos::EntryType::FailLow
            } else {
                transpos::EntryType::Exact
            }
        }
    );

    if depth_elapsed == 0 {
        search_info.root_best_move = Some(best_move_idx as u8);
    }

    best_eval
}

pub fn search(
    board: &Board, table: &mut transpos::Table, config: &SearchConfig, depth: u8,
    guessed_eval: Option<Value>,
    stop_flag: Option<&ThreadFlag>, stop_time: Option<std::time::Instant>) -> (Value, SearchInfo) {

    let mut search_info = SearchInfo::new();

    if depth >= 4 {
        // Use an aspiration window
        const WINDOW_RANGE_GUESS: Value = 0.3; // Range of the window if there is a guessed eval
        const WINDOW_RANGE_NO_GUESS: Value = 1.0; // Range of the window if there isn't guessed eval
        let window_start_center = if guessed_eval.is_some() { guessed_eval.unwrap() } else { eval_board(board) };

        let mut window_min = window_start_center;
        let mut window_max = window_start_center;

        if guessed_eval.is_some() {
            window_min -= WINDOW_RANGE_GUESS / 2.0;
            window_max += WINDOW_RANGE_GUESS / 2.0;
        } else {
            window_min -= WINDOW_RANGE_NO_GUESS / 2.0;
            window_max += WINDOW_RANGE_NO_GUESS / 2.0;
        }

        let eval = _search(
            board, table, config, &mut search_info, window_min, window_max, depth, 0, stop_flag, stop_time
        );

        if eval >= window_min && eval < window_max {
            // Window was sufficient
            return (eval, search_info);
        }
    }

    let search_result = _search(
        board, table, config, &mut search_info, -VALUE_CHECKMATE, VALUE_CHECKMATE, depth, 0, stop_flag, stop_time
    );

    (search_result, search_info)
}

pub fn determine_pv(mut board: Board, table: &transpos::Table) -> Vec<Move> {
    let mut result = Vec::new();
    let mut found_hashes = HashSet::<Hash>::new();

    loop {

        let entry = table.get_fast(board.hash);

        let valid;
        if result.is_empty() {
            // No PV yet, allow a locked entry
            valid = entry.is_set();
        } else {
            // Require full validity
            valid = entry.is_valid();
        }

        if valid {
            if found_hashes.contains(&board.hash) {
                // Looped position
                break;
            } else {
                found_hashes.insert(board.hash);
            }

            let mut moves = move_gen::MoveBuffer::new();
            move_gen::generate_moves(&board, &mut moves);

            let best_move_idx = entry.best_move_idx as usize;
            if best_move_idx >= moves.len() {
                panic!("Failed to generate PV, bad move count (hash collision?)");
            }

            let best_move = moves[best_move_idx];
            result.push(best_move);
            board.do_move(&best_move);
        } else {
            break;
        }
    }

    if result.is_empty() {
        panic!("Failed to generate PV, first table entry never found");
    }

    result
}