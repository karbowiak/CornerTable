use corner_table_api::{Game, GameCategory};
use gpui::{Context, Div, Role, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop, Page};
use crate::{daily, theme as t, widgets};

use super::solitaire::open_mode;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LibraryFilter {
    #[default]
    All,
    Category(GameCategory),
    Favorites,
}

impl LibraryFilter {
    const CHOICES: [Self; 6] = [
        Self::All,
        Self::Category(GameCategory::Cards),
        Self::Category(GameCategory::Puzzles),
        Self::Category(GameCategory::Board),
        Self::Category(GameCategory::Arcade),
        Self::Favorites,
    ];

    fn button(self) -> (&'static str, &'static str) {
        match self {
            Self::All => ("library-all", "All games"),
            Self::Category(GameCategory::Cards) => ("library-cards", "Cards"),
            Self::Category(GameCategory::Puzzles) => ("library-puzzles", "Puzzles"),
            Self::Category(GameCategory::Board) => ("library-board", "Board games"),
            Self::Category(GameCategory::Arcade) => ("library-arcade", "Arcade"),
            Self::Favorites => ("library-favorites", "Favorites"),
        }
    }
}

fn matching_games<'a>(app: &'a Desktop, query: &str) -> impl Iterator<Item = &'a Game> {
    let query = query.trim().to_lowercase();
    app.api.library.games.iter().filter(move |game| {
        let matches_filter = match app.library_filter {
            LibraryFilter::All => true,
            LibraryFilter::Category(category) => game.category == category,
            LibraryFilter::Favorites => app.favorites.contains(&game.id),
        };
        if !matches_filter || query.is_empty() {
            return matches_filter;
        }
        let mut searchable = format!(
            "{} {} {} {}",
            game.title,
            game.category.label(),
            game.description,
            game.search_terms.join(" "),
        );
        if game.id.0 == "solitaire" {
            for mode in &app.api.library.solitaire_modes {
                searchable.push(' ');
                searchable.push_str(&mode.title);
            }
        }
        let searchable = searchable.to_lowercase();
        query
            .split_whitespace()
            .all(|word| searchable.contains(word))
    })
}

fn matching_mode<'a>(
    app: &'a Desktop,
    game: &Game,
    query: &str,
) -> Option<&'a corner_table_api::Mode> {
    if game.id.0 != "solitaire" {
        return None;
    }
    let query = query.to_ascii_lowercase();
    let query = query.strip_suffix(" solitaire").unwrap_or(&query);
    app.api
        .library
        .solitaire_modes
        .iter()
        .find(|mode| mode.title.eq_ignore_ascii_case(query))
}

pub fn open_search_result(app: &Desktop, query: &str) -> Option<Action> {
    let mut matches = matching_games(app, query);
    let first = matches.next()?;
    let second = matches.next();
    let exact = |game: &Game| {
        game.title.eq_ignore_ascii_case(query) || matching_mode(app, game, query).is_some()
    };
    let game = if second.is_none() || exact(first) {
        first
    } else {
        second.into_iter().chain(matches).find(|game| exact(game))?
    };
    Some(if let Some(mode) = matching_mode(app, game, query) {
        open_mode(&mode.id)
    } else {
        Action::OpenGame(game.id.clone())
    })
}

pub fn library(app: &Desktop, content_width: f32, cx: &Context<Desktop>) -> Div {
    let palette = t::colors(cx);
    let query = app.search.read(cx).query().trim();
    let featured = app.library_filter == LibraryFilter::All && query.is_empty();
    let games = matching_games(app, query)
        .map(|game| {
            let (id, title, action) = if let Some(mode) = matching_mode(app, game, query) {
                (mode.id.as_str(), mode.title.as_str(), open_mode(&mode.id))
            } else {
                (
                    game.id.0.as_str(),
                    game.title.as_str(),
                    Action::OpenGame(game.id.clone()),
                )
            };
            let favorite = (id == game.id.0).then(|| (&game.id, app.favorites.contains(&game.id)));
            widgets::tile(id, title, None, 124., action, favorite, cx)
        })
        .collect::<Vec<_>>();
    let catalog = if games.is_empty() {
        t::panel(cx).p_6().text_color(rgb(palette.muted)).child(
            if app.library_filter == LibraryFilter::Favorites && app.favorites.is_empty() {
                "No favorites yet. Use the star on a game in the library."
            } else {
                "No games found. Try another search or category."
            },
        )
    } else {
        let columns = if content_width >= 872. {
            4
        } else if content_width >= 682. {
            3
        } else {
            2
        };
        widgets::columns(games, columns, 16.)
    };
    let filters = div()
        .id("library-filters")
        .role(Role::Group)
        .aria_label("Filter games")
        .flex()
        .flex_wrap()
        .gap_2()
        .children(LibraryFilter::CHOICES.into_iter().map(|filter| {
            let (id, label) = filter.button();
            let active = app.library_filter == filter;
            widgets::button(id, label, Action::FilterLibrary(filter), active, cx)
                .min_h(px(34.))
                .px_3()
                .text_size(px(13.))
                .aria_toggled(active.into())
        }));
    let wide = content_width >= 1092.;
    let (header_filters, second_row_filters) = if wide {
        (Some(filters), None)
    } else {
        (None, Some(filters))
    };
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            t::display_type("Game Library", if content_width < 792. { 32. } else { 36. })
                .id("library-heading")
                .role(Role::Heading)
                .aria_level(1)
                .aria_label("Game Library"),
        )
        .children(header_filters)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(246_f32.min(content_width / 3.)))
                        .child(app.search.clone()),
                )
                .child(
                    widgets::button(
                        "library-surprise",
                        "Surprise me",
                        Action::SurpriseGame,
                        false,
                        cx,
                    )
                    .aria_description("Open a random game from the library.")
                    .min_h(px(36.))
                    .px_3()
                    .text_size(px(13.))
                    .whitespace_nowrap(),
                ),
        );

    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(header)
        .children(second_row_filters)
        .when(featured, |library| {
            let daily = daily::today();
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
