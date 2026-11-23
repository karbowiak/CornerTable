use std::time::Duration;

use corner_table_api::GameId;
use corner_table_solitaire::TableCard;
use gpui::{
    Bounds, KeyUpEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels,
    TestAppContext, VisualTestContext, WindowHandle, size,
};

use super::*;

fn open(
    cx: &mut TestAppContext,
    mode: DealMode,
    seed: u64,
) -> (WindowHandle<GameView>, VisualTestContext) {
    let variant = Game::with_mode(mode, seed).variant;
    let launch = GameLaunch {
        game: GameId(format!("solitaire-deal-motion-{mode:?}")),
        entry: GameEntry::Mode(variant.title().to_ascii_lowercase()),
    };
    // The minimum window, plus the shell's content-card margin the table no longer gets.
    let window = cx.open_window(size(px(768.), px(540.)), |_, cx| {
        let mut view = GameView::new(&launch, cx);
        view.switch_mode(mode);
        view.restart(Some(seed));
        view
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    draw(&mut visual);
    (window, visual)
}

fn draw(visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
}

fn advance(visual: &mut VisualTestContext, milliseconds: u64) {
    visual
        .cx
        .executor()
        .advance_clock(Duration::from_millis(milliseconds));
    draw(visual);
}

fn click(visual: &mut VisualTestContext, id: &'static str) -> Bounds<Pixels> {
    let bounds = visual.debug_bounds(id).unwrap();
    visual.simulate_click(bounds.center(), Modifiers::none());
    draw(visual);
    bounds
}

fn press(visual: &mut VisualTestContext, keys: &'static str) {
    for key in keys.split_whitespace() {
        visual.simulate_keystrokes(key);
        visual.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        });
    }
    draw(visual);
}

fn focused(window: WindowHandle<GameView>, cx: &mut TestAppContext) -> Option<Place> {
    window
        .update(cx, |view, window, _| {
            assert!(view.focus.is_focused(window));
            view.cursor.place()
        })
        .unwrap()
}

#[gpui::test]
fn double_click_sends_an_exposed_ace_to_its_foundation(cx: &mut TestAppContext) {
    let mode = DealMode::KlondikeDrawOne;
    let seed = (0..1000)
        .find(|&seed| Game::with_mode(mode, seed).tableau[0][0].card.rank == 1)
        .unwrap();
    let (window, mut visual) = open(cx, mode, seed);
    let position = click(&mut visual, "solitaire-slot-Tableau(0, 0)").center();
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.deal.selected, Some(Place::Tableau(0, 0)))
        })
        .unwrap();
    visual.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 2,
        ..Default::default()
    });
    visual.simulate_event(MouseUpEvent {
        position,
        button: MouseButton::Left,
        click_count: 2,
        modifiers: Modifiers::none(),
    });
    draw(&mut visual);
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.deal.game.foundation_count(), 1);
            assert!(view.deal.game.tableau[0].is_empty());
            assert!(view.deal.selected.is_none());
        })
        .unwrap();
}

#[gpui::test]
fn klondike_arrows_move_focus_without_selecting_and_undo_recovers_from_empty_waste(
    cx: &mut TestAppContext,
) {
    let mode = DealMode::KlondikeDrawThree;
    let (window, mut visual) = open(cx, mode, 18);
    assert_eq!(focused(window, cx), Some(Place::Stock));
    press(&mut visual, "enter right");
    assert!(visual.debug_bounds("solitaire-flight-0").is_some());
    assert_eq!(focused(window, cx), Some(Place::Foundation(0)));
    advance(&mut visual, 350);
    press(&mut visual, "left space down");
    assert!(matches!(focused(window, cx), Some(Place::Tableau(..))));
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.deal.selected, Some(Place::Waste))
        })
        .unwrap();
    press(&mut visual, "escape up");
    assert_eq!(focused(window, cx), Some(Place::Waste));
    press(&mut visual, "u h");
    assert_eq!(focused(window, cx), Some(Place::Stock));
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.deal.game, Game::with_mode(mode, 18));
            assert!(view.deal.selected.is_none());
            assert!(view.suggestion.is_some());
        })
        .unwrap();
}

#[gpui::test]
fn freecell_arrows_reach_cells_columns_and_foundations_with_one_board_tab_stop(
    cx: &mut TestAppContext,
) {
    let (window, mut visual) = open(cx, DealMode::FreeCell, 146);
    assert_eq!(focused(window, cx), Some(Place::Cell(0)));
    press(&mut visual, "right right right right");
    assert_eq!(focused(window, cx), Some(Place::Foundation(0)));
    press(&mut visual, "left left left left down enter up space");
    assert_eq!(focused(window, cx), Some(Place::Cell(0)));
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.deal.game.moves, 1);
            assert!(view.deal.game.cells[0].is_some());
            assert!(view.deal.selected.is_none());
        })
        .unwrap();
    press(&mut visual, "down right");
    let column = Some(Place::Tableau(1, 6));
    assert_eq!(focused(window, cx), column);
    window
        .update(cx, |view, window, cx| {
            window.focus_prev(cx);
            assert!(!view.focus.is_focused(window));
            window.focus_next(cx);
            assert!(view.focus.is_focused(window));
        })
        .unwrap();
    assert_eq!(focused(window, cx), column);
}

#[gpui::test]
fn spider_keyboard_deal_and_undo_keep_the_table_reachable_during_flights(cx: &mut TestAppContext) {
    let (window, mut visual) = open(cx, DealMode::SpiderOneSuit, 266);
    press(&mut visual, "down right space");
    let second_column = Some(Place::Tableau(1, 5));
    assert_eq!(focused(window, cx), second_column);
    window
        .read_with(cx, |view, _| assert_eq!(view.deal.selected, second_column))
        .unwrap();
    press(&mut visual, "d down");
    assert_eq!(focused(window, cx), Some(Place::Stock));
    assert!(visual.debug_bounds("solitaire-flight-0").is_some());
    advance(&mut visual, 470);
    press(&mut visual, "down right");
    assert_eq!(focused(window, cx), Some(Place::Tableau(1, 6)));
    press(&mut visual, "u");
    assert_eq!(focused(window, cx), second_column);
    window
        .read_with(cx, |view, _| assert_eq!(view.deal.game.moves, 0))
        .unwrap();
}

#[gpui::test]
fn pyramid_and_tripeaks_skip_covered_cards_and_refocus_after_clearing(cx: &mut TestAppContext) {
    let pair = Move::Transfer {
        from: Place::Layout(24),
        to: Place::Layout(25),
    };
    let clear = Move::Clear(Place::Layout(22));
    for (mode, action, keys, first) in [
        (
            DealMode::Pyramid,
            pair,
            "enter right space",
            Place::Layout(24),
        ),
        (DealMode::TriPeaks, clear, "space", Place::Layout(22)),
    ] {
        let (seed, expected) = (0..100)
            .find_map(|seed| {
                let mut game = Game::with_mode(mode, seed);
                game.play(action).then_some((seed, game))
            })
            .unwrap();
        let (window, mut visual) = open(cx, mode, seed);
        if mode == DealMode::TriPeaks {
            press(&mut visual, "right");
            assert_eq!(focused(window, cx), Some(Place::Waste));
            press(&mut visual, "space left");
            assert_eq!(focused(window, cx), Some(Place::Stock));
        }
        press(&mut visual, "up");
        assert_eq!(focused(window, cx), Some(first));
        press(&mut visual, keys);
        let fallback = focused(window, cx).unwrap();
        if mode == DealMode::Pyramid {
            assert_eq!(fallback, Place::Layout(18));
        } else {
            assert!(matches!(fallback, Place::Layout(21 | 23)));
        }
        assert!(expected.selectable(fallback));
        window
            .read_with(cx, |view, _| assert_eq!(view.deal.game, expected))
            .unwrap();
        press(&mut visual, "d");
        assert_eq!(focused(window, cx), Some(fallback));
        assert!(visual.debug_bounds("solitaire-flight-0").is_some());
    }
}

#[gpui::test]
fn a_keyboard_stock_draw_can_end_the_round_and_tab_to_recovery(cx: &mut TestAppContext) {
    let (window, mut visual) = open(cx, DealMode::Pyramid, 16);
    let before = window
        .update(cx, |view, _, cx| {
            while view.deal.game.stock.len() > 1 {
                assert!(view.deal.apply(Move::Draw));
            }
            cx.notify();
            view.deal.game.clone()
        })
        .unwrap();
    draw(&mut visual);
    press(&mut visual, "space");
    assert_eq!(focused(window, cx), None);
    assert!(visual.debug_bounds("solitaire-blocked").is_none());
    advance(&mut visual, 280);
    assert_eq!(focused(window, cx), None);
    assert!(visual.debug_bounds("solitaire-blocked").is_some());
    window
        .update(cx, |_, window, cx| window.focus_next(cx))
        .unwrap();
    press(&mut visual, "enter");
    assert_eq!(focused(window, cx), Some(Place::Stock));
    window
        .read_with(cx, |view, _| assert_eq!(view.deal.game, before))
        .unwrap();

    press(&mut visual, "enter");
    advance(&mut visual, 280);
    window
        .update(cx, |_, window, cx| {
            window.focus_next(cx);
            window.focus_next(cx);
        })
        .unwrap();
    press(&mut visual, "space");
    assert_eq!(focused(window, cx), Some(Place::Stock));
    window
        .read_with(cx, |view, _| {
            assert_eq!(view.deal.game, Game::with_mode(DealMode::Pyramid, 16))
        })
        .unwrap();
}

#[gpui::test]
fn modified_arrows_follow_history_from_a_focused_solitaire_card(cx: &mut TestAppContext) {
    use crate::app::{Action, Desktop, Page};
    use corner_table_api::FixtureApi;

    cx.update(crate::install_shortcuts);
    let launch = GameLaunch {
        game: GameId("solitaire".into()),
        entry: GameEntry::Mode("klondike".into()),
    };
    let page = Page::Game(launch);
    let window = cx.open_window(size(px(900.), px(660.)), |window, cx| {
        let desktop = Desktop::new(
            FixtureApi::load().unwrap(),
            t::Appearance::Light,
            window,
            cx,
        );
        desktop.focus.focus(window, cx);
        desktop
    });
    window
        .update(cx, |desktop, window, cx| {
            desktop.dispatch(Action::Navigate(page.clone()), window, cx);
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    draw(&mut visual);
    draw(&mut visual);
    press(&mut visual, "down h");
    assert!(visual.debug_bounds("solitaire-hint-source").is_some());
    assert_eq!(
        window
            .read_with(cx, |desktop, _| desktop.page.clone())
            .unwrap(),
        page
    );
    press(&mut visual, "alt-left");
    assert_eq!(
        window
            .read_with(cx, |desktop, _| desktop.page.clone())
            .unwrap(),
        Page::Library
    );
    press(&mut visual, "alt-right");
    assert_eq!(
        window
            .read_with(cx, |desktop, _| desktop.page.clone())
            .unwrap(),
        page
    );
}

fn near_destination(flight: Bounds<Pixels>, slot: Bounds<Pixels>) {
    assert!((flight.origin.x - slot.origin.x).abs() <= px(2.));
    assert!((flight.origin.y - slot.origin.y).abs() <= px(2.));
    assert_eq!(flight.size, slot.size);
}

#[test]
fn a_spider_run_stays_visible_until_its_dealt_ace_lands() {
    let mut before = Game::with_mode(DealMode::SpiderOneSuit, 266);
    before.tableau[0] = (2..=13)
        .rev()
        .map(|rank| TableCard {
            card: Card {
                suit: Suit::Spades,
                rank,
            },
            face_up: true,
        })
        .collect();
    *before.stock.last_mut().unwrap() = Card {
        suit: Suit::Spades,
        rank: 1,
    };
    let mut after = before.clone();
    assert!(after.play(Move::Draw));
    assert_eq!(after.runs, 1);
    assert!(after.tableau[0].is_empty());

    let started = std::time::Instant::now();
    let motion = DealMotion::from_draw(before, &after, started).unwrap();
    let landed = motion.frame(started + Duration::from_millis(270));
    assert_eq!(landed.previous_column(0).unwrap().len(), 12);
    let collected = motion.frame(started + Duration::from_millis(304));
    assert!(collected.previous_column(0).is_none());
}

#[gpui::test]
fn stock_cards_fly_to_each_waste_layout_and_reduced_motion_updates_immediately(
    cx: &mut TestAppContext,
) {
    for mode in [
        DealMode::KlondikeDrawOne,
        DealMode::KlondikeDrawThree,
        DealMode::Pyramid,
        DealMode::TriPeaks,
    ] {
        let (window, mut visual) = open(cx, mode, 18);
        let stock = click(&mut visual, "solitaire-slot-Stock");
        let mut expected = Game::with_mode(mode, 18);
        assert!(expected.play(Move::Draw));
        window
            .read_with(cx, |view, _| assert_eq!(view.deal.game, expected))
            .unwrap();

        let first = visual.debug_bounds("solitaire-flight-0").unwrap();
        assert_eq!(first, stock);
        advance(&mut visual, 120);
        let in_flight = visual.debug_bounds("solitaire-flight-0").unwrap();
        assert!(in_flight.origin.x > first.origin.x);
        assert!(in_flight.origin.y < first.origin.y);
        let last_flight = if mode == DealMode::KlondikeDrawThree {
            assert!(visual.debug_bounds("solitaire-flight-1").is_some());
            assert!(visual.debug_bounds("solitaire-flight-2").is_some());
            advance(&mut visual, 224);
            "solitaire-flight-2"
        } else {
            advance(&mut visual, 114);
            "solitaire-flight-0"
        };
        let landing = visual.debug_bounds(last_flight).unwrap();
        advance(&mut visual, 2);
        assert!(visual.debug_bounds(last_flight).is_none());
        near_destination(
            landing,
            visual.debug_bounds("solitaire-slot-Waste").unwrap(),
        );

        cx.update(|cx| cx.set_reduce_motion(true));
        click(&mut visual, "solitaire-slot-Stock");
        assert!(expected.play(Move::Draw));
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.deal.game, expected);
                assert!(view.deal_motion.is_none());
            })
            .unwrap();
        assert!(visual.debug_bounds("solitaire-slot-Waste").is_some());
        cx.update(|cx| cx.set_reduce_motion(false));
    }
}

#[gpui::test]
fn recycling_flies_the_waste_back_and_undo_clears_current_motion(cx: &mut TestAppContext) {
    let (window, mut visual) = open(cx, DealMode::KlondikeDrawThree, 18);
    let before = window
        .update(cx, |view, _, cx| {
            while !view.deal.game.stock.is_empty() {
                assert!(view.deal.apply(Move::Draw));
            }
            cx.notify();
            view.deal.game.clone()
        })
        .unwrap();
    draw(&mut visual);
    let waste = visual.debug_bounds("solitaire-slot-Waste").unwrap();
    let stock = click(&mut visual, "solitaire-slot-Stock");
    assert_eq!(visual.debug_bounds("solitaire-flight-0"), Some(waste));
    advance(&mut visual, 254);
    near_destination(visual.debug_bounds("solitaire-flight-0").unwrap(), stock);
    advance(&mut visual, 2);
    assert!(visual.debug_bounds("solitaire-flight-0").is_none());
    let mut recycled = before.clone();
    assert!(recycled.play(Move::Draw));

    click(&mut visual, "solitaire-slot-Stock");
    assert!(visual.debug_bounds("solitaire-flight-0").is_some());
    click(&mut visual, "solitaire-undo-button");
    assert!(visual.debug_bounds("solitaire-flight-0").is_none());
    window
        .read_with(cx, |view, _| assert_eq!(view.deal.game, recycled))
        .unwrap();
}

#[gpui::test]
fn the_last_stock_card_can_end_pyramid_and_tripeaks(cx: &mut TestAppContext) {
    for (mode, seed) in [(DealMode::Pyramid, 16), (DealMode::TriPeaks, 0)] {
        let (window, mut visual) = open(cx, mode, seed);
        let before = window
            .update(cx, |view, _, cx| {
                while view.deal.game.stock.len() > 1 {
                    assert!(view.deal.apply(Move::Draw));
                }
                cx.notify();
                view.deal.game.clone()
            })
            .unwrap();
        draw(&mut visual);
        assert!(visual.debug_bounds("solitaire-blocked").is_none());
        click(&mut visual, "solitaire-slot-Stock");
        assert!(visual.debug_bounds("solitaire-flight-0").is_some());
        assert!(visual.debug_bounds("solitaire-blocked").is_none());
        advance(&mut visual, 280);
        assert!(visual.debug_bounds("solitaire-blocked").is_some());
        click(&mut visual, "solitaire-undo-button");
        assert!(visual.debug_bounds("solitaire-blocked").is_none());
        window
            .read_with(cx, |view, _| assert_eq!(view.deal.game, before))
            .unwrap();
    }
}

#[gpui::test]
fn freecell_has_a_blocked_result_without_a_stock(cx: &mut TestAppContext) {
    let (window, mut visual) = open(cx, DealMode::FreeCell, 146);
    let before = window
        .update(cx, |view, _, cx| {
            for column in 0..3 {
                assert!(view.deal.apply(Move::Transfer {
                    from: Place::Tableau(column, 6),
                    to: Place::Cell(column),
                }));
            }
            cx.notify();
            view.deal.game.clone()
        })
        .unwrap();
    draw(&mut visual);
    assert!(visual.debug_bounds("solitaire-slot-Stock").is_none());
    assert!(visual.debug_bounds("solitaire-blocked").is_none());
    click(&mut visual, "solitaire-slot-Tableau(3, 6)");
    click(&mut visual, "solitaire-slot-Cell(3)");
    assert!(visual.debug_bounds("solitaire-blocked").is_some());
    assert!(visual.debug_bounds("solitaire-won").is_none());
    click(&mut visual, "solitaire-undo-button");
    window
        .read_with(cx, |view, _| assert_eq!(view.deal.game, before))
        .unwrap();
}

#[gpui::test]
fn spider_rows_arrive_in_their_columns_and_a_blocked_deal_can_undo_replay_or_replace(
    cx: &mut TestAppContext,
) {
    let mode = DealMode::SpiderOneSuit;
    // This seed deals all five rows without a transfer and leaves no legal moves.
    let seed = 266;
    let (window, mut visual) = open(cx, mode, seed);
    let initial = Game::with_mode(mode, seed);
    let mut expected = initial.clone();
    let stock = click(&mut visual, "solitaire-slot-Stock");
    assert_eq!(visual.debug_bounds("solitaire-flight-0"), Some(stock));
    assert!(expected.play(Move::Draw));
    let targets = [
        "solitaire-slot-Tableau(0, 6)",
        "solitaire-slot-Tableau(9, 5)",
    ];
    advance(&mut visual, 237);
    let first_landing = visual.debug_bounds("solitaire-flight-0").unwrap();
    assert!(first_landing.origin.y > stock.bottom());
    assert!(visual.debug_bounds(targets[0]).is_none());
    advance(&mut visual, 2);
    near_destination(first_landing, visual.debug_bounds(targets[0]).unwrap());
    assert!(visual.debug_bounds("solitaire-flight-9").is_some());
    advance(&mut visual, 214);
    let last_landing = visual.debug_bounds("solitaire-flight-9").unwrap();
    assert!(last_landing.origin.x > first_landing.right());
    advance(&mut visual, 2);
    near_destination(last_landing, visual.debug_bounds(targets[1]).unwrap());
    assert!(visual.debug_bounds("solitaire-flight-9").is_none());

    for _ in 0..3 {
        click(&mut visual, "solitaire-slot-Stock");
        assert!(expected.play(Move::Draw));
        advance(&mut visual, 500);
        assert!(visual.debug_bounds("solitaire-blocked").is_none());
    }
    let before = expected.clone();
    for action in [
        "solitaire-undo-button",
        "solitaire-finish-replay",
        "solitaire-finish-new",
    ] {
        click(&mut visual, "solitaire-slot-Stock");
        window
            .read_with(cx, |view, _| {
                assert!(view.deal.game.stalled());
                assert!(!view.deal.game.won());
            })
            .unwrap();
        assert!(visual.debug_bounds("solitaire-blocked").is_none());
        advance(&mut visual, 800);
        let result = visual.debug_bounds("solitaire-blocked").unwrap();
        assert!(result.bottom() < px(540.));
        assert!(visual.debug_bounds("solitaire-won").is_none());
        click(&mut visual, action);
        assert!(visual.debug_bounds("solitaire-blocked").is_none());
        window
            .read_with(cx, |view, _| match action {
                "solitaire-undo-button" => assert_eq!(view.deal.game, before),
                "solitaire-finish-replay" => assert_eq!(view.deal.game, initial),
                _ => assert_ne!(view.deal.game, initial),
            })
            .unwrap();

        if action == "solitaire-finish-replay" {
            for _ in 0..4 {
                click(&mut visual, "solitaire-slot-Stock");
                advance(&mut visual, 500);
            }
        }
    }
}
