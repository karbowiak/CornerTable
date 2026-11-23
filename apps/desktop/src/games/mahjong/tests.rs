use corner_table_api::GameId;
use gpui::{KeyUpEvent, Keystroke, TestAppContext, VisualTestContext, size};

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
