use corner_table_api::GameId;
use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::*;

#[gpui::test]
fn blocked_well_finishes_impact_before_offering_retry(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("gem-cascade-result-test".into()),
        entry: GameEntry::Classic,
    };
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
        let mut view = GameView::new(&launch, cx);
        view.mode = Mode::Practice;
        view.phase = Phase::Playing;
        let mut run = Run::new(14);
        loop {
            let mut after = run.game.clone();
            after.lock_pair().unwrap();
            if after.is_over() {
                break;
            }
            run.play(Move::Drop).unwrap();
        }
        view.runs[Mode::Practice.index()] = run;
        view
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    window
        .update(&mut visual.cx, |view, window, cx| {
            view.focus.focus(window, cx)
        })
        .unwrap();
    visual.simulate_keystrokes("space");
    assert!(visual.debug_bounds("gem-overlay-primary").is_none());
    let attempt = window
        .update(&mut visual.cx, |view, _, cx| {
            assert!(view.run().game.is_over());
            assert!(view.overlay(cx).is_none());
            assert!(view.motion.is_some() || view.cascade.is_some());
            cx.set_reduce_motion(true);
            cx.notify();
            view.run().record.attempt
        })
        .unwrap();
    visual.run_until_parked();
    let board = visual.debug_bounds("gem-cascade-board").unwrap();
    let retry = visual.debug_bounds("gem-overlay-primary").unwrap();
    let new_deal = visual.debug_bounds("gem-new-deal").unwrap();
    assert!(retry.is_contained_within(&board) && new_deal.is_contained_within(&board));
    visual.simulate_click(retry.center(), Modifiers::none());
    window
        .read_with(&visual.cx, |view, _| {
            assert_eq!(view.mode, Mode::Practice);
            assert_eq!(view.run().game.pairs(), 0);
            assert!(view.run().record.attempt != attempt);
        })
        .unwrap();
}
