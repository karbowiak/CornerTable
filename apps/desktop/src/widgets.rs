use corner_table_api::GameId;
use gpui::{
    Context, Div, ElementId, FontWeight, NavigationDirection, ObjectFit, Role, SharedString,
    Stateful, Toggled, Window, div, img, prelude::*, px, relative, rgb, rgba,
};

use crate::app::{Action, CARD_MARGIN, Desktop, Page, ShellLayout, SidebarLayout};
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

/// A compact button for filters and toggles, sized like the game screens' controls.
pub fn chip(
    id: &'static str,
    label: impl Into<SharedString>,
    action: Action,
    selected: bool,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    button(id, label, action, selected, cx)
        .min_h(px(34.))
        .px_3()
        .text_size(px(13.))
        .whitespace_nowrap()
}

pub(crate) fn sidebar(
    app: &Desktop,
    layout: &ShellLayout,
    short: bool,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
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
        .text_color(rgb(t::PAPER))
        .flex()
        .flex_col()
        .gap(px(if short { 12. } else { 16. }))
        .px(px(if compact { 9. } else { 12. }))
        .pt(px(if short { 4. } else { 8. }))
        .pb(px(CARD_MARGIN))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(if compact || narrow { 4. } else { 6. }))
                .children(pages.map(|destination| {
                    page_button(destination, &current, layout.sidebar, short, cx)
                })),
        )
        .children(last_played(app, compact, cx))
}

/// Height of the strip that holds the macOS traffic lights above the top bar.
pub const TRAFFIC_LIGHT_STRIP: f32 = 28.;
pub const TOP_BAR_HEIGHT: f32 = 44.;
/// Header controls follow the sidebar and page in keyboard order.
const HEADER_TAB_ORDER: isize = 1;

/// The window's header: brand and history on the left, sound and appearance on the right.
/// On macOS it doubles as the title bar, so its empty space drags the window.
pub(crate) fn top_bar(app: &Desktop, window: &Window, cx: &Context<Desktop>) -> Div {
    let draggable = cfg!(target_os = "macos") && !window.is_fullscreen();
    let interactive = |group: Div| group.when(draggable, |group| group.block_mouse_except_scroll());
    let brand = clickable("top-bar-brand", Action::Navigate(Page::Library), cx)
        .tab_index(HEADER_TAB_ORDER)
        .aria_label("Corner Table, open the library")
        .h(px(34.))
        .pl(px(6.))
        .pr_3()
        .flex()
        .items_center()
        .gap_2()
        .rounded_full()
        .border_1()
        .border_color(rgb(0x4a3d30))
        .bg(rgb(0x2b231b))
        .hover(|style| style.bg(rgb(0x3a2f24)))
        .child(
            div()
                .size(px(24.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(0x3a2f24))
                .child(symbol("mark", t::GOLD, 17.)),
        )
        .child(t::display_type("Corner Table", 15.).whitespace_nowrap());
    div()
        .flex_none()
        .flex()
        .flex_col()
        .text_color(rgb(t::PAPER))
        .when(draggable, |header| {
            header
                .child(div().h(px(TRAFFIC_LIGHT_STRIP)))
                .on_mouse_down(gpui::MouseButton::Left, drag_window)
        })
        .child(
            div()
                .h(px(TOP_BAR_HEIGHT))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    interactive(div().flex().items_center().gap_2())
                        .child(brand)
                        .child(history_button(NavigationDirection::Back, app, cx))
                        .child(history_button(NavigationDirection::Forward, app, cx)),
                )
                .child(
                    interactive(div().flex().items_center().gap_2())
                        .when(audio::is_supported(), |controls| {
                            controls.child(sound_control(app, cx))
                        })
                        .child(appearance_control(app, cx)),
                ),
        )
}

fn drag_window(event: &gpui::MouseDownEvent, window: &mut Window, _: &mut gpui::App) {
    // AppKit needs the original mouse-down event to begin a native window drag.
    if event.click_count == 2 {
        window.titlebar_double_click();
    } else {
        window.start_window_move();
    }
}

/// A rounded control for the header.
fn header_pill(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(HEADER_TAB_ORDER)
        .cursor_pointer()
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .gap_1p5()
        .rounded_full()
        .border_1()
        .border_color(rgb(0x4a3d30))
        .bg(rgb(0x2b231b))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|style| style.bg(rgb(0x3a2f24)))
        .focus_visible(|style| style.border_color(rgb(t::GOLD)))
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
        .rounded_lg()
        .border_1()
        .border_color(rgba(if active { (t::GOLD << 8) | 0x66 } else { 0 }))
        .bg(rgb(if active {
            palette.rail_active
        } else {
            palette.rail
        }))
        .hover(|style| style.bg(rgb(palette.rail_active)))
        .focus_visible(|style| style.border_color(rgb(t::PAPER)));
    if layout == SidebarLayout::Compact {
        return rail_icon(button, icon, if active { t::GOLD } else { t::PAPER }, label);
    }
    button
        .w_full()
        .h(px(if short { 34. } else { 38. }))
        .flex()
        .items_center()
        .gap_2p5()
        .px_2p5()
        .child(symbol(icon, if active { t::GOLD } else { 0xc4b6a3 }, 18.))
        .child(
            div()
                .text_size(px(if layout == SidebarLayout::Medium {
                    13.
                } else {
                    14.
                }))
                .font_weight(if active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(rgb(if active { t::PAPER } else { 0xe4dacb }))
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
        .rounded_lg()
        .border_1()
        .border_color(rgb(if active { 0x6c5c49 } else { 0x41362a }))
        .bg(rgb(0x2b231b));
    let card = if compact {
        rail_icon(
            card,
            "recent",
            if active { t::GOLD } else { t::PAPER },
            format!("Last played: {title}"),
        )
        .border_2()
    } else {
        let art = match &launch.entry {
            GameEntry::Mode(id) if launch.game.0 == "solitaire" => id.as_str(),
            _ => launch.game.0.as_str(),
        };
        card.flex()
            .items_center()
            .gap_2p5()
            .p(px(6.))
            .child(
                div()
                    .size(px(38.))
                    .flex_none()
                    .rounded_md()
                    .overflow_hidden()
                    .child(artwork(art)),
            )
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(11.))
                            .line_height(px(15.))
                            .text_color(rgb(0xc4b6a3))
                            .child("Last played"),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(19.))
                            .font_weight(FontWeight::BOLD)
                            .truncate()
                            .child(title.to_owned()),
                    ),
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

fn sound_control(app: &Desktop, cx: &Context<Desktop>) -> Stateful<Div> {
    let (icon, color) = if app.sound_enabled {
        ("sound-on", t::GOLD)
    } else {
        ("sound-off", 0xab9c88)
    };
    header_pill("audio-toggle", "Game sounds")
        .aria_toggled(app.sound_enabled.into())
        .px_3()
        .child(symbol(icon, color, 16.))
        .child(if app.sound_enabled {
            "Sounds on"
        } else {
            "Sounds off"
        })
        .on_click(cx.listener({
            let enabled = app.sound_enabled;
            move |desktop, _, window, cx| {
                desktop.dispatch(Action::SetSound(!enabled), window, cx);
            }
        }))
}

fn appearance_control(app: &Desktop, cx: &Context<Desktop>) -> Stateful<Div> {
    div()
        .id("appearance")
        .role(Role::Group)
        .aria_label("Appearance")
        .h(px(32.))
        .p(px(3.))
        .flex()
        .items_center()
        .gap(px(2.))
        .rounded_full()
        .border_1()
        .border_color(rgb(0x4a3d30))
        .bg(rgb(0x2b231b))
        .children(t::Appearance::ALL.map(|appearance| {
            let active = appearance == app.appearance;
            clickable(
                SharedString::from(format!("appearance-{}", appearance.label())),
                Action::SetAppearance(appearance),
                cx,
            )
            .aria_label(format!("{} appearance", appearance.label()))
            .tab_index(HEADER_TAB_ORDER)
            .aria_toggled(if active {
                Toggled::True
            } else {
                Toggled::False
            })
            .h_full()
            .px_3()
            .flex()
            .items_center()
            .rounded_full()
            .bg(rgb(if active { t::GOLD } else { 0x2b231b }))
            .text_color(rgb(if active { t::INK } else { t::PAPER }))
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .hover(|style| style.bg(rgb(if active { t::GOLD_HOVER } else { 0x3a2f24 })))
            .focus_visible(|style| style.border_1().border_color(rgb(t::GOLD)))
            .child(appearance.label())
        }))
}

fn history_button(
    direction: NavigationDirection,
    app: &Desktop,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let (id, label, icon) = match direction {
        NavigationDirection::Back => ("nav-back", "Back", "arrow-left"),
        NavigationDirection::Forward => ("nav-forward", "Forward", "arrow-right"),
    };
    let enabled = app.can_step_history(direction);
    let button = div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(rgb(0x4a3d30))
        .bg(rgb(0x2b231b))
        .tooltip(move |_, cx| cx.new(|_| ControlTooltip(label.into())).into())
        .child(symbol(icon, t::PAPER, 16.));
    if !enabled {
        return button.aria_description("Unavailable").opacity(0.4);
    }
    button
        .tab_index(HEADER_TAB_ORDER)
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x3a2f24)))
        .focus_visible(|style| style.border_color(rgb(t::GOLD)))
        .on_click(cx.listener(move |desktop, _, window, cx| {
            desktop.dispatch(Action::History(direction), window, cx);
        }))
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

/// A game card: artwork, its title, and an optional one-line `detail` under the title.
pub fn tile(
    id: &str,
    title: &str,
    detail: Option<&str>,
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
        .h(px(height + if detail.is_some() { 84. } else { 43. }))
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
        )
        .children(detail.map(|detail| {
            div()
                .px_3()
                .text_size(px(12.))
                .line_height(px(17.))
                .text_color(rgb(palette.muted))
                .child(detail.to_owned())
        }));
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
