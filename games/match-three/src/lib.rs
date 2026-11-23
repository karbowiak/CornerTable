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
    ClearMarked(MarkedCells),
}

impl Goal {
    pub fn target(self) -> u32 {
        match self {
            Self::Score(target) | Self::Collect(_, target) | Self::Chains(target) => target,
            Self::ClearMarked(cells) => cells.count(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MarkedCells(u64);

impl MarkedCells {
    pub const fn from_indices(indices: &[usize]) -> Self {
        let mut bits = 0;
        let mut i = 0;
        while i < indices.len() {
            assert!(indices[i] < CELLS);
            bits |= 1 << indices[i];
            i += 1;
        }
        Self(bits)
    }

    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub fn contains(self, cell: Cell) -> bool {
        cell.index().is_some_and(|index| self.0 & (1 << index) != 0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Stage {
    pub name: &'static str,
    pub moves: u32,
    pub goals: &'static [Goal],
}

pub const STAGES: [Stage; 7] = [
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
    Stage {
        name: "Etched Gallery",
        moves: 20,
        goals: &[Goal::ClearMarked(MarkedCells::from_indices(&[
            19, 20, 26, 29, 34, 37, 43, 44, 51, 52,
        ]))],
    },
    Stage {
        name: "Twin Seams",
        moves: 26,
        goals: &[
            Goal::Collect(Gem::Aquamarine, 19),
            Goal::Collect(Gem::Amethyst, 18),
        ],
    },
    Stage {
        name: "Deep Mosaic",
        moves: 30,
        goals: &[
            Goal::ClearMarked(MarkedCells::from_indices(&[
                18, 21, 27, 28, 34, 37, 41, 46, 50, 53, 59, 60,
            ])),
            Goal::Collect(Gem::Ruby, 16),
        ],
    },
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub score: u32,
    collected: [u32; 6],
    pub chains: u32,
    pub best_chain: u32,
    pub remaining_marks: MarkedCells,
}

impl Progress {
    /// Progress toward a goal of this game.
    pub fn value(self, goal: Goal) -> u32 {
        match goal {
            Goal::Score(_) => self.score,
            Goal::Collect(gem, _) => self.collected[gem as usize],
            Goal::Chains(_) => self.chains,
            Goal::ClearMarked(cells) => MarkedCells(cells.0 & !self.remaining_marks.0).count(),
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
    pub cleared_marks: MarkedCells,
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
        let mut progress = Progress::default();
        for &goal in stage.goals {
            if let Goal::ClearMarked(cells) = goal {
                progress.remaining_marks.0 |= cells.0;
            }
        }
        let mut game = Self {
            board: [Gem::Ruby; CELLS],
            stage,
            progress,
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
        let (pair, (wins, gain, _)) = self
            .trials()
            .map(|(pair, trial)| {
                (
                    pair,
                    (
                        trial.status() == Status::Won,
                        self.goal_gain(&trial),
                        trial.score(),
                    ),
                )
            })
            .max_by_key(|&(_, rank)| rank)?;
        if wins || gain > 0 || self.moves <= 1 {
            return Some(pair);
        }

        // An isolated mark or rare jewel can need a setup move. Look one move
        // further if no current swap advances an unfinished goal.
        self.trials()
            .max_by_key(|(_, trial)| {
                let follow_up = trial
                    .trials()
                    .map(|(_, next)| (next.status() == Status::Won, trial.goal_gain(&next)))
                    .max()
                    .unwrap_or_default();
                (follow_up, trial.score())
            })
            .map(|(pair, _)| pair)
    }

    fn trials(&self) -> impl Iterator<Item = ((Cell, Cell), Self)> + '_ {
        self.valid_swaps().map(|(first, second)| {
            let mut trial = self.clone();
            trial.swap(first, second).expect("a legal move");
            ((first, second), trial)
        })
    }

    fn goal_gain(&self, trial: &Self) -> u64 {
        self.stage
            .goals
            .iter()
            .map(|&goal| {
                let target = goal.target();
                let before = self.progress.value(goal).min(target);
                let after = trial.progress.value(goal).min(target);
                1_000 * u64::from(after - before) / u64::from(target.max(1))
            })
            .sum()
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
            let marks_before = self.progress.remaining_marks;
            let (gems, points) = self.record_clear(&cleared, turn.cascades.len() as u32 + 1);
            let fall_distance = self.drop_and_refill(&cleared);
            turn.gems += gems;
            turn.points += points;
            turn.cascades.push(Cascade {
                before,
                cleared,
                after: self.board,
                cleared_marks: MarkedCells(marks_before.0 & !self.progress.remaining_marks.0),
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
        for (index, (&gem, &clear)) in self.board.iter().zip(cleared).enumerate() {
            if clear {
                self.progress.collected[gem as usize] += 1;
                self.progress.remaining_marks.0 &= !(1 << index);
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

    #[test]
    fn marks_stay_at_their_coordinates_and_count_only_once_across_cascades() {
        const MARKS: Goal = Goal::ClearMarked(MarkedCells::from_indices(&[3, 11, 19]));
        const STAGE: Stage = Stage {
            name: "Last move",
            moves: 1,
            goals: &[MARKS, Goal::Collect(Gem::Amethyst, 3)],
        };
        let mut game = Game::for_stage(0, STAGE);
        let first = Cell::from_index(11);
        let second = Cell::from_index(19);
        let turn = game.swap(first, second).unwrap();

        let [initial, cascade] = turn.cascades.as_slice() else {
            panic!("the last turn should have two matches");
        };
        assert_eq!(initial.cleared_marks, MarkedCells::from_indices(&[11]));
        assert!(initial.progress.remaining_marks.contains(second));
        assert_eq!(initial.progress.value(MARKS), 1);
        assert!(cascade.cleared[11]);
        assert_eq!(cascade.cleared_marks, MarkedCells::from_indices(&[3, 19]));
        assert_eq!(cascade.progress.value(MARKS), 3);
        assert_eq!(game.moves_left(), 0);
        assert_eq!(game.status(), Status::Won);

        let mut game = Game::for_stage(
            0,
            Stage {
                goals: &[MARKS, Goal::Collect(Gem::Amethyst, 4)],
                ..STAGE
            },
        );
        game.swap(first, second).unwrap();
        assert_eq!(game.progress().value(MARKS), 3);
        assert_eq!(game.moves_left(), 0);
        assert_eq!(game.status(), Status::Lost);
        assert_eq!(game.swap(first, second).unwrap_err(), SwapError::Finished);
    }

    #[test]
    fn a_hint_can_set_up_a_mark_that_no_current_swap_clears() {
        const STAGE: Stage = Stage {
            name: "Setup",
            moves: 2,
            goals: &[Goal::ClearMarked(MarkedCells::from_indices(&[0]))],
        };
        let mut game = Game::for_stage(0, STAGE);
        assert!(
            game.trials()
                .all(|(_, trial)| trial.status() != Status::Won)
        );

        let (a, b) = game.suggestion().unwrap();
        game.swap(a, b).unwrap();
        assert_eq!(game.status(), Status::Playing);
        let (a, b) = game.suggestion().unwrap();
        game.swap(a, b).unwrap();
        assert_eq!(game.status(), Status::Won);
    }

    #[test]
    fn an_automatic_reshuffle_preserves_remaining_marks() {
        const STAGE: Stage = Stage {
            name: "Reshuffle",
            moves: 1_000,
            goals: &[
                Goal::Score(u32::MAX),
                Goal::ClearMarked(MarkedCells::from_indices(&[0, 7, 56, 63])),
            ],
        };
        let mut game = Game::for_stage(1, STAGE);
        while game.status() == Status::Playing {
            let (a, b) = game.hint().unwrap();
            let turn = game.swap(a, b).unwrap();
            if turn.reshuffled {
                let before_shuffle = turn.cascades.last().unwrap();
                assert_ne!(game.board(), &before_shuffle.after);
                assert_eq!(game.progress(), before_shuffle.progress);
                assert_eq!(game.progress().remaining_marks.count(), 1);
                assert!(game.hint().is_some());
                return;
            }
        }
        panic!("expected the deal to produce a reshuffle");
    }
}
