use std::{
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use corner_table_mahjong_cubes::{Board, Face, MoveResult, Position, Suit};
use gpui::{
    Bounds, Context, CursorStyle, DispatchPhase, Div, FontWeight, HitboxBehavior, MouseButton,
    MouseDownEvent, MouseMoveEvent, PathBuilder, Pixels, Point, Stateful, Window, canvas, div,
    point, prelude::*, px, rgb,
};

use super::{GameEntry, GameLaunch};
use crate::theme;

const INSTRUCTION: &str =
    "Match identical marks on cubes with a clear top and at least one open side.";

pub struct GameView {
    board: Board,
    orientation: Orientation,
    hinted: Option<[usize; 2]>,
    status: &'static str,
    challenge: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _cx: &mut Context<Self>) -> Self {
        let seed = match &launch.entry {
            GameEntry::Challenge(id) => id.bytes().fold(0x434f_524e_4355_4245_u64, |seed, byte| {
                seed.wrapping_mul(1099511628211) ^ u64::from(byte)
            }),
            _ => new_seed(),
        };
        Self {
            board: Board::new(seed),
            orientation: Orientation::SouthEast,
            hinted: None,
            status: INSTRUCTION,
            challenge: matches!(launch.entry, GameEntry::Challenge(_)),
        }
    }

    fn choose(&mut self, id: usize, cx: &mut Context<Self>) {
        let result = self.board.select(id);
        if result == MoveResult::Matched || self.hinted.is_some_and(|pair| !pair.contains(&id)) {
            self.hinted = None;
        }
        self.status = match result {
            MoveResult::Blocked => "That cube needs a clear top and an open side.",
            MoveResult::Selected => "Choose another free cube with the same marks.",
            MoveResult::Deselected => INSTRUCTION,
            MoveResult::DifferentFace => "Those marks differ. Your last choice is selected.",
            MoveResult::Matched if self.board.remaining() == 0 => "Board cleared!",
            MoveResult::Matched => "Pair matched.",
        };
        cx.notify();
    }

    fn clear_help(&mut self) {
        self.hinted = None;
        self.status = INSTRUCTION;
    }

    fn playfield(height: Pixels, cx: &Context<Self>) -> Div {
        let entity = cx.entity();
        let weak = entity.downgrade();
        div()
            .w_full()
            .h(height)
            .rounded_xl()
            .overflow_hidden()
            .bg(rgb(0x204b3d))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let scene = Scene::new(entity.read(cx), bounds);
                        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                        (scene, hitbox)
                    },
                    move |_, (scene, hitbox), window, _| {
                        let hovered = scene
                            .hit(window.mouse_position())
                            .filter(|cube| cube.free && hitbox.is_hovered(window));
                        let hovered_id = hovered.map(|cube| cube.id);
                        scene.paint(hovered_id, window);
                        if hovered.is_some() {
                            window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
                        }

                        let scene = Rc::new(scene);
                        let move_scene = scene.clone();
                        let move_hitbox = hitbox.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            let next = move_scene
                                .hit(event.position)
                                .filter(|cube| cube.free && move_hitbox.is_hovered(window))
                                .map(|cube| cube.id);
                            if next != hovered_id {
                                window.refresh();
                            }
                        });
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                            if phase == DispatchPhase::Bubble
                                && event.button == MouseButton::Left
                                && hitbox.is_hovered(window)
                                && let Some(cube) = scene.hit(event.position)
                            {
                                let _ = weak.update(cx, |view, cx| view.choose(cube.id, cx));
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pairs = self.board.remaining() / 2;
        let remaining = if pairs == 1 {
            "1 pair left".to_owned()
        } else {
            format!("{pairs} pairs left")
        };
        let height = (f32::from(window.viewport_size().height) - 280.).clamp(244., 540.);
        let controls = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(control(
                "cubes-rotate",
                "Rotate view",
                pairs > 0,
                |view| {
                    view.orientation = view.orientation.next();
                },
                cx,
            ))
            .child(control(
                "cubes-hint",
                "Hint",
                pairs > 0,
                |view| {
                    view.hinted = view.board.hint();
                    if view.hinted.is_some() {
                        view.status = "The gold cubes match. Rotate the board to see hidden faces.";
                    }
                },
                cx,
            ))
            .child(control(
                "cubes-restart",
                "Restart",
                true,
                |view| {
                    view.board.restart();
                    view.clear_help();
                },
                cx,
            ))
            .when(!self.challenge, |controls| {
                controls.child(control(
                    "cubes-new-deal",
                    "New deal",
                    true,
                    |view| {
                        view.board = Board::new(new_seed());
                        view.clear_help();
                    },
                    cx,
                ))
            });

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(remaining))
                    .child(controls),
            )
            .child(div().text_color(rgb(theme::MUTED)).child(self.status))
            .child(Self::playfield(px(height), cx))
    }
}

fn control(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    action: impl Fn(&mut GameView) + 'static,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    div()
        .id(id)
        .min_h(px(38.))
        .px_3()
        .flex()
        .items_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(theme::BORDER))
        .bg(rgb(theme::SURFACE))
        .text_color(rgb(if enabled { theme::INK } else { theme::MUTED }))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |button| {
            button
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme::SAGE)))
                .focus(|style| style.border_color(rgb(theme::MUTED)))
                .on_click(cx.listener(move |view, _, _, cx| {
                    action(view);
                    cx.notify();
                }))
        })
        .child(label)
}

fn new_seed() -> u64 {
    static DEAL: AtomicU64 = AtomicU64::new(0);
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);
    time.wrapping_add(DEAL.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Copy)]
enum Orientation {
    SouthEast,
    NorthEast,
    NorthWest,
    SouthWest,
}

impl Orientation {
    fn next(self) -> Self {
        match self {
            Self::SouthEast => Self::NorthEast,
            Self::NorthEast => Self::NorthWest,
            Self::NorthWest => Self::SouthWest,
            Self::SouthWest => Self::SouthEast,
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
        // A ray toward the camera increases u, v and z. For overlapping
        // lattice cells, the nearer cell has the greater sum of its indices.
        (u + v) as i16 + i16::from(position.level)
    }
}

type Quad = [Point<Pixels>; 4];

fn project(u: f32, v: f32, z: f32) -> Point<Pixels> {
    point(px((u - v) * 0.866), px((u + v) * 0.52 - z * 0.81))
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

struct ProjectedCube {
    id: usize,
    face: Face,
    free: bool,
    selected: bool,
    hinted: bool,
    quads: [Quad; 3],
}

struct Scene {
    footprint: Vec<Quad>,
    cubes: Vec<ProjectedCube>,
}

impl Scene {
    fn new(view: &GameView, bounds: Bounds<Pixels>) -> Self {
        let orientation = view.orientation;
        // Frame the initial stack so its position stays fixed as pairs disappear.
        let vertices = view
            .board
            .layout()
            .flat_map(|position| cube_faces(position, orientation))
            .flatten();
        let mut min = point(px(f32::INFINITY), px(f32::INFINITY));
        let mut max = point(px(f32::NEG_INFINITY), px(f32::NEG_INFINITY));
        for vertex in vertices {
            min = point(min.x.min(vertex.x), min.y.min(vertex.y));
            max = point(max.x.max(vertex.x), max.y.max(vertex.y));
        }
        let stack = Bounds::from_corners(min, max);
        let scale = ((bounds.size.width - px(36.)) / stack.size.width)
            .min((bounds.size.height - px(36.)) / stack.size.height)
            .clamp(0.1, 74.);
        let fit = |vertex| bounds.center() + (vertex - stack.center()) * scale;

        let footprint = view
            .board
            .layout()
            .filter(|position| position.level == 0)
            .map(|position| {
                let (u, v) =
                    orientation.rotate(f32::from(position.x) + 0.5, f32::from(position.y) + 0.5);
                [(-0.43, -0.43), (0.43, -0.43), (0.43, 0.43), (-0.43, 0.43)]
                    .map(|(du, dv)| fit(project(u + du, v + dv, 0.)))
            })
            .collect();
        let mut active: Vec<_> = view.board.cubes().collect();
        active.sort_by_key(|(_, cube)| orientation.depth(cube.position));
        let cubes = active
            .into_iter()
            .map(|(id, cube)| ProjectedCube {
                id,
                face: cube.face,
                free: view.board.is_free(id),
                selected: view.board.selected() == Some(id),
                hinted: view.hinted.is_some_and(|pair| pair.contains(&id)),
                quads: cube_faces(cube.position, orientation).map(|quad| quad.map(fit)),
            })
            .collect();
        Self { footprint, cubes }
    }

    fn hit(&self, position: Point<Pixels>) -> Option<&ProjectedCube> {
        // Walk the same polygons in reverse paint order. A blocked front
        // cube still occludes every cube behind it.
        self.cubes
            .iter()
            .rev()
            .find(|cube| cube.quads.iter().rev().any(|quad| contains(quad, position)))
    }

    fn paint(&self, hovered: Option<usize>, window: &mut Window) {
        let mut slots = PathBuilder::fill();
        for quad in &self.footprint {
            add_polygon(&mut slots, quad);
        }
        paint_path(window, slots, 0x285345);
        for cube in &self.cubes {
            cube.paint(hovered == Some(cube.id), window);
        }
    }
}

impl ProjectedCube {
    fn paint(&self, hovered: bool, window: &mut Window) {
        let (rim, shades) = if self.selected {
            (0x965819, [0xd59449, 0xe8ae5d, 0xffd780])
        } else if self.hinted {
            (0xa36b24, [0xcfa354, 0xe5b967, 0xffe6a4])
        } else if hovered {
            (0x8d7950, [0xbda983, 0xe4d3ae, 0xffefc5])
        } else if self.free {
            (0x8d7e61, [0xbaac8d, 0xddd0b1, 0xfff8e7])
        } else {
            (0x6c6b53, [0x9a977d, 0xb6ae93, 0xcac3a7])
        };

        for (quad, shade) in self.quads.iter().zip(shades) {
            let center = (quad[0] + quad[2]) * 0.5;
            let inset = quad.map(|vertex| center + (vertex - center) * 0.96);
            let mut edge = PathBuilder::fill();
            add_polygon(&mut edge, quad);
            paint_path(window, edge, rim);
            let mut face = PathBuilder::fill();
            add_polygon(&mut face, &inset);
            paint_path(window, face, shade);
            paint_marks(window, quad, self.face);
        }
    }
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

fn paint_marks(window: &mut Window, quad: &Quad, face: Face) {
    const DIAGONAL: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let (color, shape): (u32, &[(f32, f32)]) = match face.suit {
        Suit::Dots => (
            0x294967,
            &[
                (1., 0.),
                (DIAGONAL, DIAGONAL),
                (0., 1.),
                (-DIAGONAL, DIAGONAL),
                (-1., 0.),
                (-DIAGONAL, -DIAGONAL),
                (0., -1.),
                (DIAGONAL, -DIAGONAL),
            ],
        ),
        Suit::Diamonds => (0x8a3d38, &[(0., -1.15), (1., 0.), (0., 1.15), (-1., 0.)]),
        Suit::Leaves => (
            0x225c43,
            &[
                (-0.97, 1.08),
                (-1., 0.25),
                (-0.58, -0.55),
                (0.96, -1.08),
                (1., -0.25),
                (0.58, 0.55),
            ],
        ),
        Suit::Blossoms => (
            0x6c426a,
            &[
                (-0.44, -1.),
                (0.44, -1.),
                (0.44, -0.44),
                (1., -0.44),
                (1., 0.44),
                (0.44, 0.44),
                (0.44, 1.),
                (-0.44, 1.),
                (-0.44, 0.44),
                (-1., 0.44),
                (-1., -0.44),
                (-0.44, -0.44),
            ],
        ),
    };
    let corners = [(0.27, 0.27), (0.73, 0.73), (0.27, 0.73), (0.73, 0.27)];
    let corner_count = match face.rank {
        1 => 0,
        2 | 3 => 2,
        _ => 4,
    };
    let centers = corners
        .into_iter()
        .take(corner_count)
        .chain((face.rank % 2 == 1).then_some((0.5, 0.5)));
    let radius = if face.rank == 1 { 0.185 } else { 0.106 };
    let right = quad[1] - quad[0];
    let down = quad[3] - quad[0];
    let mut marks = PathBuilder::fill();
    for (u, v) in centers {
        let mut points = shape
            .iter()
            .map(|&(x, y)| quad[0] + right * (u + x * radius) + down * (v + y * radius));
        if let Some(first) = points.next() {
            marks.move_to(first);
            for vertex in points {
                marks.line_to(vertex);
            }
            marks.close();
        }
    }
    paint_path(window, marks, color);
}

fn add_polygon(path: &mut PathBuilder, quad: &Quad) {
    path.move_to(quad[0]);
    for &vertex in &quad[1..] {
        path.line_to(vertex);
    }
    path.close();
}

fn paint_path(window: &mut Window, path: PathBuilder, color: u32) {
    if let Ok(path) = path.build() {
        window.paint_path(path, rgb(color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_follows_visible_faces_instead_of_cube_bounds_or_eligibility() {
        let cube = |id, position, free| ProjectedCube {
            id,
            face: Face {
                suit: Suit::Dots,
                rank: 1,
            },
            free,
            selected: false,
            hinted: false,
            quads: cube_faces(position, Orientation::SouthEast),
        };
        let rear = cube(
            0,
            Position {
                x: 0,
                y: 0,
                level: 0,
            },
            true,
        );
        let front = cube(
            1,
            Position {
                x: 1,
                y: 1,
                level: 1,
            },
            false,
        );
        let overlapping = point(px(0.), px(0.));
        assert!(contains(&rear.quads[2], overlapping));
        assert!(contains(&front.quads[2], overlapping));

        let scene = Scene {
            footprint: Vec::new(),
            cubes: vec![rear, front],
        };
        assert_eq!(scene.hit(overlapping).map(|cube| cube.id), Some(1));
        assert_eq!(
            scene.hit(point(px(0.), px(-0.61))).map(|cube| cube.id),
            Some(0)
        );
        assert!(scene.hit(point(px(0.75), px(-0.68))).is_none());
    }
}
