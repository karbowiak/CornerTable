use gpui::{
    Context, Div, ElementId, FontWeight, NavigationDirection, ObjectFit, Role, SharedString,
    Stateful, Toggled, div, img, prelude::*, px, relative, rgb,
};

use crate::app::{Action, Desktop, Page};
use crate::{games::GameEntry, theme as t};

pub fn clickable(id: impl Into<ElementId>, action: Action, cx: &Context<Desktop>) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .tab_index(0)
        .cursor_pointer()
        .on_click(cx.listener(move |desktop, _, window, cx| {
            desktop.dispatch(action.clone(), window, cx);
        }))
}

pub fn button(
    id: &'static str,
    label: impl Into<SharedString>,
    action: Action,
    primary: bool,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let palette = t::colors(cx);
    let bg = if primary { t::GOLD } else { palette.surface };
    let hover = if primary {
        t::GOLD_HOVER
    } else {
        palette.hover
    };
    clickable(id, action, cx)
        .min_h(px(42.))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if primary { t::GOLD } else { palette.border }))
        .bg(rgb(bg))
        .text_color(rgb(if primary { t::INK } else { palette.ink }))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|style| style.bg(rgb(hover)))
        .focus_visible(|style| style.border_color(rgb(palette.ink)))
        .child(label.into())
}

pub fn sidebar(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let palette = t::colors(cx);
    let current = match &app.page {
        Page::Game(launch) => match launch.entry {
            GameEntry::Mode(_) if launch.game.0 == "solitaire" => Page::Solitaire,
            GameEntry::Challenge(_) => Page::DailyChallenge,
            _ => Page::Library,
        },
        page => page.clone(),
    };
    let guest = &app.api.home.guest;
    let pages = [
        ("nav-library", "library", "Library", Page::Library),
        ("nav-solitaire", "solitaire", "Solitaire", Page::Solitaire),
        (
            "nav-daily",
            "calendar",
            "Daily Challenge",
            Page::DailyChallenge,
        ),
        (
            "nav-leaderboard",
            "trophy",
            "Leaderboard",
            Page::Leaderboard,
        ),
    ];
    div()
        .w(px(if narrow { 174. } else { 220. }))
        .flex_shrink_0()
        .bg(rgb(palette.rail))
        .text_color(rgb(t::PAPER))
        .flex()
        .flex_col()
        .justify_between()
        .px_3()
        .pt_7()
        .pb_5()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .child(symbol("mark", t::GOLD, 25.))
                        .child(
                            t::display_type("Corner Table", if narrow { 16. } else { 20. })
                                .whitespace_nowrap(),
                        ),
                )
                .child(div().flex().gap_2().children([
                    history_button("nav-back", "Back", NavigationDirection::Back, app, cx),
                    history_button(
                        "nav-forward",
                        "Forward",
                        NavigationDirection::Forward,
                        app,
                        cx,
                    ),
                ]))
                .child(div().flex().flex_col().gap_2().children(pages.map(
                    |(id, icon, label, page)| {
                        let active = page == current;
                        clickable(id, Action::Navigate(page), cx)
                            .w_full()
                            .h(px(46.))
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_2()
                            .rounded_md()
                            .border_l_4()
                            .border_color(rgb(if active { t::GOLD } else { palette.rail }))
                            .bg(rgb(if active {
                                palette.rail_active
                            } else {
                                palette.rail
                            }))
                            .hover(|style| style.bg(rgb(palette.rail_active)))
                            .focus(|style| style.border_color(rgb(t::GOLD)))
                            .child(symbol(icon, t::PAPER, 21.))
                            .child(
                                div()
                                    .text_size(px(if narrow { 13. } else { 15. }))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(label),
                            )
                    },
                )))
                .children(app.recent.as_ref().and_then(|launch| {
                    let game = app.api.game(&launch.game)?;
                    let title = match &launch.entry {
                        GameEntry::Mode(id) if launch.game.0 == "solitaire" => app
                            .api
                            .library
                            .solitaire_modes
                            .iter()
                            .find(|mode| &mode.id == id)
                            .map_or(game.title.as_str(), |mode| mode.title.as_str()),
                        _ => game.title.as_str(),
                    };
                    let active = matches!(&app.page, Page::Game(current) if current == launch);
                    let content = div()
                        .id("nav-recent")
                        .flex()
                        .flex_col()
                        .gap_1()
                        .px_2()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(if active { 0x567265 } else { 0x36564a }))
                        .bg(rgb(0x1b3b32))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(0xd5dfd6))
                                .child("Last played"),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .truncate()
                                .child(title.to_owned()),
                        )
                        .when(!active, |button| {
                            let launch = launch.clone();
                            button
                                .role(Role::Button)
                                .tab_index(0)
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(palette.rail_active)))
                                .focus_visible(|style| style.border_color(rgb(t::GOLD)))
                                .on_click(cx.listener(move |desktop, _, window, cx| {
                                    desktop.dispatch(
                                        Action::Navigate(Page::Game(launch.clone())),
                                        window,
                                        cx,
                                    );
                                }))
                        });
                    Some(content)
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(appearance_control(app, cx))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_2()
                        .child(avatar(&guest.initials, t::PAPER, t::INK))
                        .child(
                            div()
                                .line_height(px(20.))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .child(guest.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(rgb(0xcad6cc))
                                        .child("Guest"),
                                ),
                        ),
                ),
        )
}

fn appearance_control(app: &Desktop, cx: &Context<Desktop>) -> Div {
    let palette = t::colors(cx);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .px_1()
                .text_size(px(12.))
                .text_color(rgb(0xd5dfd6))
                .child("Appearance"),
        )
        .child(
            div()
                .flex()
                .gap_1()
                .children(t::Appearance::ALL.map(|appearance| {
                    let active = appearance == app.appearance;
                    clickable(
                        SharedString::from(format!("appearance-{}", appearance.label())),
                        Action::SetAppearance(appearance),
                        cx,
                    )
                    .aria_label(format!("{} appearance", appearance.label()))
                    .aria_toggled(if active {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .flex_1()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(if active { t::GOLD } else { 0x587669 }))
                    .bg(rgb(if active { t::GOLD } else { palette.rail }))
                    .text_color(rgb(if active { t::INK } else { t::PAPER }))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .hover(|style| {
                        style.bg(rgb(if active {
                            t::GOLD_HOVER
                        } else {
                            palette.rail_active
                        }))
                    })
                    .focus_visible(|style| style.border_color(rgb(t::PAPER)))
                    .child(appearance.label())
                })),
        )
}

fn history_button(
    id: &'static str,
    label: &'static str,
    direction: NavigationDirection,
    app: &Desktop,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let enabled = app.can_step_history(direction);
    let palette = t::colors(cx);
    div()
        .id(id)
        .flex_1()
        .h(px(36.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if enabled { 0x587669 } else { 0x36564a }))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(if enabled { t::PAPER } else { 0x9fb4a9 }))
        .child(label)
        .when(enabled, |button| {
            button
                .role(Role::Button)
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(palette.rail_active)))
                .focus_visible(|style| style.border_color(rgb(t::GOLD)))
                .on_click(cx.listener(move |desktop, _, window, cx| {
                    desktop.dispatch(Action::History(direction), window, cx);
                }))
        })
}

pub fn avatar(label: &str, background: u32, foreground: u32) -> Div {
    div()
        .size(px(36.))
        .flex_shrink_0()
        .rounded_full()
        .bg(rgb(background))
        .text_color(rgb(foreground))
        .text_size(px(12.))
        .font_weight(FontWeight::BOLD)
        .flex()
        .items_center()
        .justify_center()
        .child(label.to_owned())
}

pub fn symbol(name: &str, color: u32, size: f32) -> impl IntoElement {
    gpui::svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}

pub fn artwork(id: &str) -> impl IntoElement {
    img(format!("art/{id}.jpg"))
        .size_full()
        .object_fit(ObjectFit::Cover)
}

pub fn challenge_art(game_id: &str) -> &str {
    if game_id == "mahjong" {
        "mahjong-feature"
    } else {
        game_id
    }
}

pub fn hero(title: &str, image: &str, action: Stateful<Div>) -> Div {
    div()
        .w_full()
        .h(px(147.))
        .flex()
        .rounded_xl()
        .overflow_hidden()
        .bg(rgb(t::RAIL))
        .text_color(rgb(t::PAPER))
        .child(
            div()
                .w(relative(0.40))
                .min_w(px(220.))
                .px(px(24.))
                .flex()
                .flex_col()
                .justify_center()
                .items_start()
                .gap_2()
                .child(t::display_type(title.to_owned(), 29.))
                .child(div().mt_1().child(action)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .overflow_hidden()
                .child(artwork(image)),
        )
}

pub fn columns<E: IntoElement>(items: impl IntoIterator<Item = E>, count: usize, gap: f32) -> Div {
    let mut items = items.into_iter().peekable();
    let mut grid = div().flex().flex_col().gap(px(gap));
    while items.peek().is_some() {
        let columns = (0..count).map(|_| div().flex_1().min_w_0().children(items.next()));
        grid = grid.child(div().flex().gap(px(gap)).children(columns));
    }
    grid
}

pub fn tile(
    id: &str,
    title: &str,
    height: f32,
    action: Action,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let palette = t::colors(cx);
    clickable(SharedString::from(format!("tile-{id}")), action, cx)
        .w_full()
        .min_w_0()
        .h(px(height + 43.))
        .flex()
        .flex_col()
        .rounded_lg()
        .overflow_hidden()
        .bg(rgb(palette.surface))
        .border_1()
        .border_color(rgb(palette.border))
        .hover(|style| {
            style
                .bg(rgb(palette.hover))
                .border_color(rgb(palette.muted))
        })
        .focus_visible(|style| style.border_color(rgb(palette.ink)))
        .child(
            div()
                .w_full()
                .h(px(height))
                .flex_shrink_0()
                .overflow_hidden()
                .child(artwork(id)),
        )
        .child(
            t::display_type(title.to_owned(), 19.)
                .truncate()
                .px_3()
                .py_2(),
        )
}
