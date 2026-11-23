use corner_table_api::GameId;
use corner_table_bubble_shooter::{Point, SHOOTER};
use gpui::{Modifiers, TestAppContext, VisualTestContext, size};

use super::*;
use crate::games::GameEntry;

fn launch(entry: GameEntry) -> GameLaunch {
    let mut launch = GameLaunch {
        game: GameId("bubble-puzzle-test".into()),
        entry,
    };
    launch.game.0.push_str(&launch.seed().to_string());
    launch
}

fn click(cx: &mut VisualTestContext, id: &'static str) {
    let bounds = cx.debug_bounds(id).unwrap();
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn compact_puzzles_support_mouse_retry_and_keyboard_bank_shots(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let launch = launch(GameEntry::Classic);
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| GameView::new(&launch, cx));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    window
        .update(&mut cx.cx, |view, window, cx| view.focus.focus(window, cx))
        .unwrap();
    cx.simulate_keystrokes("space");
    click(&mut cx, "bubble-mode-puzzles");
    let stages = cx.debug_bounds("bubble-mode-stages").unwrap();
    let classic = cx.debug_bounds("bubble-mode-classic").unwrap();
    let puzzles = cx.debug_bounds("bubble-mode-puzzles").unwrap();
    assert!(stages.right() <= classic.left() && classic.right() <= puzzles.left());
    window
        .read_with(&cx.cx, |view, _| {
            assert_eq!(view.progress.mode(), Mode::Puzzles);
            assert_eq!(view.progress.round.shots_left(), 1);
        })
        .unwrap();

    let board = cx.debug_bounds("bubble-shooter-board").unwrap();
    let field = Playfield::fit(board.dilate(px(-2.)));
    let direction = Angle::from_ticks(80).unwrap().direction();
    cx.simulate_click(
        field.screen(Point {
            x: SHOOTER.x + 5. * direction.x,
            y: SHOOTER.y + 5. * direction.y,
        }),
        Modifiers::none(),
    );
    cx.run_until_parked();
    window
        .read_with(&cx.cx, |view, _| {
            assert_eq!(view.progress.round.status(), Status::BankGoalMissed);
            assert!(!view.progress.puzzle_solved(0));
        })
        .unwrap();

    click(&mut cx, "bubble-restart");
    cx.simulate_keystrokes(&["left"; 22].join(" "));
    cx.simulate_keystrokes("shift-left");
    window
        .read_with(&cx.cx, |view, _| {
            assert_eq!(view.angle.ticks(), -354);
            assert_eq!(view.aim.as_ref().unwrap().bounces(), 1);
        })
        .unwrap();
    cx.simulate_keystrokes("space");
    window
        .read_with(&cx.cx, |view, _| {
            assert_eq!(view.progress.round.status(), Status::Won);
            assert!(view.progress.puzzle_solved(0));
        })
        .unwrap();

    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes(&["right"; 22].join(" "));
    cx.simulate_keystrokes("space");
    let restored = Progress::new(&launch);
    assert_eq!(restored.mode(), Mode::Puzzles);
    assert_eq!(restored.puzzle(), 1);
    assert!(restored.puzzle_solved(0));
    assert_eq!(restored.round.shots_left(), 1);
    assert_eq!(restored.round.banked_matches(), 1);

    cx.simulate_keystrokes("r");
    window
        .read_with(&cx.cx, |view, _| {
            assert_eq!(view.progress.puzzle(), 1);
            assert_eq!(view.progress.round.shots_left(), 2);
            assert_eq!(view.progress.round.banked_matches(), 0);
            assert!(view.progress.puzzle_solved(0));
        })
        .unwrap();

    click(&mut cx, "bubble-puzzle-3");
    click(&mut cx, "bubble-mode-stages");
    window
        .read_with(&cx.cx, |view, _| {
            assert_eq!(view.progress.mode(), Mode::Stages);
            assert_eq!(view.progress.round.shots_used(), 1);
        })
        .unwrap();
    click(&mut cx, "bubble-mode-puzzles");
    window
        .read_with(&cx.cx, |view, _| {
            assert_eq!(view.progress.puzzle(), 3);
            assert_eq!(view.progress.round.shots_left(), 4);
        })
        .unwrap();
}

#[gpui::test]
fn puzzles_keep_flight_and_drop_effects_across_successive_shots(cx: &mut TestAppContext) {
    let launch = launch(GameEntry::Mode("puzzles".into()));
    let window = cx.open_window(size(px(1380.), px(900.)), |window, cx| {
        t::apply(t::Appearance::Dark, window, cx);
        GameView::new(&launch, cx)
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    click(&mut cx, "bubble-puzzle-1");
    cx.simulate_keystrokes(&["right"; 22].join(" "));
    cx.simulate_keystrokes("space");
    window
        .read_with(&cx.cx, |view, _| {
            assert!(view.flight.is_some());
            assert_eq!(view.progress.round.shots_left(), 2);
        })
        .unwrap();

    // Flight uses a wall clock and caps its duration below half a second.
    std::thread::sleep(std::time::Duration::from_millis(475));
    window
        .update(&mut cx.cx, |view, _, cx| {
            view.animate(cx);
            assert!(view.flight.is_none());
            assert!(view.impact.is_some());
            assert_eq!(view.progress.round.banked_matches(), 1);
            assert_eq!(view.progress.round.shots_left(), 1);
        })
        .unwrap();

    cx.simulate_keystrokes("up");
    cx.simulate_keystrokes(&["left"; 22].join(" "));
    cx.simulate_keystrokes("space");
    window
        .read_with(&cx.cx, |view, _| {
            assert!(view.flight.is_some());
            assert!(view.impact.is_none());
        })
        .unwrap();
    cx.simulate_keystrokes("r");
    window
        .read_with(&cx.cx, |view, _| {
            assert!(view.flight.is_none());
            assert!(view.impact.is_none());
            assert_eq!(view.progress.round.shots_left(), 2);
        })
        .unwrap();
}
