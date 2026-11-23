use std::collections::HashMap;

use chrono::{DateTime, Datelike as _, Days, Local, NaiveDate};
use corner_table_api::{GameId, Period};
use gpui::{Context, Div, FontWeight, Role, SharedString, Stateful, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop, Page};
use crate::storage::{GameResult, Outcome, ResultMetric};
use crate::{daily, theme as t, widgets};

const PAGE_SIZE: usize = 12;

#[derive(Clone)]
pub enum RecordAction {
    OpenPicker,
    FilterGame(Option<GameId>),
    FilterPeriod(Period),
    ShowBests(bool),
    ShowMore,
}

pub struct Records {
    pub history: Option<Vec<GameResult>>,
    game: Option<GameId>,
    period: Period,
    picker_open: bool,
    bests: bool,
    visible: usize,
}

impl Default for Records {
    fn default() -> Self {
        Self {
            history: None,
            game: None,
            period: Period::AllTime,
            picker_open: false,
            bests: false,
            visible: PAGE_SIZE,
        }
    }
}

impl Records {
    pub fn close_picker(&mut self) {
        self.picker_open = false;
    }

    pub fn apply(&mut self, action: RecordAction) {
        match action {
            RecordAction::OpenPicker => {
                self.picker_open = !self.picker_open;
                return;
            }
            RecordAction::ShowMore => {
                self.visible = self.visible.saturating_add(PAGE_SIZE);
                return;
            }
            RecordAction::FilterGame(game) => {
                self.game = game;
                self.close_picker();
            }
            RecordAction::FilterPeriod(period) => self.period = period,
            RecordAction::ShowBests(bests) => self.bests = bests,
        }
        self.visible = PAGE_SIZE;
    }
}

pub fn records(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let palette = t::colors(cx);
    let state = &app.records;
    let today = Local::now().date_naive();
    let rounds: Vec<_> = state
        .history
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|result| {
            state.game.as_ref().is_none_or(|game| &result.game == game)
                && in_period(result, state.period, today)
        })
        .collect();
    let count = rounds.len();
    let total = |outcome| {
        rounds
            .iter()
            .filter(|result| result.outcome == outcome)
            .count()
    };
    let (wins, losses, draws) = (
        total(Outcome::Won),
        total(Outcome::Lost),
        total(Outcome::Draw),
    );
    let listed = if state.bests {
        personal_bests(rounds)
    } else {
        rounds
    };
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .gap_3()
                .child(
                    t::display_type("Records", if narrow { 32. } else { 36. })
                        .id("records-title")
                        .role(Role::Heading)
                        .aria_level(1)
                        .aria_label("Records"),
                )
                .when(count > 0, |line| {
                    let mut summary = format!(
                        "{count} {} · {wins} {}",
                        if count == 1 { "round" } else { "rounds" },
                        if wins == 1 { "win" } else { "wins" },
                    );
                    if losses > 0 {
                        let label = if losses == 1 { "loss" } else { "losses" };
                        summary.push_str(&format!(" · {losses} {label}"));
                    }
                    if draws > 0 {
                        let label = if draws == 1 { "draw" } else { "draws" };
                        summary.push_str(&format!(" · {draws} {label}"));
                    }
                    line.child(
                        div()
                            .id("records-count")
                            .role(Role::Label)
                            .aria_label(summary.clone())
                            .text_size(px(13.))
                            .text_color(rgb(palette.muted))
                            .child(summary),
                    )
                }),
        )
        .child(filters(app, narrow, cx))
        .child(div().flex().gap_2().children([false, true].map(|bests| {
            let (id, title) = if bests {
                ("records-bests", "Personal bests")
            } else {
                ("records-recent", "Recent results")
            };
            widgets::button(
                id,
                title,
                Action::Record(RecordAction::ShowBests(bests)),
                state.bests == bests,
                cx,
            )
            .min_h(px(34.))
            .aria_toggled((state.bests == bests).into())
        })))
        .child(
            t::panel(cx)
                .id("records-list")
                .overflow_hidden()
                .role(Role::List)
                .aria_label(if state.bests {
                    "Personal bests"
                } else {
                    "Recent results"
                })
                .when(listed.is_empty(), |panel| {
                    let text = if state.history.is_none() {
                        "Loading records…"
                    } else if state.history.as_ref().is_some_and(Vec::is_empty) {
                        "Finish a round to start your records."
                    } else if count > 0 {
                        "No wins or completed runs for this selection."
                    } else {
                        "No recorded rounds for this selection."
                    };
                    panel.child(div().p_4().text_color(rgb(palette.muted)).child(text))
                })
                .children(
                    listed
                        .iter()
                        .take(state.visible)
                        .enumerate()
                        .map(|(index, result)| {
                            let game = app.api.game(&result.game);
                            let title = game.map_or(result.game.0.as_str(), |game| &game.title);
                            let open_game = game.map(|_| {
                                result
                                    .launch
                                    .as_ref()
                                    .filter(|launch| launch.game == result.game)
                                    .map_or_else(
                                        || ("Open", Action::OpenGame(result.game.clone())),
                                        |launch| {
                                            let label = if daily::from_launch(launch).is_some() {
                                                "Open daily"
                                            } else {
                                                "Open"
                                            };
                                            (label, Action::Navigate(Page::Game(launch.clone())))
                                        },
                                    )
                            });
                            result_row(index, result, title, open_game, cx).when(index > 0, |row| {
                                row.border_t_1().border_color(rgb(palette.border))
                            })
                        }),
                ),
        )
        .when(listed.len() > state.visible, |page| {
            page.child(div().child(widgets::button(
                "records-more",
                "Show more",
                Action::Record(RecordAction::ShowMore),
                false,
                cx,
            )))
        })
}

fn in_period(result: &GameResult, period: Period, today: NaiveDate) -> bool {
    if period == Period::AllTime {
        return true;
    }
    let start = match period {
        Period::ThisWeek => today
            .checked_sub_days(Days::new(today.weekday().num_days_from_monday().into()))
            .unwrap_or(today),
        _ => today,
    };
    DateTime::from_timestamp_millis(result.completed_at_ms)
        .map(|date| date.with_timezone(&Local).date_naive())
        .is_some_and(|date| (start..=today).contains(&date))
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum MetricKind {
    Moves,
    Pairs,
    Discs,
    Points,
    Penalties,
    Time,
}

fn metric_rank(metric: ResultMetric) -> (MetricKind, u64) {
    match metric {
        ResultMetric::Moves(count) => (MetricKind::Moves, u64::MAX - count),
        ResultMetric::Pairs(count) => (MetricKind::Pairs, count),
        ResultMetric::Discs(count) => (MetricKind::Discs, count),
        ResultMetric::Points(count) => (MetricKind::Points, count),
        ResultMetric::PenaltyPoints(count) => (MetricKind::Penalties, u64::MAX - count),
        ResultMetric::TimeMillis(milliseconds) => (MetricKind::Time, u64::MAX - milliseconds),
    }
}

fn personal_bests(rounds: Vec<&GameResult>) -> Vec<&GameResult> {
    let mut best = HashMap::new();
    for result in rounds {
        if !matches!(result.outcome, Outcome::Won | Outcome::Finished) {
            continue;
        }
        let (kind, rank) = metric_rank(result.metric);
        let entry = best.entry((&result.game, result.mode.as_str(), kind));
        let value = entry.or_insert((result, rank));
        if rank > value.1 {
            *value = (result, rank);
        }
    }
    let mut best: Vec<_> = best.into_values().map(|(result, _)| result).collect();
    best.sort_by(|left, right| {
        left.game
            .0
            .cmp(&right.game.0)
            .then_with(|| left.mode.cmp(&right.mode))
            .then_with(|| right.completed_at_ms.cmp(&left.completed_at_ms))
    });
    best
}

fn result_row(
    index: usize,
    result: &GameResult,
    title: &str,
    open_game: Option<(&'static str, Action)>,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let palette = t::colors(cx);
    let (outcome, outcome_color) = match result.outcome {
        Outcome::Won => ("Won", palette.success),
        Outcome::Lost => ("Lost", palette.danger),
        Outcome::Draw => ("Draw", palette.muted),
        Outcome::Finished => ("Finished", palette.ink),
    };
    let count_label = |count, singular, plural| {
        format!(
            "{} {}",
            format_score(count),
            if count == 1 { singular } else { plural }
        )
    };
    let metric = match result.metric {
        ResultMetric::Moves(count) => count_label(count, "move", "moves"),
        ResultMetric::Pairs(count) => count_label(count, "pair", "pairs"),
        ResultMetric::Discs(count) => count_label(count, "disc", "discs"),
        ResultMetric::Points(count) => count_label(count, "point", "points"),
        ResultMetric::PenaltyPoints(count) => count_label(count, "penalty point", "penalty points"),
        ResultMetric::TimeMillis(milliseconds) => {
            let seconds = milliseconds / 1_000;
            format!(
                "{}:{:02}.{}",
                seconds / 60,
                seconds % 60,
                milliseconds / 100 % 10
            )
        }
    };
    let date = DateTime::from_timestamp_millis(result.completed_at_ms).map_or_else(
        || "Date unavailable".to_owned(),
        |date| {
            date.with_timezone(&Local)
                .format("%-d %b %Y · %H:%M")
                .to_string()
        },
    );
    div()
        .id(("record", index))
        .role(Role::ListItem)
        .aria_label(format!(
            "{title}, {}, {outcome}, {metric}, {date}",
            result.mode
        ))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_4()
        .py_2()
        .line_height(px(18.))
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_owned()),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(rgb(palette.muted))
                        .child(result.mode.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .flex_shrink_0()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap_2()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(div().text_color(rgb(outcome_color)).child(outcome))
                        .child(div().text_color(rgb(palette.muted)).child("·"))
                        .child(metric),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(palette.muted))
                        .child(date),
                ),
        )
        .children(open_game.map(|(label, action)| {
            widgets::button("open-record-game", label, action, false, cx)
                .aria_label(format!("Open {title}, {}", result.mode))
                .min_h(px(32.))
                .flex_shrink_0()
                .px_2()
                .text_size(px(13.))
        }))
}

fn filters(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let palette = t::colors(cx);
    let state = &app.records;
    let title = state
        .game
        .as_ref()
        .and_then(|id| app.api.game(id))
        .map_or("All games", |game| game.title.as_str());
    let picker_label = format!("{title}  {}", if state.picker_open { "▴" } else { "▾" });
    let periods = [
        ("records-today", "Today", Period::Today),
        ("records-week", "This week", Period::ThisWeek),
        ("records-all-time", "All time", Period::AllTime),
    ];

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .flex_wrap()
                .gap_2()
                .child(
                    widgets::button(
                        "records-game-picker",
                        picker_label,
                        Action::Record(RecordAction::OpenPicker),
                        false,
                        cx,
                    )
                    .aria_label(format!("Filter game, {title}"))
                    .aria_expanded(state.picker_open)
                    .min_w(px(164.))
                    .justify_between(),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .children(periods.map(|(id, title, period)| {
                            widgets::button(
                                id,
                                title,
                                Action::Record(RecordAction::FilterPeriod(period)),
                                state.period == period,
                                cx,
                            )
                            .aria_toggled((state.period == period).into())
                        })),
                ),
        )
        .when(state.picker_open, |filters| {
            let games = std::iter::once((None, "All games")).chain(
                app.api
                    .library
                    .games
                    .iter()
                    .map(|game| (Some(&game.id), game.title.as_str())),
            );
            filters.child(t::panel(cx).p_3().child(widgets::columns(
                games.map(|(id, title)| {
                    let current = state.game.as_ref() == id;
                    let key = id.map_or("all", |game| game.0.as_str());
                    widgets::clickable(
                        SharedString::from(format!("records-filter-{key}")),
                        Action::Record(RecordAction::FilterGame(id.cloned())),
                        cx,
                    )
                    .aria_label(title.to_owned())
                    .aria_toggled(current.into())
                    .rounded_md()
                    .px_3()
                    .py_2()
                    .truncate()
                    .bg(rgb(if current {
                        palette.hover
                    } else {
                        palette.surface
                    }))
                    .hover(|style| style.bg(rgb(palette.hover)))
                    .focus_visible(|style| style.bg(rgb(palette.hover)))
                    .child(title.to_owned())
                }),
                if narrow { 2 } else { 4 },
                8.,
            )))
        })
}

fn format_score(score: u64) -> String {
    let digits = score.to_string();
    let mut formatted = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index != 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(digit);
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bests_rank_only_completions_and_keep_metrics_and_modes_separate() {
        let round = |mode: &str, outcome, metric| GameResult {
            game: GameId("game".into()),
            launch: None,
            mode: mode.into(),
            outcome,
            metric,
            completed_at_ms: 0,
        };
        let rounds = [
            round("Casual", Outcome::Won, ResultMetric::TimeMillis(4_000)),
            round("Casual", Outcome::Lost, ResultMetric::TimeMillis(1_000)),
            round("Casual", Outcome::Won, ResultMetric::TimeMillis(2_000)),
            round("Casual", Outcome::Finished, ResultMetric::Points(700)),
            round("Casual", Outcome::Finished, ResultMetric::Points(900)),
            round("Timed", Outcome::Won, ResultMetric::TimeMillis(8_000)),
        ];
        let bests = personal_bests(rounds.iter().collect());
        assert_eq!(bests.len(), 3);
        for expected in [2, 4, 5] {
            assert!(
                bests
                    .iter()
                    .any(|best| std::ptr::eq(*best, &rounds[expected]))
            );
        }
    }
}
