use corner_table_api::GameId;
use gpui::{Modifiers, TestAppContext, VisualTestContext, size};

use super::*;
use crate::{games::GameEntry, storage};

const CENTER_FINISH: [[u8; 2]; 31] = [
    [10, 24],
    [15, 17],
    [2, 16],
    [4, 2],
    [17, 15],
    [14, 16],
    [18, 4],
    [20, 18],
    [23, 9],
    [2, 16],
    [21, 23],
    [23, 9],
    [25, 11],
    [4, 18],
    [27, 25],
    [25, 11],
    [37, 23],
    [28, 30],
    [30, 16],
    [9, 23],
    [23, 25],
    [32, 18],
    [11, 25],
    [34, 32],
    [31, 33],
    [46, 32],
    [25, 39],
    [44, 46],
    [46, 32],
    [33, 31],
    [38, 24],
];

#[gpui::test]
fn first_peg_lifts_and_settles_in_the_center(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("peg-solitaire".into()),
        entry: GameEntry::Challenge("peg-flight-test".into()),
    };
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| GameView::new(&launch, cx));
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    let draw = |ui: &mut VisualTestContext| {
        ui.update(|window, cx| {
            window.simulate_next_frame(cx);
            window.draw(cx).clear(cx);
        });
    };
    window
        .update(&mut ui.cx, |view, _, cx| {
            view.activate(Hole::from_index(10).unwrap(), cx);
            view.activate(Hole::CENTER, cx);
            assert!(view.position().occupied(Hole::CENTER));
        })
        .unwrap();
    draw(&mut ui);
    let start = ui.debug_bounds("peg-solitaire-flight").unwrap();
    ui.cx.executor().advance_clock(JUMP_DURATION / 2);
    draw(&mut ui);
    let middle = ui.debug_bounds("peg-solitaire-flight").unwrap();
    let landing = ui.debug_bounds("peg-solitaire-hole-D4").unwrap();
    assert_eq!(start.left(), middle.left());
    assert!(start.top() < middle.top() && middle.top() < landing.top());
    ui.cx.executor().advance_clock(JUMP_DURATION);
    draw(&mut ui);
    assert!(ui.debug_bounds("peg-solitaire-flight").is_none());
}

#[gpui::test]
fn peg_solitaire_compact_input_resume_and_results(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let launch = GameLaunch {
        game: GameId("peg-solitaire".into()),
        entry: GameEntry::Challenge("peg-solitaire-controls-test".into()),
    };
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| GameView::new(&launch, cx));
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    ui.run_until_parked();

    let board = ui.debug_bounds("peg-solitaire-board").unwrap();
    let rail = ui.debug_bounds("peg-solitaire-rail").unwrap();
    assert!(board.right() < rail.left());
    assert!(rail.right() < px(760.) && board.bottom() < px(540.));
    assert!(ui.debug_bounds("peg-solitaire-hole-A1").is_none());

    let source = ui.debug_bounds("peg-solitaire-hole-D2").unwrap();
    let target = ui.debug_bounds("peg-solitaire-hole-D4").unwrap();
    ui.update(|window, cx| window.simulate_next_frame(cx));
    ui.simulate_keystrokes("h");
    window
        .read_with(&ui.cx, |view, _| {
            assert_eq!(view.hint.unwrap().to, Hole::CENTER);
            assert_eq!(view.position().peg_count(), 32);
        })
        .unwrap();
    ui.simulate_click(source.center(), Modifiers::none());
    ui.simulate_click(target.center(), Modifiers::none());
    ui.run_until_parked();
    assert_eq!(
        Session::new(&launch).load::<Saved>().unwrap().jumps,
        vec![[10, 24]]
    );

    let resumed = ui.cx.open_window(size(px(760.), px(540.)), |window, cx| {
        let view = GameView::new(&launch, cx);
        view.focus.focus(window, cx);
        view
    });
    let mut ui = VisualTestContext::from_window(resumed.into(), &mut ui.cx);
    ui.run_until_parked();
    resumed
        .read_with(&ui.cx, |view, _| {
            assert_eq!(view.position().peg_count(), 31)
        })
        .unwrap();
    ui.simulate_keystrokes("u enter down down enter");
    resumed
        .read_with(&ui.cx, |view, _| assert_eq!(view.round.history().len(), 1))
        .unwrap();

    resumed
        .update(&mut ui.cx, |view, _, cx| {
            while let Some(jump) = view.position().jumps().next() {
                view.activate(jump.from, cx);
                view.activate(jump.to, cx);
            }
            assert_eq!(view.position().status(), Status::Stuck);
        })
        .unwrap();
    ui.run_until_parked();
    assert!(ui.debug_bounds("peg-solitaire-result").is_some());
    let result = storage::recent_results(10)
        .into_iter()
        .find(|result| result.launch.as_ref() == Some(&launch))
        .unwrap();
    assert_eq!(result.outcome, Outcome::Lost);

    resumed
        .update(&mut ui.cx, |view, window, cx| view.replay(window, cx))
        .unwrap();
    assert!(
        Session::new(&launch)
            .load::<Saved>()
            .unwrap()
            .jumps
            .is_empty()
    );
}

#[gpui::test]
fn peg_solitaire_center_finish_has_a_distinct_result(cx: &mut TestAppContext) {
    let launch = GameLaunch {
        game: GameId("peg-solitaire".into()),
        entry: GameEntry::Challenge("peg-solitaire-center-test".into()),
    };
    let mut jumps = CENTER_FINISH.to_vec();
    let [from, to] = jumps.pop().unwrap();
    Session::new(&launch).save(&Saved {
        version: SAVE_VERSION,
        attempt: AttemptId::default(),
        jumps,
    });

    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| GameView::new(&launch, cx));
    window
        .update(cx, |view, _, cx| {
            assert_eq!(view.position().status(), Status::Playing);
            view.activate(Hole::from_index(from).unwrap(), cx);
            view.activate(Hole::from_index(to).unwrap(), cx);
            let (outcome, title, _) = result_copy(view.position()).unwrap();
            assert_eq!(outcome, Outcome::Won);
            assert_eq!(title, "Center finish");
        })
        .unwrap();

    let result = storage::recent_results(10)
        .into_iter()
        .find(|result| result.launch.as_ref() == Some(&launch))
        .unwrap();
    assert_eq!(result.outcome, Outcome::Won);
    assert_eq!(result.metric, ResultMetric::Moves(31));
}
