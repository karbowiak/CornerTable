use std::time::Duration;

use corner_table_reversi::{Disc, Square, Squares};
use gpui::{
    Animation, AnimationExt, BoxShadow, Context, Div, FontWeight, Role, Stateful, Window, div,
    linear_color_stop, linear_gradient, prelude::*, px, rgb, rgba,
};

use super::GameView;

pub(super) const FRAME: f32 = 24.;
const COPPER: u32 = 0xf1b77e;
const FOCUS: u32 = 0xfff2c9;

impl GameView {
    pub(super) fn board(&self, size: f32, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let legal = if self.human_turn() {
            self.position().legal_moves()
        } else {
            Squares::default()
        };
        let focused = self.board_focus.is_focused(window);
        div()
            .id("reversi-board")
            .role(Role::Grid)
            .aria_label("Reversi board")
            .aria_row_count(8)
            .aria_column_count(8)
            .aria_description("Arrow keys select a square. Enter or Space places a disc.")
            .track_focus(&self.board_focus)
            .tab_index(0)
            .capture_key_down(cx.listener(Self::key_down))
            .flex()
            .flex_none()
            .flex_col()
            .rounded(px(12.))
            .bg(linear_gradient(
                152.,
                linear_color_stop(rgb(0x543d34), 0.),
                linear_color_stop(rgb(0x292224), 1.),
            ))
            .shadow(vec![
                BoxShadow::new(px(0.), px(6.), rgba(0x120e113d).into()).blur_radius(px(18.)),
                BoxShadow::new(px(0.), px(1.), rgba(0xe1ad8043).into()).inset(),
            ])
            .child(axis_row(size, self.display.coordinates))
            .children((0..8_u8).map(|row| {
                let number = self.display.coordinates.then(|| char::from(b'1' + row));
                div()
                    .id(("reversi-row", usize::from(row)))
                    .role(Role::Row)
                    .aria_row_index(usize::from(row))
                    .flex()
                    .child(axis_label(FRAME, size, number))
                    .children((0..8).map(|column| {
                        let square = Square::new(row, column).unwrap();
                        self.cell(square, size, legal.contains(square), focused, cx)
                    }))
                    .child(axis_label(FRAME, size, number))
            }))
            .child(axis_row(size, self.display.coordinates))
    }

    fn cell(
        &self,
        square: Square,
        size: f32,
        playable: bool,
        focused: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let color = self.position().disc(square);
        let last = self.round().last();
        let placed = last.is_some_and(|last| last.square == square);
        let flipped = last.is_some_and(|last| last.flipped.contains(square));
        let selected = focused && square == self.cursor;
        let surface = if (square.row() + square.column()).is_multiple_of(2) {
            0x3e5561
        } else {
            0x394f5b
        };
        let label = match color {
            Some(disc) if placed => format!("{square}, {}, last placement", disc.label()),
            Some(disc) if flipped => format!("{square}, {}, just flipped", disc.label()),
            Some(disc) => format!("{square}, {}", disc.label()),
            None if playable => format!(
                "{square}, legal move, flips {}",
                self.position().flips(square).count()
            ),
            None => format!("{square}, empty"),
        };

        let mut cell = div()
            .id(("reversi-cell", usize::from(square.index())))
            .role(Role::GridCell)
            .aria_column_index(usize::from(square.column()))
            .aria_label(label)
            .aria_selected(selected)
            .size(px(size))
            .flex()
            .flex_none()
            .relative()
            .items_center()
            .justify_center()
            .border_r_1()
            .border_b_1()
            .border_color(rgb(0x293944))
            .bg(rgb(surface))
            .when(playable, |cell| {
                cell.cursor_pointer()
                    .hover(|style| style.bg(rgb(0x54727d)))
                    .active(|style| style.bg(rgb(0x46616d)))
            })
            .on_click(cx.listener(move |view, _, window, cx| {
                view.cursor = square;
                view.board_focus.focus(window, cx);
                view.activate(square, cx);
                cx.notify();
            }));

        if placed || flipped || selected {
            let (inset, stroke, width) = match (selected, placed) {
                (true, _) => (2., rgba((FOCUS << 8) | 0xff), 2.),
                (false, true) => (3., rgba((COPPER << 8) | 0xff), 2.),
                (false, false) => (4., rgba((COPPER << 8) | 0xbd), 1.),
            };
            cell = cell.child(
                div()
                    .absolute()
                    .inset(px(inset))
                    .rounded(px(4.))
                    .border(px(width))
                    .border_color(stroke),
            );
        }

        if let Some(color) = color {
            let diameter = (size * 0.75).floor();
            cell = if self.animate && flipped {
                cell.child(div().with_animation(
                    ("reversi-flip", self.revision),
                    Animation::new(Duration::from_millis(270)),
                    move |slot, fraction| {
                        let (face, amount) = if fraction < 0.47 {
                            (color.opposite(), 1. - fraction / 0.47)
                        } else {
                            let open = (fraction - 0.47) / 0.53;
                            (color, 1. - (1. - open).powi(2))
                        };
                        slot.child(disc(face, diameter * (0.12 + amount * 0.88), diameter))
                    },
                ))
            } else {
                cell.child(disc(color, diameter, diameter))
            };
        } else if playable && self.display.hints {
            cell = cell.child(
                div()
                    .size(px((size * 0.17).clamp(6., 10.)))
                    .rounded_full()
                    .bg(rgb(0xe5c1a1)),
            );
        }
        cell
    }
}

fn axis_row(cell: f32, coordinates: bool) -> Div {
    div().flex().px(px(FRAME)).children(
        (b'A'..=b'H')
            .map(|column| axis_label(cell, FRAME, coordinates.then(|| char::from(column)))),
    )
}

fn axis_label(width: f32, height: f32, label: Option<char>) -> Div {
    div()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .flex()
        .items_center()
        .justify_center()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(11.))
        .line_height(px(13.))
        .text_color(rgb(0xe8c9ad))
        .children(label.map(|letter| letter.to_string()))
}

pub(super) fn disc(color: Disc, width: f32, diameter: f32) -> Div {
    let (light, dark, edge, rim, engraving) = match color {
        Disc::Black => (0x3a4249, 0x141b23, 0x0f131a, 0xa2a6ab, 0x9babb640),
        Disc::White => (0xfff9e9, 0xe1ceb0, 0x9d8069, 0xfff5dd, 0x8f725b58),
    };
    let depth = (diameter * 0.077).max(1.);
    let inset = (diameter * 0.155).min(width / 4.);
    div()
        .w(px(width))
        .h(px(diameter))
        .flex_none()
        .relative()
        .rounded_full()
        .bg(rgb(edge))
        .shadow(vec![
            BoxShadow::new(px(0.), px(depth), rgba(0x17141966).into()).blur_radius(px(depth * 1.6)),
        ])
        .child(
            div()
                .absolute()
                .inset_0()
                .bottom(px(depth))
                .rounded_full()
                .border_1()
                .border_color(rgb(rim))
                .bg(linear_gradient(
                    140.,
                    linear_color_stop(rgb(light), 0.),
                    linear_color_stop(rgb(dark), 1.),
                ))
                .when(diameter >= 22., |face| {
                    face.child(
                        div()
                            .absolute()
                            .inset(px(inset))
                            .rounded_full()
                            .border_1()
                            .border_color(rgba(engraving)),
                    )
                }),
        )
}
