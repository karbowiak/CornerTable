mod daily_challenge;
mod guide;
mod library;
mod records;
mod solitaire;

pub use daily_challenge::{DailyCalendar, daily_challenge};
pub use guide::GuideOverlay;
pub use library::{LibraryFilter, library, open_search_result};
pub use records::{RecordAction, Records, records};
pub use solitaire::solitaire;

use gpui::{AnyView, Context, Div, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop};
use crate::games::{GameEntry, GameLaunch};
use crate::{daily, theme as t, widgets};

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
                        daily_challenge::game_calendar_button(puzzle.date, compact, cx)
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
                        .px_3()
                        .text_size(px(13.))
                        .whitespace_nowrap()
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
