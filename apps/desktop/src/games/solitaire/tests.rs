use std::time::Duration;

use corner_table_api::GameId;
use corner_table_solitaire::TableCard;
use gpui::{Bounds, Modifiers, Pixels, TestAppContext, VisualTestContext, WindowHandle, size};

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
    let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
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
