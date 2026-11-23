use corner_table_match_three::{Goal, Progress, STAGES, Status};
use gpui::{Context, Div, FontWeight, Role, SharedString, Stateful, div, prelude::*, px, rgb};

use crate::theme as t;

use super::{Action, Feedback, GameView, field};

impl GameView {
    fn status_text(&self) -> (String, String) {
        if let Some(motion) = &self.motion
            && !motion.is_celebrating()
        {
            if let Some((chain, points)) = motion.clear_feedback() {
                return (
                    if chain == 1 {
                        "Match".into()
                    } else {
                        format!("×{chain} chain")
                    },
                    format!("+{points} points"),
                );
            }
            return (String::new(), String::new());
        }
        let status = self.game.status();
        match status {
            Status::Won => (
                if self.challenge {
                    "Daily puzzle cleared"
                } else if self.journal.active + 1 == STAGES.len() {
                    "All stages cleared"
                } else {
                    "Stage cleared"
                }
                .into(),
                if self.game.moves_left() == 0 {
                    "On the final move".into()
                } else {
                    format!("{} moves to spare", self.game.moves_left())
                },
            ),
            Status::Lost => (
                "Out of moves".into(),
                if self.challenge {
                    "Retry today’s layout."
                } else {
                    "Retry this layout or deal a new board."
                }
                .into(),
            ),
            Status::Playing => match self.feedback {
                Feedback::Ready if self.selected.is_some() => {
                    ("Jewel selected".into(), "Choose a neighbor to swap.".into())
                }
                Feedback::Ready => match STAGES[self.journal.active].goals[0] {
                    Goal::Score(_) => (
                        "Line up 3 or more".into(),
                        "Matches clear in rows or columns.".into(),
                    ),
                    Goal::Collect(..) => (
                        "Collect jewels".into(),
                        "Clear the listed jewels to fill their meters.".into(),
                    ),
                    Goal::Chains(_) => (
                        "Set off a chain".into(),
                        "A falling jewel must make another match.".into(),
                    ),
                },
                Feedback::Hint => ("Hint".into(), "Swap the two marked jewels.".into()),
                Feedback::NoMatch => ("No match".into(), "Make a line of three or more.".into()),
                Feedback::Turn {
                    points,
                    chain,
                    reshuffled,
                } => (
                    if chain == 1 {
                        format!("+{points} points")
                    } else {
                        format!("×{chain} chain · +{points}")
                    },
                    if reshuffled {
                        "Board reshuffled".into()
                    } else {
                        String::new()
                    },
                ),
            },
        }
    }

    pub(super) fn hud(
        &self,
        width: f32,
        compact: bool,
        displayed: Progress,
        cx: &Context<Self>,
    ) -> Div {
        let colors = t::colors(cx);
        let stage = STAGES[self.journal.active];
        let active_motion = self.motion.is_some();
        let status = if active_motion {
            Status::Playing
        } else {
            self.game.status()
        };
        let (headline, message) = self.status_text();
        let mut hud = div()
            .w(px(width))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(if compact { 10. } else { 15. }))
            .child(t::display_type(
                if self.challenge {
                    "Daily puzzle"
                } else {
                    stage.name
                },
                if compact { 17. } else { 25. },
            ))
            .when(!self.challenge, |hud| {
                hud.child(
                    div()
                        .flex()
                        .gap(px(4.))
                        .children(STAGES.iter().enumerate().map(|(index, stage)| {
                            let cleared = self.journal.is_cleared(index);
                            let unlocked = self.journal.is_unlocked(index);
                            let current = self.journal.active == index;
                            let label = format!(
                                "Stage {}, {}{}",
                                index + 1,
                                stage.name,
                                if !unlocked {
                                    ", locked"
                                } else if cleared {
                                    ", cleared"
                                } else {
                                    ""
                                },
                            );
                            self.button(
                                Action::Stage(index),
                                format!("{}", index + 1),
                                current,
                                unlocked && !current && !active_motion,
                                cx,
                            )
                            .flex_1()
                            .min_w_0()
                            .aria_label(label)
                            .aria_selected(current)
                            .when(cleared && !current, |button| {
                                button.bg(rgb(colors.hover)).text_color(rgb(colors.ink))
                            })
                        })),
                )
            })
            .child(
                div()
                    .flex()
                    .border_t_1()
                    .border_color(rgb(colors.border))
                    .pt(px(if compact { 8. } else { 12. }))
                    .gap(px(10.))
                    .child(metric(
                        "Score",
                        display_score(displayed.score),
                        false,
                        colors,
                    ))
                    .child(metric(
                        "Moves",
                        self.game.moves_left().to_string(),
                        self.game.moves_left() <= 5,
                        colors,
                    )),
            )
            .children(
                stage
                    .goals
                    .iter()
                    .map(|&goal| goal_meter(goal, displayed, colors)),
            );

        let feedback = div()
            .id("match-three-feedback")
            .role(Role::Status)
            .when(!headline.is_empty(), |feedback| {
                feedback.aria_label(if message.is_empty() {
                    headline.clone()
                } else {
                    format!("{headline}. {message}")
                })
            })
            .flex()
            .flex_col()
            .min_h(px(if compact { 39. } else { 43. }))
            .line_height(px(16.))
            .child(
                div()
                    .text_size(px(if compact { 13. } else { 16. }))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(if headline.is_empty() {
                        colors.muted
                    } else {
                        colors.gold_text
                    }))
                    .child(headline),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(colors.muted))
                    .child(message),
            );
        hud = hud.child(feedback);

        let mut buttons = div().flex().gap(px(5.)).flex_wrap();
        match status {
            Status::Playing => {
                buttons = buttons
                    .child(self.button(Action::Hint, "Hint", false, !active_motion, cx))
                    .child(self.button(Action::Replay, "Restart", false, !active_motion, cx))
                    .when(!self.challenge, |buttons| {
                        buttons.child(self.button(
                            Action::NewBoard,
                            "New board",
                            false,
                            !active_motion,
                            cx,
                        ))
                    });
            }
            Status::Lost => {
                buttons = buttons
                    .child(self.button(Action::Replay, "Retry", true, true, cx))
                    .when(!self.challenge, |buttons| {
                        buttons.child(self.button(Action::NewBoard, "New board", false, true, cx))
                    });
            }
            Status::Won => {
                buttons = if self.challenge {
                    buttons.child(self.button(Action::Replay, "Replay", true, true, cx))
                } else if self.journal.active + 1 < STAGES.len() {
                    buttons
                        .child(self.button(Action::NextStage, "Next stage", true, true, cx))
                        .child(self.button(Action::Replay, "Replay", false, true, cx))
                } else {
                    buttons
                        .child(self.button(Action::Replay, "Replay", true, true, cx))
                        .child(self.button(Action::NewBoard, "New board", false, true, cx))
                };
            }
        };
        let (best_score, best_chain) = self.journal.record();
        hud.child(buttons).when(best_score > 0, |hud| {
            hud.child(
                div()
                    .line_height(px(16.))
                    .text_size(px(11.))
                    .text_color(rgb(colors.muted))
                    .child(format!(
                        "Best {} · ×{best_chain} chain",
                        display_score(best_score)
                    )),
            )
        })
    }

    fn button(
        &self,
        action: Action,
        label: impl Into<SharedString>,
        primary: bool,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let colors = t::colors(cx);
        let label = label.into();
        let id = match action {
            Action::Hint => ("match-three-hint", 0),
            Action::Replay => ("match-three-replay", 0),
            Action::NewBoard => ("match-three-new", 0),
            Action::Stage(index) => ("match-three-stage", index),
            Action::NextStage => ("match-three-next", 0),
        };
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label.clone())
            .min_h(px(31.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.))
            .border_1()
            .border_color(rgb(if primary { t::GOLD } else { colors.border }))
            .px(px(if matches!(action, Action::Stage(_)) {
                1.
            } else {
                8.
            }))
            .bg(rgb(if primary { t::GOLD } else { colors.surface }))
            .text_color(rgb(if primary {
                t::INK
            } else if enabled {
                colors.ink
            } else {
                colors.muted
            }))
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(12.))
            .line_height(px(17.))
            .child(label)
            .when(enabled, |button| {
                button
                    .tab_index(0)
                    .cursor_pointer()
                    .hover(|style| {
                        style.bg(rgb(if primary { t::GOLD_HOVER } else { colors.hover }))
                    })
                    .focus(|style| {
                        style.border_color(rgb(if primary { t::INK } else { colors.gold_text }))
                    })
                    .on_click(
                        cx.listener(move |view, _, window, cx| view.perform(action, window, cx)),
                    )
            })
    }
}

fn display_score(value: u32) -> String {
    if value >= 1_000 {
        format!("{},{:03}", value / 1_000, value % 1_000)
    } else {
        value.to_string()
    }
}

fn metric(label: &'static str, value: String, urgent: bool, colors: t::Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .child(
            div()
                .text_size(px(11.))
                .line_height(px(16.))
                .text_color(rgb(colors.muted))
                .child(label),
        )
        .child(
            div()
                .text_size(px(24.))
                .line_height(px(27.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(if urgent { colors.gold_text } else { colors.ink }))
                .child(value),
        )
}

fn goal_meter(goal: Goal, displayed: Progress, colors: t::Palette) -> Stateful<Div> {
    let (name, jewel) = match goal {
        Goal::Score(_) => ("Points goal", None),
        Goal::Collect(gem, _) => (gem.name(), Some(gem)),
        Goal::Chains(_) => ("2+ match chains", None),
    };
    let target = goal.target();
    let progress = displayed.value(goal).min(target);
    let position = format!("{} / {}", display_score(progress), display_score(target));
    let percentage = progress as f32 / target as f32;
    div()
        .id(name)
        .role(Role::ProgressIndicator)
        .aria_label(format!("{name}, {position}"))
        .aria_numeric_value(f64::from(progress))
        .aria_min_numeric_value(0.)
        .aria_max_numeric_value(f64::from(target))
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(12.))
                .line_height(px(17.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .children(jewel.map(|gem| field::jewel(gem).size(px(17.))))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(name),
                )
                .child(div().text_color(rgb(colors.muted)).child(position)),
        )
        .child(
            div()
                .w_full()
                .h(px(4.))
                .overflow_hidden()
                .rounded_full()
                .bg(rgb(colors.hover))
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(percentage))
                        .rounded_full()
                        .bg(rgb(if progress == target {
                            colors.gold_text
                        } else {
                            0xb18741
                        })),
                ),
        )
}
