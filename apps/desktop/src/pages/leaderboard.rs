use chrono::{DateTime, Local};
use corner_table_api::{Period, Standing};
use gpui::{Context, Div, FontWeight, Role, Stateful, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop, Page};
use crate::storage::{self, GameResult, Outcome, ResultMetric};
use crate::{theme as t, widgets};

const RECENT_RESULTS: usize = 5;

pub fn leaderboard(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let palette = t::colors(cx);
    let board = app.api.leaderboard(&app.board_game, app.board_period);
    let guest = &app.api.home.guest;
    let headings = row(
        div().child("Rank"),
        div().child("Player"),
        "Score".into(),
        "Time".into(),
    )
    .bg(rgb(palette.hover))
    .text_color(rgb(palette.muted))
    .font_weight(FontWeight::BOLD);
    let table = t::panel(cx)
        .overflow_hidden()
        .child(headings)
        .map(|table| match board {
            Some(board) if !board.leaders.is_empty() => {
                table.children(board.leaders.iter().map(|entry| {
                    standing_row(
                        entry,
                        if entry.player_id == guest.id {
                            Some(&guest.initials)
                        } else {
                            None
                        },
                        palette,
                    )
                    .border_t_1()
                    .border_color(rgb(palette.border))
                }))
            }
            _ => table.child(
                div()
                    .p_6()
                    .text_color(rgb(palette.muted))
                    .child("No sample scores for this game and period."),
            ),
        });

    let guest_row = board.and_then(|board| {
        board.guest_standing.as_ref().filter(|guest| {
            !board
                .leaders
                .iter()
                .any(|leader| leader.player_id == guest.player_id)
        })
    });
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    t::display_type("Leaderboard", 36.)
                        .id("board-title")
                        .role(Role::Heading)
                        .aria_level(1)
                        .aria_label("Leaderboard"),
                )
                .child(widgets::button(
                    "board-daily",
                    "View daily challenge",
                    Action::Navigate(Page::DailyChallenge),
                    false,
                    cx,
                )),
        )
        .child(your_results(app, cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    t::display_type("Sample rankings", 23.)
                        .id("board-sample-title")
                        .role(Role::Heading)
                        .aria_level(2)
                        .aria_label("Sample rankings"),
                )
                .child(filters(app, narrow, cx))
                .child(table)
                .children(guest_row.map(|entry| {
                    t::panel(cx).overflow_hidden().child(standing_row(
                        entry,
                        Some(&guest.initials),
                        palette,
                    ))
                })),
        )
}

fn your_results(app: &Desktop, cx: &Context<Desktop>) -> Div {
    const EMPTY_HELP: &str = "Clear a Solitaire or Mahjong board, finish a Match Three stage, or complete a Gem Cascade run.";
    let palette = t::colors(cx);
    let recent = storage::recent_results(RECENT_RESULTS);
    let panel = t::panel(cx).id("board-results").overflow_hidden();
    let panel = if recent.is_empty() {
        panel.child(
            div()
                .id("board-results-empty")
                .role(Role::Label)
                .aria_label("No results yet")
                .aria_description(EMPTY_HELP)
                .px_4()
                .py_3()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("No results yet"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(rgb(palette.muted))
                        .child(EMPTY_HELP),
                ),
        )
    } else {
        panel
            .role(Role::List)
            .aria_label("Recent results saved on this device")
            .children(recent.iter().enumerate().map(|(index, result)| {
                let title = app
                    .api
                    .game(&result.game)
                    .map_or(result.game.0.as_str(), |game| game.title.as_str());
                result_row(index, result, title, palette).when(index > 0, |row| {
                    row.border_t_1().border_color(rgb(palette.border))
                })
            }))
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .gap_2()
                .child(
                    t::display_type("Your results", 23.)
                        .id("board-results-title")
                        .role(Role::Heading)
                        .aria_level(2)
                        .aria_label("Your results"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(palette.muted))
                        .child("Saved on this device"),
                ),
        )
        .child(panel)
}

fn result_row(
    index: usize,
    result: &GameResult,
    title: &str,
    palette: t::Palette,
) -> Stateful<Div> {
    let (outcome, outcome_color) = match result.outcome {
        Outcome::Won => ("Won", palette.success),
        Outcome::Lost => ("Lost", palette.muted),
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
        ResultMetric::Points(count) => count_label(count, "point", "points"),
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
        .id(("board-result", index))
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
}

fn filters(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let palette = t::colors(cx);
    let games_per_row = if narrow { 2 } else { 4 };
    let title = app
        .api
        .game(&app.board_game)
        .map(|game| game.title.as_str())
        .unwrap_or("Choose game");
    let picker_label = format!("{title}  {}", if app.game_picker_open { "▴" } else { "▾" });
    let periods = [
        ("board-today", "Today", Period::Today),
        ("board-week", "This week", Period::ThisWeek),
        ("board-all-time", "All time", Period::AllTime),
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
                        "board-game-picker",
                        picker_label,
                        Action::ToggleGamePicker,
                        false,
                        cx,
                    )
                    .min_w(px(156.))
                    .justify_between(),
                )
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .children(periods.map(|(id, title, period)| {
                            widgets::button(
                                id,
                                title,
                                Action::SelectPeriod(period),
                                period == app.board_period,
                                cx,
                            )
                        })),
                ),
        )
        .when(app.game_picker_open, |filters| {
            filters.child(t::panel(cx).p_3().child(widgets::columns(
                app.api.library.games.iter().map(|game| {
                    widgets::clickable(
                        gpui::SharedString::from(format!("board-filter-{}", game.id.0)),
                        Action::SelectGame(game.id.clone()),
                        cx,
                    )
                    .rounded_md()
                    .px_3()
                    .py_2()
                    .truncate()
                    .bg(rgb(if game.id == app.board_game {
                        palette.hover
                    } else {
                        palette.surface
                    }))
                    .hover(|style| style.bg(rgb(palette.hover)))
                    .focus(|style| style.bg(rgb(palette.hover)))
                    .child(game.title.clone())
                }),
                games_per_row,
                8.,
            )))
        })
}

fn standing_row(entry: &Standing, guest: Option<&str>, palette: t::Palette) -> Div {
    let initials = guest.map(str::to_owned).unwrap_or_else(|| {
        entry
            .name
            .chars()
            .take(2)
            .flat_map(char::to_uppercase)
            .collect()
    });
    let colors = [0x355a48, 0x516d87, 0x596a3e, 0x89563b, 0x35636e];
    let background = if guest.is_some() {
        t::RAIL
    } else {
        colors[entry.rank.saturating_sub(1) as usize % colors.len()]
    };
    let player = if guest.is_some() {
        format!("{} (sample guest)", entry.name)
    } else {
        entry.name.clone()
    };
    let rank = div()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(if entry.rank == 1 {
            palette.gold_text
        } else {
            palette.ink
        }))
        .child(entry.rank.to_string());
    let player = div()
        .flex()
        .items_center()
        .gap_3()
        .min_w_0()
        .child(widgets::avatar(&initials, background, t::PAPER))
        .child(div().truncate().child(player));
    let elapsed = format!(
        "{:02}:{:02}",
        entry.elapsed_seconds / 60,
        entry.elapsed_seconds % 60
    );
    row(rank, player, format_score(entry.score), elapsed).when(guest.is_some(), |row| {
        row.bg(rgb(palette.hover)).font_weight(FontWeight::BOLD)
    })
}

fn row(rank: Div, player: Div, score: String, time: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap_3()
        .min_h(px(51.))
        .px_5()
        .child(rank.w(px(49.)).flex_shrink_0())
        .child(player.flex_1().min_w_0())
        .child(div().w(px(100.)).flex_shrink_0().text_right().child(score))
        .child(div().w(px(76.)).flex_shrink_0().text_right().child(time))
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
