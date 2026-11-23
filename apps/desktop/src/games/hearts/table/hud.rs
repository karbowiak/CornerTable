use corner_table_hearts::{HandResult, MATCH_LIMIT, Phase, Seat, TRICKS_PER_HAND};
use gpui::{
    AnyElement, Context, Div, FontWeight, Role, Stateful, Toggled, div, prelude::*, px, rgb, rgba,
};

use crate::{
    games::{round_result, themed_control},
    storage::Outcome,
    theme,
};

use super::{
    BRASS, BRASS_HOVER, CARD_BAY, CHIP, GameView, INK, Layout, MUTED, RIM, RIM_SOFT,
    copy::{PhaseCopy, player, points},
};

impl GameView {
    pub(super) fn toolbar(&self, cx: &Context<Self>) -> Div {
        let game = self.session.game();
        let palette = theme::colors(cx);
        let finished =
            matches!(game.phase(), Phase::HandComplete(result) if result.winner.is_some());
        let playing = matches!(game.phase(), Phase::Playing(_) | Phase::TrickComplete(_))
            && !self.session.ready_to_open();
        let hearts = if game.hearts_broken() {
            "Hearts broken"
        } else {
            "Hearts unbroken"
        };
        let hand_status = if playing {
            format!(
                "Hand {}, trick {} of {TRICKS_PER_HAND}. {hearts}.",
                game.hand_number(),
                game.trick_number(),
            )
        } else {
            format!("Hand {}", game.hand_number())
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .id("hearts-hand-progress")
                    .role(Role::Group)
                    .aria_label(hand_status)
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .child(format!("Hand {}", game.hand_number())),
                    )
                    .when(playing, |row| {
                        row.child(
                            div()
                                .text_size(px(13.))
                                .text_color(rgb(palette.muted))
                                .child(format!(
                                    "Trick {} of {TRICKS_PER_HAND}",
                                    game.trick_number()
                                )),
                        )
                        .child(
                            div()
                                .px_2()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(palette.border))
                                .text_size(px(12.))
                                .text_color(rgb(if game.hearts_broken() {
                                    palette.danger
                                } else {
                                    palette.muted
                                }))
                                .child(format!("♥ {hearts}")),
                        )
                    }),
            )
            .child(div().flex().flex_wrap().gap_2().when(!finished, |row| {
                let paused = self.session.pause_tricks();
                row.child(
                    themed_control(cx, "hearts-trick-pacing", "Pause after each trick", true)
                        .aria_toggled(if paused {
                            Toggled::True
                        } else {
                            Toggled::False
                        })
                        .when(paused, |button| {
                            button
                                .bg(rgb(palette.hover))
                                .border_color(rgb(palette.success))
                        })
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.session.toggle_pause();
                            view.timer = None;
                            cx.notify();
                        })),
                )
                .child(
                    themed_control(cx, "hearts-new-match", "New match", !self.confirm_new).when(
                        !self.confirm_new,
                        |button| {
                            button.on_click(cx.listener(|view, _, window, cx| {
                                view.confirm_new = true;
                                view.timer = None;
                                cx.on_next_frame(window, |view, window, cx| {
                                    if view.confirm_new {
                                        view.cancel_focus.focus(window, cx);
                                    }
                                });
                                cx.notify();
                            }))
                        },
                    ),
                )
            }))
    }

    pub(super) fn confirmation(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        theme::panel(cx)
            .p_3()
            .flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(rgb(palette.ink))
                    .child("Start a new match? This will replace your saved match."),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        themed_control(cx, "hearts-keep", "Keep playing", true)
                            .track_focus(&self.cancel_focus)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.cancel_restart(window, cx);
                            })),
                    )
                    .child(
                        themed_control(cx, "hearts-confirm", "Start new match", true).on_click(
                            cx.listener(|view, _, window, cx| {
                                view.restart(cx);
                                view.focus.focus(window, cx);
                            }),
                        ),
                    ),
            )
    }

    pub(super) fn phase_bar(&self, cx: &Context<Self>) -> Div {
        let copy = PhaseCopy::for_table(&self.session);
        let action = if self.session.ready_to_open() {
            Some(("Begin hand", true))
        } else {
            match self.session.game().phase() {
                Phase::Passing => Some(("Pass 3 cards", self.session.pass_turn().is_some())),
                Phase::Playing(_) => None,
                Phase::TrickComplete(_)
                    if self.session.game().trick_number() == TRICKS_PER_HAND =>
                {
                    Some(("Score hand", true))
                }
                Phase::TrickComplete(_) => Some(("Next trick", true)),
                Phase::HandComplete(_) => None,
            }
        };
        div()
            .flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(rgba(RIM_SOFT))
            .bg(rgba(CARD_BAY))
            .px_3()
            .py_2()
            .child(
                div()
                    .id("hearts-phase")
                    .role(Role::Status)
                    .aria_label(format!("{}. {}", copy.title, copy.detail))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(theme::display_type(copy.title, 18.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .text_color(rgb(MUTED))
                            .child(copy.detail),
                    ),
            )
            .children(action.map(|(label, enabled)| {
                let enabled = enabled && !self.confirm_new;
                primary_button("hearts-advance", label, enabled).when(enabled, |button| {
                    button.on_click(cx.listener(|view, _, window, cx| {
                        if view.session.game().phase() == Phase::Passing {
                            if let Some(turn) = view.session.pass_turn() {
                                view.perform(turn, cx);
                            }
                        } else {
                            view.advance(cx);
                        }
                        view.focus.focus(window, cx);
                    }))
                })
            }))
    }

    pub(super) fn hand_result(&self, result: HandResult, cx: &Context<Self>) -> AnyElement {
        let (outcome, label) = match result.winner {
            Some(Seat::You) => (Outcome::Won, "Play again"),
            Some(_) => (Outcome::Lost, "Play again"),
            None => (Outcome::Finished, "Deal next hand"),
        };
        let copy = PhaseCopy::score(self.session.game(), result);
        let enabled = !self.confirm_new;
        let action = primary_button("hearts-advance", label, enabled).when(enabled, |button| {
            button.on_click(cx.listener(|view, _, window, cx| {
                view.advance(cx);
                view.focus.focus(window, cx);
            }))
        });
        round_result(
            cx,
            "hearts-result",
            outcome,
            copy.title,
            copy.detail,
            div().flex().child(action),
        )
    }

    pub(super) fn score_sheet(&self, result: HandResult, layout: Layout) -> Div {
        let scores = self.session.game().scores();
        let mut rows = [Seat::You, Seat::West, Seat::North, Seat::East];
        if result.winner.is_some() {
            rows.sort_by_key(|seat| scores[seat.index()]);
        }
        div()
            .w_full()
            .max_w(px(if layout.compact { 480. } else { 560. }))
            .mx_auto()
            .my_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(rgba(RIM_SOFT))
            .bg(rgba(CARD_BAY))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        theme::display_type("Scorecard", 24.)
                            .id("hearts-scorecard-title")
                            .role(Role::Heading)
                            .aria_level(2)
                            .aria_label("Scorecard"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(format!("Match to {MATCH_LIMIT}")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(div().flex_1().child("Player"))
                    .child(div().w(px(62.)).text_right().child("This hand"))
                    .child(div().w(px(62.)).text_right().child("Total")),
            )
            .children(rows.map(|seat| {
                let index = seat.index();
                let winner = result.winner == Some(seat);
                let name = if winner {
                    format!("{} · Winner", player(seat))
                } else if result.moon == Some(seat) {
                    format!("{} · Shot the moon", player(seat))
                } else {
                    player(seat).into()
                };
                div()
                    .id(("hearts-score", index))
                    .role(Role::Group)
                    .aria_label(format!(
                        "{name}, {} this hand, {} total",
                        points(u16::from(result.points[index])),
                        points(scores[index]),
                    ))
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(rgba(RIM_SOFT))
                    .pt_2()
                    .text_size(px(14.))
                    .text_color(rgb(if winner { BRASS } else { INK }))
                    .child(div().flex_1().font_weight(FontWeight::BOLD).child(name))
                    .child(
                        div()
                            .w(px(62.))
                            .text_right()
                            .child(format!("+{}", result.points[index])),
                    )
                    .child(
                        div()
                            .w(px(62.))
                            .text_right()
                            .font_weight(FontWeight::BOLD)
                            .child(scores[index].to_string()),
                    )
            }))
    }
}

fn primary_button(id: &'static str, label: &'static str, enabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .flex()
        .justify_center()
        .items_center()
        .min_h(px(39.))
        .px_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(if enabled { BRASS } else { RIM }))
        .bg(rgb(if enabled { BRASS } else { CHIP }))
        .text_color(rgb(if enabled { CHIP } else { MUTED }))
        .text_size(px(13.))
        .font_weight(FontWeight::BOLD)
        .when(enabled, |button| {
            button
                .role(Role::Button)
                .aria_label(label)
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(BRASS_HOVER)))
                .focus_visible(|style| style.border_color(rgb(INK)).bg(rgb(BRASS_HOVER)))
        })
        .child(label)
}

#[cfg(test)]
mod tests {
    use corner_table_api::GameId;
    use gpui::{KeyUpEvent, Keystroke, PlatformInput, TestAppContext, size};

    use super::*;
    use crate::games::{GameEntry, GameLaunch, hearts::session::Turn};

    fn finish_hand(view: &mut GameView) -> HandResult {
        loop {
            let game = view.session.game();
            let turn = if view.session.ready_to_open() {
                Turn::Begin
            } else {
                match game.phase() {
                    Phase::Passing => {
                        let hand = game.hand(Seat::You);
                        Turn::Pass {
                            cards: [hand[0].id(), hand[1].id(), hand[2].id()],
                        }
                    }
                    Phase::Playing(_) => Turn::Play {
                        card: game.suggested_play().unwrap().id(),
                    },
                    Phase::TrickComplete(_) => Turn::Collect,
                    Phase::HandComplete(result) => return result,
                }
            };
            assert!(view.session.apply(turn));
        }
    }

    #[gpui::test]
    fn scored_hand_continues_and_match_result_can_start_over(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId("hearts-result-test".into()),
            entry: GameEntry::Classic,
        };
        let window = cx.open_window(size(px(812.), px(575.)), |window, cx| {
            cx.set_reduce_motion(true);
            let mut view = GameView::new(&launch, cx);
            view.session.restart(1);
            let result = finish_hand(&mut view);
            assert_eq!(result.winner, None);
            let copy = PhaseCopy::score(view.session.game(), result);
            assert!(copy.detail.contains(&points(result.points[0].into())));
            view.focus.focus(window, cx);
            view
        });
        let use_result_action = |cx: &mut TestAppContext, tab_stops| {
            cx.run_until_parked();
            cx.update_window(window.into(), |_, window, cx| {
                for _ in 0..tab_stops {
                    window.focus_next(cx);
                }
            })
            .unwrap();
            let enter = Keystroke::parse("enter").unwrap();
            cx.dispatch_keystroke(window.into(), enter.clone());
            cx.update_window(window.into(), |_, window, cx| {
                window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: enter }), cx);
            })
            .unwrap();
        };
        use_result_action(cx, 3);
        window
            .update(cx, |view, _, cx| {
                assert_eq!(view.session.game().hand_number(), 2);
                let winner = loop {
                    let result = finish_hand(view);
                    if let Some(winner) = result.winner {
                        let copy = PhaseCopy::score(view.session.game(), result);
                        let scores = view.session.game().scores();
                        assert_eq!(copy.title, "You lose the match");
                        assert!(copy.detail.contains(player(winner)));
                        assert!(copy.detail.contains(&points(scores[winner.index()])));
                        assert!(copy.detail.contains("107 points"));
                        break winner;
                    }
                    view.advance(cx);
                };
                assert_ne!(winner, Seat::You);
                cx.notify();
            })
            .unwrap();
        use_result_action(cx, 2);
        window
            .update(cx, |view, window, _| {
                assert_eq!(view.session.game().hand_number(), 1);
                assert_eq!(view.session.game().scores(), [0; 4]);
                assert!(view.focus.is_focused(window));
            })
            .unwrap();
    }
}
