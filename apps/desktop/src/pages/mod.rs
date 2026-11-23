mod guide;
mod leaderboard;
mod library;
mod solitaire;

pub use guide::GuideOverlay;
pub use leaderboard::leaderboard;
pub use library::{LibraryFilter, library, matching_games};
pub use solitaire::solitaire;

use gpui::{AnyView, Context, Div, Role, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop, Page};
use crate::games::{GameEntry, GameLaunch};
use crate::{daily, theme as t, widgets};

pub fn daily_challenge(cx: &Context<Desktop>) -> Div {
    let today = daily::today();
    let palette = t::colors(cx);
    let completed = daily::completed_days();
    let streak = daily::current_streak(today.date, &completed);
    let won_today = completed.contains(&today.date);
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
                .child(t::display_type("Daily Challenge", 36.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .when(streak != 0, |details| {
                            details.child(
                                div()
                                    .text_color(rgb(palette.success))
                                    .child(format!("{streak}-day streak")),
                            )
                        })
                        .child(
                            div()
                                .text_color(rgb(palette.muted))
                                .child(today.date.format("%A, %B %-d").to_string()),
                        ),
                ),
        )
        .child(widgets::hero(
            today.title,
            widgets::challenge_art(today.game),
            widgets::button(
                "launch-daily",
                if won_today {
                    "Play again"
                } else {
                    "Play today’s puzzle"
                },
                Action::Navigate(Page::Game(today.launch())),
                true,
                cx,
            ),
        ))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(div().text_color(rgb(palette.muted)).child(today.objective))
                .when(won_today, |status| {
                    status.child(div().text_color(rgb(palette.success)).child("Completed"))
                }),
        )
        .child(
            t::panel(cx)
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .child(t::display_type("Previous puzzles", 22.))
                .children(daily::previous(today.date).map(|puzzle| {
                    let date = puzzle.date.format("%a, %b %-d").to_string();
                    let won = completed.contains(&puzzle.date);
                    let status = if won {
                        format!("{date} · Completed")
                    } else {
                        date.clone()
                    };
                    widgets::clickable(
                        gpui::SharedString::from(format!("daily-archive-{}", puzzle.date)),
                        Action::Navigate(Page::Game(puzzle.launch())),
                        cx,
                    )
                    .aria_label(format!("Play {} from {status}", puzzle.title))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(rgb(palette.border))
                    .hover(|style| style.bg(rgb(palette.hover)))
                    .focus_visible(|style| style.border_color(rgb(palette.ink)))
                    .child(puzzle.title)
                    .child(
                        div()
                            .text_color(rgb(if won { palette.success } else { palette.muted }))
                            .child(status),
                    )
                })),
        )
}

pub fn game(
    app: &Desktop,
    launch: &GameLaunch,
    view: Option<AnyView>,
    compact: bool,
    cx: &Context<Desktop>,
) -> Div {
    let favorite = app.favorites.contains(&launch.game);
    let daily = daily::from_launch(launch);
    let compact_daily = compact && daily.is_some();
    let (title, description) = app
        .api
        .game(&launch.game)
        .map(|game| (game.title.as_str(), game.description.as_str()))
        .unwrap_or(("Game unavailable", "This game is missing from the library."));
    let (title, description) = match &launch.entry {
        GameEntry::Mode(id) if launch.game.0 == "solitaire" => {
            let mode = app
                .api
                .library
                .solitaire_modes
                .iter()
                .find(|mode| &mode.id == id);
            mode.map(|mode| (mode.title.as_str(), mode.description.as_str()))
                .unwrap_or((title, description))
        }
        GameEntry::Challenge(id) => {
            let daily = &app.api.home.challenge;
            let description = if id == &daily.id {
                daily.description.as_str()
            } else {
                description
            };
            (title, description)
        }
        _ => (title, description),
    };
    let header = div().flex().flex_col().items_start().gap_3().child(
        div()
            .w_full()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .child(t::display_type(
                title.to_owned(),
                if compact_daily { 25. } else { 30. },
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(if compact_daily { 6. } else { 8. }))
                    .children(daily.map(|puzzle| {
                        let date = puzzle.date.format("%b %-d, %Y");
                        let full_label = format!("Daily puzzle · {date}");
                        let label = if compact {
                            format!("Daily · {}", puzzle.date.format("%b %-d"))
                        } else {
                            full_label.clone()
                        };
                        div()
                            .id("game-daily-date")
                            .role(Role::Label)
                            .aria_label(full_label)
                            .text_size(px(if compact_daily { 12. } else { 13. }))
                            .text_color(rgb(t::colors(cx).muted))
                            .child(label)
                    }))
                    .children(
                        app.guide
                            .button(launch, cx)
                            .map(|button| button.when(compact_daily, |button| button.px_2())),
                    )
                    .child(
                        widgets::button(
                            "game-favorite",
                            if favorite {
                                "Remove favorite"
                            } else {
                                "Add favorite"
                            },
                            Action::ToggleFavorite(launch.game.clone()),
                            false,
                            cx,
                        )
                        .min_h(px(34.))
                        .when(compact, |button| button.px_3().text_size(px(13.)))
                        .when(compact_daily, |button| button.px_2()),
                    ),
            ),
    );

    match view {
        Some(view) => header.child(div().w_full().child(view)),
        None => header.child(
            t::panel(cx)
                .w_full()
                .overflow_hidden()
                .child(
                    div()
                        .h(px(240.))
                        .w_full()
                        .overflow_hidden()
                        .child(widgets::artwork(&launch.game.0)),
                )
                .child(
                    div()
                        .p_6()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(t::display_type("Game in development", 24.))
                        .child(
                            div()
                                .text_color(rgb(t::colors(cx).muted))
                                .child(description.to_owned()),
                        ),
                ),
        ),
    }
}
