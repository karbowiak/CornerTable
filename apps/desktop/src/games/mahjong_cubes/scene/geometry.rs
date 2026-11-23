use std::{
    ops::{Add, Mul, Sub},
    sync::LazyLock,
};

use corner_table_mahjong_cubes::Position;
use gpui::{Bounds, Pixels, Point, point, px};
use serde::{Deserialize, Serialize};

const OPENING_YAW: f32 = 45.;
const OPENING_PITCH: f32 = 38.;
const MIN_PITCH: f32 = 14.;
const MAX_PITCH: f32 = 82.;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub(crate) struct Orbit {
    yaw: f32,
    pitch: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Self::new(OPENING_YAW, OPENING_PITCH)
    }
}

impl Orbit {
    pub fn new(yaw: f32, pitch: f32) -> Self {
        let yaw = if yaw.is_finite() { yaw } else { OPENING_YAW };
        let pitch = if pitch.is_finite() {
            pitch
        } else {
            OPENING_PITCH
        };
        Self {
            yaw: yaw.rem_euclid(360.),
            pitch: pitch.clamp(MIN_PITCH, MAX_PITCH),
        }
    }

    pub fn normalized(self) -> Self {
        Self::new(self.yaw, self.pitch)
    }

    pub fn offset(self, yaw: f32, pitch: f32) -> Self {
        Self::new(self.yaw + yaw, self.pitch + pitch)
    }

    pub fn raised(self) -> Self {
        Self::new(self.yaw, 72.)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const X: Self = Self::new(1., 0., 0.);
    pub const Y: Self = Self::new(0., 1., 0.);
    pub const Z: Self = Self::new(0., 0., 1.);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, v: Self) -> f32 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }

    fn cross(self, v: Self) -> Self {
        Self::new(
            self.y * v.z - self.z * v.y,
            self.z * v.x - self.x * v.z,
            self.x * v.y - self.y * v.x,
        )
    }

    pub fn unit(self) -> Self {
        self * self.dot(self).sqrt().recip()
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, v: Self) -> Self {
        Self::new(self.x + v.x, self.y + v.y, self.z + v.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, v: Self) -> Self {
        Self::new(self.x - v.x, self.y - v.y, self.z - v.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, scale: f32) -> Self {
        Self::new(self.x * scale, self.y * scale, self.z * scale)
    }
}

pub(super) struct Camera {
    pub toward: Vec3,
    right: Vec3,
    down: Vec3,
    pivot: Vec3,
    pub scale: Pixels,
    origin: Point<Pixels>,
}

impl Camera {
    pub fn fit(orbit: Orbit, bounds: Bounds<Pixels>, pivot: Vec3, framing: &[Vec3]) -> Self {
        let (sin_yaw, cos_yaw) = orbit.yaw.to_radians().sin_cos();
        let (sin_pitch, cos_pitch) = orbit.pitch.to_radians().sin_cos();
        let front = Vec3::new(cos_yaw, sin_yaw, 0.);

        // Fit the original layout's rotational envelope. The pivot and scale
        // stay fixed through a full turn and through removals.
        let height_range = |pitch: f32| {
            let (sin, cos) = pitch.to_radians().sin_cos();
            framing.iter().fold((0_f32, 0_f32), |(top, bottom), &p| {
                let p = p - pivot;
                let radial = p.x.hypot(p.y) * sin;
                (
                    top.min(-p.z * cos - radial),
                    bottom.max(-p.z * cos + radial),
                )
            })
        };
        let radius = framing
            .iter()
            .map(|&p| (p.x - pivot.x).hypot(p.y - pivot.y))
            .fold(0_f32, f32::max);
        let (top, bottom) = height_range(orbit.pitch);
        let (opening_top, opening_bottom) = height_range(OPENING_PITCH);
        let min_height = (opening_bottom - opening_top) * 0.82;
        let padding = (bounds.size.width.min(bounds.size.height) * 0.065).clamp(px(12.), px(40.));
        let scale = ((bounds.size.width - padding * 2.) / (2. * radius))
            .min((bounds.size.height - padding * 2.) / (bottom - top).max(min_height))
            .clamp(px(0.1), px(112.));

        Self {
            toward: front * cos_pitch + Vec3::Z * sin_pitch,
            right: Vec3::new(sin_yaw, -cos_yaw, 0.),
            down: front * sin_pitch - Vec3::Z * cos_pitch,
            pivot,
            scale,
            origin: bounds.center() - point(px(0.), scale * ((top + bottom) * 0.5)),
        }
    }

    pub fn project(&self, p: Vec3) -> Point<Pixels> {
        self.origin + self.project_offset(p - self.pivot)
    }

    pub fn project_offset(&self, offset: Vec3) -> Point<Pixels> {
        point(
            self.scale * offset.dot(self.right),
            self.scale * offset.dot(self.down),
        )
    }

    #[cfg(test)]
    pub fn ray_origin(&self, point: Point<Pixels>) -> Vec3 {
        let delta = point - self.origin;
        self.pivot + self.right * (delta.x / self.scale) + self.down * (delta.y / self.scale)
    }
}

pub(super) const CUBE_HALF: Vec3 = Vec3::new(0.473, 0.473, 0.5);
const BEVEL: f32 = 0.086;

pub(super) fn cube_center(position: Position) -> Vec3 {
    Vec3::new(
        f32::from(position.x) + 0.5,
        f32::from(position.y) + 0.5,
        f32::from(position.level) + 0.5,
    )
}

pub(super) struct Facet {
    pub normal: Vec3,
    pub vertices: Vec<Vec3>,
    /// Coordinates for painting the mark on a flat face.
    pub mark: Option<[Vec3; 4]>,
}

pub(super) static MESH: LazyLock<Vec<Facet>> = LazyLock::new(|| {
    let axes = [
        (Vec3::X, CUBE_HALF.x),
        (Vec3::Y, CUBE_HALF.y),
        (Vec3::Z, CUBE_HALF.z),
    ];
    let mut faces = Vec::with_capacity(78);

    for (index, &(axis, half)) in axes.iter().enumerate() {
        for sign in [-1., 1.] {
            let normal = axis * sign;
            let (right, right_half, down, down_half) = if index == 2 {
                (Vec3::X, CUBE_HALF.x, Vec3::Y, CUBE_HALF.y)
            } else {
                (
                    Vec3::new(normal.y, -normal.x, 0.),
                    CUBE_HALF.x,
                    Vec3::Z * -1.,
                    CUBE_HALF.z,
                )
            };
            let point = |u, v| normal * half + right * u + down * v;
            let w = right_half;
            let h = down_half;
            let b = BEVEL;
            let vertices = [
                (-w + b, -h + b),
                (w - b, -h + b),
                (w - b, h - b),
                (-w + b, h - b),
            ]
            .map(|(u, v)| point(u, v));
            faces.push(Facet {
                normal,
                vertices: vertices.to_vec(),
                mark: Some(vertices),
            });
        }
    }

    for first in 0..axes.len() {
        let (a, a_half) = axes[first];
        for second in first + 1..axes.len() {
            let (b, b_half) = axes[second];
            let (c, c_half) = axes[3 - first - second];
            for a_sign in [-1., 1.] {
                for b_sign in [-1., 1.] {
                    let a = a * a_sign;
                    let b = b * b_sign;
                    let axis = a * (a_half - BEVEL) + b * (b_half - BEVEL);
                    let extent = c * (c_half - BEVEL);
                    for [first, second] in [[a, (a + b).unit()], [(a + b).unit(), b]] {
                        let start = axis + first * BEVEL;
                        let end = axis + second * BEVEL;
                        faces.push(Facet {
                            normal: (first + second).unit(),
                            vertices: vec![
                                start - extent,
                                end - extent,
                                end + extent,
                                start + extent,
                            ],
                            mark: None,
                        });
                    }
                }
            }
        }
    }

    for x_sign in [-1., 1.] {
        for y_sign in [-1., 1.] {
            for z_sign in [-1., 1.] {
                let a = Vec3::X * x_sign;
                let b = Vec3::Y * y_sign;
                let c = Vec3::Z * z_sign;
                let corner = Vec3::new(
                    x_sign * (CUBE_HALF.x - BEVEL),
                    y_sign * (CUBE_HALF.y - BEVEL),
                    z_sign * (CUBE_HALF.z - BEVEL),
                );
                let round = |direction: Vec3| corner + direction.unit() * BEVEL;
                let tip = round(a + b + c);
                let arc = [a, a + b, b, b + c, c, c + a].map(round);
                for (from, to) in edges(&arc) {
                    let normal = (from - tip).cross(to - tip).unit();
                    let normal = normal * normal.dot(corner).signum();
                    faces.push(Facet {
                        normal,
                        vertices: vec![from, to, tip],
                        mark: None,
                    });
                }
            }
        }
    }
    faces
});

pub(super) fn edge_side(a: Point<Pixels>, b: Point<Pixels>, p: Point<Pixels>) -> f32 {
    let edge = b - a;
    let offset = p - a;
    f32::from(edge.x) * f32::from(offset.y) - f32::from(edge.y) * f32::from(offset.x)
}

pub(super) fn edges<T: Copy>(vertices: &[T]) -> impl Iterator<Item = (T, T)> + '_ {
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
}

pub(super) fn convex_hull(mut points: Vec<Point<Pixels>>) -> Vec<Point<Pixels>> {
    points.sort_by(|a, b| a.x.cmp(&b.x).then_with(|| a.y.cmp(&b.y)));
    points.dedup();
    let mut hull = Vec::with_capacity(12);
    // Positive cross products for every boundary edge. The same winding is
    // used by point picking and polygon clipping.
    for p in points.iter().copied() {
        push_hull_point(&mut hull, p, 2);
    }
    let lower = hull.len();
    for p in points.into_iter().rev().skip(1) {
        push_hull_point(&mut hull, p, lower + 1);
    }
    hull.pop();
    hull
}

fn push_hull_point(hull: &mut Vec<Point<Pixels>>, p: Point<Pixels>, min: usize) {
    while hull.len() >= min && edge_side(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0001 {
        hull.pop();
    }
    hull.push(p);
}

pub(super) fn on_quad(quad: &[Point<Pixels>; 4], u: f32, v: f32) -> Point<Pixels> {
    quad[0] + (quad[1] - quad[0]) * u + (quad[3] - quad[0]) * v
}
