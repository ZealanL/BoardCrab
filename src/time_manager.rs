use crate::board::*;

pub struct TimeState {
    pub max_time: Option<f64>, // Hard maximum time
    pub remaining_time: Option<f64>, // Remaining time on our clock
    pub time_inc: Option<f64>, // Time given per ply
    pub moves_till_time_control: Option<u64> // Plies remaining until the next time control
}

impl TimeState {
    pub fn new() -> TimeState {
        TimeState {
            max_time: None,
            remaining_time: None,
            time_inc: None,
            moves_till_time_control: None
        }
    }
}

// Determines how much time to use on the next search, given the board
// If no time limit is needed, returns None
pub fn get_max_time_to_use(board: &Board, time_state: TimeState) -> Option<f64> {

    if time_state.remaining_time.is_none() {
        return if time_state.max_time.is_some() {
            // Just return the max time
            time_state.max_time
        } else {
            // No time limits
            None
        }
    }

    let num_pieces = board.combined_occupancy().count_ones();

    // Very rough estimate of how many plies remain
    let remaining_pieces_ratio = (num_pieces as f64) / 32.0;
    let mut remaining_moves = remaining_pieces_ratio * 40.0 + 6.0;

    if time_state.moves_till_time_control.is_some() {
        remaining_moves = f64::min(remaining_moves, time_state.moves_till_time_control.unwrap() as f64);
    }

    let mut real_remaining_time = time_state.remaining_time.unwrap();
    if time_state.time_inc.is_some() {
        real_remaining_time += time_state.time_inc.unwrap() * remaining_moves;
    }

    let base_time_to_use = real_remaining_time / f64::max(remaining_moves, 1.0);

    // We'll say that the maximum is 1.3x the base time to use
    let mut max_time_to_use = f64::min(base_time_to_use * 1.3, real_remaining_time);
    if time_state.max_time.is_some() {
        // Clamp to maximum time
        max_time_to_use = f64::min(max_time_to_use, time_state.max_time.unwrap());
    }

    // Always leave a little buffer so we don't run out of time
    const TIME_BUFFER: f64 = 0.05;
    max_time_to_use = f64::max(0.0, max_time_to_use - TIME_BUFFER);

    Some(max_time_to_use)
}

// Determines whether we should stop searching early
pub fn should_exit_early(time_given_to_use: f64, time_remaining: f64, best_moves: &Vec<u8>) -> bool {
    let last_depth = best_moves.len();

    if last_depth < 5 {
        // Too early to know, and it doesn't matter much anyway
        return false;
    }

    if time_remaining > time_given_to_use * 0.9 {
        // We still have 90% of our time, don't stop
        return false;
    }

    // We can say that the confidence we are right is the portion of the best moves that match the latest best move
    let latest_best_move = best_moves[last_depth - 1];
    let mut matching_best_moves = 0;
    for &best_move in best_moves {
        if best_move == latest_best_move {
            matching_best_moves += 1;
        }
    }

    let confidence = (matching_best_moves as f64) / (last_depth as f64);
    let time_remaining_frac = time_remaining / time_given_to_use;

    // Ramp down confidence, so that lower values are even less confident
    let scaled_confidence = confidence.powf(1.5);

    if scaled_confidence >= time_remaining_frac {
        // We're confident enough
        true
    } else {
        false
    }
}