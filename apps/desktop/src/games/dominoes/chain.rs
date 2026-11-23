use std::time::Duration;

use corner_table_dominoes::End;
use gpui::{
    Animation, AnimationExt, Context, Div, FontWeight, Role, Stateful, div, ease_out_quint,
    prelude::*, px, rgb,
};

use crate::motion;

use super::{GameView, pieces};

pub(super) const LEFT_TONE: u32 = 0xffcf8e;
pub(super) const RIGHT_TONE: u32 = 0xa7e5e1;

const LINK: u32 = 0x758198;
const GAP: f32 = 5.;
const TAG: f32 = 32.;
const TAG_GAP: f32 = 7.;
const SIDE_MARGIN: f32 = 48.;
const TURN_DEPTH: f32 = 12.;

#[derive(Clone, Copy)]
struct Slot {
    x: f32,
    y: f32,
    row: usize,
}

impl Slot {
    fn reversed(self) -> bool {
        !self.row.is_multiple_of(2)
    }
}

struct Layout {
    columns: usize,
    unit: f32,
    left: f32,
    top: f32,
    stride: f32,
    height: f32,
}

impl Layout {
    /// `fill` is the height the table has to spare; the chain area grows into it.
    fn new(width: f32, count: usize, narrow: bool, short: bool, fill: f32) -> Self {
        let (unit, row_gap, padding, minimum_height) = if short {
            ((width / 20.).floor().clamp(21., 33.), 8., 20., 84_f32)
        } else {
            let unit = (width / 17.).floor().clamp(24., 42.);
            if narrow {
                (unit, 13., 34., 96.)
            } else {
                (unit, 17., 48., 140.)
            }
        };
        let stride = 2. * unit + GAP;
        let columns = (((width - 2. * SIDE_MARGIN + GAP) / stride).floor() as usize)
            .max(1)
            .min(count);
        let rows = count.div_ceil(columns);
        let row_stride = unit + row_gap;
        let chain_height = (rows - 1) as f32 * row_stride + unit;
        let height = minimum_height.max(fill).max(chain_height + padding);
        Self {
            columns,
            unit,
            left: (width - (columns as f32 * stride - GAP)) * 0.5,
            top: (height - chain_height) * 0.5,
            stride: row_stride,
            height,
        }
    }

    fn slot(&self, index: usize) -> Slot {
        let row = index / self.columns;
        let column = index % self.columns;
        let column = if row.is_multiple_of(2) {
            column
        } else {
            self.columns - 1 - column
        };
        Slot {
            x: self.left + column as f32 * (2. * self.unit + GAP),
            y: self.top + row as f32 * self.stride,
            row,
        }
    }

    fn end_x(&self, slot: Slot, points_left: bool) -> f32 {
        if points_left {
            slot.x - TAG - TAG_GAP
        } else {
            slot.x + 2. * self.unit + TAG_GAP
        }
    }
}

impl GameView {
    pub(super) fn chain(
        &self,
        width: f32,
        fill: f32,
        narrow: bool,
        short: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let round = self.round();
        let tiles = round.chain();
        let layout = Layout::new(width, tiles.len(), narrow, short, fill);
        let first = layout.slot(0);
        let last = layout.slot(tiles.len() - 1);
        let arrival = self.placement.filter(|_| motion::enabled(cx));
        div()
            .id("dominoes-chain")
            .role(Role::List)
            .aria_label("Domino chain from left end to right end")
            .relative()
            .mx_auto()
            .w(px(width))
            .h(px(layout.height))
            .children((1..tiles.len()).flat_map(|index| links(&layout, index)))
            .child(self.end_tag(End::Left, &layout, first, true, cx))
            .child(self.end_tag(End::Right, &layout, last, last.reversed(), cx))
            .children(tiles.iter().enumerate().map(|(index, placed)| {
                let slot = layout.slot(index);
                let mut faces = placed.faces;
                if slot.reversed() {
                    faces.reverse();
                }
                let recent = placed.tile == round.last_placement();
                let piece = pieces::ivory(faces, layout.unit, pieces::Pose::Horizontal, false);
                let [a, b] = placed.tile.pips();
                let key = usize::from(a) * 7 + usize::from(b);
                div()
                    .id(("dominoes-chain-tile", key))
                    .role(Role::ListItem)
                    .aria_label(format!(
                        "{} and {}{}",
                        placed.faces[0],
                        placed.faces[1],
                        if recent { ", last placed tile" } else { "" }
                    ))
                    .absolute()
                    .left(px(slot.x))
                    .top(px(slot.y))
                    .child(if let Some(end) = arrival.filter(|_| recent) {
                        let offset = if end == End::Left || slot.reversed() {
                            -8.
                        } else {
                            8.
                        };
                        div().child(
                            piece.with_animation(
                                ("dominoes-placement", self.revision),
                                Animation::new(Duration::from_millis(215))
                                    .with_easing(ease_out_quint()),
                                move |tile, progress| {
                                    let rest = 1. - progress;
                                    tile.relative()
                                        .left(px(offset * rest))
                                        .opacity(1. - 0.32 * rest)
                                },
                            ),
                        )
                    } else {
                        div().child(piece)
                    })
                    .when(recent, |tile| {
                        tile.child(
                            div()
                                .absolute()
                                .left(px(layout.unit - 3.))
                                .top(px(-8.))
                                .w(px(6.))
                                .h(px(3.))
                                .rounded_full()
                                .bg(rgb(LEFT_TONE)),
                        )
                    })
            }))
    }

    fn end_tag(
        &self,
        end: End,
        layout: &Layout,
        slot: Slot,
        points_left: bool,
        cx: &Context<Self>,
    ) -> Div {
        let tone = end_tone(end);
        let active = self.valid_end(end);
        let y = slot.y + layout.unit / 2.;
        let x = layout.end_x(slot, points_left);
        let line_x = if points_left { x + TAG } else { x - TAG_GAP };
        let key = match end {
            End::Left => "L",
            End::Right => "R",
        };
        div().child(stroke(line_x, y, TAG_GAP, 1., tone)).child(
            div()
                .id(("dominoes-chain-end", end.index()))
                .absolute()
                .left(px(x))
                .top(px(y - TAG / 2.))
                .size(px(TAG))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(tone))
                .font_weight(FontWeight::BOLD)
                .text_size(px(13.))
                .bg(rgb(if active { tone } else { 0x242b3d }))
                .text_color(rgb(if active { 0x24212a } else { tone }))
                .child(key)
                .when(active, |tag| {
                    tag.cursor_pointer()
                        .hover(|tag| tag.bg(rgb(pieces::BONE)))
                        .on_click(
                            cx.listener(move |view, _, window, cx| view.place(end, window, cx)),
                        )
                }),
        )
    }
}

pub(super) fn end_tone(end: End) -> u32 {
    match end {
        End::Left => LEFT_TONE,
        End::Right => RIGHT_TONE,
    }
}

fn stroke(x: f32, y: f32, width: f32, height: f32, color: u32) -> Div {
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(width))
        .h(px(height))
        .bg(rgb(color))
}

fn links(layout: &Layout, index: usize) -> Vec<Div> {
    let from = layout.slot(index - 1);
    let to = layout.slot(index);
    let width = layout.unit * 2.;
    let y = from.y + layout.unit * 0.5;
    if from.row == to.row {
        return vec![stroke(from.x.min(to.x) + width - 1., y, GAP + 2., 1., LINK)];
    }
    let edge = if from.reversed() {
        from.x
    } else {
        from.x + width
    };
    let bend = if from.reversed() {
        edge - TURN_DEPTH
    } else {
        edge + TURN_DEPTH
    };
    let left = edge.min(bend);
    vec![
        stroke(left, y, TURN_DEPTH + 1., 1., LINK),
        stroke(bend, y, 1., layout.stride, LINK),
        stroke(left, y + layout.stride, TURN_DEPTH + 1., 1., LINK),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tiles_and_end_tags_fit_while_rows_keep_their_direction() {
        for width in [318., 480., 650., 768., 1000., 1138.] {
            for (count, short) in (1..=28).flat_map(|count| [(count, false), (count, true)]) {
                let layout = Layout::new(width, count, width < 660., short, 0.);
                let first = layout.slot(0);
                let last = layout.slot(count - 1);
                assert!(layout.end_x(first, true) >= 0.);
                assert!(layout.end_x(last, last.reversed()) >= 0.);
                assert!(layout.end_x(last, last.reversed()) + TAG <= width);
                assert!(last.y + layout.unit <= layout.height);
                for index in 1..count {
                    let a = layout.slot(index - 1);
                    let b = layout.slot(index);
                    if a.row != b.row {
                        assert_eq!(a.x, b.x);
                        assert_ne!(a.reversed(), b.reversed());
                    } else if a.reversed() {
                        assert!(a.x > b.x);
                    } else {
                        assert!(a.x < b.x);
                    }
                }
            }
        }
    }

    #[test]
    fn a_full_chain_leaves_room_for_the_rack_at_the_minimum_window_size() {
        let layout = Layout::new(494., 28, true, true, 0.);
        assert!(layout.height <= 144.);
    }
}
