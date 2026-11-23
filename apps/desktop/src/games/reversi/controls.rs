use corner_table_reversi::{Disc, Outcome, Position, Turn};
use gpui::{
    AnyElement, Context, Div, ElementId, FontWeight, Role, Stateful, div, prelude::*, px, rgb,
};

use crate::{
    games::round_result,
    storage::Outcome as RecordOutcome,
    theme::{self, Palette},
};

use super::{GameView, board, state::Opponent};

impl GameView {
    pub(super) fn result_card(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let position = self.position();
        let Turn::Finished(result) = position.turn() else {
            return None;
        };
        let (outcome, title, detail) = result_copy(self.opponent, position, result);
        Some(round_result(
            cx,
            "reversi-result",
            outcome,
            title,
            detail,
            self.actions(cx),
        ))
    }

    pub(super) fn scoreboard(&self, cx: &Context<Self>) -> Div {
        let position = self.position();
        let (headline, detail) = match position.turn() {
            Turn::Play(side) => (self.headline(side), self.detail()),
            Turn::Finished(_) => (
                "Match over".into(),
                "Undo to review, or start a new match.".into(),
            ),
        };
        let palette = theme::colors(cx);
        div()
            .w_full()
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
                    .px_4()
                    .py_4()
                    .child(theme::display_type(headline, 21.))
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
        let active = position.turn() == Turn::Play(side);
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

    fn actions(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let played = self.round().last().is_some();
        let terminal = matches!(self.position().turn(), Turn::Finished(_));
        let undo = control("reversi-undo", "Undo", false, played, palette).when(played, |button| {
            button.on_click(cx.listener(|view, _, window, cx| view.undo(window, cx)))
        });
        let restart = control(
            "reversi-restart",
            if terminal {
                "New match"
            } else if self.confirm_restart {
                "Reset match"
            } else {
                "Restart"
            },
            self.confirm_restart || terminal,
            played,
            palette,
        )
        .when(played, |button| {
            button.on_click(cx.listener(|view, _, window, cx| view.restart(window, cx)))
        });
        let row = div().flex().flex_wrap().items_center().gap_2();
        if terminal {
            row.child(restart).child(undo)
        } else if self.confirm_restart {
            row.child(restart).child(
                control("reversi-cancel-restart", "Cancel", false, true, palette).on_click(
                    cx.listener(|view, _, _, cx| {
                        view.confirm_restart = false;
                        cx.notify();
                    }),
                ),
            )
        } else {
            row.child(undo).child(restart)
        }
    }

    pub(super) fn controls(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let terminal = matches!(self.position().turn(), Turn::Finished(_));
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
            .when(!terminal, |column| {
                column.child(
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
                        .child(self.actions(cx)),
                )
            })
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
    }
}

fn result_copy(
    opponent: Opponent,
    position: Position,
    result: Outcome,
) -> (RecordOutcome, String, String) {
    let (outcome, title) = match (opponent, result) {
        (_, Outcome::Draw) => (RecordOutcome::Draw, "Draw".into()),
        (Opponent::Local, Outcome::Won(disc)) => {
            (RecordOutcome::Finished, format!("{} wins", disc.label()))
        }
        (_, Outcome::Won(Disc::Black)) => (RecordOutcome::Won, "You win".into()),
        (_, Outcome::Won(Disc::White)) => (RecordOutcome::Lost, "You lose".into()),
    };
    let (black, white) = (position.score(Disc::Black), position.score(Disc::White));
    let reason = if position.empty_count() == 0 {
        "The board is full."
    } else {
        "Neither side can move."
    };
    (
        outcome,
        title,
        format!("Final discs — Black {black}, White {white}. {reason}"),
    )
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
        .border_color(rgb(if selected {
            theme::GOLD
        } else {
            palette.border
        }))
        .bg(rgb(if selected {
            theme::GOLD
        } else {
            palette.surface
        }))
        .text_color(rgb(if selected {
            theme::INK
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
                .hover(move |style| {
                    style.bg(rgb(if selected {
                        theme::GOLD_HOVER
                    } else {
                        palette.hover
                    }))
                })
                .focus_visible(move |style| style.border_color(rgb(palette.ink)))
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

#[cfg(test)]
mod tests {
    use corner_table_api::GameId;
    use gpui::{KeyUpEvent, Keystroke, PlatformInput, TestAppContext, size};

    use super::*;
    use crate::games::{GameEntry, GameLaunch};

    #[gpui::test]
    fn result_uses_final_disc_counts_and_new_match_resets_the_board(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId("reversi-result-test".into()),
            entry: GameEntry::Classic,
        };
        let window = cx.open_window(size(px(1360.), px(760.)), |window, cx| {
            cx.set_reduce_motion(true);
            let mut view = GameView::new(&launch, cx);
            while let Some(square) = view.position().legal_moves().next() {
                view.play(square, cx);
            }
            let position = view.position();
            let Turn::Finished(Outcome::Won(winner)) = position.turn() else {
                panic!("expected the fixture to produce a winner");
            };
            let (outcome, title, detail) =
                result_copy(Opponent::Local, position, Outcome::Won(winner));
            assert_eq!(outcome, RecordOutcome::Finished);
            assert_eq!(title, format!("{} wins", winner.label()));
            assert!(detail.contains(&format!(
                "Black {}, White {}",
                position.score(Disc::Black),
                position.score(Disc::White)
            )));
            view.board_focus.focus(window, cx);
            view
        });
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| window.focus_next(cx))
            .unwrap();
        let enter = Keystroke::parse("enter").unwrap();
        cx.dispatch_keystroke(window.into(), enter.clone());
        cx.update_window(window.into(), |_, window, cx| {
            window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: enter }), cx);
        })
        .unwrap();
        window
            .update(cx, |view, window, _| {
                assert_eq!(view.position(), Position::new());
                assert!(view.board_focus.is_focused(window));
            })
            .unwrap();
    }
}
