use gpui::{
    Context, Div, ElementId, FontWeight, ObjectFit, SharedString, Stateful, div, img, prelude::*,
    px, relative, rgb,
};

use crate::app::{Action, Desktop, Page};
use crate::{games::GameEntry, theme as t};

pub fn clickable(id: impl Into<ElementId>, action: Action, cx: &Context<Desktop>) -> Stateful<Div> {
    div()
        .id(id)
        .tab_index(0)
        .cursor_pointer()
        .on_click(cx.listener(move |desktop, _, _, cx| desktop.dispatch(action.clone(), cx)))
}

pub fn button(
    id: &'static str,
    label: impl Into<SharedString>,
    action: Action,
    primary: bool,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let bg = if primary { t::GOLD } else { t::SURFACE };
    let hover = if primary { t::GOLD_HOVER } else { t::SAGE };
    clickable(id, action, cx)
        .min_h(px(42.))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if primary { t::GOLD } else { t::BORDER }))
        .bg(rgb(bg))
        .text_color(rgb(t::INK))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|style| style.bg(rgb(hover)))
        .focus(|style| style.border_color(rgb(t::MUTED)))
        .child(label.into())
}

pub fn sidebar(app: &Desktop, narrow: bool, cx: &Context<Desktop>) -> Div {
    let current = match &app.page {
        Page::Game(launch) => match launch.entry {
            GameEntry::Classic => Page::Library,
            GameEntry::Mode(_) => Page::Solitaire,
            GameEntry::Challenge(_) => Page::DailyChallenge,
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
        .bg(rgb(t::RAIL))
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
                .gap_7()
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
                            .border_color(rgb(if active { t::GOLD } else { t::RAIL }))
                            .bg(rgb(if active { t::RAIL_ACTIVE } else { t::RAIL }))
                            .hover(|style| style.bg(rgb(t::RAIL_ACTIVE)))
                            .focus(|style| style.border_color(rgb(t::GOLD)))
                            .child(symbol(icon, t::PAPER, 21.))
                            .child(
                                div()
                                    .text_size(px(if narrow { 13. } else { 15. }))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(label),
                            )
                    },
                ))),
        )
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
        )
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

pub fn hero(eyebrow: &str, title: &str, image: &str, action: Stateful<Div>) -> Div {
    div()
        .w_full()
        .h(px(191.))
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
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(eyebrow.to_owned()),
                )
                .child(t::display_type(title.to_owned(), 40.))
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
    clickable(SharedString::from(format!("tile-{id}")), action, cx)
        .w_full()
        .min_w_0()
        .h(px(height + 43.))
        .flex()
        .flex_col()
        .rounded_lg()
        .overflow_hidden()
        .bg(rgb(t::SURFACE))
        .border_1()
        .border_color(rgb(t::BORDER))
        .hover(|style| style.bg(rgb(t::SAGE)).border_color(rgb(t::MUTED)))
        .focus(|style| style.border_color(rgb(t::MUTED)))
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
