use corner_table_api::GameId;
use corner_table_sudoku::CageIssue;
use gpui::{Modifiers, TestAppContext, VisualTestContext, size};

use super::*;
use crate::games::{GameEntry, GameLaunch};

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let point = cx.debug_bounds(selector).expect(selector).center();
    cx.simulate_mouse_move(point, None, Modifiers::default());
    cx.simulate_click(point, Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn killer_plays_at_minimum_size_and_restores_each_board(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId(format!("sudoku-killer-harness-{}", std::process::id())),
        entry: GameEntry::Classic,
    };
    let viewport = size(px(760.), px(540.));
    let window = cx.open_window(viewport, |_, cx| GameView::new(&launch, cx));
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(window.into(), cx);

    click(&mut ui, "sudoku-killer");
    let board = ui.debug_bounds("sudoku-board").unwrap();
    let first = ui.debug_bounds("sudoku-cell-0").unwrap();
    let last = ui.debug_bounds("sudoku-cell-80").unwrap();
    assert_eq!(board.size.width, board.size.height);
    assert!(first.size.width >= px(40.));
    assert!(last.right() <= board.right());
    assert!(last.bottom() <= board.bottom());
    let digit = ui.debug_bounds("sudoku-digit-9").unwrap();
    assert!(digit.left() > board.right());
    assert!(digit.bottom() < viewport.height);

    ui.simulate_keystrokes("n 1 2 3 4 5 6 7 8 9");
    let label = ui.debug_bounds("killer-total-0").unwrap();
    let notes = ui.debug_bounds("killer-notes-0").unwrap();
    assert!(label.bottom() <= notes.top());
    assert!(notes.bottom() + px(3.) <= first.bottom());
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.round().game.notes(0), (1 << 10) - 2);
        })
        .unwrap();

    click(&mut ui, "sudoku-notes");
    click(&mut ui, "sudoku-digit-9");
    click(&mut ui, "sudoku-cell-9");
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.round().selected, 9);
            assert_eq!(view.round().game.value(0), 9);
            assert_eq!(view.round().game.cage_issue(9), Some(CageIssue::OverTotal));
        })
        .unwrap();

    ui.simulate_keystrokes("h");
    let apply = ui.debug_bounds("sudoku-apply-hint").unwrap();
    assert!(apply.left() > board.right());
    assert!(apply.bottom() < viewport.height);
    ui.simulate_keystrokes("enter");
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.round().game.value(0), 0);
            assert_eq!(view.round().hints_used, 1);
        })
        .unwrap();
    ui.simulate_keystrokes("ctrl-z");
    click(&mut ui, "sudoku-killer-next");
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.puzzle, Puzzle::Killer(KillerPuzzle::Crossings));
            assert_eq!(view.round().game.filled(), 0);
        })
        .unwrap();
    ui.simulate_keystrokes("right 2");
    click(&mut ui, "sudoku-killer-previous");
    window
        .read_with(&ui, |view, _| {
            assert_eq!(view.puzzle, Puzzle::Killer(KillerPuzzle::FirstLight));
            assert_eq!(view.round().game.value(0), 9);
        })
        .unwrap();

    let reopened = cx.open_window(viewport, |_, cx| GameView::new(&launch, cx));
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(reopened.into(), cx);
    reopened
        .read_with(&ui, |view, _| {
            assert_eq!(view.puzzle, Puzzle::Killer(KillerPuzzle::FirstLight));
            assert_eq!(view.round().game.value(0), 9);
            assert_eq!(view.round().hints_used, 1);
        })
        .unwrap();
    click(&mut ui, "sudoku-killer-next");
    reopened
        .read_with(&ui, |view, _| {
            assert_eq!(view.puzzle, Puzzle::Killer(KillerPuzzle::Crossings));
            assert_eq!(view.round().selected, 1);
            assert_eq!(view.round().game.value(1), 2);
        })
        .unwrap();
}
