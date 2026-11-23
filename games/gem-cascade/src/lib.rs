//! Deterministic rules for Gem Cascade, a turn-based dropping-pair game.

pub const COLUMNS: usize = 6;
pub const ROWS: usize = 12;
pub const GROUP_SIZE: usize = 4;
pub const SPAWN_COLUMN: usize = COLUMNS / 2 - 1;
pub const SPAWN_ROW: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gem {
    Ruby,
    Amber,
    Jade,
    Amethyst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedGem {
    pub column: usize,
    pub row: usize,
    pub gem: Gem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    Clockwise,
    Counterclockwise,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clear {
    pub gems: usize,
    pub chain: u32,
    pub points: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    board: [[Option<Gem>; COLUMNS]; ROWS],
    active: Option<ActivePair>,
    next: [Gem; 2],
    seed: u64,
    random: u64,
    score: u32,
    last_clear: Clear,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut random = seed;
        let pair = next_pair(&mut random);
        let next = next_pair(&mut random);
        Self {
            board: [[None; COLUMNS]; ROWS],
            active: Some(ActivePair::spawn(pair)),
            next,
            seed,
            random,
            score: 0,
            last_clear: Clear::default(),
        }
    }

    pub fn restart(&mut self) {
        *self = Self::new(self.seed);
    }

    pub fn board(&self) -> &[[Option<Gem>; COLUMNS]; ROWS] {
        &self.board
    }

    /// The pivot comes first in both the current pair and the preview.
    pub fn active(&self) -> Option<[PlacedGem; 2]> {
        self.active.and_then(ActivePair::cells)
    }

    pub fn next_pair(&self) -> [Gem; 2] {
        self.next
    }

    pub fn score(&self) -> u32 {
        self.score
    }

    /// The result of the most recent drop, including a zero for drops without a clear.
    pub fn last_clear(&self) -> Clear {
        self.last_clear
    }

    pub fn is_over(&self) -> bool {
        self.active.is_none()
    }

    pub fn shift(&mut self, direction: Direction) -> bool {
        let Some(mut pair) = self.active else {
            return false;
        };
        pair.column += match direction {
            Direction::Left => -1,
            Direction::Right => 1,
        };
        self.place_active(pair)
    }

    pub fn rotate(&mut self, rotation: Rotation) -> bool {
        let Some(pair) = self.active else {
            return false;
        };
        let orientation = pair.orientation.rotate(rotation);
        // Try a one-column kick when the second gem would hit a wall or a stack.
        [0, -1, 1].into_iter().any(|kick| {
            self.place_active(ActivePair {
                column: pair.column + kick,
                orientation,
                ..pair
            })
        })
    }

    /// Where the two gems will settle before any groups clear.
    pub fn landing(&self) -> Option<[PlacedGem; 2]> {
        let [mut pivot, mut satellite] = self.active()?;
        if pivot.column != satellite.column {
            self.fall(&mut pivot);
            self.fall(&mut satellite);
        } else if pivot.row > satellite.row {
            self.fall(&mut pivot);
            satellite.row = pivot.row - 1;
        } else {
            self.fall(&mut satellite);
            pivot.row = satellite.row - 1;
        }
        Some([pivot, satellite])
    }

    pub fn drop_pair(&mut self) -> Option<Clear> {
        for cell in self.landing()? {
            self.board[cell.row][cell.column] = Some(cell.gem);
        }
        self.last_clear = self.resolve();
        self.score = self.score.saturating_add(self.last_clear.points);

        self.active = None;
        if self.place_active(ActivePair::spawn(self.next)) {
            self.next = next_pair(&mut self.random);
        }
        Some(self.last_clear)
    }

    fn place_active(&mut self, pair: ActivePair) -> bool {
        if pair.cells().is_some_and(|cells| {
            cells
                .iter()
                .all(|cell| self.board[cell.row][cell.column].is_none())
        }) {
            self.active = Some(pair);
            return true;
        }
        false
    }

    fn fall(&self, cell: &mut PlacedGem) {
        while cell.row + 1 < ROWS && self.board[cell.row + 1][cell.column].is_none() {
            cell.row += 1;
        }
    }

    fn resolve(&mut self) -> Clear {
        let mut result = Clear::default();
        loop {
            let matches = self.matching_cells();
            if matches.is_empty() {
                return result;
            }
            result.chain += 1;
            result.gems += matches.len();
            result.points += 10 * matches.len() as u32 * result.chain;
            for (row, column) in matches {
                self.board[row][column] = None;
            }
            self.settle();
        }
    }

    fn matching_cells(&self) -> Vec<(usize, usize)> {
        let mut seen = [[false; COLUMNS]; ROWS];
        let mut group = Vec::new();
        let mut matches = Vec::new();
        for (row, cells) in self.board.iter().enumerate() {
            for (column, &gem) in cells.iter().enumerate() {
                if gem.is_none() || seen[row][column] {
                    continue;
                }
                group.clear();
                group.push((row, column));
                seen[row][column] = true;
                let mut index = 0;
                while let Some(&(y, x)) = group.get(index) {
                    index += 1;
                    for (ny, nx) in neighbors(y, x) {
                        if !seen[ny][nx] && self.board[ny][nx] == gem {
                            seen[ny][nx] = true;
                            group.push((ny, nx));
                        }
                    }
                }
                if group.len() >= GROUP_SIZE {
                    matches.extend(&group);
                }
            }
        }
        matches
    }

    fn settle(&mut self) {
        for column in 0..COLUMNS {
            let mut destination = ROWS;
            for row in (0..ROWS).rev() {
                if let Some(gem) = self.board[row][column].take() {
                    destination -= 1;
                    self.board[destination][column] = Some(gem);
                }
            }
        }
    }
}

fn neighbors(row: usize, column: usize) -> impl Iterator<Item = (usize, usize)> {
    [
        row.checked_sub(1).map(|above| (above, column)),
        (row + 1 < ROWS).then_some((row + 1, column)),
        column.checked_sub(1).map(|left| (row, left)),
        (column + 1 < COLUMNS).then_some((row, column + 1)),
    ]
    .into_iter()
    .flatten()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActivePair {
    gems: [Gem; 2],
    column: i8,
    orientation: Orientation,
}

impl ActivePair {
    fn spawn(gems: [Gem; 2]) -> Self {
        Self {
            gems,
            column: SPAWN_COLUMN as i8,
            orientation: Orientation::Above,
        }
    }

    fn cells(self) -> Option<[PlacedGem; 2]> {
        let (row, offset) = match self.orientation {
            Orientation::Above => (SPAWN_ROW - 1, 0),
            Orientation::Right => (SPAWN_ROW, 1),
            Orientation::Below => (SPAWN_ROW + 1, 0),
            Orientation::Left => (SPAWN_ROW, -1),
        };
        let satellite = self.column + offset;
        if !(0..COLUMNS as i8).contains(&self.column) || !(0..COLUMNS as i8).contains(&satellite) {
            return None;
        }
        Some([
            PlacedGem {
                row: SPAWN_ROW,
                column: self.column as usize,
                gem: self.gems[0],
            },
            PlacedGem {
                row,
                column: satellite as usize,
                gem: self.gems[1],
            },
        ])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Orientation {
    Above,
    Right,
    Below,
    Left,
}

impl Orientation {
    fn rotate(self, rotation: Rotation) -> Self {
        use Orientation::*;
        use Rotation::*;
        match (self, rotation) {
            (Above, Clockwise) | (Below, Counterclockwise) => Right,
            (Right, Clockwise) | (Left, Counterclockwise) => Below,
            (Below, Clockwise) | (Above, Counterclockwise) => Left,
            (Left, Clockwise) | (Right, Counterclockwise) => Above,
        }
    }
}

fn next_pair(random: &mut u64) -> [Gem; 2] {
    let mut gem = || {
        // SplitMix64 yields the same stream for a seed on every supported platform.
        *random = random.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = *random;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        match (value ^ (value >> 31)) & 3 {
            0 => Gem::Ruby,
            1 => Gem::Amber,
            2 => Gem::Jade,
            _ => Gem::Amethyst,
        }
    };
    [gem(), gem()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use Gem::*;

    #[test]
    fn restart_replays_the_seed_and_preview_matches_the_next_spawn() {
        let initial = Game::new(7);
        let mut game = initial.clone();
        let next = game.next_pair();
        game.shift(Direction::Right);
        game.rotate(Rotation::Clockwise);
        game.drop_pair().unwrap();
        assert_eq!(game.active().unwrap().map(|cell| cell.gem), next);
        game.restart();
        assert_eq!(game, initial);
    }

    #[test]
    fn wall_rotation_and_uneven_stacks_use_actual_landing_cells() {
        let mut game = Game::new(0);
        game.board[ROWS - 1][0] = Some(Amber);
        game.active = Some(ActivePair::spawn([Jade, Ruby]));
        while game.shift(Direction::Left) {}
        assert!(game.rotate(Rotation::Counterclockwise));
        assert!(!game.shift(Direction::Left));
        let [pivot, satellite] = game.landing().unwrap();
        assert_eq!((pivot.row, pivot.column), (ROWS - 1, 1));
        assert_eq!((satellite.row, satellite.column), (ROWS - 2, 0));
        game.drop_pair().unwrap();
        assert_eq!(game.board[pivot.row][pivot.column], Some(Jade));
        assert_eq!(game.board[satellite.row][satellite.column], Some(Ruby));
    }

    #[test]
    fn gravity_connects_a_second_group_and_doubles_its_points() {
        let mut game = Game::new(0);
        game.active = Some(ActivePair::spawn([Ruby, Amber]));
        game.board[ROWS - 1][0] = Some(Jade);
        for column in 1..=3 {
            game.board[ROWS - 1][column] = Some(Ruby);
            game.board[ROWS - 2][column] = Some(Jade);
        }
        game.shift(Direction::Right);
        game.shift(Direction::Right);
        let result = game.drop_pair().unwrap();
        assert_eq!(
            result,
            Clear {
                gems: 8,
                chain: 2,
                points: 120,
            }
        );
        assert_eq!(game.score(), 120);
        assert_eq!(game.board[ROWS - 1][4], Some(Amber));
        assert_eq!(
            game.board
                .iter()
                .flatten()
                .filter(|cell| cell.is_some())
                .count(),
            1
        );
    }

    #[test]
    fn a_blocked_spawn_ends_the_game() {
        let mut game = Game::new(0);
        game.active = Some(ActivePair::spawn([Ruby, Amethyst]));
        for row in 2..ROWS {
            game.board[row][SPAWN_COLUMN] = Some(if row % 2 == 0 { Amber } else { Jade });
        }
        assert_eq!(game.drop_pair(), Some(Clear::default()));
        assert!(game.is_over());
        assert_eq!(game.drop_pair(), None);
        assert!(!game.shift(Direction::Left));
        assert!(!game.rotate(Rotation::Clockwise));
    }
}
