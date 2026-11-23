use corner_table_api::GameId;
use gpui::{Context, Div, div, prelude::*};

use crate::app::{Action, Desktop, Page};
use crate::games::{GameEntry, GameLaunch};
use crate::{theme as t, widgets};

pub fn solitaire(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(t::display_type("Solitaire", 36.))
        .child(widgets::hero(
            "Klondike",
            "solitaire-feature",
            widgets::button(
                "featured-klondike",
                "Play Klondike",
                open_mode("klondike"),
                true,
                cx,
            ),
        ))
        .child(t::display_type("Choose a game", 28.))
        .child(widgets::columns(
            app.api.library.solitaire_modes.iter().map(|mode| {
                widgets::tile(
                    &mode.id,
                    &mode.title,
                    Some(&mode.description),
                    180.,
                    open_mode(&mode.id),
                    None,
                    cx,
                )
            }),
            if narrow { 3 } else { 5 },
            12.,
        ))
}

pub(super) fn open_mode(id: &str) -> Action {
    Action::Navigate(Page::Game(GameLaunch {
        game: GameId("solitaire".into()),
        entry: GameEntry::Mode(id.into()),
    }))
}
