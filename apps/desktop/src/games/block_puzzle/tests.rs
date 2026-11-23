use super::*;
use gpui::{point, size};

#[cfg(debug_assertions)]
#[gpui::test]
fn puzzle_results_follow_placement_and_preserve_undo(cx: &mut gpui::TestAppContext) {
    use corner_table_api::GameId;
    use gpui::{KeyUpEvent, Keystroke, Modifiers, VisualTestContext};

    let launch = GameLaunch {
        game: GameId("block-puzzle-result-test".into()),
        entry: super::super::GameEntry::Classic,
    };
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
        let mut view = GameView::new(&launch, cx);
        view.switch(Mode::PuzzleBoards);
        view
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    let attempt = window
        .update(&mut visual.cx, |view, window, cx| {
            view.focus.focus(window, cx);
            view.round().attempt
        })
        .unwrap();
    let place = |visual: &mut VisualTestContext| {
        visual.simulate_keystrokes("1 enter");
        visual.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
        });
    };
    place(&mut visual);
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.round_outcome(), Some(Outcome::Lost));
            let seat = view.seat.as_mut().expect("piece seats before the result");
            seat.started -= SEAT_DURATION;
            cx.notify();
        })
        .unwrap();
    visual.run_until_parked();
    let board = visual.debug_bounds("block-puzzle-board").unwrap();
    let undo = visual.debug_bounds("block-puzzle-undo").unwrap();
    assert!(undo.is_contained_within(&board));
    visual.simulate_click(undo.center(), Modifiers::none());
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.round_outcome(), None);
            assert!(view.round().attempt == attempt);
            assert!(view.round().turns.is_empty());
            view.cursor = Anchor { column: 4, row: 3 };
            cx.notify();
        })
        .unwrap();
    place(&mut visual);
    assert!(visual.debug_bounds("block-puzzle-next").is_none());
    window
        .update(&mut visual.cx, |view, _, cx| {
            assert_eq!(view.round_outcome(), Some(Outcome::Won));
            assert!(view.round().attempt == attempt);
            view.clear
                .as_mut()
                .expect("crossing clears two lines")
                .started -= Duration::from_secs(1);
            cx.notify();
        })
        .unwrap();
    visual.run_until_parked();
    let next = visual.debug_bounds("block-puzzle-next").unwrap();
    assert!(next.is_contained_within(&board));
    visual.simulate_click(next.center(), Modifiers::none());
    window
        .read_with(&visual.cx, |view, _| assert_eq!(view.puzzles.active, 1))
        .unwrap();
}

#[test]
fn play_layout_fits_the_board_and_tray_in_supported_windows() {
    let roomy = PlayLayout::fit(1056., 900.);
    assert!((600. ..=670.).contains(&roomy.board));
    assert!(!roomy.stacked);
    assert!(roomy.panel_width() <= 1092.);

    let short = PlayLayout::fit(1056., 540.);
    assert!(short.board + 112. <= 540.);

    let compact = PlayLayout::fit(506., 540.);
    assert!(!compact.stacked);
    assert!(compact.cell >= 28.);
    assert!(compact.controls < 230.);
    assert!(compact.panel_width() <= 542.);

    let stacked = PlayLayout::fit(420., 540.);
    assert!(stacked.stacked);
    assert_eq!(stacked.controls, stacked.board);
}

#[test]
fn original_replay_restores_score_tray_and_undo() {
    let json = r#"{"seed":0,"turns":[[0,0,0]],"can_undo":true}"#;
    let mut saved: SavedSession = serde_json::from_str(json).unwrap();
    assert!(saved.mode == Mode::Classic);
    assert!(saved.ten_trays.is_none());

    saved.ten_trays = Some(SeededRound::new(Game::ten_trays, CHALLENGE_SEED).saved());
    saved.mode = Mode::PuzzleBoards;
    let encoded = serde_json::to_string(&saved).unwrap();
    let payload: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(payload["seed"], 0);
    assert_eq!(payload["turns"], serde_json::json!([[0, 0, 0]]));
    assert_eq!(payload["can_undo"], true);
    assert_eq!(payload["ten_trays"]["seed"], CHALLENGE_SEED);
    let restored: SavedSession = serde_json::from_str(&encoded).unwrap();
    assert!(restored.classic_attempt == saved.classic_attempt);
    assert!(restored.ten_trays_attempt == saved.ten_trays_attempt);

    let seeded =
        SeededRound::restore(Game::new, restored.classic, restored.classic_attempt).unwrap();
    let round = seeded.round;
    assert!(round.attempt == saved.classic_attempt);
    assert_eq!(round.game.score(), 3);
    assert_eq!(round.game.board()[0][1], Some(Wood::Plum));
    assert_eq!(round.game.board()[1][0], Some(Wood::Plum));
    assert_eq!(round.game.tray()[0], None);
    assert_eq!(seeded.best, 3);
    assert_eq!(round.undo, Some(Game::new(0)));
}

#[test]
fn existing_ten_trays_journal_keeps_its_seed_score_and_undo() {
    let legacy = serde_json::json!({
        "seed": 712,
        "turns": [],
        "can_undo": false,
        "mode": "ten-trays",
        "ten_trays": {
            "seed": CHALLENGE_SEED,
            "turns": [[2, 0, 0], [1, 0, 4]],
            "can_undo": true,
            "best": 23
        }
    });
    let saved: SavedSession = serde_json::from_value(legacy).unwrap();
    assert!(saved.mode == Mode::TenTrays);
    let round = SeededRound::restore(
        Game::ten_trays,
        saved.ten_trays.unwrap(),
        saved.ten_trays_attempt,
    )
    .unwrap();
    let mut expected = Game::ten_trays(CHALLENGE_SEED);
    expected
        .place(Move {
            slot: 2,
            column: 0,
            row: 0,
        })
        .unwrap();
    assert_eq!(round.round.undo.as_ref(), Some(&expected));
    expected
        .place(Move {
            slot: 1,
            column: 0,
            row: 4,
        })
        .unwrap();
    assert_eq!(round.seed, CHALLENGE_SEED);
    assert_eq!(round.best, 23);
    assert_eq!(round.round.game, expected);
    assert_eq!(round.round.game.placements_left(), Some(28));
}

#[test]
fn drop_uses_grabbed_tile_at_both_board_edges() {
    let cell = 30.;
    let side = SIDE as f32 * (cell + CELL_GAP) - CELL_GAP;
    let grid = Grid {
        bounds: Bounds::new(point(px(12.), px(20.)), size(px(side), px(side))),
        cell,
    };
    let piece = Piece {
        wood: Wood::Cherry,
        tiles: &[(0, 0), (1, 0)],
    };
    let grabbed = closest_tile(piece, point(px(8.), px(0.)), 16.);
    assert_eq!(grabbed, (1, 0));

    let pointer = grid.bounds.origin + point(px(side - 0.1), px(side - 0.1));
    let target = grid.anchor_at(pointer).unwrap();
    assert_eq!(target, Anchor { column: 7, row: 7 });
    assert_eq!(
        target.offset((-(grabbed.0 as i32), 0)).turn(2),
        Some(Move {
            slot: 2,
            column: 6,
            row: 7,
        })
    );

    let target = grid.anchor_at(grid.bounds.origin).unwrap();
    let anchor = target.offset((-(grabbed.0 as i32), 0));
    assert_eq!(anchor.turn(2), None);
    assert!(
        Preview {
            anchor,
            piece,
            fits: false
        }
        .covers(0, 0)
    );

    let outside = grid.bounds.origin + point(px(side), px(side));
    assert_eq!(grid.anchor_at(outside), None);
}

#[test]
fn invalid_preview_marks_anchor_when_a_shape_is_entirely_off_board() {
    let preview = Preview {
        anchor: Anchor { column: 7, row: 7 },
        piece: Piece {
            wood: Wood::Plum,
            tiles: &[(1, 0), (0, 1), (1, 1)],
        },
        fits: false,
    };
    assert!(preview.covers(7, 7));
    assert!(!preview.covers(6, 7));
}
