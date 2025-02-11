use crate::board::*;

// Determines how much time to use on the next search, given the board
pub fn get_time_to_use(board: &Board, remaining_time: f64) -> f64 {
    let num_pieces = board.combined_occupancy().count_ones();

    // Very rough estimate of how many plies remain

    let remaining_pieces_ratio = (num_pieces as f64) / 32.0;
    let remaining_plies = remaining_pieces_ratio * 70.0 + 10.0;

    let mut time_to_use = remaining_time / remaining_plies;

    // Add wait to the start of the game
    time_to_use *= 1.0 + remaining_pieces_ratio * 0.8;

    time_to_use
}