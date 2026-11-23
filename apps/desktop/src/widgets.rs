use corner_table_api::GameId;
use gpui::{
    Context, Div, ElementId, FontWeight, NavigationDirection, ObjectFit, Role, SharedString,
    Stateful, Toggled, Window, div, img, prelude::*, px, relative, rgb, rgba,
};

use crate::app::{Action, Desktop, Page, ShellLayout, SidebarLayout};
use crate::{audio, games::GameEntry, theme as t};

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
    let label = label.into();
    let bg = if primary { t::GOLD } else { palette.surface };
    let hover = if primary {
        t::GOLD_HOVER
    } else {
        palette.hover
    };
    clickable(id, action, cx)
        .aria_label(label.clone())
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
        .child(label)
}

pub(crate) fn sidebar(
    app: &Desktop,
    layout: &ShellLayout,
    short: bool,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let palette = t::colors(cx);
    let compact = layout.sidebar == SidebarLayout::Compact;
    let narrow = layout.sidebar == SidebarLayout::Medium;
    let current = match &app.page {
        Page::Game(launch) => match launch.entry {
            GameEntry::Mode(_) if launch.game.0 == "solitaire" => Page::Solitaire,
            GameEntry::Challenge(_) => Page::DailyChallenge,
            _ => Page::Library,
        },
        page => page.clone(),
    };
    let pages = [
        ("nav-library", "library", "Library", Page::Library),
        ("nav-solitaire", "solitaire", "Solitaire", Page::Solitaire),
        (
            "nav-daily",
            "calendar",
            "Daily Challenge",
            Page::DailyChallenge,
        ),
        ("nav-records", "trophy", "Records", Page::Records),
    ];
    div()
        .id("sidebar")
        .role(Role::Navigation)
        .aria_label("Sidebar")
        .w(px(layout.sidebar_width()))
        .flex_shrink_0()
        .bg(rgb(palette.rail))
        .text_color(rgb(t::PAPER))
        .flex()
        .flex_col()
        .justify_between()
        .px(px(if compact { 9. } else { 12. }))
        .pt(px(if short { 16. } else { 28. }))
        .pb(px(if short { 12. } else { 20. }))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(if short { 12. } else { 16. }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .when(compact, |brand| brand.justify_center())
                        .child(symbol("mark", t::GOLD, 25.))
                        .when(!compact, |brand| {
                            brand.child(
                                t::display_type("Corner Table", if narrow { 16. } else { 20. })
                                    .whitespace_nowrap(),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .when(compact, |history| history.flex_col().gap_1())
                        .children([
                            history_button(NavigationDirection::Back, app, compact, cx),
                            history_button(NavigationDirection::Forward, app, compact, cx),
                        ]),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(if compact { 4. } else { 8. }))
                        .children(pages.map(|destination| {
                            page_button(destination, &current, layout.sidebar, short, cx)
                        })),
                )
                .children(last_played(app, compact, cx)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .when(audio::is_supported(), |settings| {
                    settings.child(sound_control(app, compact, cx))
                })
                .child(appearance_control(app, compact, cx)),
        )
}

fn page_button(
    (id, icon, label, page): (&'static str, &'static str, &'static str, Page),
    current: &Page,
    layout: SidebarLayout,
    short: bool,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let palette = t::colors(cx);
    let active = &page == current;
    let button = clickable(id, Action::Navigate(page), cx)
        .aria_label(label)
        .aria_selected(active)
        .rounded_md()
        .bg(rgb(if active {
            palette.rail_active
        } else {
            palette.rail
        }))
        .hover(|style| style.bg(rgb(palette.rail_active)))
        .focus_visible(|style| style.border_color(rgb(t::PAPER)));
    if layout == SidebarLayout::Compact {
        return rail_icon(button, icon, if active { t::GOLD } else { t::PAPER }, label)
            .border_2()
            .border_color(gpui::transparent_black());
    }
    button
        .w_full()
        .h(px(if short { 38. } else { 46. }))
        .flex()
        .items_center()
        .gap_3()
        .px_2()
        .border_l_4()
        .border_color(rgb(if active { t::GOLD } else { palette.rail }))
        .child(symbol(icon, t::PAPER, 21.))
        .child(
            div()
                .text_size(px(if layout == SidebarLayout::Medium {
                    13.
                } else {
                    15.
                }))
                .font_weight(FontWeight::SEMIBOLD)
                .child(label),
        )
}

fn last_played(app: &Desktop, compact: bool, cx: &Context<Desktop>) -> Option<Stateful<Div>> {
    let launch = app.recent.as_ref()?;
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
    let card = div()
        .id("nav-recent")
        .rounded_md()
        .border_1()
        .border_color(rgb(if active { 0x567265 } else { 0x36564a }))
        .bg(rgb(0x1b3b32));
    let card = if compact {
        rail_icon(
            card,
            "recent",
            if active { t::GOLD } else { t::PAPER },
            format!("Last played: {title}"),
        )
        .border_2()
    } else {
        card.flex()
            .flex_col()
            .gap_1()
            .px_2()
            .py_2()
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
    };
    Some(if active {
        card.role(Role::Label)
            .aria_label(format!("Last played: {title}"))
    } else {
        let launch = launch.clone();
        let palette = t::colors(cx);
        card.role(Role::Button)
            .aria_label(format!("Open last played {title}"))
            .tab_index(0)
            .cursor_pointer()
            .hover(|style| style.bg(rgb(palette.rail_active)))
            .focus_visible(|style| style.border_color(rgb(t::PAPER)))
            .on_click(cx.listener(move |desktop, _, window, cx| {
                desktop.dispatch(Action::Navigate(Page::Game(launch.clone())), window, cx);
            }))
    })
}

fn sound_control(app: &Desktop, compact: bool, cx: &Context<Desktop>) -> Stateful<Div> {
    let palette = t::colors(cx);
    let button = clickable("audio-toggle", Action::SetSound(!app.sound_enabled), cx)
        .aria_label("Game sounds")
        .aria_toggled(app.sound_enabled.into())
        .rounded_sm()
        .border_1()
        .border_color(rgb(0x587669))
        .hover(|style| style.bg(rgb(palette.rail_active)))
        .focus_visible(|style| style.border_color(rgb(t::PAPER)));
    if compact {
        let (icon, color, label) = if app.sound_enabled {
            ("sound-on", t::GOLD, "Game sounds on")
        } else {
            ("sound-off", t::PAPER, "Game sounds off")
        };
        return rail_icon(button, icon, color, label).h(px(36.)).border_2();
    }
    button
        .flex()
        .items_center()
        .justify_between()
        .h(px(32.))
        .px_2()
        .text_size(px(12.))
        .child("Game sounds")
        .child(
            div()
                .text_color(rgb(if app.sound_enabled { t::GOLD } else { 0xb9c9bd }))
                .child(if app.sound_enabled { "On" } else { "Off" }),
        )
}

fn appearance_control(app: &Desktop, compact: bool, cx: &Context<Desktop>) -> Stateful<Div> {
    let palette = t::colors(cx);
    div()
        .id("appearance")
        .role(Role::Group)
        .aria_label("Appearance")
        .flex()
        .flex_col()
        .gap_1()
        .when(!compact, |control| {
            control.child(
                div()
                    .px_1()
                    .text_size(px(12.))
                    .text_color(rgb(0xd5dfd6))
                    .child("Appearance"),
            )
        })
        .child(
            div()
                .flex()
                .gap_1()
                .when(compact, |choices| choices.flex_col())
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
                    .h(px(32.))
                    .when(!compact, |button| button.flex_1())
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .border_1()
                    .when(compact, |button| button.border_2().h(px(29.)))
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
    direction: NavigationDirection,
    app: &Desktop,
    compact: bool,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let (id, label, icon) = match direction {
        NavigationDirection::Back => ("nav-back", "Back", "arrow-left"),
        NavigationDirection::Forward => ("nav-forward", "Forward", "arrow-right"),
    };
    let enabled = app.can_step_history(direction);
    let palette = t::colors(cx);
    let color = if enabled { t::PAPER } else { 0x9fb4a9 };
    let button = div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .when(!enabled, |button| button.aria_description("Unavailable"))
        .rounded_md()
        .border_1()
        .border_color(rgb(if enabled { 0x587669 } else { 0x36564a }));
    let button = if compact {
        rail_icon(button, icon, color, label).h(px(31.)).border_2()
    } else {
        button
            .flex_1()
            .h(px(36.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(color))
            .child(label)
    };
    button.when(enabled, |button| {
        button
            .tab_index(0)
            .cursor_pointer()
            .hover(|style| style.bg(rgb(palette.rail_active)))
            .focus_visible(|style| style.border_color(rgb(t::PAPER)))
            .on_click(cx.listener(move |desktop, _, window, cx| {
                desktop.dispatch(Action::History(direction), window, cx);
            }))
    })
}

fn rail_icon(
    button: Stateful<Div>,
    icon: &str,
    color: u32,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    let label = label.into();
    button
        .w_full()
        .h(px(40.))
        .flex()
        .items_center()
        .justify_center()
        .child(symbol(icon, color, 21.))
        .tooltip(move |_, cx| cx.new(|_| ControlTooltip(label.clone())).into())
}

struct ControlTooltip(SharedString);

impl Render for ControlTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = t::colors(cx);
        div()
            .bg(rgb(palette.surface))
            .text_color(rgb(palette.ink))
            .text_size(px(12.))
            .line_height(px(18.))
            .max_w(px(250.))
            .rounded_md()
            .border_1()
            .border_color(rgb(palette.border))
            .px_2()
            .py_1()
            .child(self.0.clone())
    }
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
                .min_w(px(296.))
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
    favorite: Option<(&GameId, bool)>,
    cx: &Context<Desktop>,
) -> Div {
    let palette = t::colors(cx);
    let tile = clickable(SharedString::from(format!("tile-{id}")), action, cx)
        .aria_label(title.to_owned())
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
                .pl_3()
                .pr(px(if favorite.is_some() { 42. } else { 12. }))
                .py_2(),
        );
    div()
        .relative()
        .child(tile)
        .children(favorite.map(|(game, active)| {
            let (label, tooltip) = if active {
                (format!("Remove {title} from favorites"), "Remove favorite")
            } else {
                (format!("Add {title} to favorites"), "Add favorite")
            };
            let id = format!("library-favorite-{id}");
            clickable(
                SharedString::from(id.clone()),
                Action::ToggleFavorite(game.clone()),
                cx,
            )
            .debug_selector(move || id)
            .aria_label(label)
            .aria_toggled(active.into())
            .absolute()
            .bottom(px(7.))
            .right(px(7.))
            .size(px(30.))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(rgba(0))
            .text_color(rgb(if active {
                palette.gold_text
            } else {
                palette.muted
            }))
            .text_size(px(22.))
            .line_height(px(24.))
            .hover(|style| {
                style
                    .bg(rgb(palette.hover))
                    .text_color(rgb(palette.gold_text))
            })
            .focus_visible(|style| style.border_color(rgb(palette.ink)))
            .tooltip(move |_, cx| cx.new(|_| ControlTooltip(tooltip.into())).into())
            .child(if active { "★" } else { "☆" })
        }))
}
