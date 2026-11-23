use corner_table_api::GameId;
use corner_table_mahjong::Face;
use gpui::{KeyUpEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext, size};

use super::*;

#[gpui::test]
fn a_real_dead_end_offers_undo_and_returns_to_keyboard_play(cx: &mut TestAppContext) {
    let layout = Layout::Crown;
    let seed = 1;
    let mut board = Board::with_layout(layout, seed);
    while let Some([first, second]) = board.hint() {
        assert_eq!(board.select(first), MoveResult::Selected);
        assert_eq!(board.select(second), MoveResult::Matched);
    }
    assert_eq!(board.remaining(), 42);
    let (&[first, second], before) = board.matched_pairs().split_last().unwrap();

    let launch = GameLaunch {
        game: GameId("mahjong-round-dead-end-test".into()),
        entry: GameEntry::Classic,
    };
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
        let mut view = GameView::new(&launch, cx);
        assert!(view.resume(SavedRound {
            layout: layout.id().into(),
            seed,
            attempt: AttemptId::default(),
            matches: before.to_vec(),
            assisted: false,
            clean_clear: true,
        }));
        view
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.debug_bounds("mahjong-hint").is_some());
    window
        .update(cx, |view, _, cx| {
            view.choose(first, cx);
            view.choose(second, cx);
            assert_eq!(view.round_outcome(), Some(Outcome::Lost));
        })
        .unwrap();
    cx.run_until_parked();
    assert!(visual.debug_bounds("mahjong-hint").is_none());
    assert!(visual.debug_bounds("mahjong-result-undo").is_some());

    // The first tab stop after the table is the recovery action.
    window
        .update(cx, |_, window, cx| window.focus_next(cx))
        .unwrap();
    visual.simulate_keystrokes("enter");
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    let hint = window
        .update(cx, |view, window, _| {
            assert!(view.focus.is_focused(window));
            assert_eq!(view.round_outcome(), None);
            assert_eq!(view.board.matched_pairs(), before);
            assert!(view.board.used_assistance());
            view.board.hint().unwrap()
        })
        .unwrap();
    assert!(visual.debug_bounds("mahjong-result-undo").is_none());
    visual.simulate_keystrokes("h");
    window
        .read_with(cx, |view, _| assert_eq!(view.highlighted, Some(hint)))
        .unwrap();
}

#[gpui::test]
fn a_cleared_deal_replays_with_the_same_seed_and_a_new_attempt(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let launch = GameLaunch {
        game: GameId("mahjong-round-replay-test".into()),
        entry: GameEntry::Classic,
    };
    let layout = Layout::TeaTray;
    let seed = 0;
    let attempt = AttemptId::default();
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
        let mut view = GameView::new(&launch, cx);
        view.seed = seed;
        view.attempt = attempt;
        view.board = Board::with_layout(layout, seed);
        view.clean_clear = true;
        view
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    window
        .update(cx, |view, _, cx| {
            while let Some([first, second]) = view.board.hint() {
                view.choose(first, cx);
                view.choose(second, cx);
            }
            assert_eq!(view.round_outcome(), Some(Outcome::Won));
            assert_eq!(view.board.matched_pairs().len(), 12);
            assert!(!view.board.used_assistance());
            assert!(view.departure.is_none());
        })
        .unwrap();
    cx.run_until_parked();
    assert!(visual.debug_bounds("mahjong-result-new").is_some());
    assert!(visual.debug_bounds("mahjong-copy-deal").is_some());
    assert!(visual.debug_bounds("mahjong-play-copied").is_some());
    visual.simulate_keystrokes("u");
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.round_outcome(), None);
            assert!(view.attempt == attempt);
            let [first, second] = view.board.hint().unwrap();
            view.choose(first, cx);
            view.choose(second, cx);
            assert_eq!(view.round_outcome(), Some(Outcome::Won));
            assert!(view.board.used_assistance());
            assert!(view.attempt == attempt);
        })
        .unwrap();
    cx.run_until_parked();
    let replay = visual
        .debug_bounds("mahjong-result-replay")
        .unwrap()
        .center();
    visual.simulate_click(replay, gpui::Modifiers::none());
    window
        .update(cx, |view, window, _| {
            assert!(view.focus.is_focused(window));
            assert_eq!(view.round_outcome(), None);
            assert_eq!(view.seed, seed);
            assert_eq!(view.board.layout(), layout);
            assert_eq!(view.board.remaining(), 24);
            assert!(!view.board.used_assistance());
            assert!(view.attempt != attempt);
        })
        .unwrap();
}

#[test]
fn deal_codes_validate_the_game_version_layout_and_seed() {
    for layout in Layout::ALL {
        for seed in [0, 1, u64::MAX] {
            assert_eq!(
                decode_deal(&encode_deal(layout, seed)),
                Some((layout, seed))
            );
        }
    }
    assert_eq!(
        decode_deal("\nmahjong:v1:tea-tray:FFFFFFFFFFFFFFFF\t"),
        Some((Layout::TeaTray, u64::MAX))
    );
    for invalid in [
        "",
        "mahjong-cubes:v1:tea-tray:0000000000000000",
        "mahjong:v2:tea-tray:0000000000000000",
        "mahjong:v1:unknown:0000000000000000",
        "mahjong:v1:tea-tray",
        "mahjong:v1:tea-tray:0",
        "mahjong:v1:tea-tray:+000000000000000",
        "mahjong:v1:tea-tray:xxxxxxxxxxxxxxxx",
        "mahjong:v1:tea-tray:10000000000000000",
        "mahjong:v1:tea-tray:0000000000000000:extra",
    ] {
        assert_eq!(decode_deal(invalid), None, "{invalid:?}");
    }
}

fn tile_signature(board: &Board) -> Vec<(u8, u8, u8, Face)> {
    board
        .all_tiles()
        .iter()
        .map(|tile| (tile.x, tile.y, tile.level, tile.face))
        .collect()
}

fn saved_round(view: &GameView) -> String {
    serde_json::to_string(&view.session.load::<SavedRound>().unwrap()).unwrap()
}

#[gpui::test]
fn clipboard_round_trip_keeps_the_source_and_starts_an_exact_fresh_deal(cx: &mut TestAppContext) {
    let source_launch = GameLaunch {
        game: GameId("mahjong-copy-source-test".into()),
        entry: GameEntry::Classic,
    };
    let layout = Layout::TeaTray;
    let seed = 0xfedc_ba98_7654_3210;
    let source = cx.open_window(size(px(760.), px(540.)), |_, cx| {
        let mut view = GameView::new(&source_launch, cx);
        view.start_deal(layout, seed);
        view.toggle_clean_clear();
        let [first, second] = view.board.hint().unwrap();
        assert_eq!(view.board.select(first), MoveResult::Selected);
        assert_eq!(view.board.select(second), MoveResult::Matched);
        view.hint();
        assert_eq!(
            view.board.select(view.highlighted.unwrap()[0]),
            MoveResult::Selected
        );
        view
    });
    cx.run_until_parked();
    let (tiles, source_save, source_attempt, source_selection) = source
        .read_with(cx, |view, _| {
            (
                tile_signature(&view.board),
                saved_round(view),
                view.attempt,
                view.board.selected(),
            )
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(source.into(), cx);
    let copy = visual.debug_bounds("mahjong-copy-deal").unwrap().center();
    visual.simulate_click(copy, Modifiers::none());
    let code = visual.read_from_clipboard().unwrap().text().unwrap();
    assert_eq!(code, "mahjong:v1:tea-tray:fedcba9876543210");
    source
        .read_with(cx, |view, _| {
            assert!(matches!(&view.status, Feedback::Share("Deal code copied.")));
            assert_eq!(view.board.remaining(), layout.tile_count() - 2);
            assert_eq!(view.board.selected(), source_selection);
            assert!(view.board.used_assistance());
            assert!(view.clean_clear);
            assert!(view.attempt == source_attempt);
            assert_eq!(saved_round(view), source_save);
        })
        .unwrap();

    let recipient_launch = GameLaunch {
        game: GameId("mahjong-copy-recipient-test".into()),
        entry: GameEntry::Classic,
    };
    let recipient = cx.open_window(size(px(760.), px(540.)), |_, cx| {
        let mut view = GameView::new(&recipient_launch, cx);
        view.start_deal(Layout::Atrium, 1);
        view.toggle_clean_clear();
        view.hint();
        view
    });
    cx.run_until_parked();
    let (old_save, old_attempt) = recipient
        .read_with(cx, |view, _| (saved_round(view), view.attempt))
        .unwrap();
    let mut visual = VisualTestContext::from_window(recipient.into(), cx);
    let play = |visual: &mut VisualTestContext| {
        let button = visual.debug_bounds("mahjong-play-copied").unwrap().center();
        visual.simulate_click(button, Modifiers::none());
    };
    visual.write_to_clipboard(ClipboardItem::new_string("not a deal code".into()));
    play(&mut visual);
    recipient
        .read_with(cx, |view, _| {
            assert!(matches!(
                &view.status,
                Feedback::Share("No valid Mahjong deal code in the clipboard.")
            ));
            assert_eq!(view.board.layout(), Layout::Atrium);
            assert_eq!(view.seed, 1);
            assert!(view.attempt == old_attempt);
            assert_eq!(saved_round(view), old_save);
        })
        .unwrap();

    visual.write_to_clipboard(ClipboardItem::new_string(format!("\n{code}\t")));
    play(&mut visual);
    recipient
        .update(cx, |view, window, _| {
            assert!(matches!(
                &view.status,
                Feedback::Share("Deal started from clipboard.")
            ));
            assert_eq!(view.board.layout(), layout);
            assert_eq!(view.seed, seed);
            assert_eq!(tile_signature(&view.board), tiles);
            assert_eq!(view.board.remaining(), layout.tile_count());
            assert_eq!(view.board.selected(), None);
            assert!(view.board.matched_pairs().is_empty());
            assert!(!view.board.used_assistance());
            assert!(view.highlighted.is_none());
            assert!(view.clean_clear);
            assert!(view.attempt != old_attempt);
            assert!(view.focus.is_focused(window));
            let saved = view.session.load::<SavedRound>().unwrap();
            assert_eq!(saved.layout, layout.id());
            assert_eq!(saved.seed, seed);
            assert!(saved.matches.is_empty());
            assert!(!saved.assisted);
            assert!(saved.clean_clear);
            assert!(saved.attempt == view.attempt);
        })
        .unwrap();
    source
        .read_with(cx, |view, _| assert_eq!(saved_round(view), source_save))
        .unwrap();
}

#[gpui::test]
fn daily_deals_have_no_clipboard_controls(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("mahjong-daily-clipboard-test".into()),
        entry: GameEntry::Challenge("fixture-mahjong-001".into()),
    };
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| GameView::new(&launch, cx));
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.debug_bounds("mahjong-restart").is_some());
    assert!(visual.debug_bounds("mahjong-copy-deal").is_none());
    assert!(visual.debug_bounds("mahjong-play-copied").is_none());
}
