use std::{f32::consts::TAU, time::Instant};

use corner_table_mahjong_cubes::{Board, Face, Position, Suit};
use gpui::{
    Background, Bounds, PathBuilder, Pixels, Point, Window, linear_color_stop, linear_gradient,
    point, px, rgb, size,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Orientation {
    #[default]
    SouthEast,
    NorthEast,
    NorthWest,
    SouthWest,
}

impl Orientation {
    const ALL: [Self; 4] = [
        Self::SouthEast,
        Self::NorthEast,
        Self::NorthWest,
        Self::SouthWest,
    ];

    pub fn turn(self, quarters: i32) -> Self {
        Self::ALL[(self as i32 + quarters).rem_euclid(4) as usize]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SouthEast => "SE",
            Self::NorthEast => "NE",
            Self::NorthWest => "NW",
            Self::SouthWest => "SW",
        }
    }

    fn rotate(self, x: f32, y: f32) -> (f32, f32) {
        match self {
            Self::SouthEast => (x, y),
            Self::NorthEast => (-y, x),
            Self::NorthWest => (-x, -y),
            Self::SouthWest => (y, -x),
        }
    }

    fn depth(self, position: Position) -> i16 {
        let (u, v) = self.rotate(f32::from(position.x), f32::from(position.y));
        // Along every ray toward the camera, u, v and z increase. Any two
        // overlapping lattice cubes therefore have this same depth order.
        (u + v) as i16 + i16::from(position.level)
    }
}

type Quad = [Point<Pixels>; 4];

fn project(u: f32, v: f32, z: f32) -> Point<Pixels> {
    point(px((u - v) * 0.866), px((u + v) * 0.55 - z * 0.77))
}

fn cube_faces(position: Position, orientation: Orientation) -> [Quad; 3] {
    let (u, v) = orientation.rotate(f32::from(position.x) + 0.5, f32::from(position.y) + 0.5);
    let base = f32::from(position.level);
    let top = base + 0.94;
    let right = project(u + 0.47, v - 0.47, top);
    let front = project(u + 0.47, v + 0.47, top);
    let left = project(u - 0.47, v + 0.47, top);
    let back = project(u - 0.47, v - 0.47, top);
    let right_base = project(u + 0.47, v - 0.47, base);
    let front_base = project(u + 0.47, v + 0.47, base);
    let left_base = project(u - 0.47, v + 0.47, base);
    [
        [left, front, front_base, left_base],
        [front, right, right_base, front_base],
        [back, right, front, left],
    ]
}

fn ground(position: Position, orientation: Orientation) -> Quad {
    let (u, v) = orientation.rotate(f32::from(position.x) + 0.5, f32::from(position.y) + 0.5);
    [(-0.43, -0.43), (0.43, -0.43), (0.43, 0.43), (-0.43, 0.43)]
        .map(|(du, dv)| project(u + du, v + dv, 0.))
}

pub(super) struct ProjectedCube {
    pub id: usize,
    pub free: bool,
    face: Face,
    quads: [Quad; 3],
}

#[derive(Clone, Copy)]
pub(super) struct FadingPair {
    pub ids: [usize; 2],
    pub at: Instant,
}

#[derive(Clone, Copy)]
pub(super) struct TileStates {
    pub selected: Option<usize>,
    pub hinted: Option<[usize; 2]>,
    pub keyed: Option<usize>,
    pub fading: Option<FadingPair>,
    pub pressed: Option<usize>,
}

pub(super) struct Scene {
    bounds: Bounds<Pixels>,
    footprint: Vec<(usize, Quad)>,
    original: Vec<[Quad; 3]>,
    cubes: Vec<ProjectedCube>,
}

pub(super) struct Hint {
    pub orientation: Orientation,
    pub pair: [usize; 2],
    pub visible: [bool; 2],
}

impl Hint {
    pub fn target(&self, selected: Option<usize>) -> Option<usize> {
        self.pair
            .into_iter()
            .zip(self.visible)
            .find(|&(id, visible)| visible && selected != Some(id))
            .map(|(id, _)| id)
    }
}

impl Scene {
    pub fn new(board: &Board, bounds: Bounds<Pixels>, orientation: Orientation) -> Self {
        // Always fit the full layout so removing tiles never moves or zooms the stack.
        let original: Vec<_> = board
            .layout()
            .map(|pos| cube_faces(pos, orientation))
            .collect();
        let mut min = point(px(f32::INFINITY), px(f32::INFINITY));
        let mut max = point(px(f32::NEG_INFINITY), px(f32::NEG_INFINITY));
        for vertex in original.iter().flatten().flatten() {
            min = point(min.x.min(vertex.x), min.y.min(vertex.y));
            max = point(max.x.max(vertex.x), max.y.max(vertex.y));
        }
        let stack = Bounds::from_corners(min, max);
        let vertical_padding = px(if bounds.size.height < px(330.) {
            32.
        } else {
            40.
        });
        let scale = ((bounds.size.width - px(42.)) / stack.size.width)
            .min((bounds.size.height - vertical_padding) / stack.size.height)
            .clamp(0.1, 108.);
        let fit = |vertex| bounds.center() + (vertex - stack.center()) * scale;
        let original: Vec<_> = original
            .into_iter()
            .map(|faces| faces.map(|quad| quad.map(fit)))
            .collect();
        let footprint = board
            .layout()
            .enumerate()
            .filter(|(_, pos)| pos.level == 0)
            .map(|(id, pos)| (id, ground(pos, orientation).map(fit)))
            .collect();
        let mut active: Vec<_> = board.cubes().collect();
        active.sort_by_key(|(_, cube)| orientation.depth(cube.position));
        let cubes = active
            .into_iter()
            .map(|(id, cube)| ProjectedCube {
                id,
                face: cube.face,
                free: board.is_free(id),
                quads: original[id],
            })
            .collect();
        Self {
            bounds,
            footprint,
            original,
            cubes,
        }
    }

    pub fn hit(&self, position: Point<Pixels>) -> Option<&ProjectedCube> {
        // The same polygons and reverse paint order keep blocked front faces
        // from sending clicks through to any deeper cube.
        self.cubes
            .iter()
            .rev()
            .find(|cube| cube.quads.iter().rev().any(|quad| contains(quad, position)))
    }

    fn visible_point(&self, cube: &ProjectedCube) -> Option<Point<Pixels>> {
        const SAMPLES: [(f32, f32); 9] = [
            (0.50, 0.50),
            (0.29, 0.31),
            (0.71, 0.31),
            (0.29, 0.69),
            (0.71, 0.69),
            (0.50, 0.15),
            (0.85, 0.50),
            (0.50, 0.85),
            (0.15, 0.50),
        ];
        cube.quads.iter().rev().find_map(|quad| {
            SAMPLES.into_iter().find_map(|(u, v)| {
                let sample = on_quad(quad, u, v);
                self.hit(sample)
                    .is_some_and(|hit| hit.id == cube.id)
                    .then_some(sample)
            })
        })
    }

    fn for_view(board: &Board, orientation: Orientation) -> Self {
        Self::new(
            board,
            Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(1000.), px(700.)),
            },
            orientation,
        )
    }

    pub fn hint(board: &Board, current: Orientation) -> Option<Hint> {
        let mut pairs = board.available_pairs();
        let selected = board
            .selected()
            .filter(|id| pairs.iter().any(|pair| pair.contains(id)));
        if let Some(selected) = selected {
            pairs.retain(|pair| pair.contains(&selected));
        }
        let mut fallback = Hint {
            orientation: current,
            pair: *pairs.first()?,
            visible: [false; 2],
        };
        for offset in [0, 1, -1, 2] {
            let orientation = current.turn(offset);
            let scene = Self::for_view(board, orientation);
            for &pair in &pairs {
                let visible = pair.map(|id| {
                    scene
                        .cubes
                        .iter()
                        .find(|cube| cube.id == id)
                        .and_then(|cube| scene.visible_point(cube))
                        .is_some()
                });
                let hint = Hint {
                    orientation,
                    pair,
                    visible,
                };
                if visible == [true; 2] {
                    return Some(hint);
                }
                if fallback.target(selected).is_none() && hint.target(selected).is_some() {
                    fallback = hint;
                }
            }
        }
        Some(fallback)
    }

    pub fn keyboard_target(
        board: &Board,
        orientation: Orientation,
        focused: Option<usize>,
        step: Point<f32>,
    ) -> Option<usize> {
        let scene = Self::for_view(board, orientation);
        let targets: Vec<_> = scene
            .cubes
            .iter()
            .filter(|cube| cube.free)
            .filter_map(|cube| scene.visible_point(cube).map(|point| (cube.id, point)))
            .collect();
        let Some(&(_, origin)) = targets.iter().find(|(id, _)| Some(*id) == focused) else {
            return targets
                .iter()
                .min_by_key(|(_, position)| position.y)
                .map(|(id, _)| *id);
        };
        targets
            .into_iter()
            .filter_map(|(id, position)| {
                let dx = f32::from(position.x - origin.x);
                let dy = f32::from(position.y - origin.y);
                let forward = dx * step.x + dy * step.y;
                let across = dx * step.y - dy * step.x;
                (forward > 1.).then_some((id, forward.hypot(across) + across.abs()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
            .or(focused)
    }

    pub fn paint(&self, states: TileStates, hovered: Option<usize>, window: &mut Window) {
        paint_felt(self.bounds, window);

        let mut wells = PathBuilder::fill();
        let mut rims = PathBuilder::stroke(px(0.9));
        let mut shadows = PathBuilder::fill();
        let mut contact = PathBuilder::fill();
        for (id, quad) in &self.footprint {
            let center = on_quad(quad, 0.5, 0.5);
            add_polygon(&mut wells, quad);
            add_polygon(&mut rims, &quad.map(|p| center + (p - center) * 0.92));
            if self.cubes.iter().any(|cube| cube.id == *id) {
                add_polygon(
                    &mut shadows,
                    &quad.map(|p| center + (p - center) * 1.14 + point(px(2.), px(5.))),
                );
                add_polygon(&mut contact, &quad.map(|p| p + point(px(1.), px(2.5))));
            }
        }
        paint(window, wells, rgb(0x0d2b21).opacity(0.48));
        paint(window, rims, rgb(0x98a779).opacity(0.28));
        paint(window, shadows, rgb(0x061b15).opacity(0.28));
        paint(window, contact, rgb(0x091a13).opacity(0.52));

        if let Some(pair) = states.fading {
            let progress = pair.at.elapsed().as_secs_f32() / 0.24;
            if progress < 1. {
                let mut edges = PathBuilder::stroke(px(1.4));
                for id in pair.ids {
                    let top = &self.original[id][2];
                    let center = on_quad(top, 0.5, 0.5);
                    let outline = top.map(|vertex| {
                        center + (vertex - center) * (1. - progress * 0.14)
                            - point(px(0.), px(8. * progress))
                    });
                    add_polygon(&mut edges, &outline);
                }
                paint(window, edges, rgb(0xffdd94).opacity(0.82 * (1. - progress)));
                window.request_animation_frame();
            }
        }
        for cube in &self.cubes {
            cube.paint(states, hovered == Some(cube.id), window);
        }
    }
}

fn paint_felt(bounds: Bounds<Pixels>, window: &mut Window) {
    let width = f32::from(bounds.size.width) as u32;
    let height = f32::from(bounds.size.height) as u32;
    let start = bounds.origin;
    let mut nap = PathBuilder::fill();
    let mut weave = PathBuilder::fill();
    for row in (8..height.saturating_sub(8)).step_by(9) {
        for col in (8..width.saturating_sub(8)).step_by(11) {
            let hash = (row * 233 + col * 941).wrapping_mul(0x9e37_79b9);
            let x = col as f32 + ((hash >> 8) & 7) as f32;
            let y = row as f32 + ((hash >> 20) & 7) as f32;
            let fiber = [
                (x, y),
                (x + 2., y - 0.6),
                (x + 3.2, y - 0.4),
                (x + 0.8, y + 0.55),
            ]
            .map(|(x, y)| start + point(px(x), px(y)));
            add_polygon(if hash & 1 == 0 { &mut nap } else { &mut weave }, &fiber);
        }
    }
    paint(window, weave, rgb(0x060f09).opacity(0.16));
    paint(window, nap, rgb(0xdde6a1).opacity(0.12));

    let (w, h) = (width as f32, height as f32);
    let mut inlay = PathBuilder::stroke(px(0.8));
    let border = [
        (16., 9.),
        (w - 16., 9.),
        (w - 9., 16.),
        (w - 9., h - 16.),
        (w - 16., h - 9.),
        (16., h - 9.),
        (9., h - 16.),
        (9., 16.),
    ]
    .map(|(x, y)| start + point(px(x), px(y)));
    add_polygon(&mut inlay, &border);
    paint(window, inlay, rgb(0xccb77a).opacity(0.27));
}

impl ProjectedCube {
    fn paint(&self, states: TileStates, hovered: bool, window: &mut Window) {
        let selected = states.selected == Some(self.id);
        let hinted = states.hinted.is_some_and(|ids| ids.contains(&self.id));
        let keyed = states.keyed == Some(self.id);
        let pressed = states.pressed == Some(self.id);
        let enamel = enamel(self.face.suit);
        let rim = if selected {
            0x90601d
        } else if hinted {
            0x986d25
        } else if self.free {
            0x74654c
        } else {
            0x64593f
        };
        let ivory = [0xb29f7c, 0xd8c7a5, 0xf6ebd0];
        for (index, quad) in self.quads.iter().enumerate() {
            let glaze = mix(ivory[index], enamel.tint, [0.23, 0.32, 0.48][index]);
            let glaze = if selected {
                mix(glaze, 0xffcc65, 0.52)
            } else if !self.free || pressed {
                mix(glaze, 0x8a8168, if pressed { 0.24 } else { 0.28 })
            } else if hovered {
                mix(glaze, 0xfffae6, 0.45)
            } else {
                glaze
            };
            polygon(window, quad, rgb(rim));
            let border = if selected || hinted { 0.058 } else { 0.027 };
            let lip = chamfer(quad, 0.014, 0.04);
            let edge = if hinted || selected {
                [0xe3aa42, 0xf4c560, 0xffde85][index]
            } else {
                mix(glaze, 0xffffe9, [0.26, 0.40, 0.65][index])
            };
            polygon(window, &lip, rgb(edge));
            let body = chamfer(quad, border, 0.045);
            polygon(
                window,
                &body,
                linear_gradient(
                    155.,
                    linear_color_stop(rgb(mix(glaze, 0xfff6dd, 0.21)), 0.),
                    linear_color_stop(rgb(mix(glaze, 0x7a6549, 0.16)), 1.),
                ),
            );
            let lower_bevel = [(0.09, 0.89), (0.91, 0.89), (0.94, 0.93), (0.07, 0.93)]
                .map(|(u, v)| on_quad(quad, u, v));
            polygon(
                window,
                &lower_bevel,
                rgb(0x69533b).opacity(if index == 2 { 0.09 } else { 0.15 }),
            );
            paint_marks(window, quad, self.face, enamel, glaze);
        }
        if keyed {
            let mut focus = PathBuilder::stroke(px(1.8));
            for quad in &self.quads {
                add_polygon(&mut focus, &chamfer(quad, 0.09, 0.05));
            }
            paint(window, focus, rgb(0x207e68));
        }
    }
}

#[derive(Clone, Copy)]
struct Enamel {
    tint: u32,
    ink: u32,
}

fn enamel(suit: Suit) -> Enamel {
    let (tint, ink) = match suit {
        Suit::Dots => (0xdceff3, 0x1b526e),
        Suit::Diamonds => (0xf9d8c8, 0x8d3329),
        Suit::Leaves => (0xd9e8c8, 0x27563c),
        Suit::Blossoms => (0xeed9e9, 0x7e3b66),
        Suit::Waves => (0xcce9dc, 0x236557),
        Suit::Suns => (0xfbe3ac, 0x8a4a0e),
    };
    Enamel { tint, ink }
}

pub(super) fn suit_name(suit: Suit) -> &'static str {
    match suit {
        Suit::Dots => "dots",
        Suit::Diamonds => "diamonds",
        Suit::Leaves => "leaves",
        Suit::Blossoms => "blossoms",
        Suit::Waves => "waves",
        Suit::Suns => "suns",
    }
}

fn paint_marks(window: &mut Window, quad: &Quad, face: Face, enamel: Enamel, glaze: u32) {
    let corners = [(0.28, 0.28), (0.72, 0.72), (0.28, 0.72), (0.72, 0.28)];
    let corner_count = match face.rank {
        1 => 0,
        2 | 3 => 2,
        _ => 4,
    };
    let centers = corners
        .into_iter()
        .take(corner_count)
        .chain((face.rank % 2 == 1).then_some((0.5, 0.5)));
    let radius = if face.rank == 1 { 0.23 } else { 0.115 };
    let mut marks = PathBuilder::fill();
    let mut highlights = PathBuilder::fill();
    for (u, v) in centers {
        let stamp = |x, y| on_quad(quad, u + x * radius, v + y * radius);
        match face.suit {
            Suit::Dots => {
                circle(&mut marks, stamp, 0., 0., 0.96);
                circle(&mut highlights, stamp, -0.24, -0.28, 0.27);
            }
            Suit::Diamonds => {
                add_polygon(
                    &mut marks,
                    &[(0., -1.12), (0.96, 0.), (0., 1.12), (-0.96, 0.)].map(|(x, y)| stamp(x, y)),
                );
                add_polygon(
                    &mut highlights,
                    &[(0., -0.78), (-0.63, 0.), (0., 0.08)].map(|(x, y)| stamp(x, y)),
                );
            }
            Suit::Leaves => {
                let leaf = [
                    (-1., 0.96),
                    (-0.88, 0.14),
                    (-0.48, -0.52),
                    (0.19, -0.84),
                    (0.99, -0.98),
                    (0.86, -0.19),
                    (0.44, 0.43),
                    (-0.16, 0.79),
                ];
                add_polygon(&mut marks, &leaf.map(|(x, y)| stamp(x, y)));
                add_polygon(
                    &mut highlights,
                    &[(-0.7, 0.69), (0.64, -0.63), (0.10, 0.14)].map(|(x, y)| stamp(x, y)),
                );
            }
            Suit::Blossoms => {
                let petals = std::array::from_fn::<_, 30, _>(|i| {
                    let angle = TAU * i as f32 / 30.;
                    let radius = 0.80 + 0.20 * (angle * 5.).cos();
                    stamp(radius * angle.sin(), -radius * angle.cos())
                });
                add_polygon(&mut marks, &petals);
                circle(&mut highlights, stamp, 0., 0., 0.25);
            }
            Suit::Waves => {
                for (radius, thickness) in [(1., 0.22), (0.59, 0.20)] {
                    let arch = std::array::from_fn::<_, 18, _>(|i| {
                        let (step, radius) = if i < 9 {
                            (i, radius)
                        } else {
                            (17 - i, radius - thickness)
                        };
                        let angle = std::f32::consts::PI * step as f32 / 8.;
                        stamp(radius * angle.cos(), 0.47 - radius * angle.sin())
                    });
                    add_polygon(&mut marks, &arch);
                }
                circle(&mut highlights, stamp, 0., 0.45, 0.28);
            }
            Suit::Suns => {
                let rays = std::array::from_fn::<_, 16, _>(|i| {
                    let angle = TAU * i as f32 / 16.;
                    let radius = if i % 2 == 0 { 1. } else { 0.57 };
                    stamp(radius * angle.sin(), -radius * angle.cos())
                });
                add_polygon(&mut marks, &rays);
                circle(&mut highlights, stamp, 0., 0., 0.29);
            }
        }
    }
    paint(window, marks, rgb(enamel.ink));
    paint(window, highlights, rgb(mix(enamel.ink, glaze, 0.49)));
}

fn circle(
    path: &mut PathBuilder,
    stamp: impl Fn(f32, f32) -> Point<Pixels>,
    x: f32,
    y: f32,
    radius: f32,
) {
    let ring = std::array::from_fn::<_, 14, _>(|i| {
        let angle = TAU * i as f32 / 14.;
        stamp(x + radius * angle.cos(), y + radius * angle.sin())
    });
    add_polygon(path, &ring);
}

fn on_quad(quad: &Quad, u: f32, v: f32) -> Point<Pixels> {
    quad[0] + (quad[1] - quad[0]) * u + (quad[3] - quad[0]) * v
}

fn chamfer(quad: &Quad, inset: f32, cut: f32) -> [Point<Pixels>; 8] {
    let n = inset;
    let f = 1. - inset;
    [
        (n + cut, n),
        (f - cut, n),
        (f, n + cut),
        (f, f - cut),
        (f - cut, f),
        (n + cut, f),
        (n, f - cut),
        (n, n + cut),
    ]
    .map(|(u, v)| on_quad(quad, u, v))
}

fn contains(quad: &Quad, point: Point<Pixels>) -> bool {
    let mut positive = false;
    let mut negative = false;
    for (start, end) in quad.iter().zip(quad.iter().cycle().skip(1)) {
        let edge = *end - *start;
        let offset = point - *start;
        let cross =
            f32::from(edge.x) * f32::from(offset.y) - f32::from(edge.y) * f32::from(offset.x);
        positive |= cross > 0.;
        negative |= cross < 0.;
    }
    !(positive && negative)
}

fn mix(a: u32, b: u32, weight: f32) -> u32 {
    let channel = |shift: u32| {
        let first = ((a >> shift) & 0xff) as f32;
        let second = ((b >> shift) & 0xff) as f32;
        (first + (second - first) * weight).round() as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

fn add_polygon(path: &mut PathBuilder, vertices: &[Point<Pixels>]) {
    path.move_to(vertices[0]);
    for &vertex in &vertices[1..] {
        path.line_to(vertex);
    }
    path.close();
}

fn polygon(window: &mut Window, vertices: &[Point<Pixels>], color: impl Into<Background>) {
    let mut path = PathBuilder::fill();
    add_polygon(&mut path, vertices);
    paint(window, path, color);
}

fn paint(window: &mut Window, path: PathBuilder, color: impl Into<Background>) {
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corner_table_mahjong_cubes::Layout;

    #[test]
    fn hit_follows_visible_faces_instead_of_cube_bounds_or_eligibility() {
        for orientation in Orientation::ALL {
            let project = |id, position, free| ProjectedCube {
                id,
                free,
                face: Face {
                    suit: Suit::Dots,
                    rank: 1,
                },
                quads: cube_faces(position, orientation),
            };
            // Rotate a known occlusion around the rear cube.
            let (dx, dy) = match orientation {
                Orientation::SouthEast => (1, 1),
                Orientation::NorthEast => (1, -1),
                Orientation::NorthWest => (-1, -1),
                Orientation::SouthWest => (-1, 1),
            };
            let rear = project(
                0,
                Position {
                    x: 1,
                    y: 1,
                    level: 0,
                },
                true,
            );
            let front = project(
                1,
                Position {
                    x: (1 + dx) as u8,
                    y: (1 + dy) as u8,
                    level: 1,
                },
                false,
            );
            let overlap = on_quad(&rear.quads[2], 0.83, 0.83);
            let exposed = on_quad(&rear.quads[2], 0.18, 0.18);
            let outside = on_quad(&rear.quads[2], 0.01, -0.12);
            assert!(contains(&front.quads[2], overlap));

            let scene = Scene {
                bounds: Bounds::default(),
                original: vec![],
                footprint: vec![],
                cubes: vec![rear, front],
            };
            assert_eq!(scene.hit(overlap).map(|cube| cube.id), Some(1));
            assert_eq!(scene.hit(exposed).map(|cube| cube.id), Some(0));
            assert!(scene.hit(outside).is_none());
        }
    }

    #[test]
    fn every_free_cube_can_be_reached_from_at_least_one_view() {
        for layout in Layout::ALL {
            let board = Board::with_layout(layout, 0);
            let positions: Vec<_> = board.layout().collect();
            for (id, target) in positions.iter().enumerate() {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    // For each side, test the most obscured possible state
                    // in which that cube is free. Additional removals can
                    // only reveal more of its faces.
                    let visible = Orientation::ALL.into_iter().any(|view| {
                        let mut scene = Scene::for_view(&board, view);
                        scene.cubes.retain(|cube| {
                            let pos = positions[cube.id];
                            let x = i16::from(pos.x) - i16::from(target.x);
                            let y = i16::from(pos.y) - i16::from(target.y);
                            let covers_top = (x, y) == (0, 0) && pos.level > target.level;
                            let covers_side = (x, y) == (dx, dy) && pos.level == target.level;
                            !(covers_top || covers_side)
                        });
                        let cube = scene.cubes.iter().find(|cube| cube.id == id).unwrap();
                        scene.visible_point(cube).is_some()
                    });
                    assert!(
                        visible,
                        "{layout:?}, cube {id} with side ({dx}, {dy}) open is obscured"
                    );
                }
            }
        }
    }
}
