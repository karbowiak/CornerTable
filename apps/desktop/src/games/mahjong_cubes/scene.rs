mod geometry;
mod paint;

use std::time::Instant;

use corner_table_mahjong_cubes::{Board, Face, Suit};
use gpui::{Bounds, Pixels, Point, point, px, size};

pub(super) use geometry::Orbit;
use geometry::{
    CUBE_HALF, Camera, MESH, Vec3, convex_hull, cube_center, edge_side, edges, on_quad,
};

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
    camera: Camera,
    table: Tabletop,
    original: Vec<Vec3>,
    mesh: ProjectedMesh,
    cubes: Vec<ProjectedCube>,
}

pub(super) struct ProjectedCube {
    pub id: usize,
    pub free: bool,
    face: Face,
    center: Vec3,
    offset: Point<Pixels>,
    bounds: Bounds<Pixels>,
}

struct ProjectedMesh {
    surfaces: Vec<Surface>,
    outline: Vec<Point<Pixels>>,
    bounds: Bounds<Pixels>,
}

struct Surface {
    normal: Vec3,
    vertices: Vec<Point<Pixels>>,
    bounds: Bounds<Pixels>,
    mark: Option<[Point<Pixels>; 4]>,
}

impl ProjectedMesh {
    fn new(camera: &Camera) -> Self {
        // Orthographic projection leaves every cube with the same silhouette
        // and facets. Project them once, then translate each cube on screen.
        let project = |vertex| camera.project_offset(vertex);
        let surfaces: Vec<_> = MESH
            .iter()
            .filter(|facet| facet.normal.dot(camera.toward) > 0.00001)
            .map(|facet| {
                let vertices: Vec<_> = facet.vertices.iter().copied().map(project).collect();
                Surface {
                    normal: facet.normal,
                    bounds: polygon_bounds(&vertices),
                    vertices,
                    mark: facet.mark.map(|quad| quad.map(project)),
                }
            })
            .collect();
        let outline = convex_hull(
            surfaces
                .iter()
                .flat_map(|surface| surface.vertices.iter().copied())
                .collect(),
        );
        Self {
            surfaces,
            bounds: polygon_bounds(&outline),
            outline,
        }
    }
}

struct Tabletop {
    min: Vec3,
    max: Vec3,
}

impl Tabletop {
    const APRON: f32 = 0.27;
    const MARGIN: f32 = 0.73;

    fn around(centers: &[Vec3]) -> Self {
        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, 0.);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, 0.);
        let pad = 0.5 + Self::MARGIN;
        for center in centers {
            min.x = min.x.min(center.x - pad);
            min.y = min.y.min(center.y - pad);
            max.x = max.x.max(center.x + pad);
            max.y = max.y.max(center.y + pad);
        }
        Self { min, max }
    }

    fn rim(&self, inset: f32, cut: f32, z: f32) -> [Vec3; 8] {
        let x0 = self.min.x + inset;
        let x1 = self.max.x - inset;
        let y0 = self.min.y + inset;
        let y1 = self.max.y - inset;
        [
            (x0 + cut, y0),
            (x1 - cut, y0),
            (x1, y0 + cut),
            (x1, y1 - cut),
            (x1 - cut, y1),
            (x0 + cut, y1),
            (x0, y1 - cut),
            (x0, y0 + cut),
        ]
        .map(|(x, y)| Vec3::new(x, y, z))
    }
}

pub(super) struct Hint {
    pub orientation: Orbit,
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
    pub fn new(board: &Board, bounds: Bounds<Pixels>, orientation: Orbit) -> Self {
        let original: Vec<_> = board.layout().map(cube_center).collect();
        let table = Tabletop::around(&original);
        let pivot = (table.min + table.max) * 0.5;
        let mut framing = table.rim(0., 0.25, -Tabletop::APRON).to_vec();
        framing.extend(table.rim(0., 0.25, 0.));
        for center in &original {
            for x in [-CUBE_HALF.x, CUBE_HALF.x] {
                for y in [-CUBE_HALF.y, CUBE_HALF.y] {
                    framing.push(*center + Vec3::new(x, y, CUBE_HALF.z));
                }
            }
        }
        let camera = Camera::fit(orientation, bounds, pivot, &framing);
        let mesh = ProjectedMesh::new(&camera);

        let mut cubes = board
            .cubes()
            .map(|(id, cube)| {
                let center = original[id];
                let offset = camera.project(center);
                ProjectedCube {
                    id,
                    free: board.is_free(id),
                    face: cube.face,
                    center,
                    offset,
                    bounds: Bounds::new(mesh.bounds.origin + offset, mesh.bounds.size),
                }
            })
            .collect::<Vec<_>>();

        // Along an orthographic ray the coordinates advance monotonically.
        // If two lattice cells overlap on screen, every differing coordinate
        // orders them the same way along that ray. The center depth therefore
        // orders entire cubes exactly, including stacked cubes at shallow pitch.
        cubes.sort_by(|a, b| {
            a.center
                .dot(camera.toward)
                .total_cmp(&b.center.dot(camera.toward))
        });
        Self {
            bounds,
            camera,
            table,
            original,
            mesh,
            cubes,
        }
    }

    pub fn hit(&self, position: Point<Pixels>) -> Option<&ProjectedCube> {
        self.bounds.contains(&position).then_some(())?;
        // A bevel belongs to its solid silhouette. Test the same solids in
        // reverse paint order, even when the nearest one is blocked.
        self.cubes
            .iter()
            .rev()
            .find(|cube| cube.contains(&self.mesh, position))
    }

    #[cfg(all(test, debug_assertions))]
    pub fn visible_cube_point(&self, id: usize) -> Option<Point<Pixels>> {
        let index = self.cubes.iter().position(|cube| cube.id == id)?;
        self.visible_point(index)
    }

    fn visible_point(&self, index: usize) -> Option<Point<Pixels>> {
        let cube = &self.cubes[index];
        let occluders: Vec<_> = self.occluders(index).collect();

        // Try the markings first so that navigation favors recognizable faces.
        for face in self
            .mesh
            .surfaces
            .iter()
            .rev()
            .filter_map(|surface| surface.mark)
        {
            for (u, v) in [(0.5, 0.5), (0.2, 0.2), (0.8, 0.2), (0.8, 0.8), (0.2, 0.8)] {
                let point = cube.offset + on_quad(&face, u, v);
                if !occluders
                    .iter()
                    .any(|front| front.contains(&self.mesh, point))
                {
                    return Some(point);
                }
            }
        }

        // An upper tile can leave only a sliver exposed at an oblique angle.
        // Subtract the very same opaque silhouettes used by pointer picking.
        let mut visible = vec![cube.translated_outline(&self.mesh)];
        for front in occluders {
            let cutter = front.translated_outline(&self.mesh);
            visible = visible
                .into_iter()
                .flat_map(|patch| subtract_polygon(patch, &cutter))
                .collect();
        }
        let patch = visible
            .into_iter()
            .max_by(|a, b| area(a).total_cmp(&area(b)))?;
        let sum = patch.iter().fold(point(px(0.), px(0.)), |sum, p| sum + *p);
        Some(sum * (1. / patch.len() as f32))
    }

    fn occluders(&self, index: usize) -> impl Iterator<Item = &ProjectedCube> {
        let cube = &self.cubes[index];
        self.cubes[index + 1..]
            .iter()
            .filter(|front| front.bounds.intersects(&cube.bounds))
    }

    fn for_view(board: &Board, orientation: Orbit) -> Self {
        Self::new(
            board,
            Bounds::new(point(px(0.), px(0.)), size(px(1000.), px(700.))),
            orientation,
        )
    }

    pub fn hint(board: &Board, current: Orbit) -> Option<Hint> {
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
        let raised = current.raised();
        for orientation in [
            current,
            raised,
            raised.offset(90., 0.),
            raised.offset(-90., 0.),
            raised.offset(180., 0.),
        ] {
            let scene = Self::for_view(board, orientation);
            for &pair in &pairs {
                let visible = pair.map(|id| {
                    scene
                        .cubes
                        .iter()
                        .position(|cube| cube.id == id)
                        .and_then(|index| scene.visible_point(index))
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
        orientation: Orbit,
        focused: Option<usize>,
        step: Point<f32>,
    ) -> Option<usize> {
        let scene = Self::for_view(board, orientation);
        let targets: Vec<_> = scene
            .cubes
            .iter()
            .enumerate()
            .filter(|(_, cube)| cube.free)
            .filter_map(|(index, cube)| scene.visible_point(index).map(|point| (cube.id, point)))
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
                (forward > 0.).then_some((id, forward.hypot(across) + across.abs()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
            .or(focused)
    }
}

impl ProjectedCube {
    fn contains(&self, mesh: &ProjectedMesh, point: Point<Pixels>) -> bool {
        if !self.bounds.contains(&point) {
            return false;
        }
        let local = point - self.offset;
        edges(&mesh.outline).all(|(a, b)| edge_side(a, b, local) >= 0.)
    }

    fn translated_outline(&self, mesh: &ProjectedMesh) -> Vec<Point<Pixels>> {
        mesh.outline.iter().map(|&p| self.offset + p).collect()
    }
}

fn polygon_bounds(vertices: &[Point<Pixels>]) -> Bounds<Pixels> {
    let first = vertices[0];
    let (min, max) = vertices[1..].iter().fold((first, first), |(min, max), p| {
        (
            point(min.x.min(p.x), min.y.min(p.y)),
            point(max.x.max(p.x), max.y.max(p.y)),
        )
    });
    Bounds::from_corners(min, max)
}

fn subtract_polygon(
    mut patch: Vec<Point<Pixels>>,
    cutter: &[Point<Pixels>],
) -> Vec<Vec<Point<Pixels>>> {
    let mut remaining = Vec::new();
    for (a, b) in edges(cutter) {
        // The outside piece survives this clip. Only the inside portion can
        // overlap the cutter's next edge.
        let mut inside = Vec::new();
        let mut outside = Vec::new();
        for (p, q) in edges(&patch) {
            let dp = edge_side(a, b, p);
            let dq = edge_side(a, b, q);
            if dp >= 0. {
                inside.push(p);
            } else {
                outside.push(p);
            }
            if (dp >= 0.) != (dq >= 0.) {
                let split = p + (q - p) * (dp / (dp - dq));
                inside.push(split);
                outside.push(split);
            }
        }
        if area(&outside) > 0.02 {
            remaining.push(outside);
        }
        patch = inside;
        if area(&patch) <= 0.02 {
            break;
        }
    }
    remaining
}

fn area(polygon: &[Point<Pixels>]) -> f32 {
    match polygon {
        [first, rest @ ..] => {
            rest.windows(2)
                .map(|edge| edge_side(*first, edge[0], edge[1]))
                .sum::<f32>()
                .abs()
                * 0.5
        }
        [] => 0.,
    }
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

#[cfg(test)]
mod tests {
    use corner_table_mahjong_cubes::Layout;

    use super::*;

    fn front_depth(p: Vec3, toward: Vec3, center: Vec3) -> Option<f32> {
        // Independently intersect a ray with the solid's face planes.
        let mut back = f32::NEG_INFINITY;
        let mut front = f32::INFINITY;
        for facet in MESH.iter() {
            let offset = facet.normal.dot(facet.vertices[0] + center - p);
            let rate = facet.normal.dot(toward);
            if rate > 0.000_001 {
                front = front.min(offset / rate);
            } else if rate < -0.000_001 {
                back = back.max(offset / rate);
            } else if offset < 0. {
                return None;
            }
        }
        (back <= front).then_some(front)
    }

    #[test]
    fn picking_agrees_with_the_nearest_ray_hit_across_yaw_and_pitch() {
        let board = Board::with_layout(Layout::Gatehouse, 75);
        let bounds = Bounds::new(point(px(8.), px(15.)), size(px(520.), px(340.)));
        let mut occluded_samples = 0;
        for yaw in [-30., 0., 9., 38., 89.9, 90., 132., 187., 254., 312.] {
            for pitch in [14., 37., 56., 82.] {
                let scene = Scene::new(&board, bounds, Orbit::new(yaw, pitch));
                for x in (4..520).step_by(13) {
                    for y in (4..340).step_by(11) {
                        let point = bounds.origin + point(px(x as f32 + 0.27), px(y as f32 + 0.32));
                        let ray = scene.camera.ray_origin(point);
                        let hits: Vec<_> = scene
                            .cubes
                            .iter()
                            .filter_map(|cube| {
                                front_depth(ray, scene.camera.toward, cube.center)
                                    .map(|depth| (depth, cube.id))
                            })
                            .collect();
                        occluded_samples += usize::from(hits.len() > 1);
                        let front = hits
                            .into_iter()
                            .max_by(|a, b| a.0.total_cmp(&b.0))
                            .map(|(_, id)| id);
                        assert_eq!(
                            scene.hit(point).map(|cube| cube.id),
                            front,
                            "yaw {yaw}, pitch {pitch}, {point:?}"
                        );
                    }
                }
            }
        }
        assert!(
            occluded_samples > 1_000,
            "{occluded_samples} overlapping rays"
        );
    }

    #[test]
    fn bevel_can_be_picked_but_its_cut_corner_is_empty() {
        let board = Board::with_layout(Layout::Terrace, 9);
        let mut scene = Scene::for_view(&board, Orbit::new(23., 71.));
        scene.cubes.retain(|cube| cube.id == 1);
        let cube = &scene.cubes[0];
        let chamfer = MESH
            .iter()
            .find(|face| face.normal.z > 0. && face.normal.x > 0. && face.normal.y > 0.)
            .unwrap();
        let center = chamfer
            .vertices
            .iter()
            .fold(Vec3::default(), |sum, &p| sum + p)
            * (1. / 3.);
        assert_eq!(
            scene
                .hit(scene.camera.project(cube.center + center))
                .map(|hit| hit.id),
            Some(cube.id)
        );
        let mut box_corners = Vec::new();
        for x in [-CUBE_HALF.x, CUBE_HALF.x] {
            for y in [-CUBE_HALF.y, CUBE_HALF.y] {
                for z in [-CUBE_HALF.z, CUBE_HALF.z] {
                    box_corners.push(scene.camera.project(cube.center + Vec3::new(x, y, z)));
                }
            }
        }
        let uncut_outline = convex_hull(box_corners);
        assert!(edges(&uncut_outline).any(|(a, b)| {
            (1..20).any(|step| {
                let point = a + (b - a) * (step as f32 / 20.);
                cube.bounds.contains(&point) && scene.hit(point).is_none()
            })
        }));
    }

    #[test]
    fn frame_fits_both_window_shapes_and_does_not_shift_after_matching() {
        for layout in Layout::ALL {
            let mut board = Board::with_layout(layout, 124);
            let pair = board.hint().unwrap();
            for (width, height) in [(632., 268.), (1020., 545.), (385., 248.)] {
                let bounds = Bounds::new(point(px(22.), px(50.)), size(px(width), px(height)));
                for yaw in [0., 44., 95., 223., 358.] {
                    for pitch in [14., 38., 82.] {
                        let view = Orbit::new(yaw, pitch);
                        let before = Scene::new(&board, bounds, view);
                        for z in [-Tabletop::APRON, 0.] {
                            for p in before.table.rim(0., 0.25, z) {
                                assert!(bounds.contains(&before.camera.project(p)));
                            }
                        }
                        for cube in &before.cubes {
                            assert!(
                                cube.translated_outline(&before.mesh)
                                    .iter()
                                    .all(|p| bounds.contains(p))
                            );
                        }

                        assert!(board.remove_pair(pair));
                        let after = Scene::new(&board, bounds, view);
                        assert_eq!(after.camera.scale, before.camera.scale);
                        assert!(after.cubes.iter().all(|cube| {
                            let original =
                                before.cubes.iter().find(|old| old.id == cube.id).unwrap();
                            cube.bounds == original.bounds
                        }));
                        assert!(board.undo());
                    }
                }
            }
        }
    }

    #[test]
    fn each_free_cube_is_reachable_from_a_high_view() {
        for layout in Layout::ALL {
            let board = Board::with_layout(layout, 0);
            let positions: Vec<_> = board.layout().collect();
            for (id, target) in positions.iter().enumerate() {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    // Restore every possible occluder except those whose
                    // absence is necessary for this cube to become free.
                    let visible = [13., 103., 193., 283.].into_iter().any(|yaw| {
                        let mut scene = Scene::for_view(&board, Orbit::new(yaw, 72.));
                        scene.cubes.retain(|cube| {
                            let pos = positions[cube.id];
                            let x = i16::from(pos.x) - i16::from(target.x);
                            let y = i16::from(pos.y) - i16::from(target.y);
                            let covers_top = (x, y) == (0, 0) && pos.level > target.level;
                            let covers_side = (x, y) == (dx, dy) && pos.level == target.level;
                            !(covers_top || covers_side)
                        });
                        let index = scene.cubes.iter().position(|cube| cube.id == id).unwrap();
                        scene
                            .visible_point(index)
                            .is_some_and(|point| scene.hit(point).is_some_and(|cube| cube.id == id))
                    });
                    assert!(visible, "{layout:?}, cube {id} with side ({dx}, {dy}) open");
                }
            }
        }
    }
}
