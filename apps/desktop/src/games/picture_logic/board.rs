use std::rc::Rc;

use corner_table_picture_logic::{Axis, Board, Cell, Puzzle};
use gpui::{
    Context, Div, FontWeight, HoverListenerMode, MouseButton, PathBuilder, Role, Stateful, Window,
    canvas, div, fill, point, prelude::*, px, rgb, size,
};

use super::{GameView, Notice, paper::Colors, plate_name};

impl GameView {
    pub(super) fn grid(
        &self,
        available: f32,
        compact: bool,
        paint: Colors,
        window: &Window,
        cx: &Context<Self>,
    ) -> (Div, bool) {
        let play = self.book.play();
        let puzzle = play.board.puzzle();
        let (width, height) = (puzzle.width(), puzzle.height());
        let clues = play.board.clues();
        let (rows_met, columns_met) = play.board.matched();
        let left_slots = clues.rows().iter().map(Vec::len).max().unwrap_or(0).max(1);
        let top_slots = clues
            .columns()
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(0)
            .max(1);
        let target: f32 = match width {
            0..=5 => 66.,
            6..=7 => 53.,
            8..=10 => 42.,
            11..=12 => 36.,
            _ => 32.,
        };
        let left_slot = if available < 420. { 18. } else { 20. };
        let max_step = ((available - left_slots as f32 * left_slot - 1.) / width as f32).floor();
        // Leave room for the game's heading, its controls, and the paper margins.
        let height_budget =
            f32::from(window.viewport_size().height) - if compact { 292. } else { 187. };
        let min_height_step = if width <= 7 { 36. } else { 22. };
        let height_step = ((height_budget - top_slots as f32 * 19. - 1.) / height as f32)
            .floor()
            .max(min_height_step);
        let step = target.min(max_step.max(12.)).min(height_step);
        let top_slot = if step < 24. { 17. } else { 19. };
        let left = left_slots as f32 * left_slot + 1.;
        let top = top_slots as f32 * top_slot + 1.;
        let visible_rows =
            (((height_budget - top) / step).floor() as usize).clamp(3.min(height), height);
        let font = if step < 24. {
            (step * 0.73).floor().clamp(9., 12.)
        } else {
            14.
        };
        let plot_size = size(px(step * width as f32), px(step * height as f32));
        let focused = self.board_focus.is_focused(window);
        let active = self.hover.or_else(|| focused.then_some(play.cursor));
        let row_focus = active.map(|index| index / width);
        let col_focus = active.map(|index| index % width);
        let plot = Rc::clone(&self.plot);

        let column_clues = div()
            .flex()
            .h(px(top))
            .bg(rgb(paint.clue))
            .border_b_1()
            .border_color(rgb(paint.major))
            .child(div().w(px(left)).flex_shrink_0())
            .children(clues.columns().iter().enumerate().map(|(column, runs)| {
                clue_label(Axis::Column, column, runs, columns_met[column])
                    .w(px(step))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_end()
                    .border_r_1()
                    .border_color(rgb(clue_rule(column, width, paint)))
                    .when(col_focus == Some(column), |gutter| {
                        gutter.bg(rgb(paint.track))
                    })
                    .children(
                        clue_values(runs, columns_met[column], paint, font)
                            .map(|label| label.h(px(top_slot)).flex().items_center()),
                    )
            }));

        let row_clues = div()
            .flex()
            .flex_col()
            .w(px(left))
            .flex_shrink_0()
            .bg(rgb(paint.clue))
            .border_r_1()
            .border_color(rgb(paint.major))
            .children(clues.rows().iter().enumerate().map(|(row, runs)| {
                clue_label(Axis::Row, row, runs, rows_met[row])
                    .h(px(step))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(clue_rule(row, height, paint)))
                    .when(row_focus == Some(row), |gutter| gutter.bg(rgb(paint.track)))
                    .children(
                        clue_values(runs, rows_met[row], paint, font)
                            .map(|label| label.w(px(left_slot)).flex().justify_center()),
                    )
            }));

        let cells = div()
            .id("picture-logic-plot")
            .relative()
            .flex_shrink_0()
            .w(plot_size.width)
            .h(plot_size.height)
            .cursor_crosshair()
            .on_any_mouse_down(cx.listener(move |view, event, window, cx| {
                view.pointer_down(event, step, window, cx);
            }))
            .on_mouse_move(cx.listener(move |view, event, _, cx| {
                view.pointer_move(event, step, cx);
            }))
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(cx.listener(|view, hovered, _, cx| {
                if !hovered {
                    view.pointer_exit(cx);
                }
            }))
            .on_mouse_exit(cx.listener(|view, _, _, cx| view.pointer_exit(cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, event, _, cx| view.pointer_up(event, Some(step), cx)),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |view, event, _, cx| view.pointer_up(event, Some(step), cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view, event, _, cx| view.pointer_up(event, None, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Right,
                cx.listener(|view, event, _, cx| view.pointer_up(event, None, cx)),
            )
            .children(play.board.cells().iter().enumerate().map(|(index, &cell)| {
                let col = index % width;
                let row = index / width;
                let conflict = matches!(self.notice, Some(Notice::Checked(n)) if n > 0)
                    && play.board.has_conflict(index);
                let hint = matches!(self.notice, Some(Notice::Hint(d)) if d.index == index);
                let cursor = focused && play.cursor == index;
                let background = if active == Some(index) {
                    paint.active
                } else if row_focus == Some(row) || col_focus == Some(col) {
                    paint.track
                } else {
                    paint.stock
                };
                tile(
                    index,
                    (row, col),
                    step,
                    cell,
                    background,
                    TileState {
                        conflict,
                        hint,
                        cursor,
                    },
                    paint,
                )
            }))
            // GPUI computes this in prepaint so hit testing follows both resizing and scrolling.
            .child(
                canvas(
                    move |bounds, _, _| plot.set(Some(bounds)),
                    move |bounds, (), window, _| {
                        for col in 1..=width {
                            let major = col % 5 == 0 && col < width;
                            let thickness = if major { 2. } else { 1. };
                            window.paint_quad(fill(
                                gpui::Bounds {
                                    origin: point(
                                        bounds.left() + px(step * col as f32 - thickness),
                                        bounds.top(),
                                    ),
                                    size: size(px(thickness), bounds.size.height),
                                },
                                rgb(if major { paint.major } else { paint.rule }),
                            ));
                        }
                        for row in 1..=height {
                            let major = row % 5 == 0 && row < height;
                            let thickness = if major { 2. } else { 1. };
                            window.paint_quad(fill(
                                gpui::Bounds {
                                    origin: point(
                                        bounds.left(),
                                        bounds.top() + px(step * row as f32 - thickness),
                                    ),
                                    size: size(bounds.size.width, px(thickness)),
                                },
                                rgb(if major { paint.major } else { paint.rule }),
                            ));
                        }
                    },
                )
                .absolute()
                .inset_0(),
            );

        let grid = div()
            .flex()
            .flex_col()
            .w(px(left) + plot_size.width + px(2.))
            .border_1()
            .border_color(rgb(paint.major))
            .bg(rgb(paint.stock))
            .child(column_clues)
            .child(
                div()
                    .id("picture-logic-rows")
                    .h(px(visible_rows as f32 * step))
                    .overflow_y_scroll()
                    .track_scroll(&self.grid_scroll)
                    .child(div().flex().child(row_clues).child(cells)),
            );
        (grid, visible_rows < height)
    }
}

struct TileState {
    conflict: bool,
    hint: bool,
    cursor: bool,
}

fn tile(
    index: usize,
    (row, col): (usize, usize),
    step: f32,
    cell: Cell,
    background: u32,
    state: TileState,
    paint: Colors,
) -> Stateful<Div> {
    let edge = (step * 0.04).round().clamp(1., 2.);
    let ink = if state.conflict {
        paint.alert
    } else {
        paint.fill
    };
    div()
        .id(("picture-square", index))
        .role(Role::GridCell)
        .aria_label(format!(
            "Row {}, column {}. {}.{}{}",
            row + 1,
            col + 1,
            cell.label(),
            if state.conflict { " Conflict." } else { "" },
            if state.hint { " Hint." } else { "" },
        ))
        .when(state.cursor, |cell| cell.aria_active_descendant())
        .absolute()
        .top(px(row as f32 * step))
        .left(px(col as f32 * step))
        .size(px(step))
        .bg(rgb(background))
        .when(cell == Cell::Filled, |cell| {
            let inset = if state.conflict || state.hint || state.cursor {
                edge + if step < 20. { 2. } else { 3. }
            } else {
                edge + 1.
            };
            cell.child(pencil_fill(index, ink, paint.stock, step, inset))
        })
        .when(cell == Cell::Marked, |cell| {
            cell.child(cross(
                if state.conflict {
                    paint.alert
                } else {
                    paint.mark
                },
                if step < 24. { 0.43 } else { 0.33 },
            ))
        })
        .when(state.hint || state.cursor || state.conflict, |cell| {
            let color = if state.hint {
                paint.hint
            } else if state.cursor && state.conflict {
                paint.ink
            } else if state.cursor {
                paint.select
            } else {
                paint.alert
            };
            cell.child(
                div()
                    .absolute()
                    .inset(px(edge))
                    .border_2()
                    .border_color(rgb(color)),
            )
        })
}

fn pencil_fill(index: usize, ink: u32, paper: u32, step: f32, inset: f32) -> Div {
    div()
        .absolute()
        .inset(px(inset))
        .bg(rgb(ink))
        .when(step >= 24., |mark| {
            mark.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        // Lightly exposed paper between passes of a broad pencil stroke.
                        let mut tooth = PathBuilder::stroke(px(0.65));
                        let shift = (index % 7) as f32 * 0.013;
                        for (x, y, length) in [
                            (0.07, 0.54, 0.46),
                            (0.12, 0.75, 0.64),
                            (0.19, 0.88, 0.69),
                            (0.35, 0.92, 0.58),
                            (0.55, 0.88, 0.38),
                        ] {
                            let start =
                                point(bounds.size.width * x, bounds.size.height * (y - shift));
                            let stroke = point(
                                bounds.size.width * length,
                                -bounds.size.height * length * 0.83,
                            );
                            tooth.move_to(bounds.origin + start);
                            tooth.line_to(bounds.origin + start + stroke);
                        }
                        if let Ok(path) = tooth.build() {
                            window.paint_path(path, rgb(paper).opacity(0.15));
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
        })
}

fn clue_rule(index: usize, count: usize, paint: Colors) -> u32 {
    let boundary = index + 1;
    if boundary.is_multiple_of(5) && boundary < count {
        paint.major
    } else {
        paint.rule
    }
}

fn clue_label(axis: Axis, index: usize, runs: &[u8], met: bool) -> Stateful<Div> {
    let runs = if runs.is_empty() {
        "no filled squares".to_owned()
    } else {
        runs.iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    div()
        .id((axis.label(), index))
        .role(match axis {
            Axis::Row => Role::RowHeader,
            Axis::Column => Role::ColumnHeader,
        })
        .aria_label(format!(
            "{} {}, {}{}",
            axis.label(),
            index + 1,
            runs,
            if met { ", matched" } else { "" }
        ))
}

fn clue_values(runs: &[u8], met: bool, paint: Colors, font: f32) -> impl Iterator<Item = Div> {
    let runs = if runs.is_empty() { &[0][..] } else { runs };
    runs.iter().map(move |number| {
        div()
            .text_size(px(font))
            .line_height(px(font + 2.))
            .text_color(rgb(if met { paint.quiet } else { paint.ink }))
            .font_weight(if met {
                FontWeight::MEDIUM
            } else {
                FontWeight::BOLD
            })
            .child(number.to_string())
    })
}

pub(super) fn cross(ink: u32, fraction: f32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let center = bounds.center();
            let half = bounds.size.width.min(bounds.size.height) * (fraction * 0.5);
            let mut path = PathBuilder::stroke(px(1.55));
            path.move_to(center + point(-half, -half));
            path.line_to(center + point(half, half));
            path.move_to(center + point(-half, half));
            path.line_to(center + point(half, -half));
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(ink));
            }
        },
    )
    .absolute()
    .inset_0()
}

/// Full-size print and gallery proofs use the same ink artwork and preserve the
/// puzzle's proportions. Unsolved thumbnails use only the player's own marks.
pub(super) fn impression(
    puzzle: &'static Puzzle,
    draft: Option<&Board>,
    step: f32,
    colors: Colors,
) -> Div {
    let width = puzzle.width();
    let height = puzzle.height();
    let completed = draft.is_none();
    let pixels = (0..width * height)
        .map(|index| {
            if let Some(board) = draft {
                match board.cells()[index] {
                    Cell::Filled => (Some(colors.fill), false),
                    Cell::Marked => (Some(colors.mark), true),
                    Cell::Blank => (None, false),
                }
            } else {
                (puzzle.color(index), false)
            }
        })
        .collect::<Vec<_>>();
    let blank = if completed {
        super::paper::PRINT_STOCK
    } else {
        colors.stock
    };
    div()
        .relative()
        .flex_shrink_0()
        .w(px(step * width as f32))
        .h(px(step * height as f32))
        .bg(rgb(if completed { blank } else { colors.rule }))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    for (index, (color, marked)) in pixels.into_iter().enumerate() {
                        if completed && color.is_none() {
                            continue;
                        }
                        let left = step * (index % width) as f32;
                        let top = step * (index / width) as f32;
                        let inset = if completed { 0. } else { 0.6 };
                        window.paint_quad(fill(
                            gpui::Bounds {
                                origin: bounds.origin + point(px(left + inset), px(top + inset)),
                                size: size(px(step - inset * 2.), px(step - inset * 2.)),
                            },
                            rgb(if marked {
                                blank
                            } else {
                                color.unwrap_or(blank)
                            }),
                        ));
                        if marked {
                            let inset = step * 0.34;
                            window.paint_quad(fill(
                                gpui::Bounds {
                                    origin: bounds.origin
                                        + point(px(left + inset), px(top + inset)),
                                    size: size(px(step - inset * 2.), px(step - inset * 2.)),
                                },
                                rgb(colors.mark),
                            ));
                        }
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
}

pub(super) fn proof(board: &Board, solved: bool, paint: Colors) -> Div {
    let puzzle = board.puzzle();
    let step = (78. / puzzle.width() as f32).floor();
    let tile = impression(puzzle, (!solved).then_some(board), step, paint);
    if solved {
        return tile;
    }
    let gutter = 14.;
    let rows = board.clues().rows().to_vec();
    let columns = board.clues().columns().to_vec();
    let (width, height) = (puzzle.width(), puzzle.height());
    div()
        .relative()
        .flex_shrink_0()
        .pt(px(gutter))
        .pl(px(gutter))
        .pb(px(4.))
        .pr(px(4.))
        .bg(rgb(paint.stock))
        .child(tile)
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    // The miniature gutters show run lengths as ticks without
                    // revealing the picture.
                    for (axis, span, clues) in
                        [(Axis::Row, width, rows), (Axis::Column, height, columns)]
                    {
                        let scale = (gutter - 3.) / span as f32;
                        for (index, runs) in clues.iter().enumerate() {
                            let mut edge = gutter - 1.5;
                            let center = gutter + (index as f32 + 0.5) * step;
                            for run in runs.iter().rev() {
                                let length = f32::from(*run) * scale;
                                let (position, dimensions) = match axis {
                                    Axis::Row => (point(edge - length, center), size(length, 1.)),
                                    Axis::Column => {
                                        (point(center, edge - length), size(1., length))
                                    }
                                };
                                window.paint_quad(fill(
                                    gpui::Bounds {
                                        origin: bounds.origin + position.map(px),
                                        size: dimensions.map(px),
                                    },
                                    rgb(paint.major),
                                ));
                                edge -= length + scale;
                            }
                        }
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
}

impl GameView {
    pub(super) fn grid_label(&self, window: &Window) -> String {
        let play = self.book.play();
        let width = play.board.puzzle().width();
        let index = play.cursor;
        let base = format!("{} grid", plate_name(self.book.active));
        if self.board_focus.is_focused(window) {
            format!(
                "{base}. Row {}, column {}, {}",
                index / width + 1,
                index % width + 1,
                play.board.cells()[index].label(),
            )
        } else {
            base
        }
    }
}
