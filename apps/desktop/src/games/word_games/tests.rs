use corner_table_api::GameId;
use corner_table_word_games::Crossword;
use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, WindowHandle, point, size};

use super::*;

fn open(cx: &mut TestAppContext, mode: &'static str) -> WindowHandle<GameView> {
    let window = cx.open_window(size(px(760.), px(540.)), |window, cx| {
        t::apply(t::Appearance::Dark, window, cx);
        cx.set_reduce_motion(true);
        let launch = GameLaunch {
            game: GameId(format!("word-result-{mode}-test")),
            entry: GameEntry::Mode(mode.into()),
        };
        GameView::new(&launch, cx)
    });
    cx.run_until_parked();
    window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap();
    window
}

fn click(ui: &mut VisualTestContext, selector: &'static str) {
    let center = ui.debug_bounds(selector).expect(selector).center();
    ui.simulate_mouse_move(center, None, Modifiers::default());
    ui.simulate_click(center, Modifiers::default());
    ui.run_until_parked();
}

fn act(window: WindowHandle<GameView>, ui: &mut VisualTestContext, action: Action) {
    window
        .update(ui, |view, window, cx| view.act(action, window, cx))
        .unwrap();
    ui.run_until_parked();
}

fn result(
    window: WindowHandle<GameView>,
    ui: &mut VisualTestContext,
    outcome: Outcome,
    title: &str,
    stat: &str,
) {
    for id in ["word-complete-next", "word-replay"] {
        let bounds = ui.debug_bounds(id).expect(id);
        assert!(bounds.top() >= px(0.) && bounds.bottom() < px(540.));
        assert!(bounds.left() >= px(0.) && bounds.right() < px(760.));
    }
    window
        .update(ui, |view, window, _| {
            let (actual, heading, detail) = view.summary().unwrap();
            assert_eq!(actual, outcome);
            assert_eq!(heading, title);
            assert!(detail.contains(stat), "{detail}");
            assert!(view.result_focus.is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn rack_and_grove_show_the_score_when_the_round_actually_ends(cx: &mut TestAppContext) {
    let window = open(cx, "letter-rack");
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    ui.simulate_keystrokes("g a r d e n enter");
    act(window, &mut ui, Action::FinishWords);
    assert!(ui.debug_bounds("word-confirm").is_some());
    assert!(ui.debug_bounds("word-replay").is_none());
    act(window, &mut ui, Action::ConfirmFinish);
    result(
        window,
        &mut ui,
        Outcome::Finished,
        "Round ended",
        "1 word found",
    );
    click(&mut ui, "word-replay");
    assert!(ui.debug_bounds("word-replay").is_none());

    act(window, &mut ui, Action::Variant(Variant::Grove));
    loop {
        let path = window
            .read_with(&ui, |view, _| {
                let Play::Words(play) = view.play() else {
                    unreachable!()
                };
                assert!(!play.round.finished());
                (play.round.score() < WORD_GOAL).then(|| {
                    let word = play
                        .round
                        .familiar_remaining()
                        .max_by_key(|word| word.len())
                        .unwrap();
                    play.round.path_for(word).unwrap()
                })
            })
            .unwrap();
        let Some(path) = path else { break };
        for tile in path {
            act(window, &mut ui, Action::Tile(tile));
        }
        ui.simulate_keystrokes("enter");
    }
    assert!(ui.debug_bounds("word-replay").is_none());
    act(window, &mut ui, Action::FinishWords);
    result(window, &mut ui, Outcome::Won, "Goal reached", "points");
    click(&mut ui, "word-complete-next");
    window
        .update(&mut ui, |view, window, _| {
            assert_eq!(view.current.variant, Variant::Grove);
            assert_eq!(view.current.source, Source::Collection(1));
            assert!(view.summary().is_none());
            assert!(view.focus.is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn crossword_solve_and_reveal_have_distinct_results_at_compact_width(cx: &mut TestAppContext) {
    let window = open(cx, "mini-crossword");
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    ui.simulate_keystrokes("c a t o r e d e n");
    result(window, &mut ui, Outcome::Won, "Crossword solved", "9 edits");
    window
        .update(&mut ui, |view, window, cx| {
            assert!(view.crossword_solved_at.is_none());
            window.focus_next(cx);
            assert!(!view.result_focus.is_focused(window));
        })
        .unwrap();
    click(&mut ui, "word-replay");
    assert!(ui.debug_bounds("word-replay").is_none());

    act(window, &mut ui, Action::Variant(Variant::Large));
    let letter = window
        .read_with(&ui, |view, _| {
            let mut solution =
                Crossword::with_size(view.current.variant.grid().unwrap(), view.current.seed());
            solution.reveal();
            char::from(solution.letters()[0].unwrap())
                .to_ascii_lowercase()
                .to_string()
        })
        .unwrap();
    ui.simulate_keystrokes(&letter);
    act(window, &mut ui, Action::HintSquare);
    act(window, &mut ui, Action::RevealCrossword);
    assert!(ui.debug_bounds("word-replay").is_none());
    act(window, &mut ui, Action::ConfirmReveal);
    result(
        window,
        &mut ui,
        Outcome::Finished,
        "Solution shown",
        "1 of 16 squares solved before reveal · 1 hint used",
    );
    window
        .read_with(&ui, |view, _| {
            assert!(view.play().result().is_none());
            let restored = Play::load(view.current, Some(view.play().save()));
            let Play::Crossword(restored) = restored else {
                unreachable!()
            };
            assert_eq!(restored.correct_before_reveal, Some(1));
        })
        .unwrap();
}

#[gpui::test]
fn search_result_and_next_puzzle_work_after_a_reversed_drag(cx: &mut TestAppContext) {
    let window = open(cx, "word-search");
    let (side, line, words) = window
        .update(cx, |view, _, cx| {
            let Play::Search(play) = view.play_mut() else {
                unreachable!()
            };
            let theme = play.board.theme();
            let last = theme.words.len() - 1;
            play.hint();
            for word in 0..last {
                let line = play.board.line(word);
                play.click(line.start);
                play.click(line.end);
            }
            cx.notify();
            (theme.side, play.board.line(last), theme.words.len())
        })
        .unwrap();
    cx.run_until_parked();
    let mut ui = VisualTestContext::from_window(window.into(), cx);
    let grid = ui.debug_bounds("word-search-grid").unwrap();
    let step = f32::from(grid.size.width) / side as f32;
    let center = |index: usize| {
        grid.origin
            + point(
                px((index % side) as f32 * step + step / 2.),
                px((index / side) as f32 * step + step / 2.),
            )
    };
    let (start, end) = (center(line.end), center(line.start));
    let modifiers = Modifiers::default();
    ui.simulate_mouse_move(start, None, modifiers);
    ui.simulate_mouse_down(start, MouseButton::Left, modifiers);
    ui.simulate_mouse_move(end, Some(MouseButton::Left), modifiers);
    assert!(ui.debug_bounds("word-replay").is_none());
    ui.simulate_mouse_up(end, MouseButton::Left, modifiers);
    ui.run_until_parked();
    result(
        window,
        &mut ui,
        Outcome::Won,
        "All words found",
        &format!("{words} words · {} moves · 1 hint used", words + 1),
    );
    window
        .read_with(&ui, |view, _| {
            let Play::Search(play) = view.play() else {
                unreachable!()
            };
            assert!(play.latest_mark.is_none());
        })
        .unwrap();
    click(&mut ui, "word-complete-next");
    assert!(ui.debug_bounds("word-replay").is_none());
    act(window, &mut ui, Action::Puzzle(0));
    result(
        window,
        &mut ui,
        Outcome::Won,
        "All words found",
        "1 hint used",
    );
    click(&mut ui, "word-replay");
    assert!(ui.debug_bounds("word-replay").is_none());
}
