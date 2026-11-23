use corner_table_gem_cascade::{Board, COLUMNS, Gem, PlacedGem, ROWS, SPAWN_COLUMN, Wave};
use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, Window, fill, point, px, rgb, size};

use super::art::{Artwork, glint, paint_sprite};

pub(super) const DARK_METAL: u32 = 0x211c1c;
pub(super) const RIM: u32 = 0x89725d;
pub(super) const GAP: f32 = 2.;
pub(super) const INSET: f32 = 9.;

const LIGHT_SITES: usize = 14;
const SPARK_SITES: usize = 6;

/// The render positions are derived from the rules engine's before-clear board.
/// Keeping the destination by cell preserves the order of repeated colors.
#[derive(Clone, Copy)]
pub(super) struct Settling {
    pub cleared: [[bool; COLUMNS]; ROWS],
    rows: [[usize; COLUMNS]; ROWS],
    light_sites: [Option<PlacedGem>; LIGHT_SITES],
    spark_sites: [Option<PlacedGem>; SPARK_SITES],
}

impl Settling {
    pub fn new(wave: &Wave) -> Self {
        let mut cleared = [[false; COLUMNS]; ROWS];
        for cell in &wave.cleared {
            cleared[cell.row][cell.column] = true;
        }
        let mut rows = std::array::from_fn(|row| [row; COLUMNS]);
        for column in 0..COLUMNS {
            let mut destination = ROWS;
            for row in (0..ROWS).rev() {
                if wave.board[row][column].is_some() && !cleared[row][column] {
                    destination -= 1;
                    rows[row][column] = destination;
                }
            }
        }
        Self {
            cleared,
            rows,
            light_sites: effect_sites(&wave.cleared),
            spark_sites: effect_sites(&wave.cleared),
        }
    }
}

// Spread light and fragments along a large match without scaling overlay cost
// with the pile. Every cleared cut still compresses and disappears.
fn effect_sites<const N: usize>(cleared: &[PlacedGem]) -> [Option<PlacedGem>; N] {
    let count = cleared.len().min(N);
    let mut sites = [None; N];
    for (index, site) in sites[..count].iter_mut().enumerate() {
        *site = Some(cleared[(2 * index + 1) * cleared.len() / (2 * count)]);
    }
    sites
}

#[derive(Clone, Copy)]
pub(super) struct ClearFrame {
    pub settling: Settling,
    pub progress: f32,
}

pub(super) struct Field {
    pub board: Board,
    pub active: Option<[PlacedGem; 2]>,
    pub landing: Option<[PlacedGem; 2]>,
    pub clear: Option<ClearFrame>,
    pub fall_offset: f32,
    pub impact: Option<([PlacedGem; 2], f32)>,
}

impl Field {
    pub fn paint(&self, art: &Artwork, bounds: Bounds<Pixels>, cell: f32, window: &mut Window) {
        let grid = Grid { bounds, cell };
        grid.inlay(art, window);
        if let Some(landing) = self.landing {
            for place in landing {
                let visible = self.active.is_none_or(|active| {
                    !active
                        .iter()
                        .any(|cell| cell.row == place.row && cell.column == place.column)
                });
                if visible {
                    paint_sprite(
                        window,
                        grid.stone(place.row as f32, place.column),
                        art.outline(place.gem),
                    );
                }
            }
        }

        if let Some(frame) = self.clear {
            grid.light_matches(&self.board, frame, window);
        }
        for (row, cells) in self.board.iter().enumerate() {
            for (column, &gem) in cells.iter().enumerate() {
                let Some(gem) = gem else { continue };
                let mut at = grid.stone(row as f32, column);
                if let Some(ClearFrame { settling, progress }) = &self.clear {
                    if settling.cleared[row][column] {
                        if *progress >= 0.36 {
                            continue;
                        }
                        let squeeze = ((progress - 0.19) / 0.17).clamp(0., 1.);
                        at = scale_about_center(at, 1. + 0.045 * (1. - squeeze) - 0.23 * squeeze);
                    } else {
                        let eased = ((progress - 0.43) / 0.57).clamp(0., 1.).powi(2);
                        let distance = settling.rows[row][column] as f32 - row as f32;
                        at.origin.y += px(distance * grid.pitch() * eased);
                    }
                }
                if let Some((cells, strength)) = self.impact
                    && cells
                        .iter()
                        .any(|cell| cell.row == row && cell.column == column)
                {
                    // Compression is anchored at the bottom of the cut.
                    let compression = at.size.height * (0.13 * strength);
                    at.origin.y += compression;
                    at.size.height -= compression;
                    let spread = at.size.width * (0.032 * strength);
                    at.origin.x -= spread;
                    at.size.width += spread * 2.;
                }
                paint_sprite(window, at, art.solid(gem));
            }
        }

        if let Some([pivot, satellite]) = self.active {
            let falling = |place: PlacedGem| {
                let mut at = grid.stone(place.row as f32, place.column);
                at.origin.y += px(self.fall_offset);
                at
            };
            paint_pair(
                art,
                [
                    (pivot.gem, falling(pivot)),
                    (satellite.gem, falling(satellite)),
                ],
                window,
            );
        }
        if let Some(frame) = self.clear {
            grid.scatter_chips(frame, window);
        }
    }
}

pub(super) fn paint_pair(
    art: &Artwork,
    [(first, first_at), (second, second_at)]: [(Gem, Bounds<Pixels>); 2],
    window: &mut Window,
) {
    let a = first_at.center();
    let b = second_at.center();
    rail(window, a, b, 5., color(DARK_METAL, 1.));
    rail(window, a, b, 2., color(0xcfad82, 1.));
    paint_sprite(window, first_at, art.solid(first));
    paint_sprite(window, second_at, art.solid(second));
}

struct Grid {
    bounds: Bounds<Pixels>,
    cell: f32,
}

impl Grid {
    fn pitch(&self) -> f32 {
        self.cell + GAP
    }

    fn origin_at(&self, row: f32, column: usize) -> Point<Pixels> {
        self.bounds.origin + point(px(column as f32 * self.pitch()), px(row * self.pitch()))
    }

    fn stone(&self, row: f32, column: usize) -> Bounds<Pixels> {
        let origin = self.origin_at(row, column) + point(px(1.), px(1.));
        Bounds::new(origin, size(px(self.cell - 2.), px(self.cell - 2.)))
    }

    fn inlay(&self, art: &Artwork, window: &mut Window) {
        let bounds = self.bounds;
        paint_sprite(window, bounds, art.lining.clone());

        // Recessed channels and etched row divisions keep locations visible on velvet.
        for column in 1..COLUMNS {
            let point = self.origin_at(0., column) - point(px(GAP), px(0.));
            bar(
                window,
                point,
                GAP,
                bounds.size.height.into(),
                color(0x13131b, 0.72),
            );
        }
        for row in 1..ROWS {
            let y = self.origin_at(row as f32, 0).y - px(GAP);
            bar(
                window,
                point(bounds.left(), y),
                bounds.size.width.into(),
                1.,
                color(0x858394, 0.17),
            );
            for edge in [bounds.left(), bounds.right() - px(4.)] {
                bar(window, point(edge, y), 4., 1., color(0xbaa28d, 0.36));
            }
        }

        let entrance = self.origin_at(0., SPAWN_COLUMN);
        let corner = color(0xe6bb83, 0.86);
        bar(window, entrance, 6., 1., corner);
        bar(window, entrance, 1., 6., corner);
        let right = entrance + point(px(self.cell - 1.), px(0.));
        bar(window, right - point(px(5.), px(0.)), 6., 1., corner);
        bar(window, right, 1., 6., corner);

        // Seat each channel with a short brass mark at the foot of the well.
        for column in 0..COLUMNS {
            let x = self.origin_at(0., column).x + px(self.cell * 0.5 - 3.);
            bar(
                window,
                point(x, bounds.bottom() + px(3.)),
                6.,
                1.,
                color(0xd5a971, 0.68),
            );
        }
    }

    fn light_matches(&self, board: &Board, frame: ClearFrame, window: &mut Window) {
        let pulse = (1. - (frame.progress - 0.18).abs() / 0.32).clamp(0., 1.);
        if pulse == 0. {
            return;
        }
        let marked = &frame.settling.cleared;
        for PlacedGem { row, column, gem } in frame.settling.light_sites.into_iter().flatten() {
            let at = self.stone(row as f32, column);
            window.paint_quad(fill(at, color(glint(gem), 0.12 * pulse)));
            for (r, c) in [(row + 1, column), (row, column + 1)] {
                if r < ROWS && c < COLUMNS && marked[r][c] && board[r][c] == Some(gem) {
                    rail(
                        window,
                        at.center(),
                        self.stone(r as f32, c).center(),
                        3.,
                        color(glint(gem), 0.85 * pulse),
                    );
                }
            }
        }
    }

    fn scatter_chips(&self, frame: ClearFrame, window: &mut Window) {
        let t = (frame.progress - 0.25) / 0.69;
        if !(0. ..1.).contains(&t) {
            return;
        }
        let drift = self.cell * (1. - (1. - t).powi(2));
        let alpha = (1. - t).powi(2);
        for cell in frame.settling.spark_sites.into_iter().flatten() {
            let center = self.stone(cell.row as f32, cell.column).center();
            let mirror = if (cell.row + cell.column).is_multiple_of(2) {
                1.
            } else {
                -1.
            };
            for (dx, dy, width, tilt) in [
                (-0.59, -0.44, 0.08, -0.8),
                (0.38, -0.70, 0.11, 0.6),
                (0.61, 0.27, 0.06, 1.2),
            ] {
                let center = center
                    + point(
                        px(dx * drift * mirror),
                        px(dy * drift + t * t * self.cell * 0.36),
                    );
                let half = self.cell * width * (1. - t * 0.62);
                let mut fragment = PathBuilder::fill();
                fragment.move_to(center + point(px(half * tilt * mirror), px(-half * 1.4)));
                fragment.line_to(center + point(px(half), px(half * 0.4)));
                fragment.line_to(center + point(px(-half), px(half * 0.72)));
                fragment.close();
                if let Ok(path) = fragment.build() {
                    window.paint_path(path, color(glint(cell.gem), alpha));
                }
            }
        }
    }
}

fn scale_about_center(bounds: Bounds<Pixels>, scale: f32) -> Bounds<Pixels> {
    let half_x = bounds.size.width * ((1. - scale) * 0.5);
    let half_y = bounds.size.height * ((1. - scale) * 0.5);
    Bounds::new(
        bounds.origin + point(half_x, half_y),
        size(bounds.size.width * scale, bounds.size.height * scale),
    )
}

fn color(hex: u32, alpha: f32) -> Hsla {
    Hsla {
        a: alpha,
        ..rgb(hex).into()
    }
}

fn bar(window: &mut Window, origin: Point<Pixels>, width: f32, height: f32, color: Hsla) {
    window.paint_quad(fill(
        Bounds::new(origin, size(px(width), px(height))),
        color,
    ));
}

fn rail(window: &mut Window, a: Point<Pixels>, b: Point<Pixels>, thickness: f32, color: Hsla) {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    if a.x == b.x {
        bar(
            window,
            point(x - px(thickness * 0.5), y),
            thickness,
            f32::from((a.y - b.y).abs()),
            color,
        );
    } else {
        bar(
            window,
            point(x, y - px(thickness * 0.5)),
            f32::from((a.x - b.x).abs()),
            thickness,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use corner_table_gem_cascade::{Direction, Game, Rotation};

    use super::*;

    #[test]
    fn visual_settling_meets_the_next_rules_snapshot() {
        let mut game = Game::new(19);
        let setup = [
            "", "L", "UUURRR", "UUU", "UR", "UUUR", "URR", "UUU", "R", "UR", "UURRR",
        ];
        for moves in setup {
            turn_and_shift(&mut game, moves);
            game.lock_pair().unwrap();
        }
        turn_and_shift(&mut game, "UUR");
        let result = game.lock_pair().unwrap();
        let [first, second] = result.waves.as_slice() else {
            panic!("expected a two-wave chain");
        };
        for (wave, next) in [(first, &second.board), (second, game.board())] {
            let layout = Settling::new(wave);
            let mut projected = [[None; COLUMNS]; ROWS];
            for (row, cells) in wave.board.iter().enumerate() {
                for (column, &gem) in cells.iter().enumerate() {
                    if gem.is_some() && !layout.cleared[row][column] {
                        projected[layout.rows[row][column]][column] = gem;
                    }
                }
            }
            assert_eq!(&projected, next);
        }
    }

    fn turn_and_shift(game: &mut Game, moves: &str) {
        for movement in moves.chars() {
            assert!(match movement {
                'U' => game.rotate(Rotation::Clockwise),
                'L' => game.shift(Direction::Left),
                'R' => game.shift(Direction::Right),
                _ => unreachable!(),
            });
        }
    }
}
