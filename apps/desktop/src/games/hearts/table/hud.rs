use corner_table_hearts::{HandResult, MATCH_LIMIT, Phase, Seat, TRICKS_PER_HAND};
use gpui::{Context, Div, FontWeight, Role, Stateful, Toggled, div, prelude::*, px, rgb, rgba};

use crate::{games::themed_control, theme};

use super::{
    BRASS, BRASS_HOVER, CARD_BAY, CHIP, GameView, INK, Layout, MUTED, RIM, RIM_SOFT,
    copy::{PhaseCopy, player, points},
};

impl GameView {
    pub(super) fn toolbar(&self, cx: &Context<Self>) -> Div {
        let game = self.session.game();
        let palette = theme::colors(cx);
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
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        themed_control(
                            cx,
                            "hearts-rules",
                            if self.show_help {
                                "Hide rules"
                            } else {
                                "How to play"
                            },
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.show_help = !view.show_help;
                            cx.notify();
                        })),
                    )
                    .child(
                        themed_control(cx, "hearts-new-match", "New match", !self.confirm_new)
                            .when(!self.confirm_new, |button| {
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
                            }),
                    ),
            )
    }

    pub(super) fn rules(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let paused = self.session.pause_tricks();
        theme::panel(cx)
            .p_4()
            .flex()
            .flex_col()
            .items_start()
            .gap_2()
            .text_size(px(13.))
            .line_height(px(20.))
            .children(
                [
                    ("Goal", "Keep your score low. When any player reaches 100, the lowest score wins. A tie for lowest goes to another hand."),
                    ("Pass", "Choose three cards to pass left, right, then across. On the fourth hand, keep your cards. The cycle repeats."),
                    ("Play", "The 2 of clubs opens each hand. Follow the led suit if you can. The highest card in that suit takes the trick and leads next."),
                    ("Hearts", "On the first trick, discard a card worth no points if possible. You can lead hearts once a heart has been played, or if your hand contains only hearts."),
                    ("Score", "Each heart is 1 point and the queen of spades is 13. Take all 26 to shoot the moon and give each opponent 26 points."),
                    ("Keyboard", "Tab to the table. Use ← and → to choose a card and Space to select or play it. Return passes three selected cards or continues."),
                ]
                .into_iter()
                .map(|(label, explanation)| {
                    div()
                        .child(div().font_weight(FontWeight::BOLD).child(label))
                        .child(div().text_color(rgb(palette.muted)).child(explanation))
                }),
            )
            .child(
                themed_control(
                    cx,
                    "hearts-trick-pacing",
                    if paused {
                        "Resume automatic tricks"
                    } else {
                        "Pause after each trick"
                    },
                    true,
                )
                .aria_label("Pause after each trick")
                .aria_toggled(if paused { Toggled::True } else { Toggled::False })
                .on_click(cx.listener(|view, _, _, cx| {
                    view.session.toggle_pause();
                    view.timer = None;
                    cx.notify();
                })),
            )
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
                Phase::HandComplete(result) if result.winner.is_some() => {
                    Some(("Play again", true))
                }
                Phase::HandComplete(_) => Some(("Deal next hand", true)),
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
                    .child(theme::display_type("Scorecard", 24.))
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
