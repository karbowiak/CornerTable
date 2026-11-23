use std::{
    collections::BTreeMap,
    f32::consts::{PI, TAU},
};

use corner_table_mahjong_cubes::{Face, Suit};
use gpui::{
    Background, FillOptions, FillRule, PathBuilder, PathStyle, Pixels, Point, Window,
    linear_color_stop, linear_gradient, point, px, rgb,
};

use super::{
    ProjectedCube, ProjectedMesh, Scene, Surface, Tabletop, TileStates,
    geometry::{CUBE_HALF, Camera, Vec3, edge_side, on_quad},
};

type Quad = [Point<Pixels>; 4];

const LIGHT: Vec3 = Vec3::new(-0.44, 0.53, 0.725);
const FELT_LIGHT: u32 = 0x38634e;
const FELT_DARK: u32 = 0x16372c;

impl Scene {
    pub fn paint(&self, states: TileStates, hovered: Option<usize>, window: &mut Window) {
        self.table.paint(&self.camera, window);
        self.paint_shadows(window);

        if let Some(pair) = states.fading {
            let progress = pair.at.elapsed().as_secs_f32() / 0.24;
            if progress < 1. {
                let mut outlines = PathBuilder::stroke(px(1.6));
                for id in pair.ids {
                    let center = self.original[id] + Vec3::Z * (0.51 + progress * 0.22);
                    let radius = (CUBE_HALF.x - 0.03) * (1. - progress * 0.16);
                    let ring = octagon(radius, radius, 0.11)
                        .map(|offset| self.camera.project(center + offset));
                    add_polygon(&mut outlines, &ring);
                }
                paint(
                    window,
                    outlines,
                    rgb(0xffdb99).opacity(0.9 * (1. - progress)),
                );
                window.request_animation_frame();
            }
        }

        let reflection = (LIGHT + self.camera.toward).unit();
        for (index, cube) in self.cubes.iter().enumerate() {
            cube.paint(
                &self.mesh,
                &self.occluders(index).collect::<Vec<_>>(),
                states,
                hovered == Some(cube.id),
                reflection,
                window,
            );
        }
    }

    fn paint_shadows(&self, window: &mut Window) {
        let mut wells = PathBuilder::stroke(px(0.75));
        for p in self.original.iter().filter(|p| p.z == 0.5) {
            let center = Vec3::new(p.x, p.y, 0.);
            let well =
                octagon(0.43, 0.43, 0.115).map(|offset| self.camera.project(center + offset));
            add_polygon(&mut wells, &well);
        }
        paint(window, wells, rgb(0xc7c69a).opacity(0.18));

        let mut columns = BTreeMap::<(i32, i32), Vec3>::new();
        for p in self.cubes.iter().map(|cube| cube.center) {
            let highest = columns.entry((p.x as i32, p.y as i32)).or_insert(p);
            if p.z > highest.z {
                *highest = p;
            }
        }

        // The projected contact shadows sit under each stack. The broader
        // penumbra drifts away from the key light as column height rises.
        for ring in (0..8).rev() {
            let spread = ring as f32 / 7.;
            let radius = 0.48 + 0.23 * spread;
            let mut shadows = PathBuilder::fill().with_style(PathStyle::Fill(FillOptions {
                fill_rule: FillRule::NonZero,
            }));
            for top in columns.values() {
                let drift = (top.z + 0.5) * (radius - 0.37) * 0.42;
                let center = Vec3::new(top.x - LIGHT.x * drift, top.y - LIGHT.y * drift, 0.);
                let shape = octagon(radius, radius, 0.22)
                    .map(|offset| self.camera.project(center + offset));
                add_polygon(&mut shadows, &shape);
            }
            let opacity = 0.026 + 0.125 * (1. - spread).powi(2);
            paint(window, shadows, rgb(0x071b15).opacity(opacity));
        }
    }
}

impl Tabletop {
    fn paint(&self, camera: &Camera, window: &mut Window) {
        let upper = self.rim(0., 0.25, 0.);
        let lower = self.rim(0.055, 0.25, -Self::APRON);
        let bevel = self.rim(0.018, 0.25, -0.058);
        let shadow_offset = camera.scale * 0.092;
        for ring in (0..12).rev() {
            let spread = ring as f32 / 11.;
            let shade = self
                .rim(-spread * 0.42, 0.26 + spread * 0.24, -Self::APRON)
                .map(|p| camera.project(p) + point(-shadow_offset * 0.42, shadow_offset));
            let opacity = 0.022 + 0.112 * (1. - spread).powi(2);
            polygon(window, &shade, rgb(0x071710).opacity(opacity));
        }

        for index in 0..upper.len() {
            let next = (index + 1) % upper.len();
            let edge = upper[next] - upper[index];
            let normal = Vec3::new(edge.y, -edge.x, 0.).unit();
            if normal.dot(camera.toward) <= 0. {
                continue;
            }
            let diffuse = normal.dot(LIGHT).max(0.);
            let apron =
                [bevel[index], bevel[next], lower[next], lower[index]].map(|p| camera.project(p));
            polygon(
                window,
                &apron,
                glaze_gradient(
                    mix(0x5c4833, 0xa58454, diffuse),
                    mix(0x312c24, 0x57432f, diffuse),
                ),
            );
            let lip =
                [upper[index], upper[next], bevel[next], bevel[index]].map(|p| camera.project(p));
            polygon(window, &lip, rgb(mix(0x7a603e, 0xc5ab77, diffuse)));
        }

        polygon(
            window,
            &upper.map(|p| camera.project(p)),
            glaze_gradient(0xa69062, 0x746044),
        );
        let baize = self.rim(0.135, 0.22, 0.).map(|p| camera.project(p));
        polygon(window, &baize, glaze_gradient(FELT_LIGHT, FELT_DARK));
        let inner = self.rim(0.176, 0.21, 0.).map(|p| camera.project(p));
        outline(window, &inner, px(0.7), rgb(0xd3cf9e).opacity(0.44));
        self.paint_fibers(camera, window);
    }

    fn paint_fibers(&self, camera: &Camera, window: &mut Window) {
        let w = (self.max.x - self.min.x) * 9.;
        let h = (self.max.y - self.min.y) * 9.;
        let mut light = PathBuilder::fill();
        let mut dark = PathBuilder::fill();
        for row in (3..(h as u32).saturating_sub(3)).step_by(2) {
            for col in (3..(w as u32).saturating_sub(3)).step_by(4) {
                let hash = (row * 251 + col * 977).wrapping_mul(0x9e37_79b9);
                let x = self.min.x + (col as f32 + (hash & 3) as f32 * 0.21) / 9.;
                let y = self.min.y + (row as f32 + ((hash >> 12) & 7) as f32 * 0.13) / 9.;
                let p = Vec3::new(x, y, 0.);
                let fiber = [
                    p,
                    p + Vec3::new(0.05, 0.006, 0.),
                    p + Vec3::new(0.014, 0.020, 0.),
                ]
                .map(|p| camera.project(p));
                add_polygon(
                    if hash & 16 == 0 {
                        &mut light
                    } else {
                        &mut dark
                    },
                    &fiber,
                );
            }
        }
        paint(window, light, rgb(0xd3db9b).opacity(0.13));
        paint(window, dark, rgb(0x070f09).opacity(0.23));
    }
}

impl ProjectedCube {
    fn paint(
        &self,
        mesh: &ProjectedMesh,
        occluders: &[&ProjectedCube],
        states: TileStates,
        hovered: bool,
        reflection: Vec3,
        window: &mut Window,
    ) {
        let selected = states.selected == Some(self.id);
        let hinted = states.hinted.is_some_and(|ids| ids.contains(&self.id));
        let keyed = states.keyed == Some(self.id);
        let pressed = states.pressed == Some(self.id);
        let enamel = enamel(self.face.suit);
        let glaze = if selected {
            0xd8efd9
        } else if pressed && self.free {
            0xf0d6aa
        } else if hovered {
            0xfff7e3
        } else if self.free {
            0xf6e8cd
        } else {
            0xaaa391
        };
        let rim = if hinted {
            Some(0xfabc43)
        } else if selected {
            Some(0x58bca4)
        } else {
            None
        };
        let foot = mix(glaze, 0x8f654f, if self.free { 0.23 } else { 0.37 });
        let ceramic = mix(glaze, enamel.tint, 0.16);
        let ink = mix(enamel.ink, 0x4a4b3c, if self.free { 0. } else { 0.40 });

        // A solid undercoat catches the antialiased seams between adjacent facets.
        polygon_at(
            window,
            &mesh.outline,
            self.offset,
            rgb(mix(foot, 0x77614c, 0.12)),
        );

        const BEVEL_SHADES: usize = 8;
        let mut bevels: [Option<PathBuilder>; BEVEL_SHADES] = std::array::from_fn(|_| None);
        let mut marks = PathBuilder::fill();
        let mut highlights = PathBuilder::fill();
        let mut key_frames = Vec::new();
        for surface in &mesh.surfaces {
            if !occluders.is_empty() && self.surface_is_covered(mesh, surface, occluders) {
                continue;
            }
            let normal = surface.normal;
            let diffuse = normal.dot(LIGHT).max(0.);
            let specular = normal.dot(reflection).max(0.).powi(9);
            let light = (0.79 + diffuse * 0.21 + normal.z * 0.12).clamp(0.62, 1.04);
            if let Some(quad) = surface.mark {
                let body = shade(ceramic, light);
                let warm = mix(body, 0xfff8e7, 0.19 + 0.31 * specular);
                polygon_at(
                    window,
                    &surface.vertices,
                    self.offset,
                    glaze_gradient(warm, shade(body, 0.87)),
                );
                let extent = projected_face_extent(&quad);
                if extent < px(0.5) {
                    continue;
                }
                let quad = quad.map(|p| p + self.offset);
                if normal.z < 0.5 {
                    let band = [(0., 0.9), (1., 0.9), (1., 1.), (0., 1.)]
                        .map(|(u, v)| on_quad(&quad, u, v));
                    polygon(window, &band, rgb(shade(foot, light)));
                }
                if let Some(rim) = rim {
                    let frame = inset_quad(&quad, 0.027);
                    outline(
                        window,
                        &frame,
                        px(1.8).min(extent * 0.045),
                        rgb(shade(mix(rim, 0x4d3928, 0.20), light)),
                    );
                }
                stamp_marks(&mut marks, &mut highlights, &quad, self.face);
                if keyed {
                    let frame = inset_quad(&quad, 0.11);
                    let width = px(2.8).min(extent * 0.17);
                    key_frames.push((frame, width));
                }
            } else {
                // Adjacent bevel facets in the same light band share a path.
                let lit = ((light - 0.62) * 1.50 + specular * 0.42).clamp(0., 1.);
                let band = (lit * (BEVEL_SHADES - 1) as f32).round() as usize;
                let path = bevels[band].get_or_insert_with(PathBuilder::fill);
                add_polygon(path, &surface.vertices);
            }
        }
        let (shadow, sheen) = match rim {
            Some(rim) => (shade(rim, 0.67), mix(rim, 0xffeab9, 0.22)),
            None => (shade(foot, 0.78), mix(ceramic, 0xfff6db, 0.66)),
        };
        for (band, path) in bevels.into_iter().enumerate() {
            if let Some(mut path) = path {
                let color = mix(shadow, sheen, band as f32 / (BEVEL_SHADES - 1) as f32);
                path.translate(self.offset);
                paint(window, path, rgb(color));
            }
        }
        paint(window, marks, rgb(ink));
        paint(window, highlights, rgb(mix(ink, glaze, 0.56)));
        for (frame, width) in key_frames {
            outline(window, &frame, width, rgb(0x163a36));
            outline(window, &frame, width * 0.39, rgb(0xfffbda));
        }
    }

    fn surface_is_covered(
        &self,
        mesh: &ProjectedMesh,
        surface: &Surface,
        occluders: &[&ProjectedCube],
    ) -> bool {
        let top_left = surface.bounds.origin + self.offset;
        let bottom_right = surface.bounds.bottom_right() + self.offset;
        occluders.iter().any(|front| {
            front.bounds.contains(&top_left)
                && front.bounds.contains(&bottom_right)
                && surface
                    .vertices
                    .iter()
                    .all(|&p| front.contains(mesh, p + self.offset))
        })
    }
}

#[derive(Clone, Copy)]
struct Enamel {
    tint: u32,
    ink: u32,
}

fn enamel(suit: Suit) -> Enamel {
    let (tint, ink) = match suit {
        Suit::Dots => (0xd8ebee, 0x144d79),
        Suit::Diamonds => (0xffd7bd, 0xa23121),
        Suit::Leaves => (0xe2ebc4, 0x285b34),
        Suit::Blossoms => (0xf6ddeb, 0x762855),
        Suit::Waves => (0xcdece1, 0x126762),
        Suit::Suns => (0xffe2a2, 0x965019),
    };
    Enamel { tint, ink }
}

fn stamp_marks(marks: &mut PathBuilder, highlights: &mut PathBuilder, quad: &Quad, face: Face) {
    let corners = [
        (0.255, 0.245),
        (0.745, 0.715),
        (0.255, 0.715),
        (0.745, 0.245),
    ];
    let corner_count = match face.rank {
        1 => 0,
        2 | 3 => 2,
        _ => 4,
    };
    let centers = corners
        .into_iter()
        .take(corner_count)
        .chain((face.rank % 2 == 1).then_some((0.5, 0.48)));
    let radius = if face.rank == 1 { 0.31 } else { 0.141 };
    for (u, v) in centers {
        let stamp = |x, y| on_quad(quad, u + x * radius, v + y * radius);
        match face.suit {
            Suit::Dots => {
                circle(marks, stamp, 0., 0., 0.95);
                circle(highlights, stamp, -0.04, -0.07, 0.48);
            }
            Suit::Diamonds => {
                let gem = [(0., -1.06), (1., 0.), (0., 1.06), (-1., 0.)];
                add_polygon(marks, &gem.map(|(x, y)| stamp(x, y)));
                let glint = [(0., -0.73), (-0.66, -0.02), (-0.07, 0.17)];
                add_polygon(highlights, &glint.map(|(x, y)| stamp(x, y)));
            }
            Suit::Leaves => {
                let leaf = [
                    (-0.98, 0.90),
                    (-0.99, 0.13),
                    (-0.61, -0.48),
                    (0.10, -0.88),
                    (1., -0.96),
                    (0.95, -0.19),
                    (0.51, 0.48),
                    (-0.18, 0.90),
                ];
                add_polygon(marks, &leaf.map(|(x, y)| stamp(x, y)));
                let rib = [(-0.69, 0.65), (0.67, -0.62), (0.03, 0.34)];
                add_polygon(highlights, &rib.map(|(x, y)| stamp(x, y)));
            }
            Suit::Blossoms => {
                let flower = std::array::from_fn::<_, 25, _>(|i| {
                    let angle = TAU * i as f32 / 25.;
                    let radius = 0.78 + 0.23 * (angle * 5.).cos();
                    stamp(radius * angle.sin(), -radius * angle.cos())
                });
                add_polygon(marks, &flower);
                circle(highlights, stamp, 0., 0., 0.31);
            }
            Suit::Waves => {
                for (radius, thickness) in [(1., 0.27), (0.58, 0.23)] {
                    let arch = std::array::from_fn::<_, 18, _>(|i| {
                        let (step, radius) = if i < 9 {
                            (i, radius)
                        } else {
                            (17 - i, radius - thickness)
                        };
                        let angle = PI * step as f32 / 8.;
                        stamp(radius * angle.cos(), 0.41 - radius * angle.sin() * 1.16)
                    });
                    add_polygon(marks, &arch);
                }
                circle(highlights, stamp, 0., 0.43, 0.25);
            }
            Suit::Suns => {
                let rays = std::array::from_fn::<_, 16, _>(|i| {
                    let angle = TAU * i as f32 / 16.;
                    let radius = if i % 2 == 0 { 1.04 } else { 0.54 };
                    stamp(radius * angle.sin(), -radius * angle.cos())
                });
                add_polygon(marks, &rays);
                circle(highlights, stamp, 0., 0., 0.32);
            }
        }
    }
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

fn octagon(x: f32, y: f32, cut: f32) -> [Vec3; 8] {
    [
        (-x + cut, -y),
        (x - cut, -y),
        (x, -y + cut),
        (x, y - cut),
        (x - cut, y),
        (-x + cut, y),
        (-x, y - cut),
        (-x, -y + cut),
    ]
    .map(|(x, y)| Vec3::new(x, y, 0.))
}

fn inset_quad(quad: &Quad, inset: f32) -> Quad {
    let center = on_quad(quad, 0.5, 0.5);
    quad.map(|p| center + (p - center) * (1. - inset * 2.))
}

fn projected_face_extent(quad: &Quad) -> Pixels {
    let length = |p: Point<Pixels>| f32::from(p.x).hypot(f32::from(p.y));
    let longest = length(quad[1] - quad[0]).max(length(quad[3] - quad[0]));
    px(edge_side(quad[0], quad[1], quad[3]).abs() / longest)
}

fn mix(a: u32, b: u32, weight: f32) -> u32 {
    let channel = |shift: u32| {
        let first = ((a >> shift) & 0xff) as f32;
        let second = ((b >> shift) & 0xff) as f32;
        (first + (second - first) * weight).round() as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

fn shade(color: u32, light: f32) -> u32 {
    let channel =
        |shift: u32| ((((color >> shift) & 0xff) as f32 * light).round().min(255.) as u32) << shift;
    channel(16) | channel(8) | channel(0)
}

fn glaze_gradient(light: u32, dark: u32) -> Background {
    linear_gradient(
        145.,
        linear_color_stop(rgb(light), 0.),
        linear_color_stop(rgb(dark), 1.),
    )
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

fn polygon_at(
    window: &mut Window,
    vertices: &[Point<Pixels>],
    offset: Point<Pixels>,
    color: impl Into<Background>,
) {
    let mut path = PathBuilder::fill();
    add_polygon(&mut path, vertices);
    path.translate(offset);
    paint(window, path, color);
}

fn outline(
    window: &mut Window,
    vertices: &[Point<Pixels>],
    width: Pixels,
    color: impl Into<Background>,
) {
    let mut path = PathBuilder::stroke(width);
    add_polygon(&mut path, vertices);
    paint(window, path, color);
}

fn paint(window: &mut Window, path: PathBuilder, color: impl Into<Background>) {
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
