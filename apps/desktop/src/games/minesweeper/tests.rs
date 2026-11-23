use corner_table_api::GameId;
use corner_table_minesweeper::Checkpoint;
use gpui::{Modifiers, TestAppContext, VisualTestContext, point, size};

use super::*;

#[gpui::test]
fn compact_summit_routes_pointer_and_keyboard_to_the_same_cells(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("minesweeper-board-input-test".into()),
        entry: GameEntry::Classic,
    };
    let difficulty = Difficulty::Summit;
    let (width, height) = difficulty.dimensions();
    let window = cx.open_window(size(px(760.), px(540.)), |window, cx| {
        t::apply(t::Appearance::Dark, window, cx);
        cx.set_reduce_motion(true);
        let mut view = GameView::new(&launch, cx);
        view.round = Round::new(difficulty, Mode::Classic, 1);
        view.round.board = Board::restore(
            difficulty,
            Checkpoint {
                seed: 1,
                opening: Some(0),
                mines: [2, 4].into_iter().chain(88..156).collect(),
                open: vec![0, 1],
                flags: Vec::new(),
                detonated: None,
            },
        )
        .unwrap();
        view
    });
    cx.run_until_parked();

    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let grid = visual.debug_bounds("minesweeper-field").unwrap();
    let tile = (f32::from(grid.size.width) - (width - 1) as f32 * CELL_GAP) / width as f32;
    assert!(tile >= 20.);
    assert_eq!(
        grid.size.height,
        px(height as f32 * tile + (height - 1) as f32 * CELL_GAP)
    );

    let mine = grid.origin + point(px(2. * (tile + CELL_GAP) + tile / 2.), px(tile / 2.));
    visual.simulate_mouse_down(mine, MouseButton::Right, Modifiers::none());
    visual.simulate_mouse_up(mine, MouseButton::Right, Modifiers::none());
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.round.selected, (2, 0));
            assert_eq!(view.round.board.tile(2, 0), Some(Tile::Flagged));
            assert!(view.round.board.can_chord(1, 0));
        })
        .unwrap();

    let before_hint = window
        .read_with(cx, |view, _| view.round.board.checkpoint())
        .unwrap();
    visual.simulate_keystrokes("h");
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.round.board.checkpoint(), before_hint);
            let (column, row) = view.round.selected;
            assert_eq!(view.hint.target(column, row), Some(false));
        })
        .unwrap();
    visual.simulate_keystrokes("escape");
    window
        .update(cx, |view, _, cx| {
            assert!(matches!(view.hint, HintState::Hidden));
            view.round.selected = (2, 0);
            cx.notify();
        })
        .unwrap();

    visual.simulate_keystrokes("left enter");
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.round.selected, (1, 0));
            assert!(matches!(view.round.board.tile(1, 1), Some(Tile::Open(_))));
            assert_eq!(view.round.board.tile(2, 0), Some(Tile::Flagged));
        })
        .unwrap();

    visual.simulate_keystrokes("right f space");
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.round.board.tile(2, 0), Some(Tile::Detonated));
            assert_eq!(view.round.board.status(), Status::Lost);
            assert!(view.field_motion.is_none());
        })
        .unwrap();
    assert!(visual.debug_bounds("minesweeper-result").is_some());
}
