mod leaderboard;
mod library;
mod solitaire;

pub use leaderboard::leaderboard;
pub use library::library;
pub use solitaire::solitaire;

use gpui::{AnyView, Context, Div, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop, Page};
use crate::games::{GameEntry, GameLaunch};
use crate::{theme as t, widgets};

pub fn daily_challenge(app: &Desktop, cx: &Context<Desktop>) -> Div {
    let daily = &app.api.home.challenge;
    let palette = t::colors(cx);
    let launch = GameLaunch {
        game: daily.game_id.clone(),
        entry: GameEntry::Challenge(daily.id.clone()),
    };
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(t::display_type("Daily Challenge", 36.))
        .child(widgets::hero(
            &daily.title,
            widgets::challenge_art(&daily.game_id.0),
            widgets::button(
                "launch-daily",
                "Play challenge",
                Action::Navigate(Page::Game(launch)),
                true,
                cx,
            ),
        ))
        .child(
            t::panel(cx)
                .p_4()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .text_color(rgb(palette.muted))
                        .child(daily.description.clone()),
                )
                .child(widgets::button(
                    "daily-rankings",
                    "View leaderboard",
                    Action::ChallengeLeaderboard,
                    false,
                    cx,
                )),
        )
}

pub fn game(
    app: &Desktop,
    launch: &GameLaunch,
    view: Option<AnyView>,
    cx: &Context<Desktop>,
) -> Div {
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
    let header = div()
        .flex()
        .flex_col()
        .items_start()
        .gap_3()
        .child(t::display_type(title.to_owned(), 30.));

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
