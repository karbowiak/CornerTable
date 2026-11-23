use super::*;

#[test]
fn preview_does_not_edit_and_hint_use_survives_dismissal_and_undo() {
    let mut round = Round::new(Puzzle::Daily(20_000), 0);
    let clues = round.game.filled();
    round.show_hint();
    let hint = round.visible_hint().unwrap();
    assert_eq!(round.selected, hint.index);
    assert_eq!(round.game.filled(), clues);

    round.hint_open = false;
    round.select_blank(false);
    round.show_hint();
    assert_eq!(round.visible_hint(), Some(hint));
    assert_eq!(round.hints_used, 1);

    round.note_mode = true;
    round.apply_hint();
    assert_eq!(round.game.value(hint.index), hint.entry());
    assert!(round.hint.is_none());
    round.undo();
    assert_eq!(round.game.filled(), clues);
    assert_eq!(round.hints_used, 1);

    while let Some(move_hint) = round.game.hint(round.selected) {
        round.selected = move_hint.index;
        round.edit(move_hint.entry(), false);
    }
    assert!(round.game.is_solved());
    assert_eq!(round.best, Some(1));
}

#[test]
fn mini_keyboard_movement_stays_in_bounds_and_skips_to_a_blank() {
    let mut round = Round::new(Puzzle::Practice(Format::Mini, Difficulty::Easy), 0);
    round.selected = 0;
    round.move_selection(-1, -1);
    assert_eq!(round.selected, 0);
    round.move_selection(1, 1);
    assert_eq!(round.selected, 7);
    round.select_blank(true);
    assert_ne!(round.selected, 7);
    assert_eq!(round.game.value(round.selected), 0);
    round.selected = 35;
    round.move_selection(1, 1);
    assert_eq!(round.selected, 35);
}
