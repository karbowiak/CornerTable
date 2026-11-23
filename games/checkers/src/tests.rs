use super::*;

fn sq(row: u8, column: u8) -> Square {
    Square::from_coords(row, column).unwrap()
}

fn step(from: (u8, u8), to: (u8, u8)) -> Step {
    Step {
        from: sq(from.0, from.1),
        to: sq(to.0, to.1),
    }
}

fn position(side: Side, red: &[(u8, u8, bool)], light: &[(u8, u8, bool)]) -> Position {
    let mut board = [None; 32];
    for (side, pieces) in [(Side::Red, red), (Side::Light, light)] {
        for &(row, column, king) in pieces {
            board[sq(row, column).0 as usize] = Some(Piece { side, king });
        }
    }
    Position {
        board,
        side,
        quiet_turns: 0,
    }
}

#[test]
fn opening_has_twelve_pieces_per_player_and_seven_red_moves() {
    let game = Game::default();
    assert_eq!(game.material(Side::Red), (12, 0));
    assert_eq!(game.material(Side::Light), (12, 0));
    assert_eq!(game.side(), Side::Red);
    assert_eq!(game.legal_steps().len(), 7);
    for step in game.legal_steps() {
        assert_eq!((step.from.row(), step.to.row()), (5, 4));
    }
    assert_eq!(Square::from_coords(0, 0), None);
    assert_eq!(Square::from_coords(4, 9), None);
    assert_eq!(Square::from_index(32), None);
}

#[test]
fn a_capture_anywhere_disallows_slides_and_locks_the_same_piece_until_done() {
    let mut game = Game::from_position(position(
        Side::Red,
        &[(5, 0, false), (7, 4, false)],
        &[(4, 1, false), (2, 3, false), (6, 5, false)],
    ));
    let first = step((5, 0), (3, 2));
    let second = step((3, 2), (1, 4));
    let other = step((7, 4), (5, 6));
    assert_eq!(game.legal_steps(), vec![first, other]);
    assert_eq!(game.play(step((7, 4), (6, 3))), None);
    assert_eq!(game.play(first).unwrap().captured, Some(sq(4, 1)));
    assert_eq!(game.piece(sq(4, 1)), None);
    assert_eq!(game.side(), Side::Red);
    assert_eq!(game.continuation(), Some(first.to));
    assert_eq!(game.last_steps(), &[first]);
    assert_eq!(game.legal_steps(), vec![second]);
    assert_eq!(game.play(other), None);
    game.play(second).unwrap();
    assert_eq!(game.side(), Side::Light);
    assert_eq!(game.continuation(), None);
    assert_eq!(game.last_steps(), &[first, second]);
}

#[test]
fn either_branch_is_legal_even_if_it_captures_fewer_pieces() {
    let mut game = Game::from_position(position(
        Side::Red,
        &[(5, 2, false)],
        &[(4, 1, false), (4, 3, false), (2, 5, false)],
    ));
    assert_eq!(game.legal_steps().len(), 2);
    game.play(step((5, 2), (3, 0))).unwrap();
    assert_eq!(game.side(), Side::Light);
    assert_eq!(game.material(Side::Light), (2, 0));
}

#[test]
fn crowning_ends_the_jump_chain_and_kings_can_move_backward() {
    let mut game = Game::from_position(position(
        Side::Red,
        &[(2, 1, false)],
        &[(1, 2, false), (1, 4, false)],
    ));
    assert!(game.play(step((2, 1), (0, 3))).unwrap().crowned);
    assert_eq!(game.side(), Side::Light);
    assert_eq!(game.continuation(), None);
    assert_eq!(game.material(Side::Red), (1, 1));
    assert_eq!(game.material(Side::Light), (1, 0));
    game.play(step((1, 4), (2, 3))).unwrap();
    assert!(game.legal_steps().contains(&step((0, 3), (1, 4))));

    let mut king = Game::from_position(position(Side::Red, &[(2, 3, true)], &[(3, 4, false)]));
    assert_eq!(king.legal_steps(), vec![step((2, 3), (4, 5))]);
    king.play(step((2, 3), (4, 5))).unwrap();
    assert_eq!(king.outcome(), Outcome::Won(Side::Red));
    assert_eq!(king.play(step((4, 5), (3, 4))), None);
}

#[test]
fn uncrowned_pieces_cannot_move_or_capture_backward() {
    let game = Game::from_position(position(Side::Red, &[(2, 3, false)], &[(3, 4, true)]));
    assert_eq!(
        game.legal_steps(),
        vec![step((2, 3), (1, 2)), step((2, 3), (1, 4))]
    );
}

#[test]
fn a_king_may_return_to_its_starting_square_but_cannot_rejump_a_piece() {
    let mut game = Game::from_position(position(
        Side::Red,
        &[(4, 3, true)],
        &[(3, 2, false), (3, 4, false), (1, 2, false), (1, 4, false)],
    ));
    for jump in [
        step((4, 3), (2, 1)),
        step((2, 1), (0, 3)),
        step((0, 3), (2, 5)),
        step((2, 5), (4, 3)),
    ] {
        game.play(jump).unwrap();
    }
    assert_eq!(game.last_steps().len(), 4);
    assert_eq!(game.outcome(), Outcome::Won(Side::Red));
    assert!(game.legal_steps().is_empty());
}

#[test]
fn blockade_ends_the_game_with_the_losing_piece_still_present() {
    let mut game = Game::from_position(position(
        Side::Red,
        &[(7, 6, false), (5, 0, false)],
        &[(6, 7, false)],
    ));
    game.play(step((5, 0), (4, 1))).unwrap();
    assert_eq!(game.material(Side::Light), (1, 0));
    assert_eq!(game.outcome(), Outcome::Won(Side::Red));
}

#[test]
fn threefold_repetition_is_automatic_and_undo_clears_the_result() {
    let mut game = Game::from_position(position(Side::Red, &[(7, 0, true)], &[(0, 7, true)]));
    let circuit = [
        step((7, 0), (6, 1)),
        step((0, 7), (1, 6)),
        step((6, 1), (7, 0)),
        step((1, 6), (0, 7)),
    ];
    for mv in circuit {
        game.play(mv).unwrap();
    }
    assert_eq!(game.outcome(), Outcome::Playing);
    for mv in circuit {
        game.play(mv).unwrap();
    }
    assert_eq!(game.outcome(), Outcome::Draw(Draw::Repetition));
    assert!(game.undo_turn());
    assert_eq!(game.side(), Side::Light);
    assert_eq!(game.outcome(), Outcome::Playing);
    game.play(circuit[3]).unwrap();
    assert_eq!(game.outcome(), Outcome::Draw(Draw::Repetition));
}

#[test]
fn no_progress_counts_forty_turns_each_and_resets_on_any_man_move() {
    let mut p = position(Side::Red, &[(7, 0, true), (5, 6, false)], &[(0, 7, true)]);
    p.quiet_turns = 78;
    let mut game = Game::from_position(p);
    game.play(step((7, 0), (6, 1))).unwrap();
    assert_eq!(game.outcome(), Outcome::Playing);
    game.play(step((0, 7), (1, 6))).unwrap();
    assert_eq!(game.outcome(), Outcome::Draw(Draw::NoProgress));
    assert!(game.undo_turn());
    assert_eq!(game.position.quiet_turns, 79);

    p.quiet_turns = 79;
    let mut game = Game::from_position(p);
    game.play(step((5, 6), (4, 5))).unwrap();
    assert_eq!(game.position.quiet_turns, 0);
    assert_eq!(game.outcome(), Outcome::Playing);
}

#[test]
fn undo_restores_every_capture_in_an_unfinished_or_completed_chain() {
    let opening = position(Side::Red, &[(5, 0, false)], &[(4, 1, false), (2, 3, false)]);
    let mut game = Game::from_position(opening);
    let first = step((5, 0), (3, 2));
    let second = step((3, 2), (1, 4));
    game.play(first).unwrap();
    assert!(game.undo_turn());
    assert_eq!(game.position, opening);
    assert_eq!(game.continuation, None);
    game.play(first).unwrap();
    game.play(second).unwrap();
    assert_eq!(game.outcome(), Outcome::Won(Side::Red));
    assert!(game.undo_turn());
    assert_eq!(game.position, opening);
    assert!(game.last_steps().is_empty());
    assert!(!game.undo_turn());
}

#[test]
fn computer_plays_the_entire_legal_branch_deterministically() {
    let game = Game::from_position(position(
        Side::Red,
        &[(5, 2, false)],
        &[(4, 1, false), (4, 3, false), (2, 5, false)],
    ));
    for difficulty in [Difficulty::Easy, Difficulty::Standard, Difficulty::Expert] {
        let chosen = game.computer_turn(difficulty);
        assert_eq!(chosen, game.computer_turn(difficulty));
        assert_eq!(chosen.len(), 2);
        let mut played = game.clone();
        for step in chosen {
            played.play(step).expect("legal jump");
        }
        assert_eq!(played.side(), Side::Light);
        assert_eq!(played.continuation(), None);
    }
}

#[test]
fn standard_search_looks_past_an_immediate_positional_gain() {
    let game = Game::from_position(position(
        Side::Red,
        &[(5, 4, false)],
        &[(3, 2, false), (2, 7, false)],
    ));
    assert_eq!(
        game.computer_turn(Difficulty::Easy),
        vec![step((5, 4), (4, 3))]
    );
    assert_eq!(
        game.computer_turn(Difficulty::Standard),
        vec![step((5, 4), (4, 5))]
    );
}
