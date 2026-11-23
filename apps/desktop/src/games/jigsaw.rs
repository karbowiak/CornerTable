use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_jigsaw::{COLUMNS, PIECE_COUNT, Puzzle, ROWS, Selection};
use gpui::{
    Context, Div, FontWeight, ObjectFit, Render, SharedString, Stateful, Window, div, img,
    prelude::*, px, rgb,
};

use crate::{
    games::{GameEntry, GameLaunch},
    theme as t,
};

const NIGHT: u32 = 0x253d43;
const SEA: u32 = 0x385b60;
const WARM_WHITE: u32 = 0xf8f2e6;
const REFERENCE: &str = "games/jigsaw/lighthouse.jpg";
const GAP: f32 = 4.;

pub struct GameView {
    puzzle: Puzzle,
    seed: u64,
    show_reference: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _cx: &mut Context<Self>) -> Self {
        let seed = match &launch.entry {
            GameEntry::Challenge(id) => id.bytes().fold(0_u64, |seed, byte| {
                seed.wrapping_mul(31).wrapping_add(u64::from(byte))
            }),
            _ => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos() as u64),
        };

        Self {
            puzzle: Puzzle::shuffled(seed),
            seed,
            show_reference: true,
        }
    }

    fn tile(&self, position: usize, side: f32, cx: &Context<Self>) -> Stateful<Div> {
        let piece = self.puzzle.pieces()[position];
        let selected = self.puzzle.selected() == Some(position);
        let placed = piece == position;
        let tile = div()
            .id(("jigsaw-tile", position))
            .relative()
            .w(px(side))
            .h(px(side))
            .flex_none()
            .overflow_hidden()
            .border_2()
            .border_color(rgb(if selected {
                t::GOLD
            } else if placed {
                0x81a794
            } else {
                NIGHT
            }))
            .child(
                img(format!("games/jigsaw/tile-{piece}.jpg"))
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            );

        if placed {
            tile.child(
                div()
                    .absolute()
                    .bottom(px(4.))
                    .right(px(4.))
                    .size(px(19.))
                    .rounded_full()
                    .bg(rgb(SEA))
                    .text_color(rgb(WARM_WHITE))
                    .text_size(px(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("✓"),
            )
        } else {
            tile.tab_index(0)
                .cursor_pointer()
                .hover(|style| style.border_color(rgb(t::GOLD_HOVER)))
                .focus(|style| style.border_color(rgb(t::GOLD)))
                .on_click(cx.listener(move |view, _, _, cx| {
                    if view.puzzle.select(position) != Selection::Ignored {
                        cx.notify();
                    }
                }))
        }
    }

    fn board(&self, side: f32, cx: &Context<Self>) -> Div {
        let width = side * COLUMNS as f32 + GAP * (COLUMNS - 1) as f32;
        let height = side * ROWS as f32 + GAP * (ROWS - 1) as f32;
        let picture = if self.puzzle.is_complete() {
            div()
                .w(px(width))
                .h(px(height))
                .child(img(REFERENCE).size_full().object_fit(ObjectFit::Cover))
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(GAP))
                .children((0..ROWS).map(|row| {
                    div().flex().gap(px(GAP)).children(
                        (0..COLUMNS).map(|column| self.tile(row * COLUMNS + column, side, cx)),
                    )
                }))
        };

        div()
            .p_2()
            .rounded_lg()
            .border_1()
            .border_color(rgb(SEA))
            .bg(rgb(NIGHT))
            .child(picture)
    }

    fn reference(&self, width: f32, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .w(px(width))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(t::display_type("Reference", 19.))
                    .child(
                        control(
                            "jigsaw-reference-toggle",
                            if self.show_reference { "Hide" } else { "Show" },
                            false,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.show_reference = !view.show_reference;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .h(px(width * 0.75))
                    .rounded_md()
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(t::BORDER))
                    .bg(rgb(0xeeeadf))
                    .when(self.show_reference, |panel| {
                        panel.child(img(REFERENCE).size_full().object_fit(ObjectFit::Cover))
                    })
                    .when(!self.show_reference, |panel| {
                        panel
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(t::MUTED))
                            .child("Reference hidden")
                    }),
            )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let window_width = f32::from(window.viewport_size().width);
        let sidebar_and_gutters = if window_width < 1080. { 218. } else { 288. };
        let available = window_width - sidebar_and_gutters - 36.;
        let side_by_side = available >= 700.;
        let board_width = if side_by_side {
            available - 224.
        } else {
            available
        };
        let tile_side = ((board_width - 30.) / COLUMNS as f32).clamp(34., 122.);
        let placed = self.puzzle.placed_count();
        let moves = self.puzzle.moves();
        let status = if self.puzzle.is_complete() {
            "Picture complete!"
        } else if self.puzzle.selected().is_some() {
            "Select another piece to swap."
        } else {
            "Select two pieces to swap. Pieces lock when they reach the right spot."
        };

        t::surface()
            .w_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(t::display_type("Lighthouse at Dusk", 26.))
                            .child(
                                div()
                                    .text_color(rgb(t::MUTED))
                                    .text_size(px(13.))
                                    .child(status),
                            ),
                    )
                    .child(control("jigsaw-shuffle", "Shuffle pieces", true).on_click(
                        cx.listener(|view, _, _, cx| {
                            view.seed = view.seed.wrapping_add(1);
                            view.puzzle = Puzzle::shuffled(view.seed);
                            cx.notify();
                        }),
                    )),
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
                            .child(self.board(tile_side, cx))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .gap_3()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(t::MUTED))
                                    .child(format!("{placed} / {PIECE_COUNT} placed"))
                                    .child(format!(
                                        "{moves} {}",
                                        if moves == 1 { "move" } else { "moves" }
                                    )),
                            ),
                    )
                    .child(self.reference(if side_by_side { 204. } else { 174. }, cx)),
            )
    }
}

fn control(id: &'static str, label: impl Into<SharedString>, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .tab_index(0)
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .min_h(px(34.))
        .px_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(if primary { t::GOLD } else { t::BORDER }))
        .bg(rgb(if primary { t::GOLD } else { t::SURFACE }))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|style| style.bg(rgb(if primary { t::GOLD_HOVER } else { t::SAGE })))
        .focus(|style| style.border_color(rgb(t::MUTED)))
        .child(label.into())
}
