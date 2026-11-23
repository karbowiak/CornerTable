use corner_table_sudoku::{Cage, CageIssue, Game, SIZE};
use gpui::{
    Bounds, FontWeight, Pixels, Role, Stateful, Window, canvas, div, fill, point, prelude::*, px,
    rgb, size,
};

use super::issue_message;
use crate::games::sudoku::{ERROR, PLAYER_INK, t};

pub(in crate::games::sudoku) const CAGE_ACTIVE_SURFACE: u32 = 0xe0eee5;
pub(in crate::games::sudoku) const CAGE_ERROR_SURFACE: u32 = 0xfbeee5;
const CAGE_LINE: u32 = 0x647770;
const CAGE_INSET: f32 = 3.;

#[derive(Clone, Copy)]
pub(in crate::games::sudoku) struct Cell {
    cage: &'static Cage,
    index: usize,
    issue: Option<CageIssue>,
    active: bool,
}

impl Cell {
    pub(in crate::games::sudoku) fn new(
        game: &Game,
        index: usize,
        selected: usize,
        solved: bool,
    ) -> Option<Self> {
        let cage = game.cage(index)?;
        Some(Self {
            cage,
            index,
            issue: game.cage_issue(index),
            active: !solved && cage.cells().contains(&selected),
        })
    }

    pub(in crate::games::sudoku) fn has_issue(self) -> bool {
        self.issue.is_some()
    }

    pub(in crate::games::sudoku) fn contains(self, index: usize) -> bool {
        self.cage.cells().contains(&index)
    }

    pub(in crate::games::sudoku) fn is_active(self) -> bool {
        self.active
    }

    pub(in crate::games::sudoku) fn render(
        self,
        cell: Stateful<gpui::Div>,
        unit: f32,
        notes: u16,
        value: u8,
        selected: bool,
        unit_conflict: bool,
    ) -> Stateful<gpui::Div> {
        let ink = if self.has_issue() {
            ERROR
        } else if self.active {
            PLAYER_INK
        } else {
            CAGE_LINE
        };
        let sum = self.cage.sum();
        let label = format!(
            "Row {}, column {}, {}, cage sum {sum}",
            self.index / SIZE + 1,
            self.index % SIZE + 1,
            if value == 0 {
                "empty".to_owned()
            } else {
                value.to_string()
            },
        );
        cell.role(Role::GridCell)
            .aria_label(label)
            .aria_selected(selected)
            .when(selected, |cell| cell.aria_active_descendant())
            .when_some(self.issue, |cell, issue| {
                cell.aria_description(issue_message(issue, sum))
            })
            .when(unit_conflict && self.issue.is_none(), |cell| {
                cell.aria_description("Repeated in a row, column or box.")
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        self.paint_outline(bounds, ink, window);
                    },
                )
                .absolute()
                .inset_0(),
            )
            .when(self.index == self.cage.anchor(), |cell| {
                cell.child(
                    div()
                        .debug_selector(move || format!("killer-total-{}", self.index))
                        .absolute()
                        .left(px(6.))
                        .top(px(3.))
                        .text_color(rgb(if self.has_issue() { ERROR } else { t::INK }))
                        .text_size(px((unit * 0.28).clamp(10., 14.)))
                        .line_height(px((unit * 0.28).clamp(10., 14.)))
                        .font_weight(FontWeight::BOLD)
                        .child(sum.to_string()),
                )
            })
            .when(value != 0, |cell| {
                cell.child(
                    div()
                        .absolute()
                        .top(px(unit * 0.29))
                        .left_0()
                        .right_0()
                        .bottom(px(3.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .line_height(px(unit * 0.56))
                        .child(value.to_string()),
                )
            })
            .when(notes != 0, |cell| {
                let digits = (1..=SIZE)
                    .filter(|&digit| notes & (1 << digit) != 0)
                    .map(|digit| digit.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let description = match self.issue {
                    Some(issue) => format!("{} Notes {digits}.", issue_message(issue, sum)),
                    None => format!("Notes {digits}."),
                };
                cell.aria_description(description)
                    .child(self.notes(unit, notes))
            })
    }

    fn notes(self, unit: f32, notes: u16) -> gpui::Div {
        let row_height = px(unit * 0.18);
        let column_width = px(unit * 0.22);
        div()
            .debug_selector(move || format!("killer-notes-{}", self.index))
            .absolute()
            .top(px(unit * 0.36))
            .left_0()
            .right_0()
            .flex()
            .flex_col()
            .items_center()
            .text_size(px(unit * 0.22))
            .line_height(row_height)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(t::MUTED))
            .children((0..3).map(|row| {
                div().flex().h(row_height).children((1..=3).map(|column| {
                    let digit = row * 3 + column;
                    div()
                        .w(column_width)
                        .text_center()
                        .children(((notes & (1 << digit)) != 0).then(|| digit.to_string()))
                }))
            }))
    }

    fn separates(self, row_delta: isize, column_delta: isize) -> bool {
        let other = (self.index / SIZE)
            .checked_add_signed(row_delta)
            .zip((self.index % SIZE).checked_add_signed(column_delta));
        other.is_none_or(|(row, column)| {
            row >= SIZE || column >= SIZE || !self.contains(row * SIZE + column)
        })
    }

    fn paint_outline(self, bounds: Bounds<Pixels>, ink: u32, window: &mut Window) {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let top = self.separates(-1, 0);
        let right = self.separates(0, 1);
        let bottom = self.separates(1, 0);
        let left = self.separates(0, -1);
        let inset = CAGE_INSET;
        let mut stroke = |horizontal, offset, start, end| {
            paint_edge(
                window,
                bounds,
                horizontal,
                offset,
                start..end,
                ink,
                self.has_issue(),
            );
        };
        let pad = |separated| if separated { inset } else { 0. };
        let x_left = pad(left);
        let x_right = width - pad(right);
        let y_top = pad(top);
        let y_bottom = height - pad(bottom);
        if top {
            stroke(true, inset, x_left, x_right);
        }
        if bottom {
            stroke(true, height - inset, x_left, x_right);
        }
        if left {
            stroke(false, inset, y_top, y_bottom);
        }
        if right {
            stroke(false, width - inset, y_top, y_bottom);
        }

        // Connect inward corners where both side neighbors belong to this cage.
        for (row, edge_y, side_y) in [(-1, 0., top), (1, height, bottom)] {
            for (column, edge_x, side_x) in [(-1, 0., left), (1, width, right)] {
                if side_x || side_y || !self.separates(row, column) {
                    continue;
                }
                let x = edge_x + if column < 0 { inset } else { -inset };
                let y = edge_y + if row < 0 { inset } else { -inset };
                stroke(true, y, edge_x.min(x), edge_x.max(x));
                stroke(false, x, edge_y.min(y), edge_y.max(y));
            }
        }
    }
}

fn paint_edge(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    horizontal: bool,
    offset: f32,
    span: std::ops::Range<f32>,
    ink: u32,
    solid: bool,
) {
    let distance = span.end - span.start;
    // Evenly spaced dashes reach the corners on every supported cell size.
    let count = if solid {
        1
    } else {
        (distance / 7.).round().max(1.) as usize
    };
    let gap = 2.5;
    let length = (distance - (count - 1) as f32 * gap) / count as f32;
    let thickness = if solid { 2. } else { 1. };
    let inset = offset - thickness / 2.;
    for dash in 0..count {
        let start = span.start + (length + gap) * dash as f32;
        let (x, y, w, h) = if horizontal {
            (start, inset, length, thickness)
        } else {
            (inset, start, thickness, length)
        };
        window.paint_quad(fill(
            Bounds::new(bounds.origin + point(px(x), px(y)), size(px(w), px(h))),
            rgb(ink),
        ));
    }
}
