use std::time::{Duration, Instant};

use corner_table_match_three::{CELLS, Cascade, Cell, Gem, Progress, Turn};
use gpui::{
    Div, Img, IntoElement, ObjectFit, ParentElement, PathBuilder, Styled, StyledImage, Window,
    canvas, div, fill, img, linear_color_stop, linear_gradient, point, prelude::FluentBuilder, px,
    rgb, size,
};

const FIELD: u32 = 0x132a27;
const WELL: [u32; 2] = [0x24423a, 0x203e37];
const LIP: u32 = 0x2d4d43;
const RIM_DARK: u32 = 0x675639;
const RIM_LIGHT: u32 = 0xad8f56;
pub const FRAME: f32 = 10.;

pub fn jewel(gem: Gem) -> Img {
    let path = match gem {
        Gem::Ruby => "games/match-three/ruby.png",
        Gem::Amber => "games/match-three/amber.png",
        Gem::Peridot => "games/match-three/peridot.png",
        Gem::Aquamarine => "games/match-three/aquamarine.png",
        Gem::Amethyst => "games/match-three/amethyst.png",
        Gem::Moonstone => "games/match-three/moonstone.png",
    };
    img(path).object_fit(ObjectFit::Contain)
}

fn gem_light(gem: Gem) -> u32 {
    match gem {
        Gem::Ruby => 0xff9d99,
        Gem::Amber => 0xffd673,
        Gem::Peridot => 0xc9f075,
        Gem::Aquamarine => 0x92f5e8,
        Gem::Amethyst => 0xe6b9ff,
        Gem::Moonstone => 0xfaf3df,
    }
}

pub fn frame(focused: bool, complete: bool) -> Div {
    div()
        .relative()
        .flex_none()
        .p(px(FRAME - 1.))
        .rounded(px(12.))
        .border_1()
        .border_color(rgb(if focused || complete {
            RIM_LIGHT
        } else {
            RIM_DARK
        }))
        .bg(linear_gradient(
            145.,
            linear_color_stop(rgb(0x3a5143), 0.),
            linear_color_stop(rgb(0x151f20), 1.),
        ))
        .child(corner(true, true))
        .child(corner(true, false))
        .child(corner(false, true))
        .child(corner(false, false))
}

fn corner(top: bool, left: bool) -> Div {
    let pin = div()
        .absolute()
        .size(px(3.))
        .rounded_full()
        .bg(rgb(RIM_LIGHT));
    let pin = if top {
        pin.top(px(4.))
    } else {
        pin.bottom(px(4.))
    };
    if left {
        pin.left(px(4.))
    } else {
        pin.right(px(4.))
    }
}

pub fn well(step: f32, cell: Cell, inlay_opacity: f32) -> Div {
    let fill = WELL[(cell.row + cell.col) % WELL.len()];
    div()
        .absolute()
        .left(px(cell.col as f32 * step + 1.))
        .top(px(cell.row as f32 * step + 1.))
        .size(px(step - 2.))
        .rounded(px((step * 0.15).clamp(4., 9.)))
        .border_1()
        .border_color(rgb(LIP))
        .bg(rgb(fill))
        .when(inlay_opacity > 0., |well| {
            well.child(inlay(step - 4.).opacity(inlay_opacity))
        })
}

pub fn inlay(edge: f32) -> Div {
    let corner = (edge * 0.23).clamp(4., 13.);
    div()
        .relative()
        .size(px(edge))
        .rounded(px((edge * 0.13).clamp(3., 7.)))
        .bg(rgb(0x285553))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    let inset = px((edge * 0.08).clamp(2.5, 5.));
                    let far = bounds.size.width - inset;
                    let corner = px(corner);
                    let bevel = corner * 0.48;
                    let mut rim = PathBuilder::stroke(px((edge * 0.043).clamp(1.3, 2.5)));
                    for (x, y, dx, dy) in [
                        (inset, inset, 1., 1.),
                        (far, inset, -1., 1.),
                        (inset, far, 1., -1.),
                        (far, far, -1., -1.),
                    ] {
                        let origin = bounds.origin + point(x, y);
                        rim.move_to(origin + point(px(0.), corner * dy));
                        rim.line_to(origin + point(px(0.), bevel * dy));
                        rim.line_to(origin + point(bevel * dx, px(0.)));
                        rim.line_to(origin + point(corner * dx, px(0.)));
                    }
                    if let Ok(path) = rim.build() {
                        window.paint_path(path, rgb(0xaeefdb));
                    }
                },
            )
            .absolute()
            .size_full(),
        )
}

pub fn floor() -> Div {
    div()
        .relative()
        .flex_none()
        .overflow_hidden()
        .bg(rgb(FIELD))
}

#[derive(Clone, Copy, Debug)]
pub struct Exchange {
    pub first: Cell,
    pub second: Cell,
    pub fraction: f32,
}

#[derive(Clone, Copy)]
pub enum Scene<'a> {
    Still(&'a [Gem; CELLS]),
    Exchange(&'a [Gem; CELLS], Exchange),
    Clear(&'a Cascade, f32),
    Fall(&'a Cascade, f32),
    Shuffle(&'a [Gem; CELLS], f32),
    Celebrate(&'a [Gem; CELLS], f32),
}

impl Scene<'_> {
    pub fn inlay_opacity(self, cell: Cell, progress: Progress) -> f32 {
        match self {
            Self::Clear(cascade, fraction) if cascade.cleared_marks.contains(cell) => 1. - fraction,
            _ if progress.remaining_marks.contains(cell) => 1.,
            _ => 0.,
        }
    }

    pub fn board(self) -> [Gem; CELLS] {
        match self {
            Self::Still(board)
            | Self::Exchange(board, _)
            | Self::Shuffle(board, _)
            | Self::Celebrate(board, _) => *board,
            Self::Clear(cascade, _) => cascade.before,
            Self::Fall(cascade, _) => cascade.after,
        }
    }

    pub fn sprites(self, step: f32) -> Div {
        let gems = self.board();
        div()
            .absolute()
            .size_full()
            .children(gems.into_iter().enumerate().filter_map(|(index, gem)| {
                let cell = Cell::from_index(index);
                let mut x = cell.col as f32;
                let mut y = cell.row as f32;
                let mut scale = 1.;
                let mut opacity = 1.;
                match self {
                    Self::Exchange(_, exchange) if cell == exchange.first => {
                        x += (exchange.second.col as f32 - x) * exchange.fraction;
                        y += (exchange.second.row as f32 - y) * exchange.fraction;
                        scale += 0.10 * (std::f32::consts::PI * exchange.fraction).sin();
                    }
                    Self::Exchange(_, exchange) if cell == exchange.second => {
                        x += (exchange.first.col as f32 - x) * exchange.fraction;
                        y += (exchange.first.row as f32 - y) * exchange.fraction;
                    }
                    Self::Clear(cascade, progress) if cascade.cleared[index] => {
                        scale += 0.16 * (progress * std::f32::consts::PI).sin();
                        scale *= 1. - 0.42 * progress.powi(3);
                        opacity = 1. - progress.powi(2);
                    }
                    Self::Fall(cascade, progress) => {
                        let distance = f32::from(cascade.fall_distance[index]);
                        y -= distance * (1. - progress);
                    }
                    Self::Shuffle(_, fraction) => {
                        scale = 0.84 + 0.16 * fraction;
                        opacity = fraction;
                    }
                    _ => {}
                }
                if y < -1. || opacity < 0.01 {
                    return None;
                }
                let gem_size = step * 0.94 * scale;
                let offset = (step - gem_size) * 0.5;
                Some(
                    div()
                        .absolute()
                        .left(px(x * step + offset))
                        .top(px(y * step + offset))
                        .size(px(gem_size))
                        .opacity(opacity)
                        .child(jewel(gem).size_full()),
                )
            }))
    }

    pub fn sparks(self, step: f32) -> Option<impl IntoElement> {
        let (bursts, fraction, count) = match self {
            Self::Clear(cascade, fraction) => (
                cascade
                    .cleared
                    .iter()
                    .zip(cascade.before)
                    .enumerate()
                    .filter(|(_, (cleared, _))| **cleared)
                    .map(|(index, (_, gem))| (Cell::from_index(index), gem_light(gem)))
                    .collect::<Vec<_>>(),
                fraction,
                5,
            ),
            Self::Celebrate(_, fraction) => (
                [(0, 1), (0, 6), (2, 3), (4, 0), (5, 7), (7, 2), (7, 6)]
                    .map(|(row, col)| (Cell { row, col }, 0xffdb8a))
                    .to_vec(),
                fraction,
                7,
            ),
            _ => return None,
        };
        Some(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    for (cell, color) in bursts {
                        let center = bounds.origin
                            + point(
                                px((cell.col as f32 + 0.5) * step),
                                px((cell.row as f32 + 0.5) * step),
                            );
                        render_sparks(window, center, cell, color, step, fraction, count);
                    }
                },
            )
            .absolute()
            .size_full(),
        )
    }
}

fn render_sparks(
    window: &mut Window,
    center: gpui::Point<gpui::Pixels>,
    cell: Cell,
    color: u32,
    step: f32,
    fraction: f32,
    count: usize,
) {
    let alpha = (fraction * 6.).min(1.) * (1. - fraction);
    let base_angle = (cell.col + cell.row * 2) as f32 * 0.19;
    for index in 0..count {
        let angle = base_angle + index as f32 * std::f32::consts::TAU / count as f32;
        let radius = step * (0.18 + 0.49 * ease_out(fraction)) * (0.8 + 0.13 * (index % 3) as f32);
        let center = center + point(px(angle.cos() * radius), px(angle.sin() * radius));
        let length = px((step * 0.11).clamp(3., 7.) * (0.3 + alpha));
        if index % 2 == 0 {
            let mut spark = PathBuilder::fill();
            spark.move_to(center - point(px(0.), length));
            spark.line_to(center + point(length * 0.4, px(0.)));
            spark.line_to(center + point(px(0.), length));
            spark.line_to(center - point(length * 0.4, px(0.)));
            spark.close();
            if let Ok(path) = spark.build() {
                window.paint_path(path, rgb(color).opacity(alpha));
            }
        } else {
            let dot = length * 0.7;
            window.paint_quad(
                fill(
                    gpui::Bounds::new(center - point(dot / 2., dot / 2.), size(dot, dot)),
                    rgb(color).opacity(alpha),
                )
                .corner_radii(dot / 2.),
            );
        }
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Swap,
    Return,
    Clear(usize),
    Fall(usize),
    Shuffle,
    Celebrate,
}

pub struct Motion {
    original: [Gem; CELLS],
    exchange: Exchange,
    progress: Progress,
    turn: Option<Turn>,
    won: bool,
    phase: Phase,
    started: Instant,
}

impl Motion {
    pub fn new(
        exchange: Exchange,
        original: [Gem; CELLS],
        progress: Progress,
        turn: Option<Turn>,
        won: bool,
    ) -> Self {
        Self {
            original,
            exchange,
            progress,
            turn,
            won,
            phase: Phase::Swap,
            started: Instant::now(),
        }
    }

    fn duration(&self) -> Duration {
        let millis = match self.phase {
            Phase::Swap => (125. - 85. * self.exchange.fraction) as u64,
            Phase::Return => 155,
            Phase::Clear(index) => {
                if index == 0 {
                    210
                } else {
                    190
                }
            }
            Phase::Fall(index) => {
                let flow = &self.cascades()[index];
                185 + 15 * u64::from(flow.fall_distance.iter().copied().max().unwrap_or(0))
            }
            Phase::Shuffle => 330,
            Phase::Celebrate => 580,
        };
        Duration::from_millis(millis)
    }

    fn cascades(&self) -> &[Cascade] {
        self.turn.as_ref().map_or(&[], |turn| &turn.cascades)
    }

    pub fn advance(&mut self, now: Instant) -> bool {
        while now.duration_since(self.started) >= self.duration() {
            let next = match self.phase {
                Phase::Swap if self.turn.is_none() => Phase::Return,
                Phase::Swap => Phase::Clear(0),
                Phase::Clear(index) => Phase::Fall(index),
                Phase::Fall(index) if index + 1 < self.cascades().len() => Phase::Clear(index + 1),
                Phase::Fall(_) if self.turn.as_ref().is_some_and(|turn| turn.reshuffled) => {
                    Phase::Shuffle
                }
                Phase::Fall(_) if self.won => Phase::Celebrate,
                _ => return false,
            };
            self.started += self.duration();
            self.phase = next;
        }
        true
    }

    pub fn scene<'a>(&'a self, now: Instant, settled: &'a [Gem; CELLS]) -> Scene<'a> {
        let fraction = (now.duration_since(self.started).as_secs_f32()
            / self.duration().as_secs_f32())
        .clamp(0., 1.);
        match self.phase {
            Phase::Swap => Scene::Exchange(
                &self.original,
                Exchange {
                    fraction: self.exchange.fraction
                        + (1. - self.exchange.fraction) * ease_out(fraction),
                    ..self.exchange
                },
            ),
            Phase::Return => Scene::Exchange(
                &self.original,
                Exchange {
                    fraction: 1. - ease_out(fraction),
                    ..self.exchange
                },
            ),
            Phase::Clear(index) => Scene::Clear(&self.cascades()[index], fraction),
            Phase::Fall(index) => Scene::Fall(&self.cascades()[index], fraction.powi(2)),
            Phase::Shuffle if fraction < 0.42 => {
                Scene::Shuffle(&self.cascades().last().unwrap().after, 1. - fraction / 0.42)
            }
            Phase::Shuffle => Scene::Shuffle(settled, (fraction - 0.42) / 0.58),
            Phase::Celebrate => Scene::Celebrate(settled, fraction),
        }
    }

    pub fn progress(&self) -> Progress {
        match self.phase {
            Phase::Swap | Phase::Return => self.progress,
            Phase::Clear(index) | Phase::Fall(index) => self.cascades()[index].progress,
            Phase::Shuffle | Phase::Celebrate => self.cascades().last().unwrap().progress,
        }
    }

    pub fn clear_feedback(&self) -> Option<(usize, u32)> {
        match self.phase {
            Phase::Clear(index) | Phase::Fall(index) => {
                Some((index + 1, self.cascades()[index].points))
            }
            _ => None,
        }
    }

    pub fn is_celebrating(&self) -> bool {
        matches!(self.phase, Phase::Celebrate)
    }
}

fn ease_out(fraction: f32) -> f32 {
    1. - (1. - fraction).powi(3)
}
