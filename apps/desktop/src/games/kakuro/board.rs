use std::sync::{Arc, OnceLock};

use corner_table_kakuro::{Cell, Direction, Game, Run};
use gpui::{
    Animation, AnimationExt, AnyElement, Context, Div, FontWeight, Image, ImageFormat, IntoElement,
    ObjectFit, PathBuilder, RenderImage, Role, Stateful, StyledImage, canvas, div, ease_in_out,
    img, linear_color_stop, linear_gradient, point, prelude::*, px, rgb, rgba,
};

use super::{GameView, RUN_PULSE, RunPulse};
use crate::{motion, theme};

pub(super) const FRAME: f32 = 2.;
pub(super) const GRID: f32 = 1.;

#[derive(Clone, Copy)]
struct BoardInk {
    edge: u32,
    rule: u32,
    block: u32,
    divider: u32,
    clue: (u32, u32),
    stock: (u32, u32),
    entry: u32,
    pencil: u32,
    across: u32,
    across_clue: (u32, u32),
    down: u32,
    down_clue: (u32, u32),
    focus: (u32, u32),
    hint: (u32, u32),
    error: (u32, u32),
    error_clue: (u32, u32),
}

const LIGHT: BoardInk = BoardInk {
    edge: 0x473b31,
    rule: 0xaf9e89,
    block: 0x33302d,
    divider: 0xa49a8b,
    clue: (0x48433d, 0xfff3da),
    stock: (0xfff8e9, 0xf1e2c7),
    entry: 0x3d3028,
    pencil: 0x554638,
    across: 0xf7e3b6,
    across_clue: (0x6b5028, 0xffe9b7),
    down: 0xdeeadb,
    down_clue: (0x30574a, 0xd6f4db),
    focus: (0xd8ead0, 0x305944),
    hint: (0xffe0a0, 0x72480f),
    error: (0xf8dcd3, 0x8b3128),
    error_clue: (0x76382e, 0xffe4d9),
};

const DARK: BoardInk = BoardInk {
    edge: 0xa49582,
    rule: 0x6b5c4e,
    block: 0x27231f,
    divider: 0x8f8378,
    clue: (0x38312c, 0xffeed1),
    stock: (0x4e4237, 0x3c332d),
    entry: 0xf9ead0,
    pencil: 0xe1cbb1,
    across: 0x58472f,
    across_clue: (0x695026, 0xffe9b1),
    down: 0x374b40,
    down_clue: (0x315849, 0xd6f4df),
    focus: (0x3e5a49, 0xafe1bc),
    hint: (0x59411e, 0xffd18a),
    error: (0x59342e, 0xffa99a),
    error_clue: (0x743b32, 0xffdfd4),
};

fn graphite(cx: &gpui::App) -> Arc<RenderImage> {
    static GRAPHITE: OnceLock<Arc<RenderImage>> = OnceLock::new();
    GRAPHITE
        .get_or_init(|| {
            let data = include_bytes!("../../../assets/games/kakuro/graphite.jpg");
            Image::from_bytes(ImageFormat::Jpeg, data.to_vec())
                .to_image_data(cx.svg_renderer())
                .expect("bundled Kakuro graphite texture")
        })
        .clone()
}

struct FocusedRuns {
    across: Option<usize>,
    down: Option<usize>,
}

impl GameView {
    pub(super) fn board(&self, size: f32, cx: &Context<Self>) -> Stateful<Div> {
        let ink = if theme::colors(cx) == theme::LIGHT {
            LIGHT
        } else {
            DARK
        };
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
            .border_color(rgb(ink.edge))
            .bg(rgb(ink.rule))
            .font_family(theme::BODY)
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
                                    Cell::Entry => self.entry(index, size, &focus, pulse, ink, cx),
                                    Cell::Block { across, down } => {
                                        block(game, index, size, (across, down), &focus, ink, cx)
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
        ink: BoardInk,
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
            Some(ink.error.0)
        } else if hinted {
            Some(ink.hint.0)
        } else if selected {
            Some(ink.focus.0)
        } else if !solved && across == focus.across {
            Some(ink.across)
        } else if !solved && down == focus.down {
            Some(ink.down)
        } else {
            None
        };
        let outline = if hinted && !conflict {
            ink.hint.1
        } else {
            ink.focus.1
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
            .bg(background.map_or_else(
                || {
                    linear_gradient(
                        150.,
                        linear_color_stop(rgb(ink.stock.0), 0.),
                        linear_color_stop(rgb(ink.stock.1), 1.),
                    )
                },
                |color| rgb(color).into(),
            ))
            .text_color(rgb(if conflict { ink.error.1 } else { ink.entry }))
            .text_size(px((size * 0.52).round()))
            .line_height(px((size * 0.64).round()))
            .font_weight(FontWeight::SEMIBOLD)
            .when(!solved, |cell| {
                cell.cursor_pointer()
                    .when(!selected && !conflict, |cell| {
                        cell.hover(move |style| style.bg(rgb(ink.focus.0)))
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
            .when(notes != 0, |cell| {
                cell.child(pencil_grid(size, notes, ink.pencil))
            })
            .when(conflict, |cell| {
                cell.child(
                    div()
                        .absolute()
                        .bottom(px(size * 0.13))
                        .w(px(size * 0.27))
                        .h(px(2.))
                        .bg(rgb(ink.error.1)),
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

fn pencil_grid(size: f32, notes: u16, ink: u32) -> Div {
    let unit = px((size * 0.29).round());
    div()
        .flex()
        .flex_col()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px((size * 0.21).round().max(9.)))
        .line_height(unit)
        .text_color(rgb(ink))
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
    fn new(game: &Game, run: usize, active: bool, direction: Direction, paint: BoardInk) -> Self {
        let sum = game.puzzle().run(run).sum;
        let (fill, ink) = if game.run_conflict(run).is_some() {
            paint.error_clue
        } else if active {
            match direction {
                Direction::Across => paint.across_clue,
                Direction::Down => paint.down_clue,
            }
        } else {
            paint.clue
        };
        Self { sum, fill, ink }
    }

    fn text(self, size: f32) -> Div {
        div()
            .absolute()
            .font_weight(FontWeight::BOLD)
            .text_size(px((size * 0.29).round().clamp(11., 19.)))
            .line_height(px((size * 0.34).round().max(13.)))
            .text_color(rgb(self.ink))
            .child(self.sum.to_string())
    }
}

fn block(
    game: &Game,
    index: usize,
    size: f32,
    (across, down): (Option<usize>, Option<usize>),
    focus: &FocusedRuns,
    paint: BoardInk,
    cx: &Context<GameView>,
) -> AnyElement {
    let show_focus = !game.is_solved();
    let across = across.map(|run| {
        Clue::new(
            game,
            run,
            show_focus && Some(run) == focus.across,
            Direction::Across,
            paint,
        )
    });
    let down = down.map(|run| {
        Clue::new(
            game,
            run,
            show_focus && Some(run) == focus.down,
            Direction::Down,
            paint,
        )
    });
    let pad = px((size * 0.082).round().max(3.));
    div()
        .id(("kakuro-clue", index))
        .relative()
        .size(px(size))
        .flex_shrink_0()
        .bg(rgb(paint.block))
        .child(
            img(graphite(cx))
                .absolute()
                .inset_0()
                .w_full()
                .h_full()
                .object_fit(ObjectFit::Fill)
                .opacity(0.26),
        )
        .when(across.is_some() || down.is_some(), |cell| {
            let label = match (across, down) {
                (Some(a), Some(d)) => format!("{} across, {} down", a.sum, d.sum),
                (Some(a), None) => format!("{} across", a.sum),
                (None, Some(d)) => format!("{} down", d.sum),
                (None, None) => unreachable!(),
            };
            cell.role(Role::Cell)
                .aria_label(label)
                .child(diagonal(across, down, paint.divider))
                .children(across.map(|clue| clue.text(size).right(pad).top(pad)))
                .children(down.map(|clue| clue.text(size).left(pad).bottom(pad)))
        })
        .into_any_element()
}

fn diagonal(across: Option<Clue>, down: Option<Clue>, divider: u32) -> impl IntoElement {
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
                        window.paint_path(path, rgb(clue.fill).opacity(0.87));
                    }
                }
            }
            let mut stroke = PathBuilder::stroke(px(1.));
            stroke.move_to(tl);
            stroke.line_to(br);
            if let Ok(path) = stroke.build() {
                window.paint_path(path, rgb(divider));
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
