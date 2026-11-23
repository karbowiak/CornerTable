//! The two shared game screens. Both centre the board on a table that fills the remaining
//! height and float round results over it:
//! - [`Layout::Rail`] splits the page roughly 70/30, with status, actions, and help in a rail on
//!   the right. It suits boards that are about as tall as they are wide.
//! - [`Layout::Bar`] puts status and actions in one strip above a full-width table, for wide
//!   tables such as card layouts.

use gpui::{
    AnyElement, App, Div, FontWeight, Global, IntoElement, Role, SharedString, Stateful, Window,
    div, prelude::*, px, relative, rgb, rgba,
};

use crate::{app::ShellLayout, theme as t, widgets};

const GAP: f32 = 16.;
const TABLE_PADDING: f32 = 16.;
const TOOLBAR_HEIGHT: f32 = 48.;
const BAR_HEIGHT: f32 = 76.;
/// Window height above the game view (title row) plus the page's bottom padding.
const PAGE_CHROME: f32 = 80.;
/// Below this content width the rail moves under the table.
const STACK_BELOW: f32 = 600.;
const MIN_BOARD: f32 = 240.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Rail,
    Bar,
}

/// The room a game has for its board, measured before it lays out tiles.
#[derive(Clone, Copy, Debug)]
pub struct Stage {
    pub board_width: f32,
    pub board_height: f32,
    layout: Layout,
    rail_width: f32,
    stacked: bool,
    toolbar: bool,
}

impl Stage {
    /// `toolbar` reserves a row above the table for mode and size pickers.
    pub fn measure(window: &Window, layout: Layout, toolbar: bool) -> Self {
        let viewport = window.viewport_size();
        let content = ShellLayout::new(viewport.width).content_width();
        let stacked = layout == Layout::Rail && content < STACK_BELOW;
        let rail_width = (content * 0.3).clamp(240., 360.).floor();
        let table_width = if layout == Layout::Rail && !stacked {
            content - rail_width - GAP
        } else {
            content
        };
        let chrome = PAGE_CHROME
            + if toolbar { TOOLBAR_HEIGHT } else { 0. }
            + if layout == Layout::Bar {
                BAR_HEIGHT + GAP
            } else {
                0.
            };
        let table_height = crate::app::content_height(window) - chrome;
        Self {
            board_width: (table_width - 2. * TABLE_PADDING).max(MIN_BOARD),
            board_height: (table_height - 2. * TABLE_PADDING).max(MIN_BOARD),
            layout,
            rail_width,
            stacked,
            toolbar,
        }
    }

    pub fn rail_width(&self) -> f32 {
        self.rail_width
    }

    /// Lays out the screen. `board` is centred on the table; `overlay` (a round result) floats
    /// over it so finishing a round never moves the board. `panel` is a [`rail`] or a [`bar`]
    /// to match the layout.
    pub fn render(
        self,
        cx: &App,
        toolbar: Option<AnyElement>,
        board: impl IntoElement,
        overlay: Option<AnyElement>,
        panel: impl IntoElement,
    ) -> Div {
        let palette = t::colors(cx);
        // A darker well under the board: deep on the dark theme, a soft wash on paper.
        let (well, wash) = if palette == t::LIGHT {
            (0x12, 0x1c)
        } else {
            (0x66, 0x40)
        };
        let table = div()
            .relative()
            .w_full()
            .when(!self.stacked, |table| {
                table.min_h(px(self.board_height + 2. * TABLE_PADDING))
            })
            .p(px(TABLE_PADDING))
            .rounded_xl()
            .border_1()
            .border_color(rgb(palette.border))
            .bg(rgba((palette.rail << 8) | well))
            .flex()
            .items_center()
            .justify_center()
            .child(board)
            .when_some(overlay, |table, overlay| {
                // Keep the final position readable: a light wash, with the card at the foot.
                table.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded_xl()
                        .bg(rgba((palette.rail << 8) | wash))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_end()
                        .p_5()
                        .child(overlay),
                )
            });
        let toolbar = self.toolbar.then(|| {
            div()
                .min_h(px(TOOLBAR_HEIGHT - 8.))
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .children(toolbar)
        });
        match self.layout {
            Layout::Bar => div()
                .w_full()
                .flex()
                .flex_col()
                .gap_2()
                .children(toolbar)
                .child(div().w_full().mb_2().child(panel))
                .child(table),
            Layout::Rail => {
                let play = div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(toolbar)
                    .child(table);
                let rail = div()
                    .flex_none()
                    .when(!self.stacked, |rail| rail.w(px(self.rail_width)))
                    .when(self.stacked, |rail| rail.w_full())
                    .when(self.toolbar && !self.stacked, |rail| {
                        rail.mt(px(TOOLBAR_HEIGHT))
                    })
                    .child(panel);
                div()
                    .w_full()
                    .flex()
                    .gap(px(GAP))
                    .when(self.stacked, |stage| stage.flex_col())
                    .child(play)
                    .child(rail)
            }
        }
    }
}

/// The top strip for [`Layout::Bar`]: status on the left, actions on the right.
pub fn bar(cx: &App, status: impl IntoElement, actions: impl IntoElement) -> Div {
    t::panel(cx)
        .w_full()
        .min_h(px(BAR_HEIGHT))
        .px_5()
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_6()
        .child(div().flex_1().min_w_0().child(status))
        .child(div().flex_none().max_w(relative(0.55)).child(actions))
}

/// Stats laid out in a row, for the top bar.
pub fn stat_row(cx: &App, stats: impl IntoIterator<Item = Stat>) -> Div {
    div()
        .flex()
        .items_center()
        .gap_6()
        .children(stats.into_iter().map(|stat| stat.render(cx)))
}

/// The rail's card. Sections inside it are separated by hairlines.
pub fn rail(cx: &App) -> Div {
    t::panel(cx).w_full().flex().flex_col().overflow_hidden()
}

pub fn rail_section(cx: &App) -> Div {
    div()
        .w_full()
        .px_4()
        .py_4()
        .flex()
        .flex_col()
        .gap_3()
        .border_b_1()
        .border_color(rgb(t::colors(cx).border))
}

/// A heading for the top of the rail: what is happening right now.
pub fn rail_heading(cx: &App, title: impl Into<SharedString>, detail: Option<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(t::display_type(title, 21.))
        .when_some(detail, |heading, detail| {
            heading.child(
                div()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(rgb(t::colors(cx).muted))
                    .child(detail),
            )
        })
}

/// A two-column grid of labelled numbers, for the rail.
pub fn stat_grid(cx: &App, stats: impl IntoIterator<Item = Stat>) -> Div {
    div()
        .w_full()
        .grid()
        .grid_cols(2)
        .gap_x_3()
        .gap_y_3()
        .children(stats.into_iter().map(|stat| stat.render(cx)))
}

pub struct Stat {
    id: &'static str,
    label: SharedString,
    value: SharedString,
    aria: Option<SharedString>,
    alert: bool,
}

impl Stat {
    pub fn new(
        id: &'static str,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            value: value.into(),
            aria: None,
            alert: false,
        }
    }

    pub fn alert(mut self, alert: bool) -> Self {
        self.alert = alert;
        self
    }

    pub fn aria(mut self, label: impl Into<SharedString>) -> Self {
        self.aria = Some(label.into());
        self
    }

    fn render(self, cx: &App) -> Stateful<Div> {
        let palette = t::colors(cx);
        div()
            .id(self.id)
            .role(Role::Status)
            .aria_label(
                self.aria
                    .unwrap_or_else(|| format!("{}: {}", self.label, self.value).into()),
            )
            .flex()
            .flex_col()
            .whitespace_nowrap()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(palette.muted))
                    .child(self.label),
            )
            .child(
                div()
                    .text_size(px(22.))
                    .line_height(px(28.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(if self.alert {
                        palette.danger
                    } else {
                        palette.ink
                    }))
                    .child(self.value),
            )
    }
}

/// A wrapping row of rail buttons.
pub fn actions() -> Div {
    div().flex().flex_wrap().gap_2()
}

/// Whether the controls help at the bottom of the rail is expanded, shared by every game.
#[derive(Default)]
struct ControlsHelp {
    collapsed: bool,
}

impl Global for ControlsHelp {}

/// The rail's last section: a disclosure listing mouse and keyboard controls.
pub fn controls_help(cx: &App, lines: &[&'static str]) -> Stateful<Div> {
    let palette = t::colors(cx);
    let open = !cx
        .try_global::<ControlsHelp>()
        .is_some_and(|help| help.collapsed);
    div()
        .id("stage-controls-help")
        .w_full()
        .flex()
        .flex_col()
        .child(
            div()
                .id("stage-controls-toggle")
                .role(Role::Button)
                .aria_label("Controls")
                .aria_expanded(open)
                .tab_index(0)
                .cursor_pointer()
                .px_4()
                .py_3()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.muted))
                .hover(|style| style.text_color(rgb(palette.ink)))
                .on_click(|_, window, cx| {
                    let help = cx.default_global::<ControlsHelp>();
                    help.collapsed = !help.collapsed;
                    window.refresh();
                })
                .child("Controls")
                .child(widgets::symbol(
                    if open { "chevron-up" } else { "chevron-down" },
                    palette.muted,
                    14.,
                )),
        )
        .when(open, |help| {
            help.child(
                div()
                    .px_4()
                    .pb_4()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(rgb(palette.muted))
                    .children(lines.iter().copied()),
            )
        })
}
