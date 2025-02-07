use rand::Rng;
use board_crab_lib::board::*;
use board_crab_lib::search;
use board_crab_lib::move_gen;
extern crate rand;

// Plays a bunch of random games and makes sure the board's persistent updates match manual full updates
#[test]
fn continuity_test_1() {
    board_crab_lib::init();

    let mut rng = rand::rng();

    const NUM_GAMES: usize = 200;
    const MAX_MOVES_PER_GAME: usize = 60;
    for _i in 0..NUM_GAMES {
        let mut board = Board::start_pos();
        for _j in 0..MAX_MOVES_PER_GAME {
            let mut moves = move_gen::MoveBuffer::new();
            move_gen::generate_moves(&board, &mut moves);

            // TODO: Move elsewhere
            for mv in moves.iter() {
                if mv.to & (board.pieces[0][PIECE_KING] | board.pieces[1][PIECE_KING]) != 0 {
                    panic!("Generated move that could attack the king");
                }
            }

            let mut board_clone = board.clone();
            board_clone.full_update();

            let mut clone_moves = move_gen::MoveBuffer::new();
            move_gen::generate_moves(&board_clone, &mut clone_moves);

            if moves.len() != clone_moves.len() {
                panic!("Continuity error");
            }

            if moves.is_empty() {
                break
            }

            // Play one of the moves
            let move_idx = rng.random_range(0..moves.len());
            board.do_move(&moves[move_idx]);
        }
    }
}

// Plays a bunch of random games and makes sure the perfts line up
#[test]
fn continuity_test_2() {
    board_crab_lib::init();

    const NUM_GAMES: usize = 5;
    const MAX_MOVES_PER_GAME: usize = 30;
    for _i in 0..NUM_GAMES {
        let mut board = Board::start_pos();
        for _j in 0..MAX_MOVES_PER_GAME {

            let outer_perft = search::perft(&board, 2, false);

            let mut moves = move_gen::MoveBuffer::new();
            move_gen::generate_moves(&board, &mut moves);
            let mut inner_perft_total = 0;
            for mv in moves.iter() {
                let mut next_board = board.clone();
                next_board.do_move(mv);
                next_board.full_update();

                inner_perft_total += search::perft(&next_board, 1, false);
            }

            if inner_perft_total != outer_perft {
                panic!("Continuity error");
            }
        }
    }
}