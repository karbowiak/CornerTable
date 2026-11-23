mod field;
mod hud;
mod journal;

use std::{cell::Cell as ValueCell, rc::Rc, time::Instant};

use corner_table_match_three::{CELLS, Cell, Game, SIDE, Status, SwapError};
use gpui::{
    Bounds, ClickEvent, Context, Div, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Role, Stateful, Window, canvas, div, prelude::*,
    px, rgb,
};

use crate::{storage::Session, theme as t};

use super::{GameLaunch, board_space};
use field::{Exchange, Motion, Scene};
use journal::Journal;

#[derive(Clone, Copy)]
enum Action {
    Hint,
    Replay,
    NewBoard,
    Stage(usize),
    NextStage,
}

#[derive(Clone, Copy, Default)]
enum Feedback {
    #[default]
    Ready,
    Hint,
    NoMatch,
    Turn {
        points: u32,
        chain: usize,
        reshuffled: bool,
    },
}

#[derive(Clone, Copy)]
struct Pointer {
    from: Cell,
    origin: Point<Pixels>,
    preview: Option<Exchange>,
}

impl Pointer {
    fn displacement(self, position: Point<Pixels>) -> (f32, f32) {
        let delta = position - self.origin;
        (f32::from(delta.x), f32::from(delta.y))
    }

    fn exchange(self, position: Point<Pixels>, step: f32) -> Option<Exchange> {
        let (dx, dy) = self.displacement(position);
        let (dr, dc, distance) = if dx.abs() > dy.abs() {
            (0, dx.signum() as isize, dx.abs())
        } else {
            (dy.signum() as isize, 0, dy.abs())
        };
        if distance < 5. {
            return None;
        }
        let second = offset(self.from, dr, dc)?;
        Some(Exchange {
            first: self.from,
            second,
            fraction: ((distance - 5.) / (step - 5.)).min(0.85),
        })
    }
}

pub struct GameView {
    session: Session,
    journal: Journal,
    game: Game,
    selected: Option<Cell>,
    hint: Option<(Cell, Cell)>,
    cursor: Cell,
    pointer: Option<Pointer>,
    bounds: Rc<ValueCell<Option<Bounds<Pixels>>>>,
    feedback: Feedback,
    motion: Option<Motion>,
    focus: FocusHandle,
    focus_on_open: bool,
    keyboard_cursor: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let session = Session::new(launch);
        let mut journal = session
            .load::<Journal>()
            .unwrap_or_else(|| Journal::new(launch.seed()));
        let game = journal.resume();
        session.save(&journal);
        Self {
            session,
            journal,
            game,
            selected: None,
            hint: None,
            cursor: Cell::from_index(0),
            pointer: None,
            bounds: Rc::default(),
            feedback: Feedback::Ready,
            motion: None,
            focus: cx.focus_handle().tab_stop(true),
            focus_on_open: true,
            keyboard_cursor: false,
        }
    }

    fn can_play(&self) -> bool {
        self.motion.is_none() && self.game.status() == Status::Playing
    }

    fn perform(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.motion.is_some() {
            return;
        }
        match action {
            Action::Hint if self.can_play() => {
                self.selected = None;
                self.hint = self.game.suggestion();
                self.feedback = Feedback::Hint;
            }
            Action::Hint => return,
            Action::Replay | Action::NewBoard => {
                self.game = self.journal.restart(matches!(action, Action::NewBoard));
            }
            Action::Stage(_) | Action::NextStage => {
                let index = match action {
                    Action::Stage(index) => index,
                    _ => self.journal.active + 1,
                };
                if index == self.journal.active || !self.journal.is_unlocked(index) {
                    return;
                }
                self.journal.active = index;
                self.game = self.journal.resume();
            }
        }
        if !matches!(action, Action::Hint) {
            self.selected = None;
            self.hint = None;
            self.pointer = None;
            self.cursor = Cell::from_index(0);
            self.feedback = Feedback::Ready;
            self.session.save(&self.journal);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn choose(&mut self, cell: Cell, cx: &mut Context<Self>) {
        if !self.can_play() {
            return;
        }
        self.hint = None;
        self.feedback = Feedback::Ready;
        self.cursor = cell;
        match self.selected.take() {
            Some(first) if first == cell => {}
            Some(first) if first.touches(cell) => self.swap(
                Exchange {
                    first,
                    second: cell,
                    fraction: 0.,
                },
                cx,
            ),
            _ => self.selected = Some(cell),
        }
    }

    fn swap(&mut self, exchange: Exchange, cx: &mut Context<Self>) {
        let Exchange { first, second, .. } = exchange;
        let original = *self.game.board();
        let progress = self.game.progress();
        self.selected = None;
        self.hint = None;
        self.cursor = second;
        let turn = match self.game.swap(first, second) {
            Ok(turn) => {
                self.journal.add_move(
                    first.index().expect("field cell"),
                    second.index().expect("field cell"),
                    &self.game,
                );
                self.session.save(&self.journal);
                self.feedback = Feedback::Turn {
                    points: turn.points,
                    chain: turn.cascades.len(),
                    reshuffled: turn.reshuffled,
                };
                Some(turn)
            }
            Err(SwapError::NoMatch) => {
                self.feedback = Feedback::NoMatch;
                None
            }
            Err(_) => return,
        };
        if !cx.reduce_motion() {
            self.motion = Some(Motion::new(
                exchange,
                original,
                progress,
                turn,
                self.game.status() == Status::Won,
            ));
        }
        cx.notify();
    }

    fn cell_at(&self, position: Point<Pixels>, step: f32) -> Option<Cell> {
        let bounds = self.bounds.get()?;
        if !bounds.contains(&position) {
            return None;
        }
        let delta = position - bounds.origin;
        let cell = Cell {
            row: (f32::from(delta.y) / step) as usize,
            col: (f32::from(delta.x) / step) as usize,
        };
        cell.index().map(|_| cell)
    }

    fn pointer_down(
        &mut self,
        position: Point<Pixels>,
        step: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.can_play()
            && let Some(from) = self.cell_at(position, step)
        {
            self.focus.focus(window, cx);
            self.keyboard_cursor = false;
            self.cursor = from;
            self.pointer = Some(Pointer {
                from,
                origin: position,
                preview: None,
            });
            cx.notify();
        }
    }

    fn pointer_up(&mut self, position: Point<Pixels>, step: f32, cx: &mut Context<Self>) {
        let Some(pointer) = self.pointer.take() else {
            return;
        };
        let (dx, dy) = pointer.displacement(position);
        if self.can_play() {
            if dx.abs().max(dy.abs()) >= step * 0.29 {
                if let Some(exchange) = pointer.exchange(position, step) {
                    self.swap(exchange, cx);
                }
            } else if self.cell_at(position, step).is_some() {
                self.choose(pointer.from, cx);
            }
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        if stroke.modifiers.alt || stroke.modifiers.control || stroke.modifiers.platform {
            return;
        }
        if !self.can_play() {
            return;
        }
        self.keyboard_cursor = true;
        let direction = match stroke.key.as_str() {
            "up" => (-1, 0),
            "down" => (1, 0),
            "left" => (0, -1),
            "right" => (0, 1),
            "enter" | "space" => {
                if !event.is_held {
                    self.choose(self.cursor, cx);
                    cx.notify();
                }
                cx.stop_propagation();
                return;
            }
            "escape" => {
                self.selected = None;
                self.hint = None;
                self.pointer = None;
                self.feedback = Feedback::Ready;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            "h" => {
                if !event.is_held {
                    self.perform(Action::Hint, window, cx);
                }
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        self.pointer = None;
        if let Some(cell) = offset(self.cursor, direction.0, direction.1) {
            self.cursor = cell;
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn board(
        &self,
        step: f32,
        scene: Scene<'_>,
        window: &Window,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let focused = self.focus.is_focused(window);
        let playing = self.can_play();
        let bounds = Rc::clone(&self.bounds);
        let board_size = step * SIDE as f32;
        let tiles = scene.board();
        let current_gem = tiles[self.cursor.index().expect("cursor within field")];

        field::frame(
            focused,
            self.motion.is_none() && self.game.status() == Status::Won,
        )
        .id("match-three-board")
        .role(Role::Grid)
        .aria_label("Match Three field")
        .aria_description(format!(
            "Arrow keys move. Space selects a jewel. H shows a hint. Row {}, column {}, {}.",
            self.cursor.row + 1,
            self.cursor.col + 1,
            current_gem.name()
        ))
        .track_focus(&self.focus)
        .on_key_down(cx.listener(Self::key_down))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                view.pointer_down(event.position, step, window, cx);
            }),
        )
        .on_mouse_move(cx.listener(move |view, event: &MouseMoveEvent, _, cx| {
            if let Some(pointer) = &mut view.pointer {
                if event.dragging() {
                    pointer.preview = if cx.reduce_motion() {
                        None
                    } else {
                        pointer.exchange(event.position, step)
                    };
                } else {
                    view.pointer = None;
                }
                cx.notify();
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |view, event: &MouseUpEvent, _, cx| {
                view.pointer_up(event.position, step, cx)
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|view, _, _, cx| {
                if view.pointer.take().is_some() {
                    cx.notify();
                }
            }),
        )
        .child(
            field::floor()
                .size(px(board_size))
                .child(
                    canvas(move |area, _, _| bounds.set(Some(area)), |_, (), _, _| {})
                        .absolute()
                        .size_full(),
                )
                .children((0..CELLS).map(|index| field::well(step, Cell::from_index(index))))
                .child(scene.sprites(step))
                .children(scene.sparks(step))
                .children(tiles.into_iter().enumerate().map(|(index, gem)| {
                    let cell = Cell::from_index(index);
                    let is_hint = self.hint.is_some_and(|(a, b)| cell == a || cell == b);
                    let armed = self.selected == Some(cell);
                    let is_cursor = focused && self.keyboard_cursor && self.cursor == cell;
                    let is_pressed = self.pointer.is_some_and(|pointer| pointer.from == cell);
                    let gold = rgb(0xffcf70);
                    let ivory = rgb(0xf2f0d4);
                    let highlight = armed || is_hint || is_cursor || is_pressed;
                    div()
                        .id(("match-three-jewel", index))
                        .role(Role::Button)
                        .aria_label(format!(
                            "{} at row {}, column {}{}",
                            gem.name(),
                            cell.row + 1,
                            cell.col + 1,
                            if is_hint { ", hint pair" } else { "" },
                        ))
                        .aria_selected(armed)
                        .when(is_cursor, |tile| tile.aria_active_descendant())
                        .absolute()
                        .left(px(cell.col as f32 * step + 1.))
                        .top(px(cell.row as f32 * step + 1.))
                        .size(px(step - 2.))
                        .rounded(px((step * 0.15).clamp(4., 9.)))
                        .when(highlight, |tile| {
                            tile.border_2()
                                .border_color(if is_cursor && !armed && !is_hint {
                                    ivory
                                } else {
                                    gold
                                })
                        })
                        .when(armed || is_pressed, |tile| tile.bg(gold.opacity(0.10)))
                        .when(is_hint && !armed, |tile| tile.bg(gold.opacity(0.13)))
                        .when(playing, |tile| {
                            tile.cursor_pointer()
                                .hover(|style| style.bg(ivory.opacity(0.12)))
                                .on_click(cx.listener(move |view, event, window, cx| {
                                    // Pointer releases go through the field's shared drag handler.
                                    if !matches!(event, ClickEvent::Mouse(_)) {
                                        view.focus.focus(window, cx);
                                        view.keyboard_cursor = true;
                                        view.choose(cell, cx);
                                        cx.notify();
                                    }
                                }))
                        })
                })),
        )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_on_open) {
            cx.on_next_frame(window, |view, window, cx| view.focus.focus(window, cx));
        }
        let now = Instant::now();
        if let Some(motion) = &mut self.motion {
            if cx.reduce_motion() || !motion.advance(now) {
                self.motion = None;
            } else {
                window.request_animation_frame();
            }
        }

        let available = board_space(window) + 36.;
        let compact = available < 660.;
        let gutter = if compact { 12. } else { 20. };
        let hud_width = ((available - gutter * 3.) * 0.315).clamp(150., 294.);
        let field_width = available - gutter * 3. - hud_width - 2.;
        let field_height = f32::from(window.viewport_size().height) - 107. - gutter * 2.;
        let step = ((field_width.min(field_height) - field::FRAME * 2.) / SIDE as f32)
            .floor()
            .clamp(30., 89.);
        let settled = self.game.board();
        let scene = match &self.motion {
            Some(motion) => motion.scene(now, settled),
            None => self
                .pointer
                .and_then(|pointer| pointer.preview)
                .map_or(Scene::Still(settled), |exchange| {
                    Scene::Exchange(settled, exchange)
                }),
        };
        let displayed = self
            .motion
            .as_ref()
            .map_or(self.game.progress(), Motion::progress);

        t::panel(cx)
            .w_full()
            .flex()
            .items_start()
            .justify_center()
            .gap(px(gutter))
            .p(px(gutter))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .flex_none()
                    .child(self.board(step, scene, window, cx))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .px(px(1.))
                            .text_size(px(if compact { 11. } else { 12. }))
                            .line_height(px(17.))
                            .text_color(rgb(t::colors(cx).muted))
                            .child("Drag or click an adjacent pair")
                            .child("Arrows + Space"),
                    ),
            )
            .child(self.hud(hud_width, compact, displayed, cx))
    }
}

fn offset(cell: Cell, row: isize, col: isize) -> Option<Cell> {
    let target = Cell {
        row: cell.row.checked_add_signed(row)?,
        col: cell.col.checked_add_signed(col)?,
    };
    target.index().map(|_| target)
}
