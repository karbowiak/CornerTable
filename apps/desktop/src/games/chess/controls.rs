use corner_table_chess::{Color, Decision, DrawClaim, Ending, Piece};
use gpui::{AnyElement, Context, Div, FontWeight, Role, div, prelude::*, px, rgb};

use crate::{games::round_result, storage::Outcome, theme};

use super::super::themed_control;
use super::{
    Confirmation, GameView,
    board::{piece_background, piece_image},
    side_name,
    state::Mode,
};

impl GameView {
    pub(super) fn status(&self, cx: &Context<Self>) -> AnyElement {
        if let Some(ending) = self.game().ending() {
            let (outcome, title, detail) = result_copy(self.table.mode, ending);
            let palette = theme::colors(cx);
            let actions = div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    themed_control(cx, "chess-restart", "New match", true)
                        .border_color(rgb(palette.gold_text))
                        .on_click(cx.listener(|view, _, window, cx| view.restart(window, cx))),
                )
                .child(
                    themed_control(cx, "chess-undo", "Undo", true)
                        .on_click(cx.listener(|view, _, window, cx| view.undo(window, cx))),
                );
            return round_result(cx, "chess-result", outcome, title, detail, actions);
        }
        let palette = theme::colors(cx);
        let status = self.status_copy();
        theme::panel(cx)
            .w_full()
            .px_3()
            .py_2()
            .when(status.in_check, |panel| {
                panel.border_color(rgb(palette.danger))
            })
            .child(
                div()
                    .id("chess-status")
                    .role(Role::Status)
                    .aria_label(format!("{}. {}", status.title, status.detail))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .size(px(24.))
                                    .rounded(px(5.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(piece_background())
                                    .child(piece_image(status.side, Piece::King, 23.)),
                            )
                            .child(theme::display_type(status.title, 17.).min_w_0()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .text_color(rgb(if status.in_check {
                                palette.danger
                            } else {
                                palette.muted
                            }))
                            .child(status.detail),
                    ),
            )
            .into_any_element()
    }

    fn status_copy(&self) -> Status {
        let mover = self.game().side_to_move();
        let side = side_name(mover);
        let checked = self.game().checked_king();
        let title = format!("{side} to move");
        let detail = if let Some(promotion) = self.promotion {
            format!("Choose the piece for {}.", promotion.movement.to)
        } else if self.claim_mode {
            "Select a marked piece and destination to announce a draw claim.".into()
        } else if let Some(notice) = &self.notice {
            notice.clone()
        } else if self.computer.is_some() {
            "Computer is thinking.".into()
        } else if let Some(square) = self.selected {
            format!("{square} selected. Choose a marked square.")
        } else if self.table.mode == Mode::Computer {
            "You play White.".into()
        } else {
            "Choose a piece to see its legal moves.".into()
        };
        let detail = if let Some(square) = checked {
            format!("Check on {square}. {detail}")
        } else {
            detail
        };
        Status {
            title,
            detail,
            side: mover,
            in_check: checked.is_some(),
        }
    }

    pub(super) fn controls(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let modal = self.promotion.is_some() || self.confirmation.is_some();
        let ended = self.game().ending().is_some();
        let can_restart = self.game().moves().len() > 0 && !modal;
        let local = self.table.mode == Mode::Local;
        let choice_blocked = modal || self.claim_mode;
        let row = div().w_full().flex().items_center().flex_wrap().gap_2();

        let actions = if let Some(confirmation) = self.confirmation {
            self.confirmation_controls(confirmation, cx)
        } else if self.claim_mode {
            row.child(
                themed_control(cx, "chess-cancel-claim", "Cancel claim", true).on_click(
                    cx.listener(|view, _, window, cx| {
                        view.cancel(window, cx);
                    }),
                ),
            )
        } else {
            let can_claim = self.claim_now.is_some() || !self.claim_moves.is_empty();
            row.when(!ended, |row| {
                row.child(themed_control(cx, "chess-undo", "Undo", can_restart).when(
                    can_restart,
                    |button| {
                        button.on_click(cx.listener(|view, _, window, cx| view.undo(window, cx)))
                    },
                ))
                .child(
                    themed_control(cx, "chess-restart", "Restart", can_restart).when(
                        can_restart,
                        |button| {
                            button.on_click(cx.listener(|view, _, window, cx| {
                                view.ask_for_confirmation(Confirmation::Restart, window, cx);
                            }))
                        },
                    ),
                )
            })
            .child(
                themed_control(cx, "chess-flip", "Flip board", !modal).when(!modal, |button| {
                    button.on_click(cx.listener(|view, _, _, cx| view.flip(cx)))
                }),
            )
            .when(self.human_turn() && !modal, |bar| {
                bar.child(
                    themed_control(cx, "chess-resign", "Resign", true).on_click(cx.listener(
                        |view, _, window, cx| {
                            view.ask_for_confirmation(Confirmation::Resign, window, cx);
                        },
                    )),
                )
                .when(local && self.game().moves().len() >= 2, |bar| {
                    bar.child(
                        themed_control(cx, "chess-offer-draw", "Offer draw", true).on_click(
                            cx.listener(|view, _, window, cx| {
                                view.ask_for_confirmation(Confirmation::OfferDraw, window, cx);
                            }),
                        ),
                    )
                })
                .when(can_claim, |bar| {
                    let label = if self.claim_now.is_some() {
                        "Claim draw"
                    } else {
                        "Claim draw…"
                    };
                    bar.child(
                        themed_control(cx, "chess-claim-draw", label, true).on_click(
                            cx.listener(|view, _, window, cx| view.enter_claim(window, cx)),
                        ),
                    )
                })
            })
        };

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .id("chess-modes")
                    .role(Role::Group)
                    .aria_label("Opponent")
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(
                        [
                            (Mode::Local, "chess-local", "Two players"),
                            (Mode::Computer, "chess-computer", "Computer"),
                        ]
                        .map(|(mode, id, label)| {
                            let chosen = self.table.mode == mode;
                            themed_control(cx, id, label, !choice_blocked)
                                .px_2()
                                .text_size(px(12.))
                                .aria_toggled(chosen.into())
                                .when(chosen, |button| {
                                    button
                                        .bg(rgb(palette.hover))
                                        .border_color(rgb(palette.gold_text))
                                })
                                .when(!choice_blocked, |button| {
                                    button.on_click(
                                        cx.listener(move |view, _, _, cx| {
                                            view.choose_mode(mode, cx)
                                        }),
                                    )
                                })
                        }),
                    ),
            )
            .child(actions)
            .when(!ended, |controls| {
                controls.child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(rgb(palette.muted))
                        .child("Tab to the board. Arrows move the cursor. Enter selects or plays."),
                )
            })
    }

    fn confirmation_controls(&self, confirmation: Confirmation, cx: &Context<Self>) -> Div {
        let side = self.game().side_to_move();
        let (prompt, action) = match confirmation {
            Confirmation::Restart => (
                "Reset this match to the opening position?".into(),
                "Reset match",
            ),
            Confirmation::Resign => (
                format!("Does {} resign?", side_name(side)),
                "Confirm resignation",
            ),
            Confirmation::OfferDraw => (
                format!(
                    "{}, accept {}’s draw offer?",
                    side_name(!side),
                    side_name(side)
                ),
                "Accept draw",
            ),
        };
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .id("chess-confirmation-prompt")
                    .role(Role::Status)
                    .aria_label(prompt.clone())
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(prompt),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        themed_control(cx, "chess-confirm-action", action, true).on_click(
                            cx.listener(move |view, _, window, cx| match confirmation {
                                Confirmation::Restart => view.restart(window, cx),
                                Confirmation::Resign => {
                                    view.decide(Decision::Resign, cx);
                                    view.board_focus.focus(window, cx);
                                }
                                Confirmation::OfferDraw => {
                                    view.decide(Decision::AgreeDraw, cx);
                                    view.board_focus.focus(window, cx);
                                }
                            }),
                        ),
                    )
                    .child(
                        themed_control(
                            cx,
                            "chess-cancel-action",
                            if confirmation == Confirmation::OfferDraw {
                                "Decline"
                            } else {
                                "Cancel"
                            },
                            true,
                        )
                        .track_focus(&self.confirmation_focus)
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.cancel(window, cx);
                        })),
                    ),
            )
    }

    pub(super) fn journal(&self, cx: &Context<Self>) -> Option<Div> {
        let moves = self.game().moves();
        let turns = moves.len().div_ceil(2);
        if turns == 0 {
            return None;
        }
        let palette = theme::colors(cx);
        let start = turns.saturating_sub(6);
        let recent: Vec<_> = moves.skip(start * 2).collect();
        let title = if start == 0 { "Moves" } else { "Latest moves" };
        let column = |text: &str| div().flex_1().min_w_0().truncate().child(text.to_owned());
        let gutter = || div().w(px(30.)).flex_none();
        Some(
            theme::panel(cx).w_full().overflow_hidden().child(
                div()
                    .id("chess-journal")
                    .role(Role::Group)
                    .aria_label(title)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(palette.border))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(title),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .px_3()
                            .py_1()
                            .gap_2()
                            .text_size(px(12.))
                            .text_color(rgb(palette.muted))
                            .child(gutter())
                            .child(column("White"))
                            .child(column("Black")),
                    )
                    .children(recent.chunks(2).enumerate().map(|(i, row)| {
                        div()
                            .flex()
                            .items_center()
                            .px_3()
                            .h(px(25.))
                            .gap_2()
                            .text_size(px(13.))
                            .when(i % 2 == 0, |line| line.bg(rgb(palette.paper)))
                            .child(
                                gutter()
                                    .text_color(rgb(palette.muted))
                                    .child(format!("{}.", start + i + 1)),
                            )
                            .child(column(&row[0].san))
                            .child(column(row.get(1).map_or("", |play| &play.san)))
                    })),
            ),
        )
    }
}

struct Status {
    title: String,
    detail: String,
    side: Color,
    in_check: bool,
}

fn result_copy(mode: Mode, ending: Ending) -> (Outcome, String, String) {
    let (winner, reason) = match ending {
        Ending::Checkmate { winner } => (Some(winner), "Checkmate."),
        Ending::Resignation { winner } => (
            Some(winner),
            if winner == Color::White {
                "Black resigned."
            } else {
                "White resigned."
            },
        ),
        Ending::ResignationDraw => (None, "The other side had only its king at resignation."),
        Ending::Stalemate => (None, "Stalemate. The side to move has no legal moves."),
        Ending::InsufficientMaterial => (None, "Neither side has enough material to give mate."),
        Ending::FivefoldRepetition => (None, "The same position occurred five times."),
        Ending::SeventyFiveMoveRule => (None, "75 moves per side without a pawn move or capture."),
        Ending::ClaimedDraw(DrawClaim::ThreefoldRepetition) => {
            (None, "Draw claimed by threefold repetition.")
        }
        Ending::ClaimedDraw(DrawClaim::FiftyMoveRule) => {
            (None, "Draw claimed under the 50-move rule.")
        }
        Ending::AgreedDraw => (None, "Both players agreed to a draw."),
    };
    let Some(winner) = winner else {
        return (Outcome::Draw, "Draw".into(), reason.into());
    };
    let side = side_name(winner);
    match mode {
        Mode::Local => (Outcome::Finished, format!("{side} wins"), reason.into()),
        Mode::Computer => {
            let (outcome, title) = if winner == Color::White {
                (Outcome::Won, "You win")
            } else {
                (Outcome::Lost, "You lose")
            };
            (outcome, title.into(), format!("{side} wins. {reason}"))
        }
    }
}
