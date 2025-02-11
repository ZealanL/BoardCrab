use crate::board::*;

// Determines how much time to use on the next search, given the board
pub fn get_time_to_use(board: &Board, remaining_time: f64) -> f64 {
    let num_pieces = board.combined_occupancy().count_ones();

    // Very rough estimate of how many plies remain
    const MINIMUM_REMAINING_PLIES: f64 = 12.0;
    let remaining_pieces_ratio = (num_pieces as f64) / 32.0;

    let remaining_plies = remaining_pieces_ratio * 2.25 + MINIMUM_REMAINING_PLIES;

    let mut time_to_use = remaining_time / remaining_plies;

    // Add additional weight towards the start of the game
    time_to_use *= 1.0 + remaining_pieces_ratio * 0.3;

    time_to_use
}