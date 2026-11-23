use corner_table_kakuro::{Cell, Direction, Game, Run};
use gpui::{
    Animation, AnimationExt, AnyElement, Context, Div, FontWeight, IntoElement, PathBuilder, Role,
    Stateful, canvas, div, ease_in_out, point, prelude::*, px, rgb, rgba,
};

use super::{GameView, RUN_PULSE, RunPulse};
use crate::motion;

pub(super) const FRAME: f32 = 2.;
pub(super) const GRID: f32 = 1.;

const BOARD_EDGE: u32 = 0x263c34;
const GRID_INK: u32 = 0x969f8e;
const BLOCK: u32 = 0x283c34;
const CLUE: u32 = 0x354a40;
const DIVIDER: u32 = 0x829587;
const CLUE_INK: u32 = 0xf6ebd7;
const PAPER: u32 = 0xfff8eb;
const ENTRY_INK: u32 = 0x17392f;
const NOTE_INK: u32 = 0x416256;
const ACROSS_PAPER: u32 = 0xf4e6c2;
const ACROSS_CLUE: u32 = 0x66501f;
const ACROSS_INK: u32 = 0xffe6ab;
const DOWN_PAPER: u32 = 0xdbebe4;
const DOWN_CLUE: u32 = 0x235d4f;
const DOWN_INK: u32 = 0xc9f3df;
const FOCUS_PAPER: u32 = 0xd8e8c7;
const FOCUS_INK: u32 = 0x315943;
const HINT_PAPER: u32 = 0xffe6a9;
const HINT_INK: u32 = 0x76530d;
const ERROR_PAPER: u32 = 0xfae0d4;
const ERROR_INK: u32 = 0x8c2b23;
const ERROR_CLUE: u32 = 0x7c352d;

struct FocusedRuns {
    across: Option<usize>,
    down: Option<usize>,
}

impl GameView {
    pub(super) fn board(&self, size: f32, cx: &Context<Self>) -> Stateful<Div> {
        let game = &self.round.game;
        let puzzle = game.puzzle();
        let side = puzzle.size().side();
        let pulse = self
            .round
            .pulse
            .filter(|pulse| pulse.started.elapsed() < RUN_PULSE && motion::enabled(cx));
        let focus = FocusedRuns {
            across: puzzle.run_at(self.round.selected, Direction::Across),
            down: puzzle.run_at(self.round.selected, Direction::Down),
        };
        div()
            .id("kakuro-board")
            .role(Role::Grid)
            .aria_label(puzzle.size().label())
            .aria_column_count(side)
            .aria_row_count(side)
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap(px(GRID))
            .p(px(FRAME - 1.))
            .rounded_sm()
            .border_1()
            .border_color(rgb(BOARD_EDGE))
            .bg(rgb(GRID_INK))
            .shadow_sm()
            .children(
                puzzle
                    .cells()
                    .chunks_exact(side)
                    .enumerate()
                    .map(|(row, cells)| {
                        div()
                            .id(("kakuro-row", row))
                            .role(Role::Row)
                            .aria_row_index(row)
                            .flex()
                            .gap(px(GRID))
                            .children(cells.iter().enumerate().map(|(column, cell)| {
                                let index = row * side + column;
                                match *cell {
                                    Cell::Entry => self.entry(index, size, &focus, pulse, cx),
                                    Cell::Block { across, down } => {
                                        block(game, index, size, across, down, &focus)
                                    }
                                }
                            }))
                    }),
            )
    }

    fn entry(
        &self,
        index: usize,
        size: f32,
        focus: &FocusedRuns,
        pulse: Option<RunPulse>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let game = &self.round.game;
        let puzzle = game.puzzle();
        let solved = game.is_solved();
        let selected = !solved && self.round.selected == index;
        let hinted = selected && self.round.hint.is_some();
        let across = puzzle.run_at(index, Direction::Across);
        let down = puzzle.run_at(index, Direction::Down);
        let value = game.value(index);
        let notes = game.notes(index);
        let conflict = value != 0
            && [across, down]
                .into_iter()
                .flatten()
                .any(|run| game.run_conflict(run).is_some());
        let pulsing = pulse.is_some_and(|pulse| {
            [across, down]
                .into_iter()
                .flatten()
                .any(|run| pulse.runs.contains(&Some(run)))
        });
        let background = if conflict {
            ERROR_PAPER
        } else if hinted {
            HINT_PAPER
        } else if selected {
            FOCUS_PAPER
        } else if !solved && across == focus.across {
            ACROSS_PAPER
        } else if !solved && down == focus.down {
            DOWN_PAPER
        } else {
            PAPER
        };
        let outline = if conflict {
            ERROR_INK
        } else if hinted {
            HINT_INK
        } else {
            FOCUS_INK
        };
        let side = puzzle.size().side();
        let mark = if value != 0 {
            value.to_string()
        } else if notes != 0 {
            format!(
                "Pencil {}",
                (1..=9)
                    .filter(|digit| notes & (1 << digit) != 0)
                    .map(|digit| digit.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            "Blank".into()
        };
        let across_sum = across.map(|run| puzzle.run(run).sum).unwrap_or_default();
        let down_sum = down.map(|run| puzzle.run(run).sum).unwrap_or_default();
        div()
            .id(("kakuro-entry", index))
            .role(Role::Cell)
            .aria_row_index(index / side)
            .aria_column_index(index % side)
            .aria_selected(selected)
            .aria_label(format!(
                "Row {}, column {}, {mark}, {across_sum} across and {down_sum} down{}",
                index / side + 1,
                index % side + 1,
                if conflict { ", run conflict" } else { "" },
            ))
            .relative()
            .size(px(size))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(background))
            .text_color(rgb(if conflict { ERROR_INK } else { ENTRY_INK }))
            .text_size(px((size * 0.51).round()))
            .line_height(px((size * 0.60).round()))
            .font_weight(FontWeight::SEMIBOLD)
            .when(!solved, |cell| {
                cell.cursor_pointer()
                    .when(!selected && !conflict, |cell| {
                        cell.hover(|style| style.bg(rgb(FOCUS_PAPER)))
                    })
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.round.select(index);
                        view.focus.focus(window, cx);
                        view.update_round(None, cx);
                    }))
            })
            .when(pulsing, |cell| {
                cell.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(rgba(0x65b39380))
                        .with_animation(
                            ("kakuro-run-pulse", self.round.moves as usize),
                            Animation::new(RUN_PULSE).with_easing(ease_in_out),
                            |ink, progress| ink.opacity(1. - progress),
                        ),
                )
            })
            .when(value != 0, |cell| cell.child(value.to_string()))
            .when(notes != 0, |cell| cell.child(pencil_grid(size, notes)))
            .when(conflict, |cell| {
                cell.child(
                    div()
                        .absolute()
                        .bottom(px(size * 0.13))
                        .w(px(size * 0.27))
                        .h(px(2.))
                        .bg(rgb(ERROR_INK)),
                )
            })
            .when(selected, |cell| {
                cell.child(
                    div()
                        .absolute()
                        .inset_0()
                        .border_2()
                        .border_color(rgb(outline)),
                )
            })
            .into_any_element()
    }
}

fn pencil_grid(size: f32, notes: u16) -> Div {
    let unit = px((size * 0.29).floor());
    div()
        .flex()
        .flex_col()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px((size * 0.20).round().max(8.)))
        .line_height(unit)
        .text_color(rgb(NOTE_INK))
        .children((0..3).map(|row| {
            div().flex().h(unit).children((1..=3).map(|column| {
                let digit = row * 3 + column;
                div()
                    .w(unit)
                    .text_center()
                    .children((notes & (1 << digit) != 0).then(|| digit.to_string()))
            }))
        }))
}

#[derive(Clone, Copy)]
struct Clue {
    sum: u8,
    fill: u32,
    ink: u32,
}

impl Clue {
    fn new(game: &Game, run: usize, active: bool, direction: Direction) -> Self {
        let sum = game.puzzle().run(run).sum;
        let (fill, ink) = if game.run_conflict(run).is_some() {
            (ERROR_CLUE, CLUE_INK)
        } else if active {
            match direction {
                Direction::Across => (ACROSS_CLUE, ACROSS_INK),
                Direction::Down => (DOWN_CLUE, DOWN_INK),
            }
        } else {
            (CLUE, CLUE_INK)
        };
        Self { sum, fill, ink }
    }

    fn text(self, size: f32) -> Div {
        div()
            .absolute()
            .font_weight(FontWeight::BOLD)
            .text_size(px((size * 0.27).round().clamp(10., 19.)))
            .line_height(px((size * 0.34).round().max(12.)))
            .text_color(rgb(self.ink))
            .child(self.sum.to_string())
    }
}

fn block(
    game: &Game,
    index: usize,
    size: f32,
    across: Option<usize>,
    down: Option<usize>,
    focus: &FocusedRuns,
) -> AnyElement {
    let show_focus = !game.is_solved();
    let across = across.map(|run| {
        Clue::new(
            game,
            run,
            show_focus && Some(run) == focus.across,
            Direction::Across,
        )
    });
    let down = down.map(|run| {
        Clue::new(
            game,
            run,
            show_focus && Some(run) == focus.down,
            Direction::Down,
        )
    });
    let pad = px((size * 0.082).round().max(3.));
    div()
        .id(("kakuro-clue", index))
        .relative()
        .size(px(size))
        .flex_shrink_0()
        .bg(rgb(BLOCK))
        .when(across.is_some() || down.is_some(), |cell| {
            let label = match (across, down) {
                (Some(a), Some(d)) => format!("{} across, {} down", a.sum, d.sum),
                (Some(a), None) => format!("{} across", a.sum),
                (None, Some(d)) => format!("{} down", d.sum),
                (None, None) => unreachable!(),
            };
            cell.role(Role::Cell)
                .aria_label(label)
                .child(diagonal(across, down))
                .children(across.map(|clue| clue.text(size).right(pad).top(pad)))
                .children(down.map(|clue| clue.text(size).left(pad).bottom(pad)))
        })
        .into_any_element()
}

fn diagonal(across: Option<Clue>, down: Option<Clue>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let tl = bounds.origin;
            let tr = tl + point(bounds.size.width, px(0.));
            let bl = tl + point(px(0.), bounds.size.height);
            let br = tl + point(bounds.size.width, bounds.size.height);
            for (corner, clue) in [(tr, across), (bl, down)] {
                if let Some(clue) = clue {
                    let mut triangle = PathBuilder::fill();
                    triangle.move_to(tl);
                    triangle.line_to(corner);
                    triangle.line_to(br);
                    triangle.close();
                    if let Ok(path) = triangle.build() {
                        window.paint_path(path, rgb(clue.fill));
                    }
                }
            }
            let mut stroke = PathBuilder::stroke(px(1.));
            stroke.move_to(tl);
            stroke.line_to(br);
            if let Ok(path) = stroke.build() {
                window.paint_path(path, rgb(DIVIDER));
            }
        },
    )
    .absolute()
    .inset_0()
}

impl GameView {
    pub(super) fn focused_run(&self, direction: Direction) -> (usize, &Run) {
        let puzzle = self.round.game.puzzle();
        let index = puzzle
            .run_at(self.round.selected, direction)
            .expect("the selected Kakuro cell has two runs");
        (index, puzzle.run(index))
    }
}
