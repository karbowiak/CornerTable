use std::sync::Arc;

use corner_table_jigsaw::{OutlinePoint, Placement, TrayFilter};
use gpui::{
    Bounds, Context, Div, KeyDownEvent, ObjectFit, PathBuilder, Pixels, Rgba, Role, Stateful,
    Window, canvas, div, img, point, prelude::*, px, rgb, rgba,
};

use crate::theme as t;

use super::artwork::PieceArt;
use super::{
    Artwork, GameView, Mode, PuzzleKey, Reference, action_button, framed_board, segment, segments,
    small_text,
};

const BOARD: u32 = 0x203b34;
const TRAY: u32 = 0xece8dc;
const SLOT: u32 = 0xe2dfd2;
const CELL_GAP: f32 = 5.;
const TRAY_PADDING: f32 = 6.;

#[derive(Clone, Copy)]
pub(super) struct TrayLayout {
    width: f32,
    columns: usize,
    rows: usize,
    slot: f32,
}

impl TrayLayout {
    pub(super) fn new(width: f32, wide: bool, viewport_height: f32) -> Self {
        let columns = if width >= 250. { 4 } else { 3 };
        let inner_width = width - 2. * TRAY_PADDING - 2.;
        let slot = (inner_width - (columns - 1) as f32 * CELL_GAP) / columns as f32;
        let rows = match (wide, viewport_height) {
            (true, height) if height >= 780. => 3,
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
    center: (f32, f32),
    slot: f32,
}

struct DragPreview {
    image: Arc<gpui::RenderImage>,
    size: (f32, f32),
    offset: (Pixels, Pixels),
    slot: f32,
}

impl gpui::Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().relative().size(px(self.slot)).child(
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

impl GameView {
    fn tray_page(&self, capacity: usize) -> TrayPage {
        let session = self.session();
        let mut pieces: Vec<_> = session.assembly.tray(session.tray.filter).collect();
        let total = pieces.len();
        let pages = total.div_ceil(capacity);
        let index = session.tray.page.min(pages.saturating_sub(1));
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

    pub(super) fn undo_placement(&mut self, capacity: usize) {
        let session = self.session_mut();
        let Some(piece) = session.assembly.undo() else {
            return;
        };
        let tray = &mut session.tray;
        if !session.assembly.tray(tray.filter).any(|item| item == piece) {
            tray.filter = TrayFilter::All;
        }
        tray.page = session
            .assembly
            .tray(tray.filter)
            .position(|item| item == piece)
            .expect("undone piece returned to tray")
            / capacity;
        tray.selected = Some(piece);
        tray.cursor = piece;
        tray.hint = None;
        tray.missed = None;
    }

    fn select_piece(&mut self, piece: usize) {
        let session = self.session_mut();
        let tray = &mut session.tray;
        tray.selected = (tray.selected != Some(piece)).then_some(piece);
        tray.hint = None;
        tray.missed = None;
        if session.assembly.is_placed(tray.cursor) {
            tray.cursor = session.assembly.tray(TrayFilter::All).min().unwrap_or(0);
        }
    }

    fn use_hint(&mut self, fallback: usize) {
        let tray = &mut self.session_mut().tray;
        let piece = tray.selected.unwrap_or(fallback);
        if tray.hint != Some(piece) {
            tray.hints_used += 1;
        }
        tray.selected = Some(piece);
        tray.hint = Some(piece);
        tray.cursor = piece;
        tray.missed = None;
    }

    fn try_place(&mut self, piece: usize, target: usize) -> Placement {
        let session = self.session_mut();
        let tray = &mut session.tray;
        let result = session.assembly.place(piece, target);
        match result {
            Placement::Missed => {
                if tray.hint != Some(piece) {
                    tray.hint = None;
                }
                tray.selected = Some(piece);
                tray.missed = Some(target);
                tray.cursor = target;
            }
            Placement::Placed | Placement::Completed => {
                tray.selected = None;
                tray.hint = None;
                tray.missed = None;
                tray.cursor = target;
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
    ) {
        let Some(piece) = self.session().tray.selected else {
            return;
        };
        let (columns, rows) = self.key.size.dimensions();
        let cursor = self.session().tray.cursor;
        let (column, row) = (cursor % columns, cursor / columns);
        let target = match event.keystroke.key.as_str() {
            "left" => row * columns + column.saturating_sub(1),
            "right" => row * columns + (column + 1).min(columns - 1),
            "up" => row.saturating_sub(1) * columns + column,
            "down" => (row + 1).min(rows - 1) * columns + column,
            "enter" | "space" => {
                if matches!(
                    self.try_place(piece, cursor),
                    Placement::Placed | Placement::Completed
                ) {
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
        let tray = &mut self.session_mut().tray;
        tray.cursor = target;
        tray.missed = None;
        cx.notify();
        cx.stop_propagation();
    }

    fn focus_tray(&self, capacity: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(&next) = self.tray_page(capacity).pieces.first() {
            self.piece_focus[next].focus(window, cx);
        } else if self.is_complete() {
            window.focus_prev(cx);
        } else {
            self.all_pieces_focus.focus(window, cx);
        }
    }

    pub(super) fn assembly_board(
        &self,
        dimensions: (f32, f32),
        art: Option<&[PieceArt]>,
        tray_layout: TrayLayout,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let (width, height) = dimensions;
        let session = self.session();
        let board = div().relative().w(px(width)).h(px(height)).bg(rgb(BOARD));

        if session.assembly.is_complete() {
            return framed_board(
                board.child(
                    img(self.painting().asset)
                        .size_full()
                        .object_fit(ObjectFit::Fill),
                ),
            );
        }

        let Some(art) = art else {
            let failed = matches!(self.artwork.get(&self.key), Some(Artwork::Failed));
            return framed_board(
                board
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
                        board.child(action_button("jigsaw-retry", "Try again", true).on_click(
                            cx.listener(|view, _, _, cx| {
                                view.prepare_artwork(cx);
                                cx.notify();
                            }),
                        ))
                    }),
            );
        };

        let (columns, rows) = self.key.size.dimensions();
        let (cell_width, cell_height) = (width / columns as f32, height / rows as f32);
        let selected = session.tray.selected.is_some();
        let capacity = tray_layout.capacity();
        let has_cursor = selected && self.board_focus.is_focused(window);
        let outlines: Vec<_> = art
            .iter()
            .enumerate()
            .map(|(piece, artwork)| {
                let placed = session.assembly.is_placed(piece);
                let mark = if Some(piece) == session.tray.missed {
                    Outline::Miss
                } else if Some(piece) == session.tray.hint {
                    Outline::Hint
                } else if has_cursor && piece == session.tray.cursor {
                    Outline::Focus
                } else if placed {
                    Outline::Placed
                } else {
                    Outline::Empty
                };
                (artwork.outline.clone(), mark)
            })
            .collect();

        framed_board(board.child(
            div()
                .id("jigsaw-assembly-board")
                .role(Role::Group)
                .aria_label("Assembly board")
                .aria_description("With a piece selected, use the arrow keys to move, Enter to try the spot, or Escape to return it to the tray.")
                .track_focus(&self.board_focus)
                .when(selected, |board| board.tab_index(0))
                .size_full()
                .relative()
                .on_key_down(cx.listener(move |view, event, window, cx| {
                    view.board_key(event, window, cx, capacity);
                }))
                .when(self.reference() == Reference::Ghost, |board| {
                    board.child(
                        img(self.painting().asset)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .object_fit(ObjectFit::Fill)
                            .opacity(0.29),
                    )
                })
                .children(
                    art.iter().enumerate()
                        .filter(|(piece, _)| session.assembly.is_placed(*piece))
                        .map(|(_, piece)| {
                            img(piece.image.clone())
                                .absolute()
                                .left(px(piece.left * width))
                                .top(px(piece.top * height))
                                .w(px(piece.width * width))
                                .h(px(piece.height * height))
                                .object_fit(ObjectFit::Fill)
                        }),
                )
                .child(canvas(
                    |_, _, _| (),
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
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if let Some(piece) = view.session().tray.selected {
                                        view.try_place(piece, target);
                                        cx.notify();
                                    }
                                }))
                        })
                        .drag_over::<DraggedPiece>(|style, _, _, _| style.bg(rgba(0xfffcf524)))
                        .on_drop(cx.listener(move |view, drag: &DraggedPiece, _, cx| {
                            if view.mode == Mode::Assemble
                                && view.key == drag.key
                                && view.session().assembly_deal == drag.deal
                            {
                                view.try_place(drag.piece, target);
                                cx.notify();
                            }
                        }))
                })),
        ))
    }

    pub(super) fn piece_tray(
        &self,
        layout: TrayLayout,
        board: (f32, f32),
        art: &[PieceArt],
        cx: &Context<Self>,
    ) -> Div {
        let session = self.session();
        let page = self.tray_page(layout.capacity());
        let first_piece = page.pieces.first().copied();
        let remaining = self.key.size.piece_count() - session.assembly.placed_count();
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
                    .child(t::display_type("Piece tray", 17.))
                    .child(small_text(format!("{remaining} left"))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .child(
                        segments().children(
                            [
                                (TrayFilter::All, "jigsaw-tray-all", "All"),
                                (TrayFilter::Edges, "jigsaw-tray-edges", "Edges"),
                                (TrayFilter::Interior, "jigsaw-tray-interior", "Interior"),
                            ]
                            .into_iter()
                            .map(|(filter, id, label)| {
                                segment(id, label, session.tray.filter == filter)
                                    .when(filter == TrayFilter::All, |button| {
                                        button.track_focus(&self.all_pieces_focus)
                                    })
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        let tray = &mut view.session_mut().tray;
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
                        action_button("jigsaw-hint", "Hint", first_piece.is_some())
                            .aria_label("Show where a tray piece fits")
                            .when_some(first_piece, |button, fallback| {
                                button.on_click(cx.listener(
                                    move |view, event: &gpui::ClickEvent, window, cx| {
                                        view.use_hint(fallback);
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
                    .border_1()
                    .border_color(rgb(t::BORDER))
                    .bg(rgb(TRAY))
                    .when(page.pieces.is_empty(), |tray| {
                        let message = match session.tray.filter {
                            TrayFilter::Edges => "All edge pieces are placed.",
                            TrayFilter::Interior => "All interior pieces are placed.",
                            TrayFilter::All => "All pieces are placed.",
                        };
                        tray.min_h(px(layout.slot))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(small_text(message))
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
                            action_button("jigsaw-tray-previous", "Previous", previous)
                                .min_h(px(30.))
                                .aria_label("Previous page of pieces")
                                .when(previous, |button| {
                                    button.on_click(cx.listener(move |view, _, _, cx| {
                                        view.goto_tray_page(page.index - 1);
                                        cx.notify();
                                    }))
                                }),
                        )
                        .child(small_text(format!(
                            "{}–{} of {}",
                            page.first + 1,
                            page.first + page.pieces.len(),
                            page.total
                        )))
                        .child(
                            action_button("jigsaw-tray-next", "Next", next)
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
        let tray = &mut self.session_mut().tray;
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
        let session = self.session();
        let selected = session.tray.selected == Some(piece);
        let description = if session.assembly.is_edge(piece) {
            "Edge piece"
        } else {
            "Interior piece"
        };
        let inner = slot - 9.;
        let (width, height) = (inner.min(inner * art.aspect), inner.min(inner / art.aspect));
        let (columns, rows) = self.key.size.dimensions();
        let center_x = (piece % columns) as f32 + 0.5;
        let center_y = (piece / columns) as f32 + 0.5;
        div()
            .id(("jigsaw-tray-piece", piece))
            .role(Role::Button)
            .aria_label(format!("{description} {}", ordinal + 1))
            .aria_selected(selected)
            .track_focus(&self.piece_focus[piece])
            .tab_index(0)
            .size(px(slot))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(rgb(if selected { t::RAIL_ACTIVE } else { SLOT }))
            .bg(rgb(if selected { t::GOLD_HOVER } else { SLOT }))
            .cursor_pointer()
            .when(!selected, |tile| {
                tile.hover(|style| style.bg(rgb(t::PAPER)))
            })
            .focus_visible(|style| style.border_color(rgb(t::INK)).bg(rgb(t::GOLD_HOVER)))
            .child(
                img(art.image.clone())
                    .w(px(width))
                    .h(px(height))
                    .object_fit(ObjectFit::Fill),
            )
            .on_drag(
                DraggedPiece {
                    key: self.key,
                    deal: session.assembly_deal,
                    piece,
                    image: art.image.clone(),
                    size: (art.width * board.0, art.height * board.1),
                    center: (
                        (center_x / columns as f32 - art.left) * board.0,
                        (center_y / rows as f32 - art.top) * board.1,
                    ),
                    slot,
                },
                |drag, offset, _, cx| {
                    cx.new(|_| DragPreview {
                        image: drag.image.clone(),
                        size: drag.size,
                        offset: (offset.x - px(drag.center.0), offset.y - px(drag.center.1)),
                        slot: drag.slot,
                    })
                },
            )
            .on_click(
                cx.listener(move |view, event: &gpui::ClickEvent, window, cx| {
                    view.select_piece(piece);
                    if event.is_keyboard() && view.session().tray.selected.is_some() {
                        view.board_focus.focus(window, cx);
                    }
                    cx.notify();
                }),
            )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outline {
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
        (Outline::Empty, 1.15, rgba(0x8ca99cbf), None),
        (Outline::Placed, 1.1, rgba(0x112f2aa0), None),
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
