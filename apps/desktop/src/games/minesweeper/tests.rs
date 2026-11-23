use corner_table_api::GameId;
use corner_table_minesweeper::Checkpoint;
use gpui::{Modifiers, TestAppContext, VisualTestContext, point, size};

use super::*;

#[gpui::test]
fn clipboard_practice_preserves_the_original_round_save_clock_and_results(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("minesweeper-clipboard-test".into()),
        entry: GameEntry::Classic,
    };
    let window = cx.open_window(size(px(900.), px(760.)), |_, cx| {
        cx.set_reduce_motion(true);
        let mut view = GameView::new(&launch, cx);
        view.round = Round::new(Difficulty::Meadow, Mode::LogicOnly, 19);
        view.round.board = Board::logical(Difficulty::Meadow, 19, 3, 4).unwrap();
        assert!(view.round.board.reveal(3, 4));
        let mine = view.round.board.checkpoint().mines[0];
        assert!(view.round.board.toggle_flag(mine % 9, mine / 9));
        view.round.selected = (mine % 9, mine / 9);
        view.round.flag_mode = true;
        view.restore_clock(Duration::from_secs(37), cx);
        view.save();
        view
    });
    cx.run_until_parked();
    let (original, attempt, selected, saved) = window
        .read_with(cx, |view, _| {
            (
                view.round.board.checkpoint(),
                view.round.attempt,
                view.round.selected,
                saved_round(view),
            )
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let click = |visual: &mut VisualTestContext, selector: &'static str| {
        let position = visual.debug_bounds(selector).unwrap().center();
        visual.simulate_click(position, Modifiers::none());
    };
    visual.write_to_clipboard(ClipboardItem::new_string("invalid".into()));
    click(&mut visual, "minesweeper-paste");
    window
        .read_with(cx, |view, _| {
            assert!(view.share_feedback.is_some());
            assert_eq!(saved_round(view), saved);
            assert!(view.original_field.is_none());
        })
        .unwrap();
    click(&mut visual, "minesweeper-copy");
    let code = visual.read_from_clipboard().unwrap().text().unwrap();
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.round.board.deal_code().unwrap(), code);
            assert_eq!(view.round.board.checkpoint(), original);
            assert_eq!(saved_round(view), saved);
        })
        .unwrap();
    click(&mut visual, "minesweeper-paste");
    // Importing again from practice must still return to the original field.
    click(&mut visual, "minesweeper-paste");
    window
        .update(cx, |view, _, cx| {
            assert!(view.round.attempt != attempt);
            assert!(view.round.replay);
            assert_eq!(view.round.mode, Mode::Classic);
            assert_eq!(view.round.selected, (3, 4));
            assert_eq!(view.clock.elapsed(), Duration::ZERO);
            let checkpoint = view.round.board.checkpoint();
            assert!(checkpoint.open.is_empty() && checkpoint.flags.is_empty());
            assert_eq!(checkpoint.mines, original.mines);
            let parked_save = saved_round(view);
            view.apply_move((3, 4), Move::Reveal, cx);
            for index in 0..81 {
                if !checkpoint.mines.contains(&index) {
                    view.apply_move((index % 9, index / 9), Move::Reveal, cx);
                }
            }
            assert_eq!(view.round.board.status(), Status::Won);
            assert!(view.bests.time(&view.round).is_none());
            assert_eq!(saved_round(view), parked_save);
            assert!(
                crate::storage::recent_results(100)
                    .iter()
                    .all(|result| result.game != launch.game)
            );
            let (resumable, elapsed, _) = view
                .storage
                .load::<SavedGame>()
                .unwrap()
                .restore(None)
                .unwrap();
            assert_eq!(resumable.board.checkpoint(), original);
            assert!(resumable.attempt == attempt);
            assert!(elapsed >= Duration::from_secs(37));
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    click(&mut visual, "minesweeper-return");
    window
        .read_with(cx, |view, _| {
            assert!(view.original_field.is_none());
            assert_eq!(view.round.board.checkpoint(), original);
            assert!(view.round.attempt == attempt && view.round.flag_mode);
            assert_eq!(view.round.selected, selected);
            assert_eq!(view.round.mode, Mode::LogicOnly);
            assert!(matches!(view.clock, Clock::Running { .. }));
            assert!(view.clock.elapsed() >= Duration::from_secs(37));
        })
        .unwrap();
}

fn saved_round(view: &GameView) -> String {
    serde_json::to_string(&view.storage.load::<SavedGame>().unwrap()).unwrap()
}

#[gpui::test]
fn daily_and_time_trial_fields_reject_clipboard_actions(cx: &mut TestAppContext) {
    let mut source = Board::new(Difficulty::Summit, 11);
    source.reveal(3, 4);
    let code = source.deal_code().unwrap();
    for entry in [
        GameEntry::Classic,
        GameEntry::Challenge("test-daily".into()),
    ] {
        let launch = GameLaunch {
            game: GameId("minesweeper-sharing-isolation".into()),
            entry,
        };
        let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
            let mut view = GameView::new(&launch, cx);
            if view.challenge_seed.is_none() {
                view.round.mode = Mode::TimeTrial;
            }
            view.round.board.reveal(0, 0);
            view
        });
        cx.run_until_parked();
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        assert!(visual.debug_bounds("minesweeper-copy").is_none());
        assert!(visual.debug_bounds("minesweeper-paste").is_none());
        visual.write_to_clipboard(ClipboardItem::new_string(code.clone()));
        window
            .update(cx, |view, window, cx| {
                let before = view.round.board.checkpoint();
                view.copy_field(cx);
                assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), code);
                view.paste_field(window, cx);
                assert_eq!(view.round.board.checkpoint(), before);
                assert!(view.original_field.is_none());
            })
            .unwrap();
    }
}

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
