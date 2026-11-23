use corner_table_jigsaw::{Selection, Size};
use gpui::{Context, Div, ObjectFit, Render, Role, Stateful, Window, div, prelude::*, px, rgb};

use crate::{
    audio::{self, Effect},
    theme as t,
};

use super::{GameView, Mode, PictureSource, PuzzleKey, framed_board};

const GAP: f32 = 2.;

#[derive(Clone)]
struct TileImage {
    source: PictureSource,
    size: Size,
    piece: usize,
    width: f32,
    height: f32,
}

impl TileImage {
    fn render(&self) -> Div {
        let (columns, rows) = self.size.dimensions();
        let image_width = self.width * columns as f32;
        let image_height = self.height * rows as f32;
        let x = (self.piece % columns) as f32 * self.width;
        let y = (self.piece / columns) as f32 * self.height;
        div()
            .w(px(self.width))
            .h(px(self.height))
            .flex_none()
            .relative()
            .overflow_hidden()
            .child(
                self.source
                    .image()
                    .absolute()
                    .left(px(-x))
                    .top(px(-y))
                    .w(px(image_width))
                    .h(px(image_height))
                    .object_fit(ObjectFit::Cover),
            )
    }
}

#[derive(Clone)]
struct DraggedTile {
    key: PuzzleKey,
    deal: u64,
    position: usize,
    tile: TileImage,
}

impl Render for DraggedTile {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.tile.render().border_2().border_color(rgb(t::GOLD))
    }
}

impl GameView {
    fn finish_selection(&mut self, selection: Selection, cx: &mut Context<Self>) {
        match selection {
            Selection::Swapped | Selection::Completed => {
                self.save();
                if selection == Selection::Completed {
                    self.record_win();
                    audio::play(cx, Effect::Complete);
                } else {
                    audio::play(cx, Effect::Move);
                }
            }
            Selection::Selected | Selection::Cleared => {}
            Selection::Ignored => return,
        }
        cx.notify();
    }

    fn tile(&self, position: usize, width: f32, height: f32, cx: &Context<Self>) -> Stateful<Div> {
        let puzzle = &self.round().swap;
        let piece = puzzle.pieces()[position];
        let selected = puzzle.selected() == Some(position);
        let placed = piece == position;
        let tile = TileImage {
            source: self.painting().source.clone(),
            size: self.key.size,
            piece,
            width,
            height,
        };
        let face = tile
            .render()
            .id(("jigsaw-tile", position))
            .rounded_sm()
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .when(selected || placed, |overlay| {
                        overlay.border_2().border_color(rgb(if selected {
                            t::GOLD
                        } else {
                            0x81a794
                        }))
                    }),
            );

        if placed {
            return face;
        }
        face.tab_index(0)
            .role(Role::Button)
            .aria_label(format!("Square tile {}", position + 1))
            .aria_selected(selected)
            .cursor_pointer()
            .hover(|style| style.opacity(0.85))
            .focus(|style| style.opacity(0.70))
            .drag_over::<DraggedTile>(|style, _, _, _| style.opacity(0.65))
            .on_drag(
                DraggedTile {
                    key: self.key,
                    deal: self.round().swap_deal,
                    position,
                    tile,
                },
                |drag, _, _, cx| cx.new(|_| drag.clone()),
            )
            .on_drop(cx.listener(move |view, dragged: &DraggedTile, _, cx| {
                if view.mode == Mode::Swap
                    && view.key == dragged.key
                    && view.round().swap_deal == dragged.deal
                    && view.round().swap.pieces().get(dragged.position) == Some(&dragged.tile.piece)
                {
                    let selection = view.round_mut().swap.swap(dragged.position, position);
                    view.finish_selection(selection, cx);
                }
            }))
            .on_click(cx.listener(move |view, _, _, cx| {
                let selection = view.round_mut().swap.select(position);
                view.finish_selection(selection, cx);
            }))
    }

    pub(super) fn swap_board(&self, width: f32, cx: &Context<Self>) -> Div {
        let (columns, rows) = self.key.size.dimensions();
        let piece_width = (width - GAP * (columns - 1) as f32) / columns as f32;
        let image_height = piece_width * columns as f32 / self.painting().aspect;
        let piece_height = image_height / rows as f32;
        let height = image_height + GAP * (rows - 1) as f32;
        let board = if self.round().swap.is_complete() {
            div()
                .w(px(width))
                .h(px(width / self.painting().aspect))
                .child(
                    self.painting()
                        .source
                        .image()
                        .size_full()
                        .object_fit(ObjectFit::Fill),
                )
        } else {
            div()
                .w(px(width))
                .h(px(height))
                .flex()
                .flex_col()
                .gap(px(GAP))
                .children((0..rows).map(|row| {
                    div()
                        .flex()
                        .gap(px(GAP))
                        .children((0..columns).map(|column| {
                            self.tile(row * columns + column, piece_width, piece_height, cx)
                        }))
                }))
        };
        framed_board(board)
    }
}
