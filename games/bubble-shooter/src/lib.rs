//! Rules and shot geometry for Bubble Shooter. Coordinates use one bubble diameter as a unit.

mod puzzles;
mod stages;

pub use puzzles::{PUZZLE_COUNT, PUZZLES, Puzzle};
pub use stages::{STAGE_COUNT, STAGES, Stage};

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
const TICKS_PER_DEGREE: f64 = 8.0;

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

    fn from_letter(letter: u8) -> Option<Self> {
        match letter {
            b'C' => Some(Self::Coral),
            b'G' => Some(Self::Gold),
            b'J' => Some(Self::Jade),
            b'V' => Some(Self::Violet),
            b'.' => None,
            _ => unreachable!("unknown rack bubble"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Playing,
    Won,
    HitDangerLine,
    OutOfShots,
    BankGoalMissed,
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

/// Shot angle from vertical in eighths of a degree, positive toward the right wall.
/// Store the ticks to replay input independently of window size and scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Angle(i16);

impl Angle {
    pub const MAX: i16 = 608;

    pub fn from_ticks(ticks: i16) -> Option<Self> {
        (-Self::MAX..=Self::MAX)
            .contains(&ticks)
            .then_some(Self(ticks))
    }

    pub fn toward(target: Point) -> Option<Self> {
        let rise = SHOOTER.y - target.y;
        if !target.x.is_finite() || !rise.is_finite() || rise < MIN_AIM_Y {
            return None;
        }
        let degrees = (target.x - SHOOTER.x).atan2(rise).to_degrees();
        let ticks = (degrees * TICKS_PER_DEGREE).round() as i16;
        Some(Self(ticks.clamp(-Self::MAX, Self::MAX)))
    }

    pub fn adjusted(self, ticks: i16) -> Self {
        Self(self.0.saturating_add(ticks).clamp(-Self::MAX, Self::MAX))
    }

    pub fn ticks(self) -> i16 {
        self.0
    }

    pub fn direction(self) -> Point {
        let radians = (f64::from(self.0) / TICKS_PER_DEGREE).to_radians();
        let (x, y) = radians.sin_cos();
        Point { x, y: -y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    row: usize,
    column: usize,
}

impl Cell {
    pub fn row(self) -> usize {
        self.row
    }

    pub fn column(self) -> usize {
        self.column
    }

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
    pub matching: usize,
}

impl Aim {
    pub fn bounces(&self) -> usize {
        self.path.len().saturating_sub(2)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    pub popped: Vec<(Cell, Color)>,
    pub dropped: Vec<(Cell, Color)>,
    pub points: usize,
    pub streak: usize,
    pub bounces: usize,
    pub clear_bonus: usize,
}

#[derive(Clone)]
pub struct Round {
    grid: Grid<Option<Color>>,
    random: u64,
    current: Color,
    next: Color,
    color_order: [Color; 4],
    stage_index: Option<usize>,
    puzzle: Option<PuzzleRun>,
    shot_limit: usize,
    shots_left: usize,
    score: usize,
    streak: usize,
    swapped: bool,
    status: Status,
}

#[derive(Clone, Copy)]
struct PuzzleRun {
    index: usize,
    banked_matches: usize,
}

impl Round {
    pub fn new(seed: u64) -> Self {
        let mut round = Self::empty(seed, None, SHOT_LIMIT);

        // Each pair of rows shares patches. Shifting the seams avoids tall columns
        // and leaves several two-bubble matches visible along the starting edge.
        for top in (0..STARTING_ROWS).step_by(2) {
            let offset = round.random_index(2);
            let patches: [Color; COLUMNS / 2 + 1] =
                std::array::from_fn(|_| Color::ALL[round.random_index(Color::ALL.len())]);
            for (row, bubbles) in round.grid[..STARTING_ROWS]
                .iter_mut()
                .enumerate()
                .skip(top)
                .take(2)
            {
                for cell in Cell::in_row(row) {
                    bubbles[cell.column] = Some(patches[(cell.column + offset) / 2]);
                }
            }
        }

        round.current = round.draw_color(None);
        round.next = round.draw_next();
        round
    }

    pub fn stage(index: usize) -> Option<Self> {
        let stage = STAGES.get(index)?;
        let seed = 0x5eab_b1e5_u64.wrapping_add(index as u64);
        let mut round = Self::empty(seed, Some(index), stage.shots);
        round.fill_rack(stage.rack, false, Color::ALL);
        round.current = if index < 2 {
            Color::Gold
        } else {
            round.draw_color(None)
        };
        round.next = round.draw_next();
        Some(round)
    }

    /// Identical challenge IDs should supply the same seed. The seed picks a
    /// later stage and remixes its rack while preserving its tested shot budget.
    pub fn challenge(seed: u64) -> Self {
        let mut random = seed;
        let index = 2 + random_index(&mut random, STAGE_COUNT - 2);
        let mirrored = random_index(&mut random, 2) == 1;
        let mut colors = Color::ALL;
        for end in (1..colors.len()).rev() {
            colors.swap(end, random_index(&mut random, end + 1));
        }
        let mut round = Self::stage(index).expect("challenge selects a stage in range");
        round.fill_rack(STAGES[index].rack, mirrored, colors);
        round.current = colors[round.current as usize];
        round.next = colors[round.next as usize];
        round.color_order = colors;
        round
    }

    pub fn puzzle(index: usize) -> Option<Self> {
        let puzzle = PUZZLES.get(index)?;
        let mut round = Self::empty(0, None, puzzle.shots());
        round.puzzle = Some(PuzzleRun {
            index,
            banked_matches: 0,
        });
        round.fill_rack(puzzle.rack, false, Color::ALL);
        round.current = puzzle.ammo[0];
        round.next = puzzle.ammo.get(1).copied().unwrap_or(round.current);
        Some(round)
    }

    fn empty(seed: u64, stage_index: Option<usize>, shot_limit: usize) -> Self {
        Self {
            grid: [[None; COLUMNS]; ROWS],
            random: seed,
            current: Color::Coral,
            next: Color::Coral,
            color_order: Color::ALL,
            stage_index,
            puzzle: None,
            shot_limit,
            shots_left: shot_limit,
            score: 0,
            streak: 0,
            swapped: false,
            status: Status::Playing,
        }
    }

    fn fill_rack(&mut self, rack: &[&str], mirrored: bool, colors: [Color; 4]) {
        for (row, layout) in rack.iter().enumerate() {
            debug_assert_eq!(layout.len(), COLUMNS - row % 2);
            for (column, letter) in layout.bytes().enumerate() {
                let col = if mirrored {
                    layout.len() - 1 - column
                } else {
                    column
                };
                self.grid[row][col] =
                    Color::from_letter(letter).map(|color| colors[color as usize]);
            }
        }
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

    pub fn shots_used(&self) -> usize {
        self.shot_limit - self.shots_left
    }

    pub fn stage_info(&self) -> Option<&'static Stage> {
        self.stage_index.map(|index| &STAGES[index])
    }

    pub fn puzzle_info(&self) -> Option<&'static Puzzle> {
        self.puzzle.map(|run| &PUZZLES[run.index])
    }

    pub fn banked_matches(&self) -> usize {
        self.puzzle.map_or(0, |run| run.banked_matches)
    }

    pub fn score(&self) -> usize {
        self.score
    }

    pub fn stars(&self) -> usize {
        if self.status != Status::Won {
            return 0;
        }
        self.stage_info().map_or(0, |stage| {
            1 + usize::from(self.shots_used() <= stage.par + 2)
                + usize::from(self.shots_used() <= stage.par)
        })
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn can_swap(&self) -> bool {
        self.status == Status::Playing && !self.swapped && self.current != self.next
    }

    pub fn swap(&mut self) -> bool {
        if !self.can_swap() {
            return false;
        }
        std::mem::swap(&mut self.current, &mut self.next);
        self.swapped = true;
        true
    }

    pub fn aim(&self, target: Point) -> Option<Aim> {
        self.aim_at(Angle::toward(target)?)
    }

    pub fn aim_at(&self, angle: Angle) -> Option<Aim> {
        if self.status != Status::Playing {
            return None;
        }

        let mut direction = angle.direction();
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
        let matching = self
            .connected(landing.neighbors(), |color| color == self.current)
            .iter()
            .flatten()
            .filter(|&&reached| reached)
            .count()
            + 1;
        Some(Aim {
            path,
            landing,
            matching,
        })
    }

    pub fn fire(&mut self, target: Point) -> Option<Shot> {
        self.fire_at(Angle::toward(target)?)
    }

    pub fn fire_at(&mut self, angle: Angle) -> Option<Shot> {
        let aim = self.aim_at(angle)?;
        let cell = aim.landing;
        self.grid[cell.row][cell.column] = Some(self.current);
        self.shots_left -= 1;
        self.swapped = false;

        let mut shot = Shot {
            popped: Vec::new(),
            dropped: Vec::new(),
            points: 0,
            streak: 0,
            bounces: aim.bounces(),
            clear_bonus: 0,
        };
        if aim.matching >= 3 {
            let cluster = self.connected([cell], |color| color == self.current);
            shot.popped = self.remove_where(cluster, true);
            let anchored = self.connected(Cell::in_row(0), |_| true);
            shot.dropped = self.remove_where(anchored, false);
            self.streak += 1;
            shot.streak = self.streak;
            shot.points = 10 * shot.popped.len()
                + 25 * shot.dropped.len()
                + 15 * usize::from(shot.bounces > 0)
                + 10 * (self.streak - 1);
            if shot.bounces > 0
                && let Some(run) = &mut self.puzzle
            {
                run.banked_matches += 1;
            }
        } else {
            self.streak = 0;
        }

        self.status = if self.bubbles().next().is_none() {
            if self
                .puzzle_info()
                .is_some_and(|puzzle| self.banked_matches() < puzzle.bank_target)
            {
                Status::BankGoalMissed
            } else {
                shot.clear_bonus = 30 * self.shots_left;
                Status::Won
            }
        } else if self.grid[ROWS - 1].iter().any(Option::is_some) {
            Status::HitDangerLine
        } else if self.shots_left == 0 {
            Status::OutOfShots
        } else {
            self.reload();
            Status::Playing
        };
        self.score += shot.points + shot.clear_bonus;
        Some(shot)
    }

    fn reload(&mut self) {
        if let Some(puzzle) = self.puzzle_info() {
            self.current = self.next;
            self.next = puzzle
                .ammo
                .get(self.shots_used() + 1)
                .copied()
                .unwrap_or(self.current);
        } else {
            self.current = if self.bubbles().any(|(_, color)| color == self.next) {
                self.next
            } else {
                self.draw_color(None)
            };
            self.next = self.draw_next();
        }
    }

    fn remove_where(&mut self, mask: Grid<bool>, remove_marked: bool) -> Vec<(Cell, Color)> {
        let mut removed = Vec::new();
        for cell in (0..ROWS).flat_map(Cell::in_row) {
            if mask[cell.row][cell.column] == remove_marked
                && let Some(color) = self.grid[cell.row][cell.column].take()
            {
                removed.push((cell, color));
            }
        }
        removed
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

    fn draw_next(&mut self) -> Color {
        let alternate = self.stage_index.map(|_| self.current);
        self.draw_color(alternate)
    }

    fn draw_color(&mut self, alternate: Option<Color>) -> Color {
        let mut visible = [false; Color::ALL.len()];
        for (cell, color) in self.bubbles() {
            if cell.neighbors().any(|neighbor| {
                neighbor.row > cell.row && self.grid[neighbor.row][neighbor.column].is_none()
            }) {
                visible[color as usize] = true;
            }
        }
        // The stage rack keeps a second color available to swap in whenever
        // possible. Classic retains its unrestricted draw.
        if let Some(color) = alternate
            && visible
                .iter()
                .enumerate()
                .any(|(index, &seen)| index != color as usize && seen)
        {
            visible[color as usize] = false;
        }
        let count = visible.iter().filter(|&&present| present).count();
        if count == 0 {
            return self.current;
        }
        let index = self.random_index(count);
        self.color_order
            .into_iter()
            .filter(|&color| visible[color as usize])
            .nth(index)
            .expect("playing round has an exposed bubble")
    }

    fn random_index(&mut self, count: usize) -> usize {
        random_index(&mut self.random, count)
    }
}

// SplitMix64 gives nearby round seeds unrelated boards and a stable shot sequence.
fn random_index(seed: &mut u64, count: usize) -> usize {
    *seed = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut random = *seed;
    random = (random ^ (random >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    random = (random ^ (random >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((random ^ (random >> 31)) % count as u64) as usize
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

    // Each pair is an angle in ticks and whether to swap before shooting.
    const PAR_ROUTES: [&[(i16, bool)]; STAGE_COUNT] = [
        &[(0, false), (0, false)],
        &[(32, true), (-8, false)],
        &[(-152, true), (80, true), (-88, true), (-88, true)],
        &[
            (176, true),
            (-144, true),
            (-136, false),
            (224, false),
            (-112, false),
            (16, false),
        ],
        &[
            (-144, false),
            (-176, false),
            (-88, false),
            (-8, false),
            (-72, true),
            (16, false),
        ],
        &[
            (-288, false),
            (8, true),
            (0, false),
            (32, true),
            (-136, true),
            (96, false),
            (8, false),
        ],
    ];

    fn play_route(mut round: Round, route: &[(i16, bool)], direction: i16) -> Option<Round> {
        for &(angle, swap) in route {
            if swap && !round.swap() {
                return None;
            }
            round.fire_at(Angle::from_ticks(direction * angle).unwrap())?;
        }
        Some(round)
    }

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
    fn every_stage_is_anchored_and_has_a_three_star_route() {
        for (index, route) in PAR_ROUTES.into_iter().enumerate() {
            let stage = Round::stage(index).unwrap();
            let anchored = stage.connected(Cell::in_row(0), |_| true);
            assert!(
                stage
                    .bubbles()
                    .all(|(cell, _)| anchored[cell.row][cell.column])
            );
            let completed = play_route(stage, route, 1).unwrap();
            assert_eq!(completed.status(), Status::Won, "stage {}", index + 1);
            assert_eq!(completed.stars(), 3);
        }
        assert!(Round::stage(STAGE_COUNT).is_none());
    }

    #[test]
    fn challenge_seeds_preserve_the_stage_solution_and_retry() {
        for seed in 0..64 {
            let first = Round::challenge(seed);
            let retry = Round::challenge(seed);
            assert_eq!(
                (first.grid, first.current, first.next),
                (retry.grid, retry.current, retry.next)
            );
            let route = PAR_ROUTES[first.stage_index.unwrap()];
            assert!(
                [1, -1].into_iter().any(|direction| {
                    play_route(first.clone(), route, direction)
                        .is_some_and(|result| result.status() == Status::Won && result.stars() == 3)
                }),
                "seed {seed}"
            );
        }
    }

    #[test]
    fn a_swap_is_free_and_resets_when_a_bubble_is_shot() {
        let mut round = Round::stage(1).unwrap();
        let (current, next) = (round.current(), round.next());
        let remaining = round.shots_left();
        assert!(round.swap());
        assert!(!round.swap());
        assert_eq!((round.current(), round.next()), (next, current));
        assert_eq!(round.shots_left(), remaining);
        round.fire_at(Angle::from_ticks(32).unwrap()).unwrap();
        assert!(!round.swapped);
        assert_eq!(round.shots_left(), remaining - 1);
    }

    #[test]
    fn score_counts_bank_matches_streaks_and_unused_shots() {
        let mut round = Round::stage(0).unwrap();
        let angle = Angle::from_ticks(-604).unwrap();
        let first = round.fire_at(angle).unwrap();
        assert!(first.bounces > 0);
        assert_eq!(first.points, 85);
        assert_eq!(first.streak, 1);
        let last = round.fire_at(angle).unwrap();
        assert!(last.bounces > 0);
        assert_eq!(last.points, 75);
        assert_eq!(last.streak, 2);
        assert_eq!(last.clear_bonus, 180);
        assert_eq!(round.score(), 340);
        assert_eq!(round.status(), Status::Won);
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
        let shot = round.fire(target).unwrap();
        assert!(shot.popped.is_empty());
        assert!(shot.dropped.is_empty());
        assert_eq!(shot.points, 0);
        assert_eq!(shot.bounces, 1);
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
        let shot = round.fire(target).unwrap();
        assert_eq!(shot.popped.len(), 3);
        assert_eq!(shot.dropped.len(), 2);
        assert_eq!(shot.points, 80);
        assert_eq!(round.score(), 80);
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
