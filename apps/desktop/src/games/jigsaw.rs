use std::collections::HashMap;

use corner_table_jigsaw::{Puzzle, Selection, Size};
use gpui::{
    Context, Div, FontWeight, ObjectFit, Render, Stateful, Window, div, img, prelude::*, px, rgb,
};

use crate::theme as t;

use super::{GameLaunch, board_space, control};

const NIGHT: u32 = 0x253d43;
const SEA: u32 = 0x385b60;
const GAP: f32 = 3.;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Picture {
    Lighthouse,
    Harbor,
}

impl Picture {
    fn title(self) -> &'static str {
        match self {
            Self::Lighthouse => "Lighthouse at Dusk",
            Self::Harbor => "Harbor Morning",
        }
    }

    fn asset(self) -> &'static str {
        match self {
            Self::Lighthouse => "games/jigsaw/lighthouse.jpg",
            Self::Harbor => "games/jigsaw/harbor.jpg",
        }
    }

    fn aspect(self) -> f32 {
        match self {
            Self::Lighthouse => 4. / 3.,
            Self::Harbor => 3. / 2.,
        }
    }
}

#[derive(Clone, Copy)]
struct TileImage {
    picture: Picture,
    size: Size,
    piece: usize,
    width: f32,
    height: f32,
}

impl TileImage {
    fn render(self) -> Div {
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
                img(self.picture.asset())
                    .absolute()
                    .left(px(-x))
                    .top(px(-y))
                    .w(px(image_width))
                    .h(px(image_height))
                    .object_fit(ObjectFit::Cover),
            )
    }
}

#[derive(Clone, Copy)]
struct DraggedTile {
    position: usize,
    tile: TileImage,
}

impl Render for DraggedTile {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.tile.render().border_2().border_color(rgb(t::GOLD))
    }
}

pub struct GameView {
    puzzles: HashMap<(Picture, Size), Puzzle>,
    picture: Picture,
    size: Size,
    seed: u64,
    show_reference: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _cx: &mut Context<Self>) -> Self {
        let seed = launch.seed();
        let picture = Picture::Lighthouse;
        let size = Size::Twelve;
        Self {
            puzzles: HashMap::from([((picture, size), Puzzle::shuffled(seed))]),
            picture,
            size,
            seed,
            show_reference: true,
        }
    }

    fn key(&self) -> (Picture, Size) {
        (self.picture, self.size)
    }

    fn puzzle(&self) -> &Puzzle {
        &self.puzzles[&self.key()]
    }

    fn puzzle_mut(&mut self) -> &mut Puzzle {
        let key = self.key();
        self.puzzles.get_mut(&key).expect("active puzzle exists")
    }

    fn choose(&mut self, picture: Picture, size: Size) {
        self.picture = picture;
        self.size = size;
        self.puzzles
            .entry((picture, size))
            .or_insert_with(|| Puzzle::with_size(size, self.seed));
    }

    fn tile(&self, position: usize, width: f32, height: f32, cx: &Context<Self>) -> Stateful<Div> {
        let puzzle = self.puzzle();
        let piece = puzzle.pieces()[position];
        let selected = puzzle.selected() == Some(position);
        let placed = piece == position;
        let tile = TileImage {
            picture: self.picture,
            size: self.size,
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
            .cursor_pointer()
            .hover(|style| style.opacity(0.85))
            .focus(|style| style.opacity(0.70))
            .drag_over::<DraggedTile>(|style, _, _, _| style.opacity(0.65))
            .on_drag(DraggedTile { position, tile }, |drag, _, _, cx| {
                cx.new(|_| *drag)
            })
            .on_drop(cx.listener(move |view, dragged: &DraggedTile, _, cx| {
                if view.key() == (dragged.tile.picture, dragged.tile.size)
                    && view.puzzle().pieces().get(dragged.position) == Some(&dragged.tile.piece)
                    && view.puzzle_mut().swap(dragged.position, position) != Selection::Ignored
                {
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |view, _, _, cx| {
                if view.puzzle_mut().select(position) != Selection::Ignored {
                    cx.notify();
                }
            }))
    }

    fn board(&self, width: f32, cx: &Context<Self>) -> Div {
        let (columns, rows) = self.size.dimensions();
        let piece_width = (width - GAP * (columns - 1) as f32) / columns as f32;
        let image_height = piece_width * columns as f32 / self.picture.aspect();
        let piece_height = image_height / rows as f32;
        let height = image_height + GAP * (rows - 1) as f32;
        let board = if self.puzzle().is_complete() {
            div().w(px(width)).h(px(height)).child(
                img(self.picture.asset())
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            )
        } else {
            div()
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
        div().p_2().rounded_lg().bg(rgb(NIGHT)).child(board)
    }

    fn reference(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .w(px(205.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(t::display_type("Reference", 18.))
                    .child(
                        control(
                            "jigsaw-reference",
                            if self.show_reference { "Hide" } else { "Show" },
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.show_reference = !view.show_reference;
                            cx.notify();
                        })),
                    ),
            )
            .when(self.show_reference, |panel| {
                panel.child(
                    div()
                        .w_full()
                        .h(px(205. / self.picture.aspect()))
                        .rounded_md()
                        .overflow_hidden()
                        .bg(rgb(SEA))
                        .child(
                            img(self.picture.asset())
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        ),
                )
            })
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let space = board_space(window);
        let side_by_side = space >= 680.;
        let width = (space - if side_by_side { 241. } else { 16. }).clamp(290., 638.);
        let puzzle = self.puzzle();
        let status = if puzzle.is_complete() {
            "Picture complete!"
        } else if puzzle.selected().is_some() {
            "Select another piece to swap."
        } else {
            "Drag a piece onto another to swap, or select them one after the other. Correct pieces lock in place."
        };

        t::surface()
            .w_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div().flex().flex_wrap().items_center().gap_2().children(
                    [Picture::Lighthouse, Picture::Harbor]
                        .into_iter()
                        .map(|picture| {
                            let id = if picture == Picture::Lighthouse {
                                "jigsaw-lighthouse"
                            } else {
                                "jigsaw-harbor"
                            };
                            control(id, picture.title(), true)
                                .when(self.picture == picture, |button| button.bg(rgb(t::GOLD)))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.choose(picture, view.size);
                                    cx.notify();
                                }))
                        }),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .children(
                        Size::ALL
                            .into_iter()
                            .zip(["jigsaw-12", "jigsaw-24", "jigsaw-48"])
                            .map(|(size, id)| {
                                control(id, format!("{} pieces", size.piece_count()), true)
                                    .when(self.size == size, |button| button.bg(rgb(t::GOLD)))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.choose(view.picture, size);
                                        cx.notify();
                                    }))
                            }),
                    )
                    .child(control("jigsaw-shuffle", "Shuffle pieces", true).on_click(
                        cx.listener(|view, _, _, cx| {
                            view.seed = view.seed.wrapping_add(1);
                            *view.puzzle_mut() = Puzzle::with_size(view.size, view.seed);
                            cx.notify();
                        }),
                    )),
            )
            .child(
                div()
                    .text_color(rgb(t::MUTED))
                    .text_size(px(13.))
                    .child(status),
            )
            .child(
                div()
                    .flex()
                    .when(!side_by_side, |layout| layout.flex_col())
                    .items_start()
                    .gap_5()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(self.board(width, cx))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .gap_3()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(t::MUTED))
                                    .child(format!(
                                        "{} / {} placed",
                                        puzzle.placed_count(),
                                        self.size.piece_count()
                                    ))
                                    .child(format!("{} moves", puzzle.moves())),
                            ),
                    )
                    .child(self.reference(cx)),
            )
    }
}
