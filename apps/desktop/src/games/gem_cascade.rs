use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_gem_cascade::{
    COLUMNS, Direction, GROUP_SIZE, Game, Gem, PlacedGem, ROWS, Rotation, SPAWN_COLUMN, SPAWN_ROW,
};
use gpui::{
    ClickEvent, Context, Div, FocusHandle, FontWeight, KeyDownEvent, MouseButton, Render, Stateful,
    Window, div, prelude::*, px, rgb,
};

use crate::games::{GameEntry, GameLaunch};
use crate::theme as t;

const GAP: f32 = 3.;
const PANEL_MIN_WIDTH: f32 = 220.;
const WELL: u32 = 0x1c3b34;
const SLOT_EDGE: u32 = 0x294e42;

pub struct GameView {
    game: Game,
    focus: FocusHandle,
    focus_on_open: bool,
}

#[derive(Clone, Copy)]
enum Action {
    Shift(Direction),
    Rotate(Rotation),
    Drop,
    Restart,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let seed = match &launch.entry {
            GameEntry::Challenge(id) => id.bytes().fold(0xcbf29ce484222325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            }),
            GameEntry::Classic | GameEntry::Mode(_) => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |time| time.as_nanos() as u64),
        };
        Self {
            game: Game::new(seed),
            focus: cx.focus_handle().tab_stop(true),
            focus_on_open: true,
        }
    }

    fn perform(&mut self, action: Action, cx: &mut Context<Self>) {
        let changed = match action {
            Action::Shift(direction) => self.game.shift(direction),
            Action::Rotate(rotation) => self.game.rotate(rotation),
            Action::Drop => self.game.drop_pair().is_some(),
            Action::Restart => {
                self.game.restart();
                true
            }
        };
        if changed {
            cx.notify();
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        if stroke.modifiers.control || stroke.modifiers.platform || stroke.modifiers.alt {
            return;
        }
        let action = match stroke.key.as_str() {
            "left" => Action::Shift(Direction::Left),
            "right" => Action::Shift(Direction::Right),
            "up" => Action::Rotate(Rotation::Clockwise),
            "z" => Action::Rotate(Rotation::Counterclockwise),
            // Focused buttons handle Space themselves.
            "space" if self.focus.is_focused(window) => Action::Drop,
            "r" => Action::Restart,
            _ => return,
        };
        cx.stop_propagation();
        if !event.is_held || matches!(action, Action::Shift(_)) {
            self.perform(action, cx);
        }
    }

    fn board(&self, size: f32, cx: &Context<Self>) -> Stateful<Div> {
        let active = self.game.active();
        let landing = self.game.landing();
        div()
            .id("gem-cascade-board")
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, cx| view.focus.focus(window, cx)),
            )
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(GAP))
            .p(px(9.))
            .rounded_lg()
            .border_2()
            .border_color(rgb(t::RAIL))
            .focus(|style| style.border_color(rgb(t::GOLD)))
            .bg(rgb(t::RAIL))
            .children(self.game.board().iter().enumerate().map(|(row, cells)| {
                div()
                    .flex()
                    .gap(px(GAP))
                    .children(cells.iter().enumerate().map(|(column, &gem)| {
                        if let Some(gem) = gem_at(active, row, column) {
                            gem_tile(gem, size, Appearance::Active)
                        } else if let Some(gem) = gem {
                            gem_tile(gem, size, Appearance::Settled)
                        } else if let Some(gem) = gem_at(landing, row, column) {
                            gem_tile(gem, size, Appearance::Landing)
                        } else {
                            slot(size).bg(rgb(WELL)).border_color(rgb(
                                if column == SPAWN_COLUMN && row <= SPAWN_ROW {
                                    0x5c7964
                                } else {
                                    SLOT_EDGE
                                },
                            ))
                        }
                    }))
            }))
    }

    fn button(
        &self,
        (id, label, key, action): (&'static str, &'static str, &'static str, Action),
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let primary = matches!(action, Action::Drop);
        let enabled = !self.game.is_over() || matches!(action, Action::Restart);
        let fill = if primary && enabled {
            t::GOLD
        } else {
            t::SURFACE
        };
        let button = div()
            .id(id)
            .flex()
            .items_center()
            .justify_between()
            .flex_1()
            .gap_2()
            .min_h(px(38.))
            .px_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(if primary && enabled {
                t::GOLD
            } else {
                t::BORDER
            }))
            .bg(rgb(fill))
            .text_color(rgb(if enabled { t::INK } else { t::MUTED }))
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(13.))
            .child(label)
            .child(
                div()
                    .text_color(rgb(if primary && enabled { t::INK } else { t::MUTED }))
                    .text_size(px(12.))
                    .child(key),
            );
        if !enabled {
            return button.opacity(0.55);
        }
        button
            .tab_index(0)
            .cursor_pointer()
            .hover(|style| style.bg(rgb(if primary { t::GOLD_HOVER } else { t::SAGE })))
            .focus(|style| style.border_color(rgb(t::MUTED)))
            .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
                if matches!(event, ClickEvent::Mouse(_)) {
                    view.focus.focus(window, cx);
                }
                view.perform(action, cx);
            }))
    }

    fn panel(&self, cx: &Context<Self>) -> Div {
        let [pivot, satellite] = self.game.next_pair();
        let clear = self.game.last_clear();
        let status = if self.game.is_over() {
            "Game over. Restart to clear the blocked entrance.".to_string()
        } else {
            match clear.chain {
                0 => "Outlines show where the pair will land.".to_string(),
                1 => format!("{} gems cleared · +{}", clear.gems, clear.points),
                chain => format!(
                    "{chain}-step chain · {} gems · +{}",
                    clear.gems, clear.points
                ),
            }
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .min_w(px(PANEL_MIN_WIDTH))
            .max_w(px(260.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .gap_1()
                            .child(label("Next pair"))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(GAP))
                                    .p_1()
                                    .rounded_md()
                                    .bg(rgb(t::RAIL))
                                    .opacity(if self.game.is_over() { 0.45 } else { 1. })
                                    .child(gem_tile(satellite, 26., Appearance::Settled))
                                    .child(gem_tile(pivot, 26., Appearance::Settled)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(label("Score"))
                            .child(t::display_type(self.game.score().to_string(), 31.)),
                    ),
            )
            .child(
                div()
                    .min_h(px(42.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(rgb(if self.game.is_over() { 0x963f33 } else { t::MUTED }))
                    .child(status),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button(
                        ("gem-left", "Move left", "←", Action::Shift(Direction::Left)),
                        cx,
                    ))
                    .child(self.button(
                        ("gem-right", "Move right", "→", Action::Shift(Direction::Right)),
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button(
                        ("gem-rotate-left", "Rotate left", "Z", Action::Rotate(Rotation::Counterclockwise)),
                        cx,
                    ))
                    .child(self.button(
                        ("gem-rotate-right", "Rotate right", "↑", Action::Rotate(Rotation::Clockwise)),
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button(("gem-drop", "Drop", "Space", Action::Drop), cx))
                    .child(self.button(("gem-restart", "Restart", "R", Action::Restart), cx)),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(rgb(t::MUTED))
                    .child(format!(
                        "Connect {GROUP_SIZE} or more matching gems along their edges. A clear that sets off another earns more points."
                    )),
            )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_on_open) {
            let focus = self.focus.clone();
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        let viewport = window.viewport_size();
        let height = f32::from(viewport.height);
        let width = f32::from(viewport.width);
        let sidebar_and_gutters = if width < 1080. { 218. } else { 288. };
        let board_space = width - sidebar_and_gutters - PANEL_MIN_WIDTH - 76.;
        let size = (((height - 230.) / ROWS as f32).min(board_space / COLUMNS as f32) - GAP)
            .floor()
            .clamp(24., 45.);
        t::surface()
            .on_key_down(cx.listener(Self::key_down))
            .text_color(rgb(t::INK))
            .w_full()
            .flex()
            .flex_wrap()
            .items_start()
            .gap_5()
            .p_4()
            .child(self.board(size, cx))
            .child(self.panel(cx))
    }
}

fn label(text: &'static str) -> Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(13.))
        .text_color(rgb(t::MUTED))
        .child(text)
}

fn gem_at(pair: Option<[PlacedGem; 2]>, row: usize, column: usize) -> Option<Gem> {
    pair?
        .into_iter()
        .find(|cell| cell.row == row && cell.column == column)
        .map(|cell| cell.gem)
}

fn slot(size: f32) -> Div {
    div().size(px(size)).flex_none().rounded_md().border_1()
}

enum Appearance {
    Active,
    Settled,
    Landing,
}

fn gem_tile(gem: Gem, size: f32, appearance: Appearance) -> Div {
    let (symbol, color, edge) = match gem {
        Gem::Ruby => ("◆", 0xed8894, 0xb75268),
        Gem::Amber => ("●", 0xf3bf60, 0xc38c38),
        Gem::Jade => ("▲", 0x83c6a8, 0x4c9779),
        Gem::Amethyst => ("■", 0xc4aae5, 0x8c6dba),
    };
    let (fill, border, ink) = match appearance {
        Appearance::Active => (color, t::SURFACE, t::RAIL),
        Appearance::Settled => (color, edge, t::RAIL),
        Appearance::Landing => (WELL, edge, color),
    };
    slot(size)
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(fill))
        .border_color(rgb(border))
        .text_color(rgb(ink))
        .text_size(px(size * 0.53))
        .line_height(px(size))
        .child(symbol)
}
