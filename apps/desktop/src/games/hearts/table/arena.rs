use std::time::Duration;

use corner_table_hearts::{Pass, Phase, Seat};
use gpui::{
    Animation, AnimationExt as _, Context, Div, FontWeight, Role, div, prelude::*, px, rgb, rgba,
};

use crate::theme;

use super::{
    BRASS, CHIP, GameView, Layout, MUTED, RIM_SOFT,
    cards::{Face, back_stack, card_name, face, slot},
    copy::player,
};

impl GameView {
    pub(super) fn arena(&self, layout: Layout, cx: &Context<Self>) -> Div {
        if layout.compact {
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(
                    div().w_full().flex().gap_2().children(
                        [Seat::West, Seat::North, Seat::East]
                            .map(|seat| div().flex_1().min_w_0().child(self.opponent(seat))),
                    ),
                )
                .child(self.mat(layout, cx))
        } else {
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .gap_4()
                .child(div().w(px(143.)).child(self.opponent(Seat::West)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(div().w(px(143.)).child(self.opponent(Seat::North)))
                        .child(self.mat(layout, cx)),
                )
                .child(div().w(px(143.)).child(self.opponent(Seat::East)))
        }
    }

    fn opponent(&self, seat: Seat) -> Div {
        let game = self.session.game();
        let count = game.hand(seat).len();
        let target =
            game.phase() == Phase::Passing && game.pass_direction().recipient(Seat::You) == seat;
        let current = !self.session.ready_to_open() && game.phase() == Phase::Playing(seat);
        let winner = matches!(game.phase(), Phase::TrickComplete(result) if result.winner == seat);
        let score = game.scores()[seat.index()];
        let hand = game.hand_points()[seat.index()];
        let name = player(seat);
        let status = if target {
            "Receiving your cards"
        } else if current {
            "Playing now"
        } else if winner {
            "Took the trick"
        } else {
            ""
        };
        div()
            .w_full()
            .min_w_0()
            .border_1()
            .border_color(if target || current || winner {
                rgb(BRASS)
            } else {
                rgba(RIM_SOFT)
            })
            .rounded_md()
            .bg(rgb(CHIP))
            .px_2()
            .py_1()
            .flex()
            .items_center()
            .justify_between()
            .gap_1()
            .child(
                div()
                    .id(("hearts-opponent", seat.index()))
                    .role(Role::Group)
                    .aria_label(format!(
                        "{name}, {count} cards, {hand} hand points, {score} match points. {status}",
                    ))
                    .flex()
                    .min_w_0()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(15.))
                            .line_height(px(22.))
                            .font_weight(FontWeight::BOLD)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .line_height(px(17.))
                            .text_color(rgb(MUTED))
                            .child(format!("Hand +{hand} · Total {score}")),
                    ),
            )
            .child(back_stack(count))
    }

    fn mat(&self, layout: Layout, cx: &Context<Self>) -> Div {
        let game = self.session.game();
        let passing = game.phase() == Phase::Passing;
        let received = self.session.ready_to_open();
        let inside = if passing || received {
            self.exchange(layout, cx)
        } else {
            div()
                .size_full()
                .relative()
                .children(Seat::ALL.map(|seat| self.trick_card(seat, layout, cx)))
        };
        div()
            .relative()
            .w(px(layout.mat_width))
            .h(px(layout.mat_height))
            .flex_none()
            .child(
                div()
                    .absolute()
                    .left(px(13.))
                    .right(px(13.))
                    .top(px(15.))
                    .bottom(px(17.))
                    .rounded_full()
                    .border_1()
                    .border_color(rgba(RIM_SOFT))
                    .child(
                        div()
                            .absolute()
                            .inset(px(5.))
                            .rounded_full()
                            .border_1()
                            .border_color(rgba(0xe5bb9c27))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(!(passing || received), |oval| {
                                oval.child(
                                    div()
                                        .text_size(px(30.))
                                        .font_family(theme::DISPLAY)
                                        .text_color(rgba(0xf2bf9844))
                                        .child("♥"),
                                )
                            }),
                    ),
            )
            .child(inside)
    }

    fn exchange(&self, layout: Layout, cx: &Context<Self>) -> Div {
        let game = self.session.game();
        let selected = self.session.selected();
        let receiving = self.session.ready_to_open();
        let incoming = game.received().filter(|_| receiving);
        if receiving && game.pass_direction() == Pass::Keep {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    div()
                        .text_size(px(29.))
                        .font_family(theme::DISPLAY)
                        .text_color(rgb(MUTED))
                        .child("♣  ♦  ♠  ♥"),
                )
                .child(div().text_size(px(13.)).child("No pass this hand"));
        }
        let cards = incoming.as_ref().map_or(selected.as_slice(), |cards| cards);
        let label = if receiving {
            format!("From {}", player(game.pass_direction().sender(Seat::You)))
        } else {
            format!("{} of 3 selected", cards.len())
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .rounded_md()
                    .px_2()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MUTED))
                    .child(label),
            )
            .child(
                div()
                    .flex()
                    .justify_center()
                    .gap_2()
                    .children((0..3).map(|index| {
                        let width = layout.mat_card;
                        let card = match cards.get(index) {
                            Some(&card) => face(card, width, Face::Normal),
                            None => slot(width, false)
                                .child(div().text_size(px(14.)).text_color(rgb(MUTED)).child("·")),
                        };
                        if incoming.is_some() && !cx.reduce_motion() {
                            card.with_animation(
                                ("hearts-received", self.revision.wrapping_mul(3) + index),
                                Animation::new(Duration::from_millis(410)),
                                move |card, elapsed| {
                                    let progress =
                                        ((elapsed - index as f32 * 0.12) / 0.76).clamp(0., 1.);
                                    let rest = (1. - progress).powi(3);
                                    card.top(px(-13. * rest)).opacity(1. - 0.38 * rest)
                                },
                            )
                            .into_any_element()
                        } else {
                            card.into_any_element()
                        }
                    })),
            )
    }

    fn trick_card(&self, seat: Seat, layout: Layout, cx: &Context<Self>) -> Div {
        let game = self.session.game();
        let play = game.trick().iter().find(|play| play.seat == seat);
        let active = game.phase() == Phase::Playing(seat);
        let won = matches!(game.phase(), Phase::TrickComplete(result) if result.winner == seat);
        let width = layout.mat_card;
        let (x, y) = layout.trick_card(seat);
        let mut card = div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(width))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.));
        if let Some(play) = play {
            let value = play.card;
            let face = face(value, width, if won { Face::Winner } else { Face::Normal })
                .id(("hearts-played", seat.index()))
                .role(Role::Group)
                .aria_label(format!("{} played {}", player(seat), card_name(value)));
            card = if self.arrival.as_ref() == Some(play) && !cx.reduce_motion() {
                let (dx, dy) = match seat {
                    Seat::West => (-17., 0.),
                    Seat::East => (17., 0.),
                    Seat::North => (0., -13.),
                    Seat::You => (0., 13.),
                };
                card.child(face.with_animation(
                    ("hearts-card-arrival", self.revision),
                    Animation::new(Duration::from_millis(235)),
                    move |card, elapsed| {
                        let rest = (1. - elapsed).powi(3);
                        card.left(px(dx * rest))
                            .top(px(dy * rest))
                            .opacity(1. - 0.33 * rest)
                    },
                ))
            } else {
                card.child(face)
            };
        } else {
            card = card.child(slot(width, active));
        }
        card.child(
            div()
                .px_1()
                .text_size(px(11.))
                .line_height(px(15.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(if active || won { BRASS } else { MUTED }))
                .child(player(seat)),
        )
    }
}
