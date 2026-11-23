use corner_table_reversi::{Disc, Outcome, Turn};
use gpui::{Context, Div, ElementId, FontWeight, Role, Stateful, div, prelude::*, px, rgb};

use crate::theme::{self, Palette};

use super::{GameView, board, state::Opponent};

impl GameView {
    pub(super) fn scoreboard(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let headline = self.headline();
        let detail = self.detail();
        let position = self.position();
        theme::panel(cx)
            .w_full()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .border_b_1()
                    .border_color(rgb(palette.border))
                    .child(self.score(Disc::Black, palette))
                    .child(
                        self.score(Disc::White, palette)
                            .border_l_1()
                            .border_color(rgb(palette.border)),
                    ),
            )
            .child(
                div()
                    .id("reversi-status")
                    .role(Role::Status)
                    .aria_label(format!(
                        "{headline}. Black {}, White {}. {detail}",
                        position.score(Disc::Black),
                        position.score(Disc::White)
                    ))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .px_3()
                    .py_2()
                    .child(theme::display_type(headline, 20.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .text_color(rgb(palette.muted))
                            .child(detail),
                    ),
            )
    }

    fn score(&self, side: Disc, palette: Palette) -> Div {
        let position = self.position();
        let active = match position.turn() {
            Turn::Play(turn) | Turn::Finished(Outcome::Won(turn)) => turn == side,
            Turn::Finished(Outcome::Draw) => false,
        };
        let score = position.score(side);
        let player = match (self.opponent, side) {
            (Opponent::Local, _) => None,
            (_, Disc::Black) => Some("You"),
            (_, Disc::White) => Some("Computer"),
        };
        div()
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .when(active, |line| {
                line.bg(rgb(palette.hover)).child(
                    div()
                        .absolute()
                        .top_0()
                        .left_3()
                        .right_3()
                        .h(px(2.))
                        .bg(rgb(palette.gold_text)),
                )
            })
            .child(board::disc(side, 18., 18.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .line_height(px(16.))
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(side.label())
                    .children(player.map(|player| {
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(palette.muted))
                            .child(player)
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(22.))
                    .line_height(px(26.))
                    .font_weight(FontWeight::BOLD)
                    .child(score.to_string()),
            )
    }

    pub(super) fn controls(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let played = self.round().last().is_some();
        let terminal = matches!(self.position().turn(), Turn::Finished(_));
        let actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .children((!self.confirm_restart).then(|| {
                control("reversi-undo", "Undo", false, played, palette).when(played, |button| {
                    button.on_click(cx.listener(|view, _, window, cx| view.undo(window, cx)))
                })
            }))
            .child(
                control(
                    "reversi-restart",
                    if self.confirm_restart {
                        "Reset match"
                    } else if terminal {
                        "New match"
                    } else {
                        "Restart"
                    },
                    self.confirm_restart || terminal,
                    played,
                    palette,
                )
                .when(played, |button| {
                    button.on_click(cx.listener(|view, _, window, cx| view.restart(window, cx)))
                }),
            )
            .when(self.confirm_restart, |row| {
                row.child(
                    control("reversi-cancel-restart", "Cancel", false, true, palette).on_click(
                        cx.listener(|view, _, _, cx| {
                            view.confirm_restart = false;
                            cx.notify();
                        }),
                    ),
                )
            });
        let description = match self.opponent {
            Opponent::Local => "Share the board with another player.",
            Opponent::Casual => "Play Black against a computer that favors quick captures.",
            Opponent::Strategist => "Play Black against a computer that plans ahead.",
        };
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.confirm_restart.then(|| {
                        div()
                            .id("reversi-confirm-restart")
                            .role(Role::Status)
                            .aria_label("Reset this match to the opening position?")
                            .text_size(px(13.))
                            .text_color(rgb(palette.ink))
                            .child("Reset this match to the opening position?")
                    }))
                    .child(actions),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Opponent"),
                    )
                    .child(
                        div()
                            .id("reversi-opponents")
                            .role(Role::Group)
                            .aria_label("Opponent")
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(Opponent::ALL.map(|opponent| {
                                let selected = self.opponent == opponent;
                                control(
                                    ("reversi-opponent", opponent.slot()),
                                    opponent.label(),
                                    selected,
                                    true,
                                    palette,
                                )
                                .aria_toggled(selected.into())
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        view.change_opponent(opponent, cx);
                                    },
                                ))
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(17.))
                            .text_color(rgb(palette.muted))
                            .child(description),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        toggle("reversi-hints", "Legal moves", self.display.hints, palette)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.display.hints = !view.display.hints;
                                view.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        toggle(
                            "reversi-coordinates",
                            "Coordinates",
                            self.display.coordinates,
                            palette,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.display.coordinates = !view.display.coordinates;
                            view.save();
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(rgb(palette.muted))
                    .child("Tab to the board. Use the arrow keys to select and Enter to play."),
            )
    }
}

fn control(
    id: impl Into<ElementId>,
    label: &'static str,
    selected: bool,
    enabled: bool,
    palette: Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_h(px(34.))
        .px_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(if selected { 0x9b6748 } else { palette.border }))
        .bg(rgb(if selected { 0x69462f } else { palette.surface }))
        .text_color(rgb(if selected {
            0xfff2dc
        } else if enabled {
            palette.ink
        } else {
            palette.muted
        }))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |button| {
            button
                .role(Role::Button)
                .aria_label(label)
                .tab_index(0)
                .cursor_pointer()
                .hover(move |style| style.bg(rgb(if selected { 0x805438 } else { palette.hover })))
                .focus_visible(move |style| {
                    style.border_color(rgb(if selected { 0xffdfac } else { palette.ink }))
                })
        })
        .child(label)
}

fn toggle(id: &'static str, label: &'static str, checked: bool, palette: Palette) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::CheckBox)
        .aria_label(label)
        .aria_toggled(checked.into())
        .tab_index(0)
        .flex()
        .flex_none()
        .items_center()
        .gap_2()
        .min_h(px(32.))
        .pl_1()
        .pr_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(palette.paper))
        .text_color(rgb(palette.ink))
        .text_size(px(12.))
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.hover)))
        .focus_visible(move |style| style.border_color(rgb(palette.ink)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(13.))
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(palette.muted))
                .when(checked, |mark| {
                    mark.child(
                        div()
                            .size(px(7.))
                            .rounded(px(1.))
                            .bg(rgb(palette.gold_text)),
                    )
                }),
        )
        .child(label)
}
