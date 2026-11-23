//! Rules and shot geometry for Bubble Shooter. Coordinates use one bubble diameter as a unit.

pub const COLUMNS: usize = 10;
pub const ROWS: usize = 12;
pub const FIELD_WIDTH: f64 = COLUMNS as f64;
pub const FIELD_HEIGHT: f64 = 12.1;
pub const RADIUS: f64 = 0.5;
pub const ROW_SPACING: f64 = 0.866_025_403_784_438_6;
pub const DANGER_Y: f64 = RADIUS / 2.0 + (ROWS - 1) as f64 * ROW_SPACING;
pub const SHOOTER: Point = Point {
    x: FIELD_WIDTH / 2.0,
    y: 11.3,
};
pub const SHOT_LIMIT: usize = 36;

const STARTING_ROWS: usize = 5;
const MIN_AIM_Y: f64 = 0.1;
const MAX_SLOPE: f64 = 4.0;

type Grid<T> = [[T; COLUMNS]; ROWS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Coral,
    Gold,
    Jade,
    Violet,
}

impl Color {
    const ALL: [Self; 4] = [Self::Coral, Self::Gold, Self::Jade, Self::Violet];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Playing,
    Won,
    HitDangerLine,
    OutOfShots,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    fn distance_squared(self, other: Self) -> f64 {
        (self.x - other.x).powi(2) + (self.y - other.y).powi(2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    row: usize,
    column: usize,
}

impl Cell {
    pub fn center(self) -> Point {
        Point {
            x: RADIUS + self.column as f64 + (self.row % 2) as f64 * RADIUS,
            y: RADIUS + self.row as f64 * ROW_SPACING,
        }
    }

    fn in_row(row: usize) -> impl Iterator<Item = Self> {
        (0..COLUMNS - row % 2).map(move |column| Self { row, column })
    }

    fn neighbors(self) -> impl Iterator<Item = Self> {
        let left = (self.row % 2) as isize - 1;
        [
            (0, -1),
            (0, 1),
            (-1, left),
            (-1, left + 1),
            (1, left),
            (1, left + 1),
        ]
        .into_iter()
        .filter_map(move |(dy, dx)| {
            let row = self.row.checked_add_signed(dy)?;
            let column = self.column.checked_add_signed(dx)?;
            (row < ROWS && column < COLUMNS - row % 2).then_some(Self { row, column })
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Aim {
    /// From the shooter through each wall contact to the first impact.
    pub path: Vec<Point>,
    pub landing: Cell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shot {
    pub popped: usize,
    pub dropped: usize,
}

pub struct Round {
    grid: Grid<Option<Color>>,
    random: u64,
    current: Color,
    next: Color,
    shots_left: usize,
    status: Status,
}

impl Round {
    pub fn new(seed: u64) -> Self {
        let mut round = Self {
            grid: [[None; COLUMNS]; ROWS],
            random: seed,
            current: Color::Coral,
            next: Color::Coral,
            shots_left: SHOT_LIMIT,
            status: Status::Playing,
        };

        // Each pair of rows shares patches. Shifting the seams avoids tall columns
        // and leaves several two-bubble matches visible along the starting edge.
        for top in (0..STARTING_ROWS).step_by(2) {
            let offset = round.random_index(2);
            let patches: [Color; COLUMNS / 2 + 1] =
                std::array::from_fn(|_| Color::ALL[round.random_index(Color::ALL.len())]);
            for row in top..(top + 2).min(STARTING_ROWS) {
                for cell in Cell::in_row(row) {
                    round.grid[row][cell.column] = Some(patches[(cell.column + offset) / 2]);
                }
            }
        }

        round.current = round.draw_color();
        round.next = round.draw_color();
        round
    }

    pub fn bubbles(&self) -> impl Iterator<Item = (Cell, Color)> + '_ {
        (0..ROWS)
            .flat_map(Cell::in_row)
            .filter_map(|cell| self.grid[cell.row][cell.column].map(|color| (cell, color)))
    }

    pub fn current(&self) -> Color {
        self.current
    }

    pub fn next(&self) -> Color {
        self.next
    }

    pub fn shots_left(&self) -> usize {
        self.shots_left
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn aim(&self, target: Point) -> Option<Aim> {
        let rise = SHOOTER.y - target.y;
        if self.status != Status::Playing
            || !target.x.is_finite()
            || !rise.is_finite()
            || rise < MIN_AIM_Y
        {
            return None;
        }

        let slope = ((target.x - SHOOTER.x) / rise).clamp(-MAX_SLOPE, MAX_SLOPE);
        let scale = slope.hypot(1.0);
        let mut direction = Point {
            x: slope / scale,
            y: -1.0 / scale,
        };
        let mut origin = SHOOTER;
        let mut path = vec![origin];

        let (impact, hit) = loop {
            let ceiling = (RADIUS - origin.y) / direction.y;
            let wall = match direction.x {
                x if x > 0.0 => (FIELD_WIDTH - RADIUS - origin.x) / x,
                x if x < 0.0 => (RADIUS - origin.x) / x,
                _ => f64::INFINITY,
            };
            let segment = wall.min(ceiling);
            let collision = self
                .bubbles()
                .filter_map(|(cell, _)| {
                    collision_distance(origin, direction, cell.center())
                        .filter(|&distance| distance <= segment)
                        .map(|distance| (cell, distance))
                })
                .min_by(|(_, a), (_, b)| a.total_cmp(b));

            if let Some((cell, distance)) = collision {
                break (advance(origin, direction, distance), Some(cell));
            }

            let end = advance(origin, direction, segment);
            if ceiling <= wall {
                break (end, None);
            }
            origin = Point {
                x: if direction.x > 0.0 {
                    FIELD_WIDTH - RADIUS
                } else {
                    RADIUS
                },
                y: end.y,
            };
            path.push(origin);
            direction.x = -direction.x;
        };
        path.push(impact);

        let landing = if let Some(hit) = hit {
            self.nearest_empty(hit.neighbors(), impact)
        } else {
            self.nearest_empty(Cell::in_row(0), impact)
        }?;
        Some(Aim { path, landing })
    }

    pub fn fire(&mut self, target: Point) -> Option<Shot> {
        let cell = self.aim(target)?.landing;
        self.grid[cell.row][cell.column] = Some(self.current);
        self.shots_left -= 1;

        let cluster = self.connected([cell], |color| color == self.current);
        let mut shot = Shot {
            popped: 0,
            dropped: 0,
        };
        if cluster.iter().flatten().filter(|&&reached| reached).count() >= 3 {
            for (row, reached) in self.grid.iter_mut().zip(cluster) {
                for (bubble, reached) in row.iter_mut().zip(reached) {
                    if reached {
                        *bubble = None;
                        shot.popped += 1;
                    }
                }
            }

            let anchored = self.connected(Cell::in_row(0), |_| true);
            for (row, reached) in self.grid.iter_mut().zip(anchored) {
                for (bubble, reached) in row.iter_mut().zip(reached) {
                    if !reached && bubble.take().is_some() {
                        shot.dropped += 1;
                    }
                }
            }
        }

        self.status = if self.bubbles().next().is_none() {
            Status::Won
        } else if self.grid[ROWS - 1].iter().any(Option::is_some) {
            Status::HitDangerLine
        } else if self.shots_left == 0 {
            Status::OutOfShots
        } else {
            self.current = self.next;
            self.next = self.draw_color();
            Status::Playing
        };
        Some(shot)
    }

    fn nearest_empty(&self, cells: impl Iterator<Item = Cell>, impact: Point) -> Option<Cell> {
        cells
            .filter(|cell| self.grid[cell.row][cell.column].is_none())
            .min_by(|a, b| {
                a.center()
                    .distance_squared(impact)
                    .total_cmp(&b.center().distance_squared(impact))
            })
    }

    fn connected(
        &self,
        seeds: impl IntoIterator<Item = Cell>,
        matches: impl Fn(Color) -> bool,
    ) -> Grid<bool> {
        let mut reached = [[false; COLUMNS]; ROWS];
        let mut frontier: Vec<_> = seeds.into_iter().collect();
        while let Some(cell) = frontier.pop() {
            let seen = &mut reached[cell.row][cell.column];
            if !*seen && self.grid[cell.row][cell.column].is_some_and(&matches) {
                *seen = true;
                frontier.extend(cell.neighbors());
            }
        }
        reached
    }

    fn draw_color(&mut self) -> Color {
        let mut visible = [false; Color::ALL.len()];
        for (cell, color) in self.bubbles() {
            if cell.neighbors().any(|neighbor| {
                neighbor.row > cell.row && self.grid[neighbor.row][neighbor.column].is_none()
            }) {
                visible[color as usize] = true;
            }
        }
        let count = visible.iter().filter(|&&present| present).count();
        let index = self.random_index(count);
        Color::ALL
            .into_iter()
            .zip(visible)
            .filter_map(|(color, present)| present.then_some(color))
            .nth(index)
            .expect("playing round has an exposed bubble")
    }

    // SplitMix64 gives nearby round seeds unrelated boards and a stable shot sequence.
    fn random_index(&mut self, count: usize) -> usize {
        self.random = self.random.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut random = self.random;
        random = (random ^ (random >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        random = (random ^ (random >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((random ^ (random >> 31)) % count as u64) as usize
    }
}

fn advance(origin: Point, direction: Point, distance: f64) -> Point {
    Point {
        x: origin.x + direction.x * distance,
        y: origin.y + direction.y * distance,
    }
}

fn collision_distance(origin: Point, direction: Point, center: Point) -> Option<f64> {
    let dx = center.x - origin.x;
    let dy = center.y - origin.y;
    let along = dx * direction.x + dy * direction.y;
    let across = dx * direction.y - dy * direction.x;
    let half_chord = (2.0 * RADIUS).powi(2) - across.powi(2);
    (along > 0.0 && half_chord >= 0.0).then(|| (along - half_chord.sqrt()).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_with(bubbles: &[(usize, usize, Color)], current: Color) -> Round {
        let mut round = Round::new(10);
        round.grid = [[None; COLUMNS]; ROWS];
        for &(row, col, color) in bubbles {
            round.grid[row][col] = Some(color);
        }
        round.current = current;
        round
    }

    #[test]
    fn a_seed_replays_the_same_board_and_shots() {
        let mut first = Round::new(12);
        let mut replay = Round::new(12);
        assert_ne!(first.grid, Round::new(13).grid);
        for x in [2.0, 3.0, 6.0, 0.5, 9.5, 4.0] {
            assert_eq!(first.grid, replay.grid);
            assert_eq!(first.current(), replay.current());
            assert_eq!(first.next(), replay.next());
            let target = Point { x, y: 6.0 };
            assert_eq!(first.fire(target), replay.fire(target));
        }
    }

    #[test]
    fn a_bank_shot_reflects_and_lands_at_the_previewed_cell() {
        let mut round = round_with(&[], Color::Gold);
        let target = Point { x: 9.5, y: 7.0 };
        let aim = round.aim(target).unwrap();
        assert_eq!(aim.path.len(), 3);
        assert_eq!(aim.path[1].x, FIELD_WIDTH - RADIUS);
        assert!(aim.path[2].x < aim.path[1].x);
        assert_eq!(aim.landing.row, 0);
        assert_eq!(
            round.fire(target),
            Some(Shot {
                popped: 0,
                dropped: 0
            })
        );
        assert_eq!(
            round.bubbles().collect::<Vec<_>>(),
            [(aim.landing, Color::Gold)]
        );
        assert_eq!(round.shots_left(), SHOT_LIMIT - 1);
    }

    #[test]
    fn popping_a_support_drops_only_the_unanchored_bubbles() {
        let mut round = round_with(
            &[
                (0, 0, Color::Violet),
                (0, 4, Color::Gold),
                (0, 5, Color::Gold),
                (1, 3, Color::Coral),
                (2, 3, Color::Jade),
            ],
            Color::Gold,
        );
        let target = Point {
            x: SHOOTER.x,
            y: 0.0,
        };
        assert_eq!(
            round.fire(target),
            Some(Shot {
                popped: 3,
                dropped: 2
            })
        );
        assert_eq!(
            round.bubbles().collect::<Vec<_>>(),
            [(Cell { row: 0, column: 0 }, Color::Violet)]
        );
        assert_eq!(round.status(), Status::Playing);
    }

    #[test]
    fn the_last_shot_can_win_and_exhaustion_ends_a_nonempty_round() {
        let rack = [(0, 4, Color::Gold), (0, 5, Color::Gold)];
        let target = Point {
            x: SHOOTER.x,
            y: 0.0,
        };
        let mut won = round_with(&rack, Color::Gold);
        won.shots_left = 1;
        won.fire(target).unwrap();
        assert_eq!(won.status(), Status::Won);
        assert!(won.fire(target).is_none());

        let mut lost = round_with(&rack, Color::Coral);
        lost.shots_left = 1;
        lost.fire(target).unwrap();
        assert_eq!(lost.status(), Status::OutOfShots);
        assert!(lost.aim(target).is_none());
    }

    #[test]
    fn a_stuck_bubble_on_the_danger_line_ends_the_round() {
        let mut round = round_with(&[(ROWS - 2, 4, Color::Jade)], Color::Gold);
        assert!(round.fire(SHOOTER).is_none());
        assert_eq!(round.shots_left(), SHOT_LIMIT);
        let target = Point {
            x: SHOOTER.x,
            y: 0.0,
        };
        assert_eq!(round.aim(target).unwrap().landing.row, ROWS - 1);
        round.fire(target).unwrap();
        assert_eq!(round.status(), Status::HitDangerLine);
    }
}
