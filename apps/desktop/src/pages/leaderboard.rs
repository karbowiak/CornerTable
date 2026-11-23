use corner_table_api::{Period, Standing};
use gpui::{Context, Div, FontWeight, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop, Page};
use crate::{theme as t, widgets};

pub fn leaderboard(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let daily = &app.api.home.challenge;
    let board = app.api.leaderboard(&app.board_game, app.board_period);
    let guest = &app.api.home.guest;
    let headings = row(
        div().child("Rank"),
        div().child("Player"),
        "Score".into(),
        "Time".into(),
    )
    .bg(rgb(0xefe9dd))
    .text_color(rgb(t::MUTED))
    .font_weight(FontWeight::BOLD);
    let table = t::surface()
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
                    )
                    .border_t_1()
                    .border_color(rgb(t::BORDER))
                }))
            }
            _ => table.child(
                div()
                    .p_6()
                    .text_color(rgb(t::MUTED))
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
        .child(t::heading("Leaderboard"))
        .child(widgets::hero(
            "Today’s Challenge",
            &daily.title,
            widgets::challenge_art(&daily.game_id.0),
            widgets::button(
                "board-daily",
                "View challenge",
                Action::Navigate(Page::DailyChallenge),
                true,
                cx,
            ),
        ))
        .child(filters(app, narrow, cx))
        .child(table)
        .children(guest_row.map(|entry| {
            t::surface()
                .overflow_hidden()
                .child(standing_row(entry, Some(&guest.initials)))
        }))
        .child(
            div()
                .text_size(px(13.))
                .text_color(rgb(t::MUTED))
                .child("Preview rankings · Sample data"),
        )
}

fn filters(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
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
            filters.child(t::surface().p_3().child(widgets::columns(
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
                        t::SAGE
                    } else {
                        t::SURFACE
                    }))
                    .hover(|style| style.bg(rgb(t::SAGE)))
                    .focus(|style| style.bg(rgb(t::SAGE)))
                    .child(game.title.clone())
                }),
                games_per_row,
                8.,
            )))
        })
}

fn standing_row(entry: &Standing, guest: Option<&str>) -> Div {
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
        format!("{} (you)", entry.name)
    } else {
        entry.name.clone()
    };
    let rank = div()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(if entry.rank == 1 { 0x855713 } else { t::INK }))
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
        row.bg(rgb(t::SAGE)).font_weight(FontWeight::BOLD)
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
