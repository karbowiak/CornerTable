use super::*;

fn at(row: u8, column: u8) -> Square {
    Square::new(row, column).unwrap()
}

fn position(black: u64, white: u64, disc: Disc) -> Position {
    let (mine, theirs) = match disc {
        Disc::Black => (black, white),
        Disc::White => (white, black),
    };
    Position {
        black,
        white,
        turn: Turn::Play(disc),
        legal: Squares(legal_moves(mine, theirs)),
    }
}

#[test]
fn starts_black_with_the_four_standard_moves() {
    let board = Position::new();
    assert_eq!(board.turn(), Turn::Play(Disc::Black));
    assert_eq!(board.disc(at(3, 3)), Some(Disc::White));
    assert_eq!(board.disc(at(3, 4)), Some(Disc::Black));
    assert_eq!(board.disc(at(4, 3)), Some(Disc::Black));
    assert_eq!(board.disc(at(4, 4)), Some(Disc::White));
    assert_eq!(
        board.legal_moves().collect::<Vec<_>>(),
        [at(2, 3), at(3, 2), at(4, 5), at(5, 4)]
    );
}

#[test]
fn rejects_occupied_unbracketed_and_out_of_range_squares() {
    let original = Position::new();
    let mut board = original;
    assert_eq!(board.play(at(3, 3)), None);
    assert_eq!(board.play(at(0, 0)), None);
    assert_eq!(board, original);
    assert_eq!(Square::new(8, 4), None);
    assert_eq!(Square::new(4, 8), None);
    assert_eq!(Square::from_index(64), None);
    assert_eq!(Square::from_index(255), None);
}

#[test]
fn flips_all_eight_directions_in_one_move() {
    let mut black = 0;
    let mut white = 0;
    for row in [-1_i8, 0, 1] {
        for column in [-1_i8, 0, 1] {
            if (row, column) != (0, 0) {
                black |= at((3 + 2 * row) as u8, (3 + 2 * column) as u8).bit();
                white |= at((3 + row) as u8, (3 + column) as u8).bit();
            }
        }
    }
    let mut board = position(black, white, Disc::Black);
    let played = board.play(at(3, 3)).unwrap();
    assert_eq!(played.flipped, Squares(white));
    assert_eq!(board.score(Disc::Black), 17);
    assert_eq!(board.score(Disc::White), 0);
    assert_eq!(board.turn(), Turn::Finished(Outcome::Won(Disc::Black)));
}

#[test]
fn captures_a_six_disc_run_but_never_wraps_a_file() {
    let mut board = position(1, 0b0111_1110, Disc::Black);
    assert_eq!(board.play(at(0, 7)).unwrap().flipped.count(), 6);
    assert_eq!(board.score(Disc::Black), 8);

    let mut board = position(at(0, 6).bit(), at(0, 7).bit(), Disc::Black);
    assert_eq!(board.play(at(1, 0)), None);
    assert!(board.legal_moves().is_empty());
}

#[test]
fn forced_pass_retains_turn_then_finishes_with_empty_squares() {
    let mut board = position(
        at(0, 0).bit() | at(7, 0).bit(),
        at(0, 1).bit() | at(7, 1).bit(),
        Disc::Black,
    );
    let first = board.play(at(0, 2)).unwrap();
    assert_eq!(first.passed, Some(Disc::White));
    assert_eq!(board.turn(), Turn::Play(Disc::Black));
    assert_eq!(board.legal_moves().collect::<Vec<_>>(), [at(7, 2)]);
    let last = board.play(at(7, 2)).unwrap();
    assert_eq!(last.passed, None);
    assert_eq!(board.turn(), Turn::Finished(Outcome::Won(Disc::Black)));
    assert_eq!(board.score(Disc::Black), 6);
    assert_eq!(board.empty_count(), 58);
    assert_eq!(board.play(at(4, 4)), None);
    assert_eq!(choose_move(board, Difficulty::Strategist), None);
}

#[test]
fn full_board_can_end_in_a_tie() {
    let empty = at(0, 0).bit();
    // A1 brackets B1 against C1. A2 and B2 are already black.
    let black = 0xffff_fffc;
    let mut board = position(black, !(black | empty), Disc::Black);
    assert_eq!(board.play(at(0, 0)).unwrap().flipped.count(), 1);
    assert_eq!(board.score(Disc::Black), 32);
    assert_eq!(board.score(Disc::White), 32);
    assert_eq!(board.turn(), Turn::Finished(Outcome::Draw));
}

#[test]
fn bit_board_matches_a_grid_scan_across_complete_games() {
    for seed in 1..=32 {
        let mut board = Position::new();
        let mut random = seed;
        let mut moves = 0;
        while let Turn::Play(disc) = board.turn() {
            let mut legal = Squares::default();
            for square in Squares(u64::MAX) {
                let flips = reference_flips(board, disc, square);
                assert_eq!(board.flips(square), flips, "seed {seed}, square {square}");
                if !flips.is_empty() {
                    legal.0 |= square.bit();
                }
            }
            assert_eq!(board.legal_moves(), legal);
            random = next_random(random);
            let choice = legal.nth(random as usize % legal.len()).unwrap();
            let played = board.play(choice).unwrap();
            moves += 1;
            assert_eq!(board.empty_count() + moves, 60);
            assert_eq!(board.black & board.white, 0);
            let other = (0..64)
                .map(|index| Square::from_index(index).unwrap())
                .any(|cell| !reference_flips(board, disc.opposite(), cell).is_empty());
            assert_eq!(other, board.turn() == Turn::Play(disc.opposite()));
            assert_eq!(played.passed.is_some(), board.turn() == Turn::Play(disc));
        }
        assert!(board.legal_moves().is_empty());
        assert!(Squares(u64::MAX).all(|square| {
            reference_flips(board, Disc::Black, square).is_empty()
                && reference_flips(board, Disc::White, square).is_empty()
        }));
    }
}

#[test]
fn computer_difficulties_play_legal_distinct_games() {
    let mut board = Position::new();
    let mut different = false;
    while let Turn::Play(disc) = board.turn() {
        let casual = choose_move(board, Difficulty::Casual).unwrap();
        let strategist = choose_move(board, Difficulty::Strategist).unwrap();
        assert!(board.legal_moves().contains(casual));
        assert!(board.legal_moves().contains(strategist));
        different |= casual != strategist;
        let square = if disc == Disc::Black {
            casual
        } else {
            strategist
        };
        board.play(square).unwrap();
    }
    assert!(different);
    assert!(board.score(Disc::White) > board.score(Disc::Black));
}

#[test]
fn strategist_finds_an_optimal_endgame_move_including_passes() {
    for seed in 1..=6 {
        let mut board = Position::new();
        let mut random = seed;
        while board.empty_count() > 7 && !board.legal_moves().is_empty() {
            let mut legal = board.legal_moves();
            random = next_random(random);
            board.play(legal.nth(random as usize % legal.len()).unwrap());
        }
        if let Turn::Play(disc) = board.turn() {
            let optimum = optimal_margin(board, disc);
            board.play(choose_move(board, Difficulty::Strategist).unwrap());
            assert_eq!(optimal_margin(board, disc), optimum);
        }
    }
}

fn optimal_margin(board: Position, perspective: Disc) -> i32 {
    let Turn::Play(disc) = board.turn() else {
        return board.score(perspective) as i32 - board.score(perspective.opposite()) as i32;
    };
    let margins = board.legal_moves().map(|choice| {
        let mut next = board;
        next.play(choice).unwrap();
        optimal_margin(next, perspective)
    });
    if disc == perspective {
        margins.max()
    } else {
        margins.min()
    }
    .unwrap()
}

fn next_random(seed: u32) -> u32 {
    seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

fn reference_flips(board: Position, disc: Disc, square: Square) -> Squares {
    if board.disc(square).is_some() {
        return Squares::default();
    }
    let mut flips = 0;
    for dr in [-1_i8, 0, 1] {
        for dc in [-1_i8, 0, 1] {
            if (dr, dc) == (0, 0) {
                continue;
            }
            let mut row = square.row();
            let mut column = square.column();
            let mut run = 0;
            while let Some(next) = row
                .checked_add_signed(dr)
                .zip(column.checked_add_signed(dc))
                .and_then(|(row, column)| Square::new(row, column))
            {
                row = next.row();
                column = next.column();
                match board.disc(next) {
                    Some(color) if color == disc.opposite() => run |= next.bit(),
                    Some(_) => {
                        flips |= run;
                        break;
                    }
                    None => break,
                }
            }
        }
    }
    Squares(flips)
}
