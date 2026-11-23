use std::time::Duration;

use corner_table_checkers::{Outcome, Piece, Side, Square, Step};
use gpui::{
    Animation, AnimationExt, BoxShadow, Context, Div, FontWeight, Img, ObjectFit, Role, Stateful,
    Window, div, ease_out_quint, img, prelude::*, px, rgb, rgba,
};

use crate::motion;

use super::GameView;

const BOARD: &str = "games/checkers/board.webp";
// The webp has a 56-pixel frame around its 912-pixel playing area.
const RIM: f32 = 56. / 1024.;
const SELECTED: u32 = 0xffd37e;
const KEY_FOCUS: u32 = 0xe4fff3;
const WALNUT: u32 = 0x402b21;

impl GameView {
    pub(super) fn board(
        &self,
        side: f32,
        legal: &[Step],
        window: &Window,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let rim = side * RIM;
        let tile = (side - 2. * rim) / 8.;
        let focused = self.board_focus.is_focused(window);
        let show_cursor = focused && window.last_input_was_keyboard();
        let arrival = self.arrival.filter(|_| motion::enabled(cx));
        let coordinates = (0..8).flat_map(|index| {
            let start = rim + f32::from(index) * tile;
            [
                rail_mark(file(index).to_string(), side)
                    .left(px(start))
                    .bottom(px(0.))
                    .w(px(tile))
                    .h(px(rim)),
                rail_mark((8 - index).to_string(), side)
                    .left(px(0.))
                    .top(px(start))
                    .h(px(tile))
                    .w(px(rim)),
            ]
        });

        div()
            .id("checkers-board")
            .role(Role::Grid)
            .aria_label("Checkers board")
            .aria_description(if self.table.game().outcome() == Outcome::Playing {
                "Arrow keys move between playable squares. Enter or Space selects and moves. U undoes."
            } else {
                "Arrow keys review the dark squares. U undoes."
            })
            .aria_row_count(8)
            .aria_column_count(8)
            .track_focus(&self.board_focus)
            .tab_index(0)
            .on_key_down(cx.listener(Self::key_down))
            .relative()
            .size(px(side))
            .flex_none()
            .rounded(px(9.))
            .overflow_hidden()
            .bg(rgb(0x70442d))
            .shadow(vec![BoxShadow::new(px(0.), px(6.), rgba(0x321d1745).into()).blur_radius(px(15.))])
            .child(div().absolute().inset_0().child(img(BOARD).size_full().object_fit(ObjectFit::Fill)))
            .children(coordinates)
            .child(
                div()
                    .absolute()
                    .top(px(rim))
                    .left(px(rim))
                    .children((0..8).map(|row| {
                        div()
                            .id(("checkers-row", usize::from(row)))
                            .role(Role::Row)
                            .aria_row_index(usize::from(row))
                            .flex()
                            .children((0..8).map(|column| {
                                match Square::from_coords(row, column) {
                                    Some(square) => self.board_square(square, tile, legal, (focused, show_cursor), arrival, cx).into_any_element(),
                                    None => div().size(px(tile)).flex_none().into_any_element(),
                                }
                            }))
                    })),
            )
    }

    fn board_square(
        &self,
        square: Square,
        side: f32,
        legal: &[Step],
        (focused, show_cursor): (bool, bool),
        arrival: Option<Step>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let game = self.table.game();
        let selected = self.selected_square() == Some(square);
        let piece = game.piece(square);
        let available = legal.iter().any(|step| step.from == square);
        let landing = legal
            .iter()
            .find(|step| Some(step.from) == self.selected_square() && step.to == square);
        let last = game
            .last_steps()
            .iter()
            .any(|step| step.from == square || step.to == square);
        let keyed = focused && self.cursor == square;

        let label = match piece {
            Some(piece) => format!(
                "{}, {} {}",
                square_name(square),
                side_name(piece.side),
                if piece.king { "king" } else { "piece" }
            ),
            None => format!("{}, empty", square_name(square)),
        };
        div()
            .id(("checkers-square", usize::from(square.index())))
            .role(Role::GridCell)
            .aria_column_index(usize::from(square.column()))
            .aria_label(label)
            .when(keyed, |square| square.aria_active_descendant())
            .when(selected, |square| square.aria_selected(true))
            .when(available && !selected, |square| {
                square.aria_description("Ready to move")
            })
            .when_some(landing, |square, step| {
                square.aria_description(if step.captured().is_some() {
                    "Jump here"
                } else {
                    "Move here"
                })
            })
            .relative()
            .size(px(side))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .when(last, |square| {
                square.child(div().absolute().inset_0().bg(rgba(0xffdb9833)))
            })
            .when(available || landing.is_some(), |square| {
                square
                    .cursor_pointer()
                    .hover(|style| style.bg(rgba(0xffe3a33b)))
            })
            .when(self.player_turn(), |cell| {
                cell.on_click(cx.listener(move |view, _, window, cx| {
                    view.board_focus.focus(window, cx);
                    view.cursor = square;
                    view.activate(square, false, cx);
                }))
            })
            .when_some(piece, |cell, piece| {
                let image = checker(piece, side * 1.02);
                if let Some(step) = arrival.filter(|step| step.to == square) {
                    let offset = (side * 0.13).min(8.);
                    let dx = if step.from.column() < square.column() {
                        -offset
                    } else {
                        offset
                    };
                    let dy = if step.from.row() < square.row() {
                        -offset
                    } else {
                        offset
                    };
                    cell.child(image.with_animation(
                        ("checkers-arrival", game.steps().len()),
                        Animation::new(Duration::from_millis(170)).with_easing(ease_out_quint()),
                        move |image, progress| {
                            image
                                .relative()
                                .left(px(dx * (1. - progress)))
                                .top(px(dy * (1. - progress)))
                        },
                    ))
                } else {
                    cell.child(image)
                }
            })
            .when(last && !selected, |square| {
                square.child(
                    div()
                        .absolute()
                        .left(px(3.))
                        .bottom(px(3.))
                        .w(px(side * 0.28))
                        .h(px(2.))
                        .rounded_full()
                        .bg(rgb(SELECTED)),
                )
            })
            .when(available && !selected, |square| {
                let dot = (side * 0.13).clamp(5., 9.);
                square.child(
                    div()
                        .absolute()
                        .right(px(side * 0.08))
                        .top(px(side * 0.08))
                        .size(px(dot))
                        .rounded_full()
                        .bg(rgb(SELECTED))
                        .border_1()
                        .border_color(rgb(WALNUT)),
                )
            })
            .when_some(landing, |square, step| {
                square.child(target(step.captured().is_some(), side))
            })
            .when(selected, |square| {
                square
                    .child(outline(1., 3., WALNUT))
                    .child(outline(2., 2., SELECTED))
            })
            .when(keyed && show_cursor, |square| {
                let inset = if selected { 5. } else { 2. };
                square
                    .child(outline(inset, 3., WALNUT))
                    .child(outline(inset + 1., 2., KEY_FOCUS))
            })
    }
}

fn target(jump: bool, tile: f32) -> Div {
    let diameter = px(tile * if jump { 0.47 } else { 0.27 });
    div()
        .size(diameter)
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .when(jump, |dot| {
            dot.border_2()
                .border_color(rgb(SELECTED))
                .bg(rgba(0x422a2599))
                .child(div().size(px(tile * 0.12)).rounded_full().bg(rgb(SELECTED)))
        })
        .when(!jump, |dot| {
            dot.bg(rgb(0xe7efd1)).border_2().border_color(rgb(0x343f2f))
        })
}

fn outline(inset: f32, width: f32, color: u32) -> Div {
    div()
        .absolute()
        .inset(px(inset))
        .rounded(px(4.))
        .border(px(width))
        .border_color(rgb(color))
}

fn rail_mark(mark: String, board: f32) -> Div {
    div()
        .absolute()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px((board * 0.023).clamp(10., 13.)))
        .line_height(px(14.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffe2b1))
        .child(mark)
}

pub(super) fn checker(piece: Piece, size: f32) -> Img {
    let asset = match (piece.side, piece.king) {
        (Side::Red, false) => "games/checkers/red.webp",
        (Side::Red, true) => "games/checkers/red-king.webp",
        (Side::Light, false) => "games/checkers/maple.webp",
        (Side::Light, true) => "games/checkers/maple-king.webp",
    };
    img(asset)
        .size(px(size))
        .object_fit(ObjectFit::Fill)
        .flex_none()
}

pub(super) fn side_name(side: Side) -> &'static str {
    match side {
        Side::Red => "Red",
        Side::Light => "Maple",
    }
}

pub(super) fn square_name(square: Square) -> String {
    format!("{}{}", file(square.column()), 8 - square.row())
}

fn file(column: u8) -> char {
    char::from(b'a' + column)
}
