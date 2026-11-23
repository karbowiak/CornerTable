use std::{cell::Cell, rc::Rc, time::Instant};

use corner_table_block_puzzle::{
    CHALLENGE_SEED, CHALLENGE_TRAYS, Game, Move, Piece, Placement, SIDE, TRAY_SIZE, Wood,
};
use gpui::{
    Bounds, ClickEvent, Context, Div, EntityId, FocusHandle, FontWeight, KeyDownEvent, MouseButton,
    ObjectFit, Pixels, Point, Role, Stateful, Window, canvas, div, img, prelude::*, px, rgb, rgba,
};
use serde::{Deserialize, Serialize};

use crate::{storage::Session, theme as t};

use super::{GameLaunch, board_space, themed_control};

const CELL_GAP: f32 = 3.;
const BOARD_EDGE: f32 = 12.;
const PANEL_EDGE: f32 = 17.;
const WALNUT: u32 = 0x4e3125;
const PREVIEW_FITS: u32 = 0xffdb86;
const PREVIEW_BLOCKED: u32 = 0xffa39a;
const CLEAR_DURATION: f32 = 0.5;

struct PlayLayout {
    cell: f32,
    board: f32,
    controls: f32,
    gap: f32,
    stacked: bool,
}

impl PlayLayout {
    fn fit(width: f32, viewport_height: f32) -> Self {
        let gap = if width < 560. { 14. } else { 22. };
        let gutters = 2. * BOARD_EDGE + (SIDE - 1) as f32 * CELL_GAP;
        let rail = (width * 0.42).clamp(200., 268.);
        let beside = width - rail - gap;
        let stacked = beside < 26. * SIDE as f32 + gutters;
        let board_room = if stacked { width } else { beside };
        // Leave room for the page title and spacing as the window grows vertically.
        let height = (viewport_height * 0.74).min(viewport_height - 112.);
        let cell = ((board_room.min(height) - gutters) / SIDE as f32)
            .floor()
            .max(20.);
        let board = cell * SIDE as f32 + gutters;
        Self {
            cell,
            board,
            controls: if stacked { board } else { rail },
            gap,
            stacked,
        }
    }

    fn panel_width(&self) -> f32 {
        let content = if self.stacked {
            self.board
        } else {
            self.board + self.gap + self.controls
        };
        content + 2. * PANEL_EDGE
    }
}

#[derive(Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    #[default]
    Classic,
    TenTrays,
}

impl Mode {
    fn game(self, seed: u64) -> Game {
        match self {
            Self::Classic => Game::new(seed),
            Self::TenTrays => Game::ten_trays(seed),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct SavedSession {
    // The original sessions stored these Classic fields directly under state.
    #[serde(flatten)]
    classic: SavedRound,
    #[serde(default)]
    mode: Mode,
    #[serde(default)]
    ten_trays: Option<SavedRound>,
}

#[derive(Deserialize, Serialize)]
struct SavedRound {
    seed: u64,
    turns: Vec<(usize, usize, usize)>,
    can_undo: bool,
    #[serde(default)]
    best: u64,
}

struct Round {
    game: Game,
    seed: u64,
    turns: Vec<(usize, usize, usize)>,
    undo: Option<Game>,
    best: u64,
}

impl Round {
    fn new(mode: Mode, seed: u64) -> Self {
        Self {
            game: mode.game(seed),
            seed,
            turns: Vec::new(),
            undo: None,
            best: 0,
        }
    }

    fn restore(mode: Mode, saved: SavedRound) -> Option<Self> {
        let mut round = Self::new(mode, saved.seed);
        for (index, &(slot, column, row)) in saved.turns.iter().enumerate() {
            if saved.can_undo && index + 1 == saved.turns.len() {
                round.undo = Some(round.game.clone());
            }
            round.game.place(Move { slot, column, row })?;
        }
        round.turns = saved.turns;
        round.best = saved.best.max(round.game.score());
        Some(round)
    }

    fn saved(&self) -> SavedRound {
        SavedRound {
            seed: self.seed,
            turns: self.turns.clone(),
            can_undo: self.undo.is_some(),
            best: self.best,
        }
    }
}

/// A piece's top-left anchor can be outside the board when held by a tile farther in.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Anchor {
    column: i32,
    row: i32,
}

impl Anchor {
    fn turn(self, slot: usize) -> Option<Move> {
        Some(Move {
            slot,
            column: self.column.try_into().ok()?,
            row: self.row.try_into().ok()?,
        })
    }

    fn offset(self, (column, row): (i32, i32)) -> Self {
        Self {
            column: self.column + column,
            row: self.row + row,
        }
    }

    fn step(self, direction: (i32, i32), piece: Option<Piece>) -> Self {
        let (width, height) = piece.map_or((1, 1), Piece::dimensions);
        let next = self.offset(direction);
        Self {
            column: next.column.clamp(0, (SIDE - width) as i32),
            row: next.row.clamp(0, (SIDE - height) as i32),
        }
    }
}

#[derive(Clone, Copy)]
struct Grid {
    bounds: Bounds<Pixels>,
    cell: f32,
}

impl Grid {
    fn anchor_at(self, position: Point<Pixels>) -> Option<Anchor> {
        if !self.bounds.contains(&position) {
            return None;
        }
        let local = position - self.bounds.origin;
        let index = |distance: Pixels| {
            ((f32::from(distance) + CELL_GAP / 2.) / (self.cell + CELL_GAP)).floor() as i32
        };
        Some(Anchor {
            column: index(local.x),
            row: index(local.y),
        })
    }
}

#[derive(Clone, Copy)]
struct Preview {
    anchor: Anchor,
    piece: Piece,
    fits: bool,
}

impl Preview {
    fn covers(self, column: usize, row: usize) -> bool {
        let cell = (column as i32, row as i32);
        let anchor = (self.anchor.column, self.anchor.row);
        let mut tiles = self
            .piece
            .tiles
            .iter()
            .map(|&(x, y)| (anchor.0 + x as i32, anchor.1 + y as i32));
        let on_board = 0..SIDE as i32;
        // If a corner clips an entire shape, still mark the rejected anchor.
        tiles.clone().any(|tile| tile == cell)
            || (cell == anchor
                && !tiles.any(|(x, y)| on_board.contains(&x) && on_board.contains(&y)))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct DragSource {
    owner: EntityId,
    revision: u64,
    slot: usize,
}

#[derive(Clone, Copy)]
struct Drag {
    source: DragSource,
    grabbed: (usize, usize),
}

impl Drag {
    fn anchor_at(self, grid: Grid, pointer: Point<Pixels>) -> Option<Anchor> {
        Some(
            grid.anchor_at(pointer)?
                .offset((-(self.grabbed.0 as i32), -(self.grabbed.1 as i32))),
        )
    }
}

struct FloatingPiece {
    grid: Rc<Cell<Option<Grid>>>,
    piece: Piece,
    cell: f32,
    offset: Point<Pixels>,
}

impl Render for FloatingPiece {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let above_grid = self
            .grid
            .get()
            .is_some_and(|grid| grid.anchor_at(window.mouse_position()).is_some());
        // The board preview replaces the floating piece as it snaps into the grid.
        div().size(px(1.)).when(!above_grid, |holder| {
            holder.child(
                div()
                    .absolute()
                    .left(self.offset.x)
                    .top(self.offset.y)
                    .opacity(0.88)
                    .child(piece_shape(self.piece, self.cell, CELL_GAP)),
            )
        })
    }
}

enum Feedback {
    Placed(Placement),
    Invalid,
    Undone,
}

struct Clear {
    started: Instant,
    tiles: [[Option<Wood>; SIDE]; SIDE],
    turn: Move,
    lines: usize,
}

impl Clear {
    fn new(placement: Placement, turn: Move, piece: Piece, before: &Game) -> Self {
        let mut tiles = *before.board();
        for &(x, y) in piece.tiles {
            tiles[turn.row + y][turn.column + x] = Some(piece.wood);
        }
        for (row, tiles) in tiles.iter_mut().enumerate() {
            for (column, tile) in tiles.iter_mut().enumerate() {
                if !placement.cleared_rows[row] && !placement.cleared_columns[column] {
                    *tile = None;
                }
            }
        }
        Self {
            started: Instant::now(),
            tiles,
            turn,
            lines: placement.lines,
        }
    }

    fn elapsed(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }

    fn tile(&self, column: usize, row: usize, cell: f32, elapsed: f32) -> Option<Div> {
        let wood = self.tiles[row][column]?;
        let distance = column.abs_diff(self.turn.column) + row.abs_diff(self.turn.row);
        let delay = 0.011 * distance as f32;
        let sweep = ((elapsed - delay) / 0.32).clamp(0., 1.);
        let eased = 1. - (1. - sweep).powi(3);
        let inset = cell * 0.23 * eased;
        (sweep < 1.).then(|| {
            div()
                .absolute()
                .left(px(inset))
                .top(px(inset))
                .opacity(1. - sweep)
                .child(wood_tile(wood, cell - 2. * inset))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded_sm()
                        .bg(rgb(PREVIEW_FITS))
                        .opacity((0.6 + 0.1 * self.lines.min(3) as f32) * (1. - eased)),
                )
        })
    }
}

pub struct GameView {
    session: Session,
    mode: Mode,
    classic: Round,
    ten_trays: Round,
    focus: FocusHandle,
    focus_on_mount: bool,
    selection: Option<usize>,
    cursor: Anchor,
    using_keyboard: bool,
    hovered: Option<Anchor>,
    grid: Rc<Cell<Option<Grid>>>,
    pending_grab: Option<(usize, (usize, usize))>,
    drag: Option<Drag>,
    revision: u64,
    feedback: Option<Feedback>,
    clear: Option<Clear>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let seed = launch.seed();
        let session = Session::new(launch);
        let saved = session.load::<SavedSession>();
        let mode = saved.as_ref().map_or(Mode::Classic, |saved| saved.mode);
        let (classic, ten_trays) = saved.map_or((None, None), |saved| {
            (
                Round::restore(Mode::Classic, saved.classic),
                saved
                    .ten_trays
                    .and_then(|round| Round::restore(Mode::TenTrays, round)),
            )
        });
        Self {
            session,
            mode,
            classic: classic.unwrap_or_else(|| Round::new(Mode::Classic, seed)),
            ten_trays: ten_trays.unwrap_or_else(|| Round::new(Mode::TenTrays, CHALLENGE_SEED)),
            focus: cx.focus_handle().tab_stop(true),
            focus_on_mount: true,
            selection: None,
            cursor: Anchor::default(),
            using_keyboard: false,
            hovered: None,
            grid: Rc::new(Cell::new(None)),
            pending_grab: None,
            drag: None,
            revision: 0,
            feedback: None,
            clear: None,
        }
    }

    fn round(&self) -> &Round {
        match self.mode {
            Mode::Classic => &self.classic,
            Mode::TenTrays => &self.ten_trays,
        }
    }

    fn round_mut(&mut self) -> &mut Round {
        match self.mode {
            Mode::Classic => &mut self.classic,
            Mode::TenTrays => &mut self.ten_trays,
        }
    }

    fn save(&self) {
        self.session.save(&SavedSession {
            classic: self.classic.saved(),
            mode: self.mode,
            ten_trays: Some(self.ten_trays.saved()),
        });
    }

    fn clear_input(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.selection = None;
        self.hovered = None;
        self.pending_grab = None;
        self.drag = None;
        self.feedback = None;
        self.clear = None;
    }

    fn switch(&mut self, mode: Mode) {
        if self.mode != mode {
            self.mode = mode;
            self.clear_input();
            self.cursor = Anchor::default();
            self.save();
        }
    }

    fn restart(&mut self, new_board: bool) {
        let mode = self.mode;
        let round = self.round_mut();
        if new_board {
            round.seed = round.seed.wrapping_add(1);
        }
        round.game = mode.game(round.seed);
        round.turns.clear();
        round.undo = None;
        self.clear_input();
        self.cursor = Anchor::default();
        self.save();
    }

    fn undo(&mut self) {
        let round = self.round_mut();
        if let Some(previous) = round.undo.take() {
            round.game = previous;
            round.turns.pop();
            self.clear_input();
            self.feedback = Some(Feedback::Undone);
            self.save();
        }
    }

    fn select(&mut self, slot: usize, keyboard: bool) {
        if let Some(piece) = self.round().game.tray()[slot] {
            self.selection = (keyboard || self.selection != Some(slot)).then_some(slot);
            self.cursor = self.cursor.step((0, 0), Some(piece));
            self.using_keyboard = keyboard;
            self.hovered = None;
            self.feedback = None;
        }
    }

    fn preview(&self, window: &Window) -> Option<Preview> {
        let (slot, anchor) = if let Some(drag) = self.drag {
            (
                drag.source.slot,
                drag.anchor_at(self.grid.get()?, window.mouse_position())?,
            )
        } else {
            let anchor = if self.using_keyboard && self.focus.is_focused(window) {
                self.cursor
            } else {
                self.hovered?
            };
            (self.selection?, anchor)
        };
        let game = &self.round().game;
        Some(Preview {
            anchor,
            piece: game.tray()[slot]?,
            fits: anchor.turn(slot).is_some_and(|turn| game.can_place(turn)),
        })
    }

    fn place(&mut self, anchor: Anchor, slot: usize, animate: bool) {
        let round = self.round_mut();
        let Some(turn) = anchor.turn(slot).filter(|&turn| round.game.can_place(turn)) else {
            self.feedback = Some(Feedback::Invalid);
            return;
        };
        let piece = round.game.tray()[turn.slot].expect("validated tray slot");
        let before = round.game.clone();
        let placement = round.game.place(turn).expect("validated placement");
        round.best = round.best.max(round.game.score());
        let clear =
            (animate && placement.lines > 0).then(|| Clear::new(placement, turn, piece, &before));
        round.undo = Some(before);
        round.turns.push((slot, turn.column, turn.row));
        self.clear = clear;
        self.revision = self.revision.wrapping_add(1);
        self.cursor = anchor;
        self.selection = None;
        self.feedback = Some(Feedback::Placed(placement));
        self.save();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.modifiers.modified() {
            if key.key == "z"
                && key.modifiers.secondary(cx)
                && key.modifiers.number_of_modifiers() == 1
            {
                cx.stop_active_drag(window);
                self.undo();
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        match key.key.as_str() {
            "escape" if self.selection.is_some() || self.drag.is_some() => {
                cx.stop_active_drag(window);
                self.selection = None;
                self.drag = None;
                self.feedback = None;
            }
            "1" | "2" | "3" if self.drag.is_none() => {
                let slot = usize::from(key.key.as_bytes()[0] - b'1');
                if self.round().game.tray()[slot].is_some() {
                    self.select(slot, true);
                    self.focus.focus(window, cx);
                }
            }
            key if self.focus.is_focused(window) && self.drag.is_none() => {
                let direction = match key {
                    "left" => (-1, 0),
                    "right" => (1, 0),
                    "up" => (0, -1),
                    "down" => (0, 1),
                    _ => return,
                };
                self.using_keyboard = true;
                let piece = self
                    .selection
                    .and_then(|slot| self.round().game.tray()[slot]);
                self.cursor = self.cursor.step(direction, piece);
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn board(&self, cell: f32, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let preview = self.preview(window);
        let keyboard = self.using_keyboard && self.focus.is_focused(window) && self.drag.is_none();
        let clear = self.clear.as_ref().map(|clear| (clear, clear.elapsed()));
        let geometry = self.grid.clone();
        let label = if keyboard {
            let fit = if let Some(preview) = preview {
                if preview.fits {
                    ", piece fits"
                } else {
                    ", piece blocked"
                }
            } else {
                ""
            };
            format!(
                "Block puzzle board, row {}, column {}{fit}",
                self.cursor.row + 1,
                self.cursor.column + 1
            )
        } else {
            "Block puzzle board".to_owned()
        };
        div()
            .id("block-puzzle-board")
            .role(Role::Grid)
            .aria_row_count(SIDE)
            .aria_column_count(SIDE)
            .aria_label(label)
            .aria_description("Select a tray piece with 1, 2 or 3, move with the arrow keys, and press Enter to place it.")
            .track_focus(&self.focus)
            .tab_index(0)
            .focus_visible(|style| style.border_color(rgb(t::GOLD)))
            .relative()
            .flex_none()
            .p(px(BOARD_EDGE - 1.))
            .rounded(px(12.))
            .border_1()
            .border_color(rgb(0x916f54))
            .bg(rgb(WALNUT))
            .shadow_md()
            .when(self.selection.is_some(), |board| board.cursor_pointer())
            .child(
                img("games/block-puzzle/walnut-board.jpg")
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill)
                    .rounded(px(11.)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap(px(CELL_GAP))
                    .child(
                        canvas(
                            move |bounds, _, _| geometry.set(Some(Grid { bounds, cell })),
                            |_, (), _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .children(self.round().game.board().iter().enumerate().map(|(row, cells)| {
                        div()
                            .id(("block-puzzle-row", row))
                            .role(Role::Row)
                            .aria_row_index(row)
                            .flex()
                            .gap(px(CELL_GAP))
                            .children(cells.iter().enumerate().map(|(column, &wood)| {
                                let ghost = preview.filter(|preview| preview.covers(column, row));
                                let cursor = keyboard
                                    && column as i32 == self.cursor.column
                                    && row as i32 == self.cursor.row;
                                let fading = clear.and_then(|(clear, elapsed)| {
                                    clear.tile(column, row, cell, elapsed)
                                });
                                let label = match wood {
                                    Some(wood) => wood_name(wood),
                                    None => "Empty",
                                };
                                div()
                                    .id(("block-puzzle-cell", column))
                                    .role(Role::GridCell)
                                    .aria_column_index(column)
                                    .aria_label(label)
                                    .aria_selected(cursor)
                                    .relative()
                                    .flex_none()
                                    .size(px(cell))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgba(0x1d1009b8))
                                    .bg(rgba(0x28180d96))
                                    .children(wood.map(|wood| {
                                        div()
                                            .absolute()
                                            .left(px(-1.))
                                            .top(px(-1.))
                                            .child(wood_tile(wood, cell))
                                    }))
                                    .children(fading)
                                    .when_some(ghost, |well, ghost| {
                                        well.child(placement_ghost(ghost, cell))
                                    })
                                    .when(cursor, |well| {
                                        well.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .rounded_sm()
                                                .border_2()
                                                .border_color(rgb(t::PAPER)),
                                        )
                                    })
                            }))
                    })),
            )
            .on_mouse_move(cx.listener(|view, event: &gpui::MouseMoveEvent, _, cx| {
                if view.drag.is_some() {
                    return;
                }
                let hovered = view.grid.get().and_then(|grid| grid.anchor_at(event.position));
                if view.hovered != hovered || view.using_keyboard {
                    view.hovered = hovered;
                    view.using_keyboard = false;
                    cx.notify();
                }
            }))
            .on_hover(cx.listener(|view, inside: &bool, _, cx| {
                if !inside && view.hovered.take().is_some() {
                    cx.notify();
                }
            }))
            .on_click(cx.listener(|view, event: &ClickEvent, window, cx| {
                let anchor = if matches!(event, ClickEvent::Keyboard(_)) {
                    view.using_keyboard = true;
                    Some(view.cursor)
                } else {
                    view.using_keyboard = false;
                    view.grid.get().and_then(|grid| grid.anchor_at(event.position()))
                };
                if let Some(anchor) = anchor {
                    view.focus.focus(window, cx);
                    if let Some(slot) = view.selection {
                        view.place(anchor, slot, !cx.reduce_motion());
                    } else {
                        view.cursor = anchor;
                    }
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|view, source: &DragSource, window, cx| {
                if let Some(drag) = view.drag.take()
                    && drag.source == *source
                    && source.owner == cx.entity_id()
                    && source.revision == view.revision
                    && let Some(anchor) = view
                        .grid
                        .get()
                        .and_then(|grid| drag.anchor_at(grid, window.mouse_position()))
                {
                    view.place(anchor, source.slot, !cx.reduce_motion());
                }
                cx.notify();
            }))
    }

    fn tray(&self, cell: f32, width: f32, cx: &Context<Self>) -> Div {
        let card_width = (width - 16.) / TRAY_SIZE as f32;
        let tray_cell = (cell * 0.39).clamp(11., 19.).min((card_width - 11.) / 4.);
        div().flex().gap_2().children(
            self.round()
                .game
                .tray()
                .iter()
                .enumerate()
                .map(|(slot, &piece)| self.tray_slot(slot, piece, cell, tray_cell, cx)),
        )
    }

    fn tray_slot(
        &self,
        slot: usize,
        piece: Option<Piece>,
        cell: f32,
        tray_cell: f32,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let palette = t::colors(cx);
        let selected =
            self.selection == Some(slot) || self.drag.is_some_and(|drag| drag.source.slot == slot);
        let card = div()
            .id(("block-puzzle-piece", slot))
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(4. * tray_cell + 11.))
            .rounded_md()
            .border_1()
            .border_color(rgb(if selected {
                palette.gold_text
            } else {
                palette.border
            }))
            .bg(rgb(if selected {
                palette.hover
            } else {
                palette.paper
            }))
            .flex()
            .items_center()
            .justify_center();
        let Some(piece) = piece else {
            return card.bg(rgb(palette.surface));
        };
        let (width, height) = piece.dimensions();
        let key = (slot + 1).to_string();
        let blocks = match piece.tiles.len() {
            1 => "1 block".to_owned(),
            count => format!("{count} blocks"),
        };
        let source = DragSource {
            slot,
            owner: cx.entity_id(),
            revision: self.revision,
        };
        let card_bounds = Rc::new(Cell::new(Bounds::default()));
        let layout_bounds = card_bounds.clone();
        let game = cx.weak_entity();
        let grid = self.grid.clone();
        card.role(Role::Button)
            .aria_label(format!(
                "Piece {key}, {blocks}, {width} wide by {height} high"
            ))
            .aria_toggled(selected.into())
            .aria_keyshortcuts(key.clone())
            .tab_index(0)
            .cursor_grab()
            .hover(|style| style.bg(rgb(palette.hover)))
            .focus_visible(|style| style.border_color(rgb(palette.ink)))
            .child(
                canvas(
                    move |bounds, _, _| layout_bounds.set(bounds),
                    |_, (), _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(piece_shape(piece, tray_cell, 1.))
            .child(
                div()
                    .absolute()
                    .left(px(5.))
                    .bottom(px(2.))
                    .text_color(rgb(palette.muted))
                    .text_size(px(10.))
                    .line_height(px(12.))
                    .child(key),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &gpui::MouseDownEvent, _, _| {
                    let grabbed = closest_tile(
                        piece,
                        event.position - card_bounds.get().center(),
                        tray_cell + 1.,
                    );
                    view.pending_grab = Some((slot, grabbed));
                }),
            )
            .on_drag(source, move |source, offset, window, cx| {
                let center_tile = closest_tile(piece, Point::default(), tray_cell + 1.);
                let grabbed = game
                    .update(cx, |view, cx| {
                        let grabbed = view
                            .pending_grab
                            .take()
                            .filter(|(slot, _)| *slot == source.slot)
                            .map_or(center_tile, |(_, tile)| tile);
                        view.selection = None;
                        view.using_keyboard = false;
                        view.feedback = None;
                        view.drag = Some(Drag {
                            source: *source,
                            grabbed,
                        });
                        view.focus.focus(window, cx);
                        cx.notify();
                        grabbed
                    })
                    .unwrap_or(center_tile);
                let floating_offset = Point::new(
                    offset.x - px(grabbed.0 as f32 * (cell + CELL_GAP) + cell / 2.),
                    offset.y - px(grabbed.1 as f32 * (cell + CELL_GAP) + cell / 2.),
                );
                cx.new(|_| FloatingPiece {
                    piece,
                    grid: grid.clone(),
                    offset: floating_offset,
                    cell,
                })
            })
            .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
                view.select(slot, matches!(event, ClickEvent::Keyboard(_)));
                view.focus.focus(window, cx);
                cx.notify();
            }))
    }

    fn controls(&self, width: f32, cell: f32, cx: &Context<Self>) -> Div {
        let palette = t::colors(cx);
        let round = self.round();
        let score = round.game.score();
        let compact = width < 230.;
        let round_control = |id, label, enabled| {
            themed_control(cx, id, label, enabled)
                .when(compact, |button| button.px_2().text_size(px(12.)))
        };
        let remaining = round.game.placements_left();
        let status = match remaining {
            Some(0) => "All ten trays placed.".into(),
            _ if !round.game.has_moves() => "No pieces fit. Undo or restart.".into(),
            _ if self.drag.is_some() => "Drop the piece onto the board.".into(),
            _ if self.selection.is_some() && self.using_keyboard => {
                "Move the piece with the arrow keys.".into()
            }
            _ if self.selection.is_some() => "Click the board to place the piece.".into(),
            Some(remaining) => format!(
                "Tray {} of {CHALLENGE_TRAYS} · {remaining} pieces left",
                CHALLENGE_TRAYS - (remaining - 1) / TRAY_SIZE
            ),
            None => "Complete rows or columns to clear space.".into(),
        };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(width))
            .gap(px(if compact { 9. } else { 12. }))
            .child(
                div().flex().gap_2().children(
                    [
                        ("block-puzzle-classic", "Classic", Mode::Classic),
                        ("block-puzzle-ten-trays", "Ten Trays", Mode::TenTrays),
                    ]
                    .into_iter()
                    .map(|(id, label, mode)| {
                        let active = self.mode == mode;
                        themed_control(cx, id, label, true)
                            .flex_1()
                            .justify_center()
                            .aria_toggled(active.into())
                            .when(active, |button| {
                                button
                                    .bg(rgb(palette.hover))
                                    .border_color(rgb(palette.gold_text))
                            })
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.switch(mode);
                                view.focus.focus(window, cx);
                                cx.notify();
                            }))
                    }),
                ),
            )
            .child(
                div()
                    .id("block-puzzle-score")
                    .role(Role::Label)
                    .aria_label(format!("{score} points, personal best {}", round.best))
                    .flex()
                    .justify_between()
                    .items_baseline()
                    .flex_wrap()
                    .gap_x_2()
                    .child(t::display_type(
                        format!("{score} points"),
                        if compact { 21. } else { 25. },
                    ))
                    .child(
                        div()
                            .text_color(rgb(palette.muted))
                            .text_size(px(13.))
                            .child(format!("Best {}", round.best)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_baseline()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.))
                                    .child("Tray"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(palette.muted))
                                    .child("Drag or click a piece"),
                            ),
                    )
                    .child(self.tray(cell, width, cx)),
            )
            .child(self.status(status, palette, compact))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        round_control("block-puzzle-undo", "Undo", round.undo.is_some())
                            .aria_keyshortcuts(if cx.platform_info().is_mac() {
                                "Meta+Z"
                            } else {
                                "Control+Z"
                            })
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.undo();
                                view.focus.focus(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        round_control("block-puzzle-restart", "Restart", true).on_click(
                            cx.listener(|view, _, window, cx| {
                                view.restart(false);
                                view.focus.focus(window, cx);
                                cx.notify();
                            }),
                        ),
                    )
                    .when(self.mode == Mode::Classic, |controls| {
                        controls.child(
                            round_control("block-puzzle-new", "New board", true).on_click(
                                cx.listener(|view, _, window, cx| {
                                    view.restart(true);
                                    view.focus.focus(window, cx);
                                    cx.notify();
                                }),
                            ),
                        )
                    }),
            )
            .child(
                div()
                    .text_color(rgb(palette.muted))
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .child("1–3 pick · Arrows move · Enter place"),
            )
    }

    fn status(&self, status: String, palette: t::Palette, compact: bool) -> Stateful<Div> {
        let mut panel = div()
            .id("block-puzzle-status")
            .role(Role::Status)
            .flex()
            .flex_col()
            .justify_center()
            .min_h(px(if compact { 46. } else { 59. }))
            .text_size(px(13.))
            .line_height(px(18.));
        if let Some(feedback) = &self.feedback {
            let (label, color) = match feedback {
                Feedback::Placed(placement) => {
                    let points = placement.points;
                    let label = match placement.lines {
                        0 => format!("+{points} points"),
                        1 => format!("+{points} points · 1 line"),
                        lines => format!("+{points} points · {lines} lines"),
                    };
                    (
                        label,
                        if placement.lines > 1 {
                            palette.gold_text
                        } else {
                            palette.success
                        },
                    )
                }
                Feedback::Invalid => ("The piece needs more space.".to_owned(), palette.danger),
                Feedback::Undone => ("Placement undone.".to_owned(), palette.ink),
            };
            let announcement = format!("{}. {status}", label.trim_end_matches('.'));
            panel = panel.aria_label(announcement).child(
                div()
                    .relative()
                    .rounded_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(color))
                    .when_some(self.clear.as_ref(), |delta, clear| {
                        let progress = (clear.elapsed() / CLEAR_DURATION).clamp(0., 1.);
                        delta.child(
                            div()
                                .absolute()
                                .inset_0()
                                .rounded_sm()
                                .bg(rgb(if clear.lines > 1 {
                                    t::GOLD
                                } else {
                                    palette.success
                                }))
                                .opacity(0.34 * (1. - progress).powi(2)),
                        )
                    })
                    .child(div().relative().child(label)),
            );
        } else {
            panel = panel.aria_label(status.clone());
        }
        panel.child(div().text_color(rgb(palette.muted)).child(status))
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_on_mount {
            self.focus_on_mount = false;
            cx.defer_in(window, |view, window, cx| view.focus.focus(window, cx));
        }
        if self.drag.is_some() && !cx.has_active_drag() {
            self.drag = None;
        }
        if let Some(clear) = &self.clear {
            if clear.elapsed() < CLEAR_DURATION && !cx.reduce_motion() {
                window.request_animation_frame();
            } else {
                self.clear = None;
            }
        }
        let layout = PlayLayout::fit(board_space(window), window.viewport_size().height.into());
        t::panel(cx)
            .id("block-puzzle-game")
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .max_w(px(layout.panel_width()))
            .p(px(PANEL_EDGE - 1.))
            .flex()
            .when(layout.stacked, |panel| panel.flex_col())
            .gap(px(layout.gap))
            .items_start()
            .when(!layout.stacked && layout.board >= 520., |panel| {
                panel.items_center()
            })
            .child(self.board(layout.cell, window, cx))
            .child(self.controls(layout.controls, layout.cell, cx))
    }
}

fn closest_tile(piece: Piece, offset_from_center: Point<Pixels>, pitch: f32) -> (usize, usize) {
    let (width, height) = piece.dimensions();
    let distance = |(column, row): (usize, usize)| {
        let x = (column as f32 - (width - 1) as f32 * 0.5) * pitch;
        let y = (row as f32 - (height - 1) as f32 * 0.5) * pitch;
        (f32::from(offset_from_center.x) - x).powi(2)
            + (f32::from(offset_from_center.y) - y).powi(2)
    };
    *piece
        .tiles
        .iter()
        .min_by(|&&a, &&b| distance(a).total_cmp(&distance(b)))
        .expect("all tray pieces have tiles")
}

fn wood_name(wood: Wood) -> &'static str {
    match wood {
        Wood::Amber => "Amber",
        Wood::Cherry => "Cherry",
        Wood::Juniper => "Juniper",
        Wood::Plum => "Plum",
        Wood::Lagoon => "Lagoon",
    }
}

fn wood_material(wood: Wood) -> (u32, &'static str) {
    match wood {
        Wood::Amber => (0xd6a456, "games/block-puzzle/amber.jpg"),
        Wood::Cherry => (0xd38374, "games/block-puzzle/cherry.jpg"),
        Wood::Juniper => (0xa3ae70, "games/block-puzzle/juniper.jpg"),
        Wood::Plum => (0xb096b8, "games/block-puzzle/plum.jpg"),
        Wood::Lagoon => (0x76adb2, "games/block-puzzle/lagoon.jpg"),
    }
}

fn wood_tile(wood: Wood, cell: f32) -> Div {
    let (base, texture) = wood_material(wood);
    div()
        .relative()
        .size(px(cell))
        .rounded_sm()
        .bg(rgba(0x23130cdd))
        .child(
            div()
                .absolute()
                .top(px(0.))
                .left(px(0.))
                .right(px(0.))
                .bottom(px(if cell > 19. { 2. } else { 1. }))
                .rounded_sm()
                .bg(rgb(base))
                .child(
                    img(texture)
                        .size_full()
                        .rounded_sm()
                        .object_fit(ObjectFit::Fill),
                ),
        )
}

fn placement_ghost(preview: Preview, cell: f32) -> Div {
    let color = if preview.fits {
        PREVIEW_FITS
    } else {
        PREVIEW_BLOCKED
    };
    div()
        .absolute()
        .inset_0()
        .rounded_sm()
        .when(preview.fits, |ghost| {
            ghost.child(
                div()
                    .absolute()
                    .inset_0()
                    .opacity(0.65)
                    .child(wood_tile(preview.piece.wood, cell - 2.)),
            )
        })
        .when(!preview.fits, |ghost| ghost.bg(rgba(0x8f312397)))
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded_sm()
                .border_2()
                .border_color(rgb(color)),
        )
}

fn piece_shape(piece: Piece, cell: f32, gap: f32) -> Div {
    let (width, height) = piece.dimensions();
    let pitch = cell + gap;
    div()
        .relative()
        .flex_none()
        .w(px(width as f32 * pitch - gap))
        .h(px(height as f32 * pitch - gap))
        .children(piece.tiles.iter().map(|&(column, row)| {
            div()
                .absolute()
                .left(px(column as f32 * pitch))
                .top(px(row as f32 * pitch))
                .child(wood_tile(piece.wood, cell))
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    #[test]
    fn play_layout_fits_the_board_and_tray_in_supported_windows() {
        let roomy = PlayLayout::fit(1056., 900.);
        assert!((600. ..=670.).contains(&roomy.board));
        assert!(!roomy.stacked);
        assert!(roomy.panel_width() <= 1092.);

        let short = PlayLayout::fit(1056., 540.);
        assert!(short.board + 112. <= 540.);

        let compact = PlayLayout::fit(506., 540.);
        assert!(!compact.stacked);
        assert!(compact.cell >= 28.);
        assert!(compact.controls < 230.);
        assert!(compact.panel_width() <= 542.);

        let stacked = PlayLayout::fit(420., 540.);
        assert!(stacked.stacked);
        assert_eq!(stacked.controls, stacked.board);
    }

    #[test]
    fn original_replay_restores_score_tray_and_undo() {
        let json = r#"{"seed":0,"turns":[[0,0,0]],"can_undo":true}"#;
        let saved: SavedSession = serde_json::from_str(json).unwrap();
        assert!(saved.mode == Mode::Classic);
        assert!(saved.ten_trays.is_none());

        let round = Round::restore(Mode::Classic, saved.classic).unwrap();
        assert_eq!(round.game.score(), 3);
        assert_eq!(round.game.board()[0][1], Some(Wood::Plum));
        assert_eq!(round.game.board()[1][0], Some(Wood::Plum));
        assert_eq!(round.game.tray()[0], None);
        assert_eq!(round.best, 3);
        assert_eq!(round.undo, Some(Game::new(0)));
    }

    #[test]
    fn drop_uses_grabbed_tile_at_both_board_edges() {
        let cell = 30.;
        let side = SIDE as f32 * (cell + CELL_GAP) - CELL_GAP;
        let grid = Grid {
            bounds: Bounds::new(point(px(12.), px(20.)), size(px(side), px(side))),
            cell,
        };
        let piece = Piece {
            wood: Wood::Cherry,
            tiles: &[(0, 0), (1, 0)],
        };
        let grabbed = closest_tile(piece, point(px(8.), px(0.)), 16.);
        assert_eq!(grabbed, (1, 0));

        let pointer = grid.bounds.origin + point(px(side - 0.1), px(side - 0.1));
        let target = grid.anchor_at(pointer).unwrap();
        assert_eq!(target, Anchor { column: 7, row: 7 });
        assert_eq!(
            target.offset((-(grabbed.0 as i32), 0)).turn(2),
            Some(Move {
                slot: 2,
                column: 6,
                row: 7,
            })
        );

        let target = grid.anchor_at(grid.bounds.origin).unwrap();
        let anchor = target.offset((-(grabbed.0 as i32), 0));
        assert_eq!(anchor.turn(2), None);
        assert!(
            Preview {
                anchor,
                piece,
                fits: false
            }
            .covers(0, 0)
        );

        let outside = grid.bounds.origin + point(px(side), px(side));
        assert_eq!(grid.anchor_at(outside), None);
    }

    #[test]
    fn invalid_preview_marks_anchor_when_a_shape_is_entirely_off_board() {
        let preview = Preview {
            anchor: Anchor { column: 7, row: 7 },
            piece: Piece {
                wood: Wood::Plum,
                tiles: &[(1, 0), (0, 1), (1, 1)],
            },
            fits: false,
        };
        assert!(preview.covers(7, 7));
        assert!(!preview.covers(6, 7));
    }
}
