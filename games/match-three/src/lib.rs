pub const SIDE: usize = 8;
pub const MOVE_LIMIT: u32 = 24;
pub const GOAL: u32 = 900;
pub const CELLS: usize = SIDE * SIDE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gem {
    Ruby,
    Amber,
    Peridot,
    Aquamarine,
    Amethyst,
    Moonstone,
}

const GEMS: [Gem; 6] = [
    Gem::Ruby,
    Gem::Amber,
    Gem::Peridot,
    Gem::Aquamarine,
    Gem::Amethyst,
    Gem::Moonstone,
];

impl Gem {
    pub fn name(self) -> &'static str {
        match self {
            Gem::Ruby => "Ruby",
            Gem::Amber => "Amber",
            Gem::Peridot => "Peridot",
            Gem::Aquamarine => "Aquamarine",
            Gem::Amethyst => "Amethyst",
            Gem::Moonstone => "Moonstone",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Goal {
    Score(u32),
    Collect(Gem, u32),
    Chains(u32),
}

impl Goal {
    pub fn target(self) -> u32 {
        match self {
            Self::Score(target) | Self::Collect(_, target) | Self::Chains(target) => target,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Stage {
    pub name: &'static str,
    pub moves: u32,
    pub goals: &'static [Goal],
}

pub const STAGES: [Stage; 4] = [
    Stage {
        name: "Lantern Grotto",
        moves: MOVE_LIMIT,
        goals: &[Goal::Score(GOAL)],
    },
    Stage {
        name: "Amber Vein",
        moves: 24,
        goals: &[
            Goal::Collect(Gem::Amber, 16),
            Goal::Collect(Gem::Peridot, 16),
        ],
    },
    Stage {
        name: "Echo Chamber",
        moves: 22,
        goals: &[Goal::Chains(4)],
    },
    Stage {
        name: "Moonstone Vault",
        moves: 24,
        goals: &[Goal::Collect(Gem::Moonstone, 18), Goal::Score(1_100)],
    },
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub score: u32,
    collected: [u32; 6],
    pub chains: u32,
    pub best_chain: u32,
}

impl Progress {
    pub fn value(self, goal: Goal) -> u32 {
        match goal {
            Goal::Score(_) => self.score,
            Goal::Collect(gem, _) => self.collected[gem as usize],
            Goal::Chains(_) => self.chains,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub row: usize,
    pub col: usize,
}

impl Cell {
    pub fn from_index(index: usize) -> Self {
        Self {
            row: index / SIDE,
            col: index % SIDE,
        }
    }

    pub fn index(self) -> Option<usize> {
        (self.row < SIDE && self.col < SIDE).then(|| self.row * SIDE + self.col)
    }

    pub fn touches(self, other: Self) -> bool {
        (self.row == other.row && self.col.abs_diff(other.col) == 1)
            || (self.col == other.col && self.row.abs_diff(other.row) == 1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwapError {
    Finished,
    InvalidCells,
    NoMatch,
}

#[derive(Clone, Debug)]
pub struct Cascade {
    /// The matched board before clearing and the resulting board after gravity and refill.
    pub before: [Gem; CELLS],
    pub cleared: [bool; CELLS],
    pub after: [Gem; CELLS],
    /// Indexed by destination. New gems have origins above the board.
    pub fall_distance: [u8; CELLS],
    pub points: u32,
    pub progress: Progress,
}

#[derive(Clone, Debug)]
pub struct Turn {
    pub points: u32,
    pub gems: u32,
    pub cascades: Vec<Cascade>,
    pub reshuffled: bool,
}

#[derive(Clone, Debug)]
pub struct Game {
    board: [Gem; CELLS],
    stage: Stage,
    progress: Progress,
    moves: u32,
    rng: u64,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Self::for_stage(seed, STAGES[0])
    }

    pub fn for_stage(seed: u64, stage: Stage) -> Self {
        let mut game = Self {
            board: [Gem::Ruby; CELLS],
            stage,
            progress: Progress::default(),
            moves: stage.moves,
            rng: seed,
        };
        game.deal();
        game
    }

    pub fn gem(&self, cell: Cell) -> Option<Gem> {
        cell.index().map(|index| self.board[index])
    }

    pub fn score(&self) -> u32 {
        self.progress.score
    }

    pub fn board(&self) -> &[Gem; CELLS] {
        &self.board
    }

    pub fn progress(&self) -> Progress {
        self.progress
    }

    pub fn moves_left(&self) -> u32 {
        self.moves
    }

    pub fn status(&self) -> Status {
        if self
            .stage
            .goals
            .iter()
            .all(|&goal| self.progress.value(goal) >= goal.target())
        {
            Status::Won
        } else if self.moves == 0 {
            Status::Lost
        } else {
            Status::Playing
        }
    }

    pub fn hint(&self) -> Option<(Cell, Cell)> {
        self.valid_swaps().next()
    }

    /// Suggest a legal swap, prioritizing progress on the active stage's remaining goals.
    pub fn suggestion(&self) -> Option<(Cell, Cell)> {
        if self.status() != Status::Playing {
            return None;
        }
        self.valid_swaps().max_by_key(|&(first, second)| {
            let mut trial = self.clone();
            trial.swap(first, second).expect("a legal move");
            let goal_progress: u64 = self
                .stage
                .goals
                .iter()
                .map(|&goal| {
                    let target = goal.target();
                    let before = self.progress.value(goal).min(target);
                    let after = trial.progress.value(goal).min(target);
                    1_000 * u64::from(after - before) / u64::from(target.max(1))
                })
                .sum();
            (trial.status() == Status::Won, goal_progress, trial.score())
        })
    }

    fn valid_swaps(&self) -> impl Iterator<Item = (Cell, Cell)> + '_ {
        let mut board = self.board;
        (0..CELLS)
            .flat_map(|index| {
                let first = Cell::from_index(index);
                let Cell { row, col } = first;
                [Cell { row: row + 1, col }, Cell { row, col: col + 1 }]
                    .into_iter()
                    .filter_map(move |second| Some((index, second.index()?, first, second)))
            })
            .filter_map(move |(a, b, first, second)| {
                if board[a] == board[b] {
                    return None;
                }
                board.swap(a, b);
                let playable = matched(&board).contains(&true);
                board.swap(a, b);
                playable.then_some((first, second))
            })
    }

    pub fn swap(&mut self, first: Cell, second: Cell) -> Result<Turn, SwapError> {
        if self.status() != Status::Playing {
            return Err(SwapError::Finished);
        }
        let (Some(first_index), Some(second_index)) = (first.index(), second.index()) else {
            return Err(SwapError::InvalidCells);
        };
        if !first.touches(second) {
            return Err(SwapError::InvalidCells);
        }
        self.board.swap(first_index, second_index);
        let mut cleared = matched(&self.board);
        if !cleared.contains(&true) {
            self.board.swap(first_index, second_index);
            return Err(SwapError::NoMatch);
        }

        self.moves -= 1;
        let mut turn = Turn {
            points: 0,
            gems: 0,
            cascades: Vec::new(),
            reshuffled: false,
        };
        while cleared.contains(&true) {
            let before = self.board;
            let (gems, points) = self.record_clear(&cleared, turn.cascades.len() as u32 + 1);
            let fall_distance = self.drop_and_refill(&cleared);
            turn.gems += gems;
            turn.points += points;
            turn.cascades.push(Cascade {
                before,
                cleared,
                after: self.board,
                fall_distance,
                points,
                progress: self.progress,
            });
            cleared = matched(&self.board);
        }

        if self.status() == Status::Playing && self.hint().is_none() {
            self.deal();
            turn.reshuffled = true;
        }
        Ok(turn)
    }

    fn record_clear(&mut self, cleared: &[bool; CELLS], step: u32) -> (u32, u32) {
        let mut gems = 0;
        for (&gem, &clear) in self.board.iter().zip(cleared) {
            if clear {
                self.progress.collected[gem as usize] += 1;
                gems += 1;
            }
        }
        let points = gems * 10 * step;
        self.progress.score += points;
        self.progress.chains += u32::from(step == 2);
        self.progress.best_chain = self.progress.best_chain.max(step);
        (gems, points)
    }

    fn next_gem(&mut self) -> Gem {
        self.rng = self.rng.wrapping_add(0x9e3779b97f4a7c15);
        let mut random = self.rng;
        random = (random ^ (random >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        random = (random ^ (random >> 27)).wrapping_mul(0x94d049bb133111eb);
        random ^= random >> 31;
        GEMS[(random % GEMS.len() as u64) as usize]
    }

    fn deal(&mut self) {
        loop {
            for index in 0..CELLS {
                loop {
                    let gem = self.next_gem();
                    let row = index / SIDE;
                    let col = index % SIDE;
                    let across =
                        col >= 2 && self.board[index - 1] == gem && self.board[index - 2] == gem;
                    let down = row >= 2
                        && self.board[index - SIDE] == gem
                        && self.board[index - 2 * SIDE] == gem;
                    if !across && !down {
                        self.board[index] = gem;
                        break;
                    }
                }
            }
            if self.hint().is_some() {
                break;
            }
        }
    }

    fn drop_and_refill(&mut self, cleared: &[bool; CELLS]) -> [u8; CELLS] {
        let mut fall_distance = [0; CELLS];
        for col in 0..SIDE {
            let mut write = SIDE;
            for row in (0..SIDE).rev() {
                let index = row * SIDE + col;
                if !cleared[index] {
                    write -= 1;
                    let destination = write * SIDE + col;
                    self.board[destination] = self.board[index];
                    fall_distance[destination] = (write - row) as u8;
                }
            }
            for row in 0..write {
                let destination = row * SIDE + col;
                self.board[destination] = self.next_gem();
                fall_distance[destination] = write as u8;
            }
        }
        fall_distance
    }
}

fn matched(board: &[Gem; CELLS]) -> [bool; CELLS] {
    let mut cleared = [false; CELLS];
    for (index, gem) in board.iter().enumerate() {
        let row = index / SIDE;
        let col = index % SIDE;
        if col + 2 < SIDE && board[index + 1] == *gem && board[index + 2] == *gem {
            cleared[index..index + 3].fill(true);
        }
        if row + 2 < SIDE && board[index + SIDE] == *gem && board[index + 2 * SIDE] == *gem {
            for offset in [0, SIDE, SIDE * 2] {
                cleared[index + offset] = true;
            }
        }
    }
    cleared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deal_is_seeded_and_always_contains_a_move() {
        for seed in 0..100 {
            let game = Game::new(seed);
            assert_eq!(game.board, Game::new(seed).board);
            assert!(!matched(&game.board).contains(&true));
            assert!(game.hint().is_some());
        }
    }

    #[test]
    fn invalid_swap_keeps_the_board_and_moves() {
        let mut game = Game::new(21);
        let board = game.board;
        let first = Cell { row: 0, col: 0 };
        assert_eq!(
            game.swap(first, Cell { row: 1, col: 1 }).unwrap_err(),
            SwapError::InvalidCells
        );
        let present = [
            game.swap(first, Cell { row: 1, col: 0 }),
            game.swap(first, Cell { row: 0, col: 1 }),
        ];
        assert!(
            present
                .into_iter()
                .all(|turn| turn.unwrap_err() == SwapError::NoMatch)
        );
        assert_eq!(game.board, board);
        assert_eq!(game.moves_left(), MOVE_LIMIT);
    }

    #[test]
    fn resolving_turns_scores_every_cascade_and_reaches_an_end_state() {
        let mut saw_cascade = false;
        for seed in 0..20 {
            let mut game = Game::new(seed);
            while game.status() == Status::Playing {
                let (first, second) = game.hint().unwrap();
                let score = game.score();
                let moves = game.moves_left();
                let turn = game.swap(first, second).unwrap();
                let mut expected_points = 0;
                for (index, cascade) in turn.cascades.iter().enumerate() {
                    let gems = cascade.cleared.iter().filter(|&&gem| gem).count() as u32;
                    assert!(gems >= 3);
                    expected_points += (index as u32 + 1) * gems * 10;
                }
                assert_eq!(turn.points, expected_points);
                assert_eq!(game.score(), score + turn.points);
                assert_eq!(game.moves_left(), moves - 1);
                assert!(!matched(&game.board).contains(&true));
                saw_cascade |= turn.cascades.len() > 1;
            }
        }
        assert!(saw_cascade);
    }

    #[test]
    fn gravity_reports_each_surviving_and_new_gems_origin() {
        for seed in 0..20 {
            let mut game = Game::new(seed);
            let (first, second) = game.hint().unwrap();
            let turn = game.swap(first, second).unwrap();
            for cascade in turn.cascades {
                let mut arrived = [false; CELLS];
                for index in 0..CELLS {
                    let Cell { row, col } = Cell::from_index(index);
                    if let Some(source_row) = row.checked_sub(cascade.fall_distance[index].into()) {
                        let source = source_row * SIDE + col;
                        assert!(!cascade.cleared[source]);
                        assert!(!arrived[source]);
                        assert_eq!(cascade.before[source], cascade.after[index]);
                        arrived[source] = true;
                    }
                }
                assert!(arrived.iter().zip(cascade.cleared).all(|(&a, b)| a != b));
            }
        }
    }

    #[test]
    fn stage_goals_track_collections_and_count_each_chained_turn_once() {
        for stage in STAGES {
            let mut game = Game::for_stage(3, stage);
            let mut counts = [0; 6];
            let mut chains = 0;
            let mut best_chain = 0;
            while game.status() == Status::Playing {
                let (first, second) = game.suggestion().unwrap();
                let turn = game.swap(first, second).unwrap();
                chains += u32::from(turn.cascades.len() > 1);
                best_chain = best_chain.max(turn.cascades.len() as u32);
                for cascade in turn.cascades {
                    for (gem, clear) in cascade.before.into_iter().zip(cascade.cleared) {
                        if clear {
                            counts[gem as usize] += 1;
                        }
                    }
                    assert_eq!(cascade.progress.collected, counts);
                }
                assert_eq!(game.progress().chains, chains);
                assert_eq!(game.progress().best_chain, best_chain);
            }
            assert_eq!(game.status(), Status::Won, "{}", stage.name);
            assert!(
                stage
                    .goals
                    .iter()
                    .all(|&goal| game.progress().value(goal) >= goal.target())
            );
            assert!(game.suggestion().is_none());
        }
    }
}
