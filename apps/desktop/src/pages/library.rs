use gpui::{Context, Div, FontWeight, KeyDownEvent, MouseButton, Window, div, prelude::*, px, rgb};

use crate::app::{Action, Desktop, Page};
use crate::{theme as t, widgets};

pub fn library(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let daily = &app.api.home.challenge;
    let query = app.search.trim().to_lowercase();
    let games = app
        .api
        .library
        .games
        .iter()
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
        t::surface()
            .p_6()
            .text_color(rgb(t::MUTED))
            .child("No games found. Try a different search.")
    } else {
        widgets::columns(games, if narrow { 2 } else { 4 }, 16.)
    };

    div()
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .flex_wrap()
                .child(t::heading("Game Library"))
                .child(search_field(app, narrow, cx)),
        )
        .child(widgets::hero(
            "Today’s Challenge",
            &daily.title,
            widgets::challenge_art(&daily.game_id.0),
            widgets::button(
                "daily-feature",
                "View challenge",
                Action::Navigate(Page::DailyChallenge),
                true,
                cx,
            ),
        ))
        .child(catalog)
}

fn search_field(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let value = if app.search.is_empty() {
        "Search games"
    } else {
        &app.search
    };
    t::surface()
        .w(px(if narrow { 214. } else { 300. }))
        .h(px(44.))
        .rounded_md()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .child(widgets::symbol("search", t::MUTED, 17.))
        .child(
            div()
                .id("game-search")
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .cursor_text()
                .tab_index(0)
                .track_focus(&app.search_focus)
                .focus(|style| style.text_color(rgb(t::INK)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|app, _, window, cx| {
                        app.search_focus.focus(window);
                        app.search_select_all = false;
                        cx.notify();
                    }),
                )
                .on_key_down(cx.listener(edit_search))
                .child(
                    div()
                        .truncate()
                        .bg(rgb(if app.search_select_all {
                            t::SAGE
                        } else {
                            t::SURFACE
                        }))
                        .text_color(rgb(if app.search.is_empty() {
                            t::MUTED
                        } else {
                            t::INK
                        }))
                        .child(value.to_owned()),
                ),
        )
        .when(!app.search.is_empty(), |field| {
            field.child(
                widgets::clickable("clear-search", Action::ClearSearch, cx)
                    .size(px(28.))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(FontWeight::BOLD)
                    .hover(|style| style.bg(rgb(t::SAGE)))
                    .focus(|style| style.bg(rgb(t::SAGE)))
                    .child("×"),
            )
        })
}

fn edit_search(
    app: &mut Desktop,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut Context<Desktop>,
) {
    let key = &event.keystroke;
    let shortcut = key.modifiers.control || key.modifiers.platform;
    match key.key.as_str() {
        "escape" => {
            app.search.clear();
            app.search_select_all = false;
            app.focus.focus(window);
        }
        "backspace" | "delete" => {
            if app.search_select_all {
                app.search.clear();
            } else {
                app.search.pop();
            }
            app.search_select_all = false;
        }
        "a" if shortcut => app.search_select_all = true,
        "v" if shortcut => {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                push_search(app, &text);
            }
        }
        _ if !shortcut => {
            let Some(text) = key.key_char.as_deref() else {
                return;
            };
            push_search(app, text);
        }
        _ => return,
    }
    cx.stop_propagation();
    cx.notify();
}

fn push_search(app: &mut Desktop, text: &str) {
    if app.search_select_all {
        app.search.clear();
    }
    app.search_select_all = false;
    let remaining = 80usize.saturating_sub(app.search.chars().count());
    app.search
        .extend(text.chars().filter(|ch| !ch.is_control()).take(remaining));
}
