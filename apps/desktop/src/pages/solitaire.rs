use corner_table_api::GameId;
use gpui::{Context, Div, div, prelude::*, rgb};

use crate::app::{Action, Desktop, Page};
use crate::games::{GameEntry, GameLaunch};
use crate::{theme as t, widgets};

pub fn solitaire(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let palette = t::colors(cx);
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
            app.api
                .library
                .solitaire_modes
                .iter()
                .map(|mode| widgets::tile(&mode.id, &mode.title, 157., open_mode(&mode.id), cx)),
            if narrow { 3 } else { 5 },
            12.,
        ))
        .child(
            t::panel(cx)
                .p_5()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .flex_wrap()
                .child(
                    div().child(t::display_type("Daily challenges", 21.)).child(
                        div()
                            .text_color(rgb(palette.muted))
                            .child("See the featured puzzle and guest rankings."),
                    ),
                )
                .child(widgets::button(
                    "solitaire-daily",
                    "Daily Challenge",
                    Action::Navigate(Page::DailyChallenge),
                    false,
                    cx,
                )),
        )
}

fn open_mode(id: &str) -> Action {
    Action::Navigate(Page::Game(GameLaunch {
        game: GameId("solitaire".into()),
        entry: GameEntry::Mode(id.into()),
    }))
}
