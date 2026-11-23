use super::*;

#[test]
fn standard_seeds_keep_their_original_puzzles() {
    for (seed, difficulty, expected) in [
        (
            0,
            Difficulty::Medium,
            "460307805000005460020000037102000500003412790706803040300028000010060009670000210",
        ),
        (
            173,
            Difficulty::Hard,
            "004000300960030000300200080000041020002056407000902000000060000000413270003800095",
        ),
    ] {
        let game = Game::new(seed, difficulty);
        let digits: String = game
            .givens
            .map(|digit| char::from(b'0' + digit))
            .into_iter()
            .collect();
        assert_eq!(digits, expected);
    }
}

#[test]
fn both_formats_generate_valid_unique_puzzles_with_ordered_clue_counts() {
    for format in [Format::Standard, Format::Mini] {
        for seed in [0, 1, 173, u64::MAX] {
            let mut previous = format.cell_count();
            for difficulty in Difficulty::ALL {
                let game = Game::with_format(seed, difficulty, format);
                assert_eq!(
                    game.givens,
                    Game::with_format(seed, difficulty, format).givens
                );
                let mut givens = game.givens;
                assert_eq!(count_solutions(format, &mut givens, 2), 1);
                assert!(game.filled() < previous);
                previous = game.filled();

                for index in 0..format.cell_count() {
                    for peer in index + 1..format.cell_count() {
                        if format.related(index, peer) {
                            assert_ne!(game.solution[index], game.solution[peer]);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mini_boxes_have_two_rows_and_three_columns() {
    let format = Format::Mini;
    assert_eq!(format.cell_count(), 36);
    assert!(format.related(0, 8));
    assert!(!format.related(0, 9));
    assert!(!format.related(0, 14));
    assert_eq!(
        Unit::Box(3).cells(format).collect::<Vec<_>>(),
        [15, 16, 17, 21, 22, 23]
    );
}

#[test]
fn daily_is_repeatable_and_changes_with_the_day() {
    let today = Game::daily(20_000);
    assert_eq!(today.givens, Game::daily(20_000).givens);
    assert_ne!(today.givens, Game::daily(20_001).givens);
    assert_eq!(today.format(), Format::Standard);
    assert_eq!(today.filled(), Difficulty::Medium.clues(today.format()));
}

#[test]
fn givens_are_locked_and_notes_and_values_are_undoable() {
    let mut game = Game::with_format(0, Difficulty::Medium, Format::Mini);
    let given = (0..36).find(|&index| game.is_given(index)).unwrap();
    let digit = game.value(given);
    let empty = (0..36)
        .find(|&index| !game.is_given(index) && Format::Mini.related(given, index))
        .unwrap();
    assert!(!game.enter(given, 0, false));
    assert!(!game.enter(empty, 7, false));
    assert!(!game.enter(36, 1, false));
    assert!(!game.can_undo());
    assert!(game.enter(empty, digit, true));
    assert_eq!(game.notes(empty), 1 << digit);
    assert!(game.enter(empty, digit, false));
    assert!(game.has_conflict(empty));
    assert!(game.has_conflict(given));
    assert_eq!(game.notes(empty), 0);
    assert!(game.undo());
    assert_eq!(game.notes(empty), 1 << digit);
    assert!(game.enter(empty, 0, false));
    assert_eq!(game.notes(empty), 0);
}

#[test]
fn placing_a_digit_prunes_peer_notes_and_undo_restores_them() {
    let mut game = Game::with_format(0, Difficulty::Easy, Format::Mini);
    let (index, peer) = (0..36)
        .filter(|&index| !game.is_given(index))
        .find_map(|index| {
            (index + 1..36)
                .find(|&peer| !game.is_given(peer) && Format::Mini.related(index, peer))
                .map(|peer| (index, peer))
        })
        .unwrap();
    let digit = game.solution[index];
    game.enter(peer, digit, true);
    game.enter(index, digit, false);
    assert_eq!(game.notes(peer), 0);
    game.undo();
    assert_eq!(game.notes(peer), 1 << digit);
}

#[test]
fn hints_explain_duplicates_and_distinguish_wrong_entries_without_duplicates() {
    let mut game = Game::new(173, Difficulty::Hard);
    let index = (0..CELLS).find(|&index| !game.is_given(index)).unwrap();
    let peer = (0..CELLS)
        .find(|&peer| game.is_given(peer) && Game::related(index, peer))
        .unwrap();
    let digit = game.value(peer);
    game.enter(index, digit, false);
    let hint = game.hint(index).unwrap();
    assert_eq!(hint.index, index);
    assert_eq!(hint.entry(), 0);
    assert!(
        matches!(hint.reason, HintReason::Duplicate { digit: seen, unit, peer, .. }
        if seen == digit && unit.contains(game.format(), index) && unit.contains(game.format(), peer))
    );
    game.undo();

    let (index, digit) = (0..CELLS)
        .filter(|&index| !game.is_given(index))
        .find_map(|index| {
            (1..=9)
                .find(|&digit| digit != game.solution[index] && allowed(&game, index, digit))
                .map(|digit| (index, digit))
        })
        .unwrap();
    game.enter(index, digit, false);
    assert!(!game.has_conflict(index));
    assert_eq!(
        game.hint(index),
        Some(Hint {
            index,
            reason: HintReason::IncorrectEntry { digit }
        })
    );
    assert_eq!(game.value(index), digit);
}

#[test]
fn hints_are_non_mutating_sound_and_can_finish_both_formats() {
    let mut only_place_seen = false;
    let mut lock_seen = false;
    let mut forced_seen = false;
    for format in [Format::Standard, Format::Mini] {
        for seed in [0, 1, 7, 173, u64::MAX] {
            let mut game = Game::with_format(seed, Difficulty::Hard, format);
            let count = format.cell_count() - game.filled();
            for _ in 0..count {
                let filled = game.filled();
                let hint = game.hint(0).expect("an unfinished puzzle has a move");
                assert_eq!(game.value(hint.index), 0);
                assert_eq!(game.filled(), filled);
                assert_eq!(hint.entry(), game.solution[hint.index]);
                match hint.reason {
                    HintReason::OnlyCandidate { digit } => {
                        assert!(
                            !(1..=format.size() as u8)
                                .any(|other| other != digit && allowed(&game, hint.index, other))
                        );
                    }
                    HintReason::OnlyPlace { digit, unit } => {
                        only_place_seen = true;
                        assert!(unit.contains(format, hint.index));
                        assert!(!unit.cells(format).any(|other| other != hint.index
                            && game.value(other) == 0
                            && allowed(&game, other, digit)));
                    }
                    HintReason::LockedCandidate {
                        digit,
                        excluded,
                        source,
                        shared,
                    } => {
                        lock_seen = true;
                        assert!(!source.contains(format, hint.index));
                        assert!(shared.contains(format, hint.index));
                        let available: Vec<_> = (1..=format.size() as u8)
                            .filter(|&candidate| allowed(&game, hint.index, candidate))
                            .collect();
                        assert_eq!(available.len(), 2);
                        assert!(available.contains(&digit) && available.contains(&excluded));
                        let source_options: Vec<_> = source
                            .cells(format)
                            .filter(|&index| {
                                game.value(index) == 0 && allowed(&game, index, excluded)
                            })
                            .collect();
                        assert!(source_options.len() > 1);
                        assert!(
                            source_options
                                .into_iter()
                                .all(|index| shared.contains(format, index))
                        );
                    }
                    HintReason::Forced { digit, candidates } => {
                        forced_seen = true;
                        assert_ne!(candidates & (1 << digit), 0);
                        for other in (1..=format.size() as u8)
                            .filter(|&other| other != digit && candidates & (1 << other) != 0)
                        {
                            let mut values = game.board.values;
                            values[hint.index] = other;
                            assert_eq!(count_solutions(format, &mut values, 1), 0);
                        }
                    }
                    _ => panic!("a correct board should receive a placement hint"),
                }
                assert!(game.enter(hint.index, hint.entry(), false));
            }
            assert!(game.is_solved());
            assert_eq!(game.hint(0), None);
            let editable = (0..format.cell_count())
                .find(|&index| !game.is_given(index))
                .unwrap();
            assert!(!game.enter(editable, 0, false));
            assert!(game.undo());
            assert!(!game.is_solved());
        }
    }
    assert!(only_place_seen);
    assert!(lock_seen);
    assert!(forced_seen);
}

fn allowed(game: &Game, index: usize, digit: u8) -> bool {
    let format = game.format();
    !(0..format.cell_count())
        .any(|peer| peer != index && format.related(index, peer) && game.value(peer) == digit)
}
