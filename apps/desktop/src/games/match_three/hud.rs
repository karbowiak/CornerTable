use corner_table_match_three::{Goal, Progress, STAGES, Status};
use gpui::{
    AnyElement, Context, Div, FontWeight, Role, SharedString, Stateful, div, prelude::*, px, rgb,
    rgba,
};

use crate::{
    games::{round_result, stage},
    storage::Outcome,
    theme as t,
};

use super::{Action, Feedback, GameView, field};

impl GameView {
    fn status_text(&self) -> (String, String) {
        if let Some(motion) = &self.motion {
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
        match self.feedback {
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
                Goal::ClearMarked(_) => (
                    "Clear the marked cells".into(),
                    if STAGES[self.journal.active].goals.len() > 1 {
                        "Match on marked cells and collect the listed jewels."
                    } else {
                        "Match jewels on a marked cell to clear it."
                    }
                    .into(),
                ),
            },
            Feedback::Hint => ("Hint".into(), "Swap the highlighted jewels.".into()),
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
        }
    }

    pub(super) fn hud(&self, width: f32, displayed: Progress, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let stage = STAGES[self.journal.active];
        let status = if self.motion.is_some() {
            Status::Playing
        } else {
            self.game.status()
        };
        let (headline, message) = self.status_text();
        // The rail's sections are padded by 16 on each side.
        let inner = width - 34.;
        let (best_score, best_chain) = self.journal.record();
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
            .min_h(px(43.))
            .line_height(px(18.))
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(if headline.is_empty() {
                        colors.muted
                    } else {
                        colors.gold_text
                    }))
                    .child(if headline.is_empty() {
                        "Make a match".to_owned()
                    } else {
                        headline
                    }),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(colors.muted))
                    .child(message),
            );
        stage::rail(cx)
            .child(
                stage::rail_section(cx)
                    .child(t::display_type(
                        if self.challenge {
                            "Daily puzzle"
                        } else {
                            stage.name
                        },
                        21.,
                    ))
                    .when(!self.challenge, |section| {
                        section.child(self.stage_picker(inner, cx))
                    }),
            )
            .child(
                stage::rail_section(cx)
                    .child(
                        div()
                            .flex()
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
                    )
                    .when(best_score > 0, |section| {
                        section.child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(colors.muted))
                                .child(format!(
                                    "Best {} · ×{best_chain} chain",
                                    display_score(best_score)
                                )),
                        )
                    }),
            )
            .when(status == Status::Playing, |rail| {
                rail.child(
                    stage::rail_section(cx)
                        .child(feedback)
                        .child(self.round_actions(status, cx)),
                )
            })
            .child(stage::controls_help(
                cx,
                &[
                    "Drag or click an adjacent pair to swap",
                    "Arrows move · Space selects and swaps",
                ],
            ))
    }

    pub(super) fn result(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if self.motion.is_some() {
            return None;
        }
        let status = self.game.status();
        let (outcome, title) = match status {
            Status::Playing => return None,
            Status::Lost => (Outcome::Lost, "Out of moves"),
            Status::Won if self.challenge => (Outcome::Won, "Daily puzzle cleared"),
            Status::Won if self.journal.active + 1 == STAGES.len() => {
                (Outcome::Won, "All stages cleared")
            }
            Status::Won => (Outcome::Won, "Stage cleared"),
        };
        let progress = self.game.progress();
        let goals = STAGES[self.journal.active].goals;
        let met = goals
            .iter()
            .filter(|&&goal| progress.value(goal) >= goal.target())
            .count();
        let moves = self.game.moves_left();
        let score = display_score(progress.score);
        let best = display_score(self.journal.record().0);
        let record = if self.new_best {
            "New personal best".to_owned()
        } else {
            format!("Best {best}")
        };
        let mut detail = format!(
            "{score} points · {met}/{} goals · {moves} moves left · {record}",
            goals.len()
        );
        if progress.best_chain > 1 {
            detail.push_str(&format!(" · ×{} best chain", progress.best_chain));
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded_lg()
                .px_4()
                .bg(rgba(0x171221b0))
                .child(round_result(
                    cx,
                    "match-three-result",
                    outcome,
                    title,
                    detail,
                    self.round_actions(status, cx),
                ))
                .into_any_element(),
        )
    }

    fn round_actions(&self, status: Status, cx: &Context<Self>) -> Div {
        let buttons = div().flex().gap_2().flex_wrap();
        match status {
            Status::Playing => {
                let enabled = self.motion.is_none();
                buttons
                    .child(self.button(Action::Hint, "Hint", false, enabled, cx))
                    .child(self.button(Action::Replay, "Restart", false, enabled, cx))
                    .when(!self.challenge, |buttons| {
                        buttons.child(self.button(
                            Action::NewBoard,
                            "New board",
                            false,
                            enabled,
                            cx,
                        ))
                    })
            }
            Status::Lost => buttons
                .child(self.button(Action::Replay, "Retry", true, true, cx))
                .when(!self.challenge, |buttons| {
                    buttons.child(self.button(Action::NewBoard, "New board", false, true, cx))
                }),
            Status::Won if self.challenge => {
                buttons.child(self.button(Action::Replay, "Replay", true, true, cx))
            }
            Status::Won if self.journal.active + 1 < STAGES.len() => buttons
                .child(self.button(Action::NextStage, "Next stage", true, true, cx))
                .child(self.button(Action::Replay, "Replay", false, true, cx)),
            Status::Won => buttons
                .child(self.button(Action::Replay, "Replay", true, true, cx))
                .child(self.button(Action::NewBoard, "New board", false, true, cx)),
        }
    }

    fn stage_picker(&self, width: f32, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let columns = if width >= 236. { STAGES.len() } else { 4 };
        let gap = 4.;
        let cell_width = ((width + gap) / columns as f32 - gap).floor();
        div()
            .flex()
            .flex_wrap()
            .gap(px(gap))
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
                    (index + 1).to_string(),
                    current,
                    unlocked && !current && self.motion.is_none(),
                    cx,
                )
                .w(px(cell_width))
                .min_w_0()
                .aria_label(label)
                .aria_selected(current)
                .when(!unlocked, |button| {
                    button.bg(rgb(colors.paper)).text_color(rgb(colors.muted))
                })
                .when(cleared && !current, |button| {
                    button.bg(rgb(colors.hover)).text_color(rgb(colors.ink))
                })
            }))
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
            .debug_selector(move || format!("{}-{}", id.0, id.1))
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
    let name = match goal {
        Goal::Score(_) => "Points goal",
        Goal::Collect(gem, _) => gem.name(),
        Goal::Chains(_) => "2+ match chains",
        Goal::ClearMarked(_) => "Marked cells",
    };
    let target = goal.target();
    let progress = displayed.value(goal).min(target);
    let position = format!("{} / {}", display_score(progress), display_score(target));
    let percentage = progress as f32 / target.max(1) as f32;
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
                        .when_some(
                            match goal {
                                Goal::Collect(gem, _) => {
                                    Some(field::jewel(gem).size(px(17.)).into_any_element())
                                }
                                Goal::ClearMarked(_) => Some(field::inlay(17.).into_any_element()),
                                _ => None,
                            },
                            |label, icon| label.child(icon),
                        )
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
