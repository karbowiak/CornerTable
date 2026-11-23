mod leaderboard;
mod library;
mod solitaire;

pub use leaderboard::leaderboard;
pub use library::library;
pub use solitaire::solitaire;

use gpui::{AnyView, Context, Div, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop, Page};
use crate::games::{GameEntry, GameLaunch};
use crate::{daily, theme as t, widgets};

pub fn daily_challenge(cx: &Context<Desktop>) -> Div {
    let today = daily::today();
    let palette = t::colors(cx);
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
                        .text_color(rgb(palette.muted))
                        .child(today.date.format("%A, %B %-d").to_string()),
                ),
        )
        .child(widgets::hero(
            today.title,
            widgets::challenge_art(today.game),
            widgets::button(
                "launch-daily",
                "Play today’s puzzle",
                Action::Navigate(Page::Game(today.launch())),
                true,
                cx,
            ),
        ))
        .child(div().text_color(rgb(palette.muted)).child(today.objective))
        .child(
            t::panel(cx)
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .child(t::display_type("Previous puzzles", 22.))
                .children(daily::previous(today.date).map(|puzzle| {
                    let date = puzzle.date.format("%a, %b %-d").to_string();
                    widgets::clickable(
                        gpui::SharedString::from(format!("daily-archive-{}", puzzle.date)),
                        Action::Navigate(Page::Game(puzzle.launch())),
                        cx,
                    )
                    .aria_label(format!("Play {} from {date}", puzzle.title))
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
                    .child(div().text_color(rgb(palette.muted)).child(date))
                })),
        )
}

pub fn game(
    app: &Desktop,
    launch: &GameLaunch,
    view: Option<AnyView>,
    cx: &Context<Desktop>,
) -> Div {
    let favorite = app.favorites.contains(&launch.game);
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
            .child(t::display_type(title.to_owned(), 30.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .children(daily::from_launch(launch).map(|puzzle| {
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(t::colors(cx).muted))
                            .child(format!(
                                "Daily puzzle · {}",
                                puzzle.date.format("%b %-d, %Y")
                            ))
                    }))
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
                        .min_h(px(34.)),
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
