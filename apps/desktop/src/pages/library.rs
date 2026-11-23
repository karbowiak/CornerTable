use gpui::{Context, Div, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop, Page};
use crate::{daily, theme as t, widgets};

pub fn library(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let daily = daily::today();
    let palette = t::colors(cx);
    let query = app.search.read(cx).query().trim().to_lowercase();
    let games = app
        .api
        .library
        .games
        .iter()
        .filter(|game| !app.favorites_only || app.favorites.contains(&game.id))
        .filter(|game| {
            game.title.to_lowercase().contains(&query)
                || game.description.to_lowercase().contains(&query)
        })
        .map(|game| {
            widgets::tile(
                &game.id.0,
                &game.title,
                124.,
                Action::OpenGame(game.id.clone()),
                cx,
            )
        })
        .collect::<Vec<_>>();
    let catalog = if games.is_empty() {
        t::panel(cx).p_6().text_color(rgb(palette.muted)).child(
            if app.favorites_only && app.favorites.is_empty() {
                "No favorites yet. Open a game and choose Add favorite."
            } else {
                "No games found. Try a different search."
            },
        )
    } else {
        widgets::columns(games, if narrow { 2 } else { 4 }, 16.)
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_x_3()
                .gap_y_2()
                .flex_wrap()
                .child(t::display_type(
                    "Game Library",
                    if narrow { 32. } else { 36. },
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            widgets::button(
                                "library-all",
                                "All games",
                                Action::ShowFavorites(false),
                                !app.favorites_only,
                                cx,
                            )
                            .aria_toggled((!app.favorites_only).into()),
                        )
                        .child(
                            widgets::button(
                                "library-favorites",
                                "Favorites",
                                Action::ShowFavorites(true),
                                app.favorites_only,
                                cx,
                            )
                            .aria_toggled(app.favorites_only.into()),
                        )
                        .child(
                            div()
                                .w(px(if narrow { 214. } else { 300. }))
                                .child(app.search.clone()),
                        ),
                ),
        )
        .when(!app.favorites_only && query.is_empty(), |library| {
            library.child(widgets::hero(
                &format!("Daily {}", daily.title),
                widgets::challenge_art(daily.game),
                widgets::button(
                    "daily-feature",
                    "View challenge",
                    Action::Navigate(Page::DailyChallenge),
                    true,
                    cx,
                ),
            ))
        })
        .child(catalog)
}
