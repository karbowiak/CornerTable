use super::*;

#[test]
fn pointer_jitter_can_click_but_dragging_never_clicks_on_release() {
    let start = point(px(90.), px(90.));
    let mut press = Press::new(start, Some(4));
    assert_eq!(press.advance(point(px(94.), px(92.))), (0., 0.));
    assert!(!press.moved);
    let (yaw, pitch) = press.advance(point(px(102.), px(98.)));
    assert!(yaw > 0. && yaw < 10.);
    assert!(pitch > 0. && pitch < 5.);
    assert_eq!(press.advance(point(px(102.), px(98.))), (0., 0.));
    assert_eq!(press.advance(start), (-yaw, -pitch));
    assert!(press.moved);

    let mut press = Press::new(start, Some(4));
    let (yaw, pitch) = press.advance(point(px(90.), px(117.)));
    assert_eq!(yaw, 0.);
    assert!(pitch > 10.);
    assert!(press.moved);
}

#[test]
fn saved_moves_replay_and_previous_daily_does_not_carry_over() {
    let yesterday = Table::Daily(1234);
    let (mut board, seed, attempt, _, _) = load_round(yesterday, None);
    let pair = board.hint().unwrap();
    assert!(board.remove_pair(pair));
    let save = RoundProgress {
        seed,
        attempt,
        moves: vec![pair],
        cleared: true,
        orientation: Orbit::new(117., 59.),
    };
    let (restored, resumed_seed, resumed_attempt, cleared, orientation) =
        load_round(yesterday, Some(&save));
    assert_eq!(resumed_seed, seed);
    assert!(resumed_attempt == attempt);
    assert!(cleared);
    assert!(restored.cubes().eq(board.cubes()));
    assert_eq!(orientation, save.orientation);

    let (today, todays_seed, todays_attempt, cleared, orientation) =
        load_round(Table::Daily(1235), Some(&save));
    assert_ne!(todays_seed, seed);
    assert!(todays_attempt != attempt);
    assert_ne!(today.shape(), board.shape());
    assert!(!cleared);
    assert!(today.moves().is_empty());
    assert_eq!(orientation, Orbit::default());
}

#[test]
fn legacy_round_replays_with_the_opening_angle() {
    let table = Table::Layout(Layout::Terrace);
    let seed = 42;
    let pair = table.deal(seed).hint().unwrap();
    let mut legacy = serde_json::json!({
        "version": 1,
        "active": "terrace",
        "rounds": { "terrace": { "seed": seed, "moves": [pair], "cleared": false } }
    });
    let save: Save = serde_json::from_value(legacy.clone()).unwrap();
    let (restored, _, _, _, orientation) = load_round(table, save.rounds.get(table.key()));
    assert_eq!(restored.moves(), &[pair]);
    assert_eq!(orientation, Orbit::default());

    legacy["rounds"]["terrace"]["orientation"] = serde_json::json!({ "yaw": 731., "pitch": 120. });
    let save: Save = serde_json::from_value(legacy).unwrap();
    let (restored, _, _, _, orientation) = load_round(table, save.rounds.get(table.key()));
    assert_eq!(restored.moves(), &[pair]);
    assert_eq!(orientation, Orbit::new(11., 82.));
}

#[test]
fn corrupt_replay_cannot_partially_remove_cubes() {
    let table = Table::Daily(1234);
    let (board, seed, attempt, _, _) = load_round(table, None);
    let pair = board.hint().unwrap();
    let corrupt = RoundProgress {
        seed,
        attempt,
        moves: vec![pair, pair],
        cleared: true,
        orientation: Orbit::new(270., 60.),
    };
    let (restored, _, restored_attempt, cleared, orientation) = load_round(table, Some(&corrupt));
    assert!(restored_attempt != attempt);
    assert_eq!(restored.remaining(), board.remaining());
    assert!(restored.moves().is_empty());
    assert!(!cleared);
    assert_eq!(orientation, Orbit::default());
}

#[cfg(debug_assertions)]
#[gpui::test]
fn daily_clear_can_be_undone_and_replayed_from_the_keyboard(cx: &mut gpui::TestAppContext) {
    use gpui::{KeyUpEvent, Keystroke, VisualTestContext, size};

    cx.update(|cx| cx.set_reduce_motion(true));
    let launch = GameLaunch {
        game: corner_table_api::GameId("cubes-daily-result-test".into()),
        entry: GameEntry::Classic,
    };
    let table = daily();
    let (mut board, seed, attempt, _, _) = load_round(table, None);
    while board.remaining() > 2 {
        assert!(board.remove_pair(board.hint().unwrap()));
    }
    let total = board.total_pairs();
    let window = cx.open_window(size(px(760.), px(540.)), |window, cx| {
        let mut view = GameView::new(&launch, cx);
        view.table = table;
        view.seed = seed;
        view.attempt = attempt;
        view.board = board;
        view.cleared = false;
        view.orientation = Orbit::new(97., 72.);
        view.board_focus.focus(window, cx);
        view
    });
    cx.run_until_parked();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    assert!(visual.debug_bounds("cubes-hint").is_some());
    visual.simulate_keystrokes("h enter h enter");
    let orbit = window
        .read_with(cx, |view, _| {
            assert_eq!(view.board.remaining(), 0);
            assert_eq!(view.board.moves().len(), total);
            assert!(view.cleared);
            assert!(view.fading.is_none());
            view.orientation
        })
        .unwrap();
    assert!(visual.debug_bounds("cubes-hint").is_none());
    assert!(visual.debug_bounds("cubes-result-undo").is_some());
    assert!(visual.debug_bounds("cubes-result-new").is_none());

    visual.simulate_keystrokes("u");
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.board.remaining(), 2);
            assert!(view.attempt == attempt);
            assert!(view.cleared);
        })
        .unwrap();
    assert!(visual.debug_bounds("cubes-result-replay").is_none());
    visual.simulate_keystrokes("h enter h enter");
    window
        .update(cx, |_, window, cx| window.focus_next(cx))
        .unwrap();
    visual.simulate_keystrokes("enter");
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    window
        .update(cx, |view, window, _| {
            assert!(view.board_focus.is_focused(window));
            assert_eq!(view.board.remaining(), total * 2);
            assert_eq!(view.table.fixed_seed(), Some(seed));
            assert_eq!(view.seed, seed);
            assert_eq!(view.orientation, orbit);
            assert!(view.attempt != attempt);
            assert!(view.cleared);
        })
        .unwrap();
    assert!(visual.debug_bounds("cubes-hint").is_some());
}

#[cfg(debug_assertions)]
#[gpui::test]
fn dragging_and_picking_work_in_wide_and_compact_windows(cx: &mut gpui::TestAppContext) {
    use gpui::{Modifiers, VisualTestContext, size};

    for (width, height, layout) in [
        (1080., 760., Layout::Gatehouse),
        (632., 540., Layout::Terrace),
    ] {
        let launch = GameLaunch {
            game: corner_table_api::GameId(format!("cubes-orbit-test-{width}")),
            entry: GameEntry::Classic,
        };
        let window = cx.open_window(size(px(width), px(height)), |_, cx| {
            let mut view = GameView::new(&launch, cx);
            view.table = Table::Layout(layout);
            view.seed = 42;
            view.board = view.table.deal(view.seed);
            view.orientation = Orbit::default();
            view
        });
        cx.run_until_parked();
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        let bounds = visual.debug_bounds("cube-table").unwrap().inset(px(1.));
        let free_point = |view: &GameView| {
            let scene = Scene::new(&view.board, bounds, view.orientation);
            view.board
                .cubes()
                .filter_map(|(id, _)| view.board.is_free(id).then_some(id))
                .find_map(|id| scene.visible_cube_point(id).map(|point| (id, point)))
                .unwrap()
        };
        let (_, start) = window.read_with(cx, |view, _| free_point(view)).unwrap();
        let pointer = Modifiers::none();
        let offset = point(px(28.), px(31.));
        visual.simulate_mouse_move(start, None, pointer);
        visual.simulate_mouse_down(start, MouseButton::Left, pointer);
        visual.simulate_mouse_move(start + offset, MouseButton::Left, pointer);
        let orbit = Orbit::default().offset(28. * 0.58, 31. * 0.42);
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.orientation, orbit);
                assert!(view.press.unwrap().moved);
                assert!(view.board.selected().is_none());
            })
            .unwrap();
        visual.simulate_mouse_up(start + offset, MouseButton::Left, pointer);
        assert_eq!(
            Session::new(&launch).load::<Save>().unwrap().rounds[layout.id()].orientation,
            orbit
        );

        let (id, position) = window.read_with(cx, |view, _| free_point(view)).unwrap();
        visual.simulate_click(position, pointer);
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.board.selected(), Some(id));
                assert!(view.press.is_none());
            })
            .unwrap();

        visual.simulate_keystrokes("escape q shift-up right");
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.orientation, orbit.offset(-90., 7.));
                assert!(view.keyed.is_some_and(|id| view.board.is_free(id)));
            })
            .unwrap();
        visual.simulate_keystrokes("h enter h enter");
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.board.moves().len(), 1);
            })
            .unwrap();
        visual.simulate_keystrokes("u");
        window
            .read_with(cx, |view, _| assert!(view.board.moves().is_empty()))
            .unwrap();

        let reset = visual.debug_bounds("cubes-reset-view").unwrap().center();
        visual.simulate_click(reset, pointer);
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.orientation, Orbit::default());
            })
            .unwrap();
    }
}
