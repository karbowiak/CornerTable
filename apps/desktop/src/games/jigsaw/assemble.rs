use std::{cell::Cell, rc::Rc, sync::Arc, time::Duration};

use corner_table_jigsaw::{OutlinePoint, Placement, TrayFilter};
use gpui::{
    Animation, AnimationExt, App, Bounds, Context, Div, Entity, Img, KeyDownEvent, MouseButton,
    ObjectFit, PathBuilder, Pixels, Point, Rgba, Role, Stateful, Window, canvas, div,
    ease_out_quint, img, point, prelude::*, px, relative, rgb, rgba,
};

use crate::{
    audio::{self, Effect},
    motion, theme as t,
};

use super::artwork::PieceArt;
use super::camera::TableBounds;
use super::{
    Artwork, FRAME_PADDING, GameView, LoosePiece, Mode, PuzzleKey, Reference, Round, action_button,
    framed_board, segment, segments, small_text,
};

const BOARD: u32 = 0x2e251c;
const CELL_GAP: f32 = 5.;
const TRAY_PADDING: f32 = 6.;
const SURFACE_GAP: f32 = 14.;

#[derive(Clone, Copy)]
pub(super) struct TableLayout {
    pub(super) tray: TrayLayout,
    picture: (f32, f32),
    height: f32,
    wide: bool,
}

impl TableLayout {
    pub(super) fn new(width: f32, height: f32, aspect: f32) -> Self {
        let width = width - 2. * (FRAME_PADDING + 1.);
        let wide = width >= 530.;
        let minimum_tray_width = if wide {
            (width * 0.31).clamp(246., 300.)
        } else {
            width.min(310.)
        };
        let picture_width = if wide {
            width - minimum_tray_width - SURFACE_GAP
        } else {
            width
        };
        let max_picture_height = (height - 225.).clamp(225., 760.);
        let picture_width = picture_width.min(max_picture_height * aspect);
        let tray_width = if wide {
            (width - picture_width - SURFACE_GAP).min(310.)
        } else {
            minimum_tray_width
        };
        Self {
            tray: TrayLayout::new(tray_width, wide, height),
            picture: (picture_width, picture_width / aspect),
            height,
            wide,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct TrayLayout {
    width: f32,
    columns: usize,
    rows: usize,
    slot: f32,
}

impl TrayLayout {
    fn new(width: f32, wide: bool, viewport_height: f32) -> Self {
        let columns = if width >= 240. { 4 } else { 3 };
        let inner_width = width - 2. * TRAY_PADDING - 2.;
        let slot = (inner_width - (columns - 1) as f32 * CELL_GAP) / columns as f32;
        let rows = match (wide, viewport_height) {
            (true, height) if height >= 680. => 3,
            (false, height) if height < 660. => 1,
            _ => 2,
        };
        Self {
            width,
            columns,
            rows,
            slot,
        }
    }

    pub(super) fn capacity(self) -> usize {
        self.columns * self.rows
    }
}

struct TrayPage {
    pieces: Vec<usize>,
    first: usize,
    index: usize,
    pages: usize,
    total: usize,
}

struct DraggedPiece {
    key: PuzzleKey,
    deal: u64,
    piece: usize,
    image: Arc<gpui::RenderImage>,
    size: (f32, f32),
    center: Point<Pixels>,
    anchor: Rc<Cell<Point<Pixels>>>,
}

struct DragPreview {
    image: Arc<gpui::RenderImage>,
    size: (f32, f32),
    offset: (Pixels, Pixels),
}

impl gpui::Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(self.size.0))
            .h(px(self.size.1))
            .child(
                img(self.image.clone())
                    .absolute()
                    .left(self.offset.0)
                    .top(self.offset.1)
                    .w(px(self.size.0))
                    .h(px(self.size.1))
                    .object_fit(ObjectFit::Fill)
                    .opacity(0.94),
            )
    }
}

impl DraggedPiece {
    fn preview(
        &self,
        pointer: Point<Pixels>,
        anchor: Point<Pixels>,
        cx: &mut App,
    ) -> Entity<DragPreview> {
        self.anchor.set(anchor);
        cx.new(|_| DragPreview {
            image: self.image.clone(),
            size: self.size,
            offset: (pointer.x - anchor.x, pointer.y - anchor.y),
        })
    }
}

impl Round {
    fn pile(&self, filter: TrayFilter) -> impl Iterator<Item = usize> + '_ {
        self.assembly
            .tray(filter)
            .filter(|piece| !self.loose.iter().any(|loose| loose.piece == *piece))
    }

    fn loose_on_table(&self) -> impl Iterator<Item = &LoosePiece> {
        self.loose
            .iter()
            .filter(|loose| !self.assembly.is_placed(loose.piece))
    }
}

impl GameView {
    fn tray_page(&self, capacity: usize) -> TrayPage {
        let round = self.round();
        let mut pieces: Vec<_> = round.pile(round.tray.filter).collect();
        let total = pieces.len();
        let pages = total.div_ceil(capacity);
        let index = round.tray.page.min(pages.saturating_sub(1));
        let first = index * capacity;
        let pieces = pieces.drain(first..(first + capacity).min(total)).collect();
        TrayPage {
            pieces,
            first,
            index,
            pages,
            total,
        }
    }

    pub(super) fn undo_placement(&mut self, capacity: usize, cx: &Context<Self>) {
        let round = self.round_mut();
        let Some(piece) = round.assembly.undo() else {
            return;
        };
        let page = round.pile(round.tray.filter).position(|item| item == piece);
        if let Some(index) = page {
            round.tray.page = index / capacity;
        } else {
            let index = round.pile(TrayFilter::All).position(|item| item == piece);
            if let Some(index) = index {
                round.tray.filter = TrayFilter::All;
                round.tray.page = index / capacity;
            }
        }
        let tray = &mut round.tray;
        tray.selected = Some(piece);
        tray.cursor = piece;
        tray.hint = None;
        tray.missed = None;
        self.settling_piece = None;
        self.save();
        audio::play(cx, Effect::Move);
    }

    fn select_piece(&mut self, piece: usize) {
        let round = self.round_mut();
        let tray = &mut round.tray;
        tray.selected = (tray.selected != Some(piece)).then_some(piece);
        tray.hint = None;
        tray.missed = None;
        if round.assembly.is_placed(tray.cursor) {
            tray.cursor = round.assembly.tray(TrayFilter::All).min().unwrap_or(0);
        }
    }

    fn use_hint(&mut self, fallback: usize) {
        let tray = &mut self.round_mut().tray;
        let piece = tray.selected.unwrap_or(fallback);
        let new_hint = tray.hint != Some(piece);
        if new_hint {
            tray.hints_used = tray.hints_used.saturating_add(1);
        }
        tray.selected = Some(piece);
        tray.hint = Some(piece);
        tray.cursor = piece;
        tray.missed = None;
        if new_hint {
            self.save();
        }
    }

    fn try_place(
        &mut self,
        piece: usize,
        target: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Placement {
        let round = self.round_mut();
        let tray = &mut round.tray;
        let result = round.assembly.place(piece, target);
        match result {
            Placement::Missed => {
                if tray.hint != Some(piece) {
                    tray.hint = None;
                }
                tray.selected = Some(piece);
                tray.missed = Some(target);
                tray.cursor = target;
                audio::play(cx, Effect::InvalidMove);
            }
            Placement::Placed | Placement::Completed => {
                round.assembly_moves = round.assembly_moves.saturating_add(1);
                tray.selected = None;
                tray.hint = None;
                tray.missed = None;
                tray.cursor = target;
                self.settling_piece =
                    (result == Placement::Placed && motion::enabled(cx)).then_some(piece);
                self.save();
                if result == Placement::Completed {
                    self.record_win();
                    self.result_focus.focus(window, cx);
                    audio::play(cx, Effect::Complete);
                } else {
                    audio::play(cx, Effect::Move);
                }
            }
            Placement::Ignored => {}
        }
        result
    }

    fn board_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
        capacity: usize,
        bounds: &TableBounds,
    ) {
        let Some(piece) = self.round().tray.selected else {
            return;
        };
        let (columns, rows) = self.key.size.dimensions();
        let cursor = self.round().tray.cursor;
        let (column, row) = (cursor % columns, cursor / columns);
        let target = match event.keystroke.key.as_str() {
            "left" => row * columns + column.saturating_sub(1),
            "right" => row * columns + (column + 1).min(columns - 1),
            "up" => row.saturating_sub(1) * columns + column,
            "down" => (row + 1).min(rows - 1) * columns + column,
            "enter" | "space" => {
                if !event.is_held && self.try_place(piece, cursor, window, cx) == Placement::Placed
                {
                    self.focus_tray(capacity, window, cx);
                }
                cx.notify();
                cx.stop_propagation();
                return;
            }
            "escape" => {
                self.select_piece(piece);
                self.focus_tray(capacity, window, cx);
                cx.notify();
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        let tray = &mut self.round_mut().tray;
        tray.cursor = target;
        tray.missed = None;
        self.reveal_target(target, bounds, cx);
        cx.notify();
        cx.stop_propagation();
    }

    fn focus_tray(&self, capacity: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next) = self
            .tray_page(capacity)
            .pieces
            .first()
            .copied()
            .or_else(|| {
                self.round()
                    .loose_on_table()
                    .next()
                    .map(|loose| loose.piece)
            })
        {
            self.piece_focus[next].focus(window, cx);
        } else {
            self.all_pieces_focus.focus(window, cx);
        }
    }

    pub(super) fn assembly_table(
        &self,
        layout: TableLayout,
        art: Option<&[PieceArt]>,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let bounds = Rc::new(TableBounds::default());
        let measure_viewport = bounds.clone();
        let measure_world = bounds.clone();
        let measure_pile = bounds.clone();
        let drop_bounds = bounds.clone();
        let pile = self.reference_panel(layout.tray.width, layout.height, cx);
        let round = self.round();
        let camera = round.camera;
        let [left, top] = camera.offset();
        let zoom = camera.zoom();
        let mut world_layout = layout;
        world_layout.picture = (layout.picture.0 * zoom, layout.picture.1 * zoom);

        let clip = || div().absolute().inset_0().overflow_hidden();
        let world = || {
            div()
                .absolute()
                .left(relative(left))
                .top(relative(top))
                .w(relative(zoom))
                .h(relative(zoom))
        };
        let picture = clip()
            .child(
                canvas(
                    move |bounds, _, _| measure_viewport.viewport.set(bounds),
                    |_, (), _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(
                world()
                    .child(
                        canvas(
                            move |bounds, _, _| measure_world.world.set(bounds),
                            |_, (), _, _| {},
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .child(self.assembly_board(world_layout, art, bounds.clone(), window, cx)),
            );
        let loose = art
            .filter(|_| round.loose_on_table().next().is_some())
            .map(|pieces| {
                clip().child(world().children(round.loose_on_table().map(|loose| {
                    self.loose_piece(loose, &pieces[loose.piece], world_layout.picture, cx)
                })))
            });
        let pile = div()
            .relative()
            .w(px(layout.tray.width))
            .flex_none()
            .flex()
            .flex_col()
            .gap_3()
            .py_2()
            .ml_auto()
            .when(!camera.is_fit(), |pile| pile.bg(rgb(BOARD)))
            .child(
                canvas(
                    move |bounds, _, _| measure_pile.pile.set(bounds),
                    |_, (), _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(pile)
            .children(art.map(|pieces| {
                self.piece_tray(
                    layout.tray,
                    world_layout.picture,
                    pieces,
                    bounds.clone(),
                    cx,
                )
            }));
        let layers = if camera.is_fit() {
            [Some(pile), loose]
        } else {
            [loose, Some(pile)]
        };
        let table = div()
            .id("jigsaw-worktable")
            .debug_selector(|| "jigsaw-worktable".into())
            .relative()
            .w_full()
            .flex();
        let table = self.camera_interactions(table, &bounds, cx);
        framed_board(
            div().w_full().child(
                table
                    .gap(px(SURFACE_GAP))
                    .items_start()
                    .when(!layout.wide, |table| table.flex_col())
                    .bg(rgb(BOARD))
                    .on_drop(cx.listener(move |view, drag: &DraggedPiece, window, cx| {
                        if !camera.is_fit()
                            && drop_bounds.pile.get().contains(&window.mouse_position())
                        {
                            return;
                        }
                        view.drop_piece(
                            drag,
                            drop_bounds.world.get(),
                            drop_bounds.picture.get(),
                            window,
                            cx,
                        );
                    }))
                    .child(
                        div()
                            .w(px(layout.picture.0))
                            .h(px(layout.picture.1))
                            .flex_none(),
                    )
                    .child(picture)
                    .children(layers.into_iter().flatten()),
            ),
        )
    }

    fn drop_piece(
        &mut self,
        drag: &DraggedPiece,
        surface: Bounds<Pixels>,
        picture: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode != Mode::Assemble
            || self.key != drag.key
            || self.round().assembly_deal != drag.deal
            || self.round().assembly.is_placed(drag.piece)
        {
            return;
        }
        let top_left = window.mouse_position() - drag.anchor.get();
        let center = top_left + drag.center;
        let (columns, rows) = self.key.size.dimensions();
        if picture.contains(&center) {
            let offset = center - picture.origin;
            let column = (offset.x / picture.size.width * columns as f32) as usize;
            let row = (offset.y / picture.size.height * rows as f32) as usize;
            if column < columns && row < rows && row * columns + column == drag.piece {
                self.try_place(drag.piece, drag.piece, window, cx);
                cx.notify();
                return;
            }
        }

        let offset = top_left - surface.origin;
        let position = [
            (f32::from(offset.x) / (f32::from(surface.size.width) - drag.size.0).max(1.))
                .clamp(0., 1.),
            (f32::from(offset.y) / (f32::from(surface.size.height) - drag.size.1).max(1.))
                .clamp(0., 1.),
        ];
        let round = self.round_mut();
        round.loose.retain(|piece| piece.piece != drag.piece);
        round.loose.push(LoosePiece {
            piece: drag.piece,
            position,
        });
        round.tray.selected = None;
        round.tray.missed = None;
        round.tray.hint = None;
        self.save();
        audio::play(cx, Effect::Move);
        cx.notify();
    }

    fn assembly_board(
        &self,
        layout: TableLayout,
        art: Option<&[PieceArt]>,
        bounds: Rc<TableBounds>,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let dimensions = layout.picture;
        let (width, height) = dimensions;
        let round = self.round();
        let board = div().relative().w(px(width)).h(px(height)).flex_none();

        let Some(art) = art else {
            let failed = matches!(self.artwork.get(&self.key), Some(Artwork::Failed));
            return board
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(rgb(t::PAPER))
                .child(if failed {
                    "Couldn’t prepare the pieces."
                } else {
                    "Preparing the pieces…"
                })
                .when(failed, |board| {
                    board.child(
                        action_button("jigsaw-retry", "Try again", true, cx).on_click(cx.listener(
                            |view, _, _, cx| {
                                view.prepare_artwork(cx);
                                cx.notify();
                            },
                        )),
                    )
                });
        };

        let (columns, rows) = self.key.size.dimensions();
        let (cell_width, cell_height) = (width / columns as f32, height / rows as f32);
        let selected = round.tray.selected.is_some();
        let capacity = layout.tray.capacity();
        let has_cursor = selected && self.board_focus.is_focused(window);
        let settling = self.settling_piece.filter(|_| motion::enabled(cx));
        let show_grid = self.show_grid;
        let key_bounds = bounds.clone();
        let outlines: Vec<_> = art
            .iter()
            .enumerate()
            .map(|(piece, artwork)| {
                let placed = round.assembly.is_placed(piece);
                let mark = if Some(piece) == round.tray.missed {
                    Outline::Miss
                } else if Some(piece) == round.tray.hint {
                    Outline::Hint
                } else if has_cursor && piece == round.tray.cursor {
                    Outline::Focus
                } else if !show_grid {
                    Outline::Hidden
                } else if placed {
                    Outline::Placed
                } else {
                    Outline::Empty
                };
                (artwork.outline.clone(), mark)
            })
            .collect();

        board.child(
            div()
                .id("jigsaw-assembly-board")
                .debug_selector(|| "jigsaw-assembly-board".into())
                .role(Role::Group)
                .aria_label("Assembly board")
                .aria_description("With a piece selected, use the arrow keys to move, Enter to try the spot, or Escape to return it to the tray.")
                .track_focus(&self.board_focus)
                .when(selected, |board| board.tab_index(0))
                .size_full()
                .relative()
                .on_key_down(cx.listener(move |view, event, window, cx| {
                    view.board_key(event, window, cx, capacity, &key_bounds);
                }))
                .when(self.reference() != Reference::Off, |board| {
                    board.child(
                        self.painting().source.image()
                            .absolute()
                            .inset_0()
                            .size_full()
                            .object_fit(ObjectFit::Fill)
                            .opacity(0.29),
                    )
                })
                .children(
                    art.iter().enumerate()
                        .filter(|(piece, _)| round.assembly.is_placed(*piece) && Some(*piece) != settling)
                        .map(|(_, piece)| placed_image(piece, dimensions)),
                )
                .children(settling.map(|piece| {
                    let art = &art[piece];
                    let (x, y) = (art.left * width, art.top * height);
                    let (w, h) = (art.width * width, art.height * height);
                    placed_image(art, dimensions).with_animation(
                        ("jigsaw-piece-settle", round.assembly_moves as usize),
                        Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint()),
                        move |image, progress| {
                            let lift = 1. - progress;
                            image.left(px(x - w * 0.025 * lift))
                                .top(px(y - (6. + h * 0.025) * lift))
                                .w(px(w * (1. + 0.05 * lift)))
                                .h(px(h * (1. + 0.05 * lift)))
                        },
                    )
                }))
                .child(canvas(
                    move |picture, _, _| bounds.picture.set(picture),
                    move |bounds, _, window, _| paint_outlines(bounds, (cell_width, cell_height), &outlines, window),
                ).absolute().inset_0().size_full())
                .children((0..self.key.size.piece_count()).map(|target| {
                    let column = target % columns;
                    let row = target / columns;
                    div()
                        .id(("jigsaw-board-cell", target))
                        .absolute()
                        .left(px(column as f32 * cell_width))
                        .top(px(row as f32 * cell_height))
                        .w(px(cell_width))
                        .h(px(cell_height))
                        .when(selected, |cell| {
                                cell.role(Role::Button)
                                    .aria_label(format!("Row {}, column {}", row + 1, column + 1))
                                    .cursor_pointer()
                                .hover(|style| style.bg(rgba(0xfffcf514)))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    if let Some(piece) = view.round().tray.selected {
                                        view.try_place(piece, target, window, cx);
                                        cx.notify();
                                    }
                                }))
                        })
                })),
        )
    }

    fn piece_tray(
        &self,
        layout: TrayLayout,
        board: (f32, f32),
        art: &[PieceArt],
        bounds: Rc<TableBounds>,
        cx: &Context<Self>,
    ) -> Div {
        let round = self.round();
        let page = self.tray_page(layout.capacity());
        let first_piece = page
            .pieces
            .first()
            .copied()
            .or_else(|| round.loose_on_table().next().map(|loose| loose.piece));
        let remaining = self.key.size.piece_count() - round.assembly.placed_count();
        div()
            .w(px(layout.width))
            .flex_none()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(t::display_type("Loose pieces", 17.).text_color(rgb(t::PAPER)))
                    .child(small_text(format!("{remaining} left"), cx).text_color(rgb(t::PAPER))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .child(
                        segments(cx).children(
                            [
                                (TrayFilter::All, "jigsaw-tray-all", "All"),
                                (TrayFilter::Edges, "jigsaw-tray-edges", "Edges"),
                                (TrayFilter::Interior, "jigsaw-tray-interior", "Interior"),
                            ]
                            .into_iter()
                            .map(|(filter, id, label)| {
                                segment(id, label, round.tray.filter == filter, cx)
                                    .when(filter == TrayFilter::All, |button| {
                                        button.track_focus(&self.all_pieces_focus)
                                    })
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        let tray = &mut view.round_mut().tray;
                                        tray.filter = filter;
                                        tray.page = 0;
                                        tray.selected = None;
                                        tray.hint = None;
                                        tray.missed = None;
                                        cx.notify();
                                    }))
                            }),
                        ),
                    )
                    .child(
                        action_button("jigsaw-hint", "Hint", first_piece.is_some(), cx)
                            .aria_label("Show where a loose piece fits")
                            .when_some(first_piece, |button, fallback| {
                                button.on_click(cx.listener(
                                    move |view, event: &gpui::ClickEvent, window, cx| {
                                        view.use_hint(fallback);
                                        view.reveal_target(view.round().tray.cursor, &bounds, cx);
                                        if event.is_keyboard() {
                                            view.board_focus.focus(window, cx);
                                        }
                                        cx.notify();
                                    },
                                ))
                            }),
                    ),
            )
            .child(
                div()
                    .p(px(TRAY_PADDING))
                    .rounded_md()
                    .when(page.pieces.is_empty(), |tray| {
                        let message = match round.tray.filter {
                            TrayFilter::Edges => "No edge pieces in the pile.",
                            TrayFilter::Interior => "No interior pieces in the pile.",
                            TrayFilter::All => "The pile is empty.",
                        };
                        tray.min_h(px(layout.slot))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(small_text(message, cx).text_color(rgb(t::PAPER)))
                    })
                    .when(!page.pieces.is_empty(), |tray| {
                        tray.flex().flex_col().gap(px(CELL_GAP)).children(
                            page.pieces
                                .chunks(layout.columns)
                                .enumerate()
                                .map(|(row, pieces)| {
                                    div().flex().gap(px(CELL_GAP)).children(
                                        pieces.iter().enumerate().map(|(column, &piece)| {
                                            let ordinal =
                                                page.first + row * layout.columns + column;
                                            self.tray_piece(
                                                piece,
                                                ordinal,
                                                &art[piece],
                                                layout.slot,
                                                board,
                                                cx,
                                            )
                                        }),
                                    )
                                }),
                        )
                    }),
            )
            .when(page.pages > 1, |tray| {
                let previous = page.index > 0;
                let next = page.index + 1 < page.pages;
                tray.child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .gap_1()
                        .child(
                            action_button("jigsaw-tray-previous", "Previous", previous, cx)
                                .min_h(px(30.))
                                .aria_label("Previous page of pieces")
                                .when(previous, |button| {
                                    button.on_click(cx.listener(move |view, _, _, cx| {
                                        view.goto_tray_page(page.index - 1);
                                        cx.notify();
                                    }))
                                }),
                        )
                        .child(
                            small_text(
                                format!(
                                    "{}–{} of {}",
                                    page.first + 1,
                                    page.first + page.pieces.len(),
                                    page.total
                                ),
                                cx,
                            )
                            .text_color(rgb(t::PAPER)),
                        )
                        .child(
                            action_button("jigsaw-tray-next", "Next", next, cx)
                                .min_h(px(30.))
                                .aria_label("Next page of pieces")
                                .when(next, |button| {
                                    button.on_click(cx.listener(move |view, _, _, cx| {
                                        view.goto_tray_page(page.index + 1);
                                        cx.notify();
                                    }))
                                }),
                        ),
                )
            })
    }

    fn goto_tray_page(&mut self, page: usize) {
        let tray = &mut self.round_mut().tray;
        tray.page = page;
        tray.selected = None;
        tray.hint = None;
        tray.missed = None;
    }

    fn tray_piece(
        &self,
        piece: usize,
        ordinal: usize,
        art: &PieceArt,
        slot: f32,
        board: (f32, f32),
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let inner = slot - 9.;
        let (width, height) = (inner.min(inner * art.aspect), inner.min(inner / art.aspect));
        let (stagger_x, stagger_y) = [(0., 2.), (3., 0.), (0., -3.), (-2., -1.)][piece % 4];
        self.piece_control("jigsaw-tray-piece", piece, ordinal, cx)
            .size(px(slot))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(
                img(art.image.clone())
                    .relative()
                    .left(px(stagger_x))
                    .top(px(stagger_y))
                    .w(px(width))
                    .h(px(height))
                    .object_fit(ObjectFit::Fill),
            )
            .on_drag(
                self.dragged_piece(piece, art, board),
                |drag, offset, _, cx| drag.preview(offset, drag.center, cx),
            )
    }

    fn loose_piece(
        &self,
        loose: &LoosePiece,
        art: &PieceArt,
        board: (f32, f32),
        cx: &Context<Self>,
    ) -> Div {
        let (width, height) = (art.width * board.0, art.height * board.1);
        let drag = self.dragged_piece(loose.piece, art, board);
        let anchor = drag.anchor.clone();
        let hitbox = Rc::new(Cell::new(Bounds::default()));
        let measure = hitbox.clone();
        div()
            .absolute()
            .inset_0()
            .right(px(width))
            .bottom(px(height))
            .child(
                self.piece_control("jigsaw-loose-piece", loose.piece, loose.piece, cx)
                    .absolute()
                    .left(relative(loose.position[0]))
                    .top(relative(loose.position[1]))
                    .w(px(width))
                    .h(px(height))
                    .child(
                        canvas(move |bounds, _, _| measure.set(bounds), |_, (), _, _| {})
                            .absolute()
                            .inset_0(),
                    )
                    .child(
                        img(art.image.clone())
                            .size_full()
                            .object_fit(ObjectFit::Fill),
                    )
                    .on_mouse_down(MouseButton::Left, move |event, _, _| {
                        anchor.set(event.position - hitbox.get().origin);
                    })
                    .on_drag(drag, |drag, offset, _, cx| {
                        drag.preview(offset, drag.anchor.get(), cx)
                    }),
            )
    }

    fn dragged_piece(&self, piece: usize, art: &PieceArt, board: (f32, f32)) -> DraggedPiece {
        let (columns, rows) = self.key.size.dimensions();
        let center_x = (piece % columns) as f32 + 0.5;
        let center_y = (piece / columns) as f32 + 0.5;
        let center = point(
            px((center_x / columns as f32 - art.left) * board.0),
            px((center_y / rows as f32 - art.top) * board.1),
        );
        DraggedPiece {
            key: self.key,
            deal: self.round().assembly_deal,
            piece,
            image: art.image.clone(),
            size: (art.width * board.0, art.height * board.1),
            center,
            anchor: Rc::new(Cell::new(center)),
        }
    }

    fn piece_control(
        &self,
        id: &'static str,
        piece: usize,
        ordinal: usize,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let round = self.round();
        let selected = round.tray.selected == Some(piece);
        let description = if round.assembly.is_edge(piece) {
            "Edge piece"
        } else {
            "Interior piece"
        };
        div()
            .id((id, piece))
            .debug_selector(move || format!("{id}-{piece}"))
            .role(Role::Button)
            .aria_label(format!("{description} {}", ordinal + 1))
            .aria_selected(selected)
            .track_focus(&self.piece_focus[piece])
            .tab_index(0)
            .rounded_sm()
            .border_1()
            .border_color(if selected {
                rgb(t::GOLD)
            } else {
                rgba(0x00000000)
            })
            .cursor_grab()
            .when(!selected, |tile| {
                tile.hover(|style| style.bg(rgba(0xfff7df18)))
            })
            .focus_visible(|style| style.border_color(rgb(t::PAPER)).bg(rgba(0xfff7df18)))
            .on_click(
                cx.listener(move |view, event: &gpui::ClickEvent, window, cx| {
                    view.select_piece(piece);
                    if event.is_keyboard() && view.round().tray.selected.is_some() {
                        view.board_focus.focus(window, cx);
                    }
                    cx.notify();
                }),
            )
    }
}

fn placed_image(art: &PieceArt, (width, height): (f32, f32)) -> Img {
    img(art.image.clone())
        .absolute()
        .left(px(art.left * width))
        .top(px(art.top * height))
        .w(px(art.width * width))
        .h(px(art.height * height))
        .object_fit(ObjectFit::Fill)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outline {
    Hidden,
    Empty,
    Placed,
    Focus,
    Hint,
    Miss,
}

fn paint_outlines(
    bounds: Bounds<Pixels>,
    cell: (f32, f32),
    outlines: &[(Arc<[OutlinePoint]>, Outline)],
    window: &mut Window,
) {
    for (mark, width, stroke, fill) in [
        (Outline::Empty, 1.15, rgba(0xb09c80bf), None),
        (Outline::Placed, 1.1, rgba(0x1f1710a0), None),
        (Outline::Focus, 2., rgb(0xfff7df), Some(rgba(0xfff7df20))),
        (Outline::Hint, 2.5, rgb(t::GOLD), Some(rgba(0xefb94d40))),
        (Outline::Miss, 2., rgb(0xf9a885), Some(rgba(0xf9a88528))),
    ] {
        let mut border = PathBuilder::stroke(px(width));
        let mut interior = fill.map(|_| PathBuilder::fill());
        for (outline, _) in outlines.iter().filter(|(_, outline)| *outline == mark) {
            let add_path = |path: &mut PathBuilder| {
                for (index, p) in outline.iter().enumerate() {
                    let position = bounds.origin + point(px(p.x * cell.0), px(p.y * cell.1));
                    if index == 0 {
                        path.move_to(position);
                    } else {
                        path.line_to(position);
                    }
                }
                path.close();
            };
            add_path(&mut border);
            if let Some(interior) = &mut interior {
                add_path(interior);
            }
        }
        if let Some((interior, fill)) = interior.zip(fill) {
            paint_path(interior, fill, window);
        }
        paint_path(border, stroke, window);
    }
}

fn paint_path(path: PathBuilder, color: Rgba, window: &mut Window) {
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
