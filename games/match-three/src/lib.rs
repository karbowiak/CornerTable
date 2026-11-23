pub const SIDE: usize = 8;
pub const MOVE_LIMIT: u32 = 24;
pub const GOAL: u32 = 900;
const CELLS: usize = SIDE * SIDE;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub row: usize,
    pub col: usize,
}

impl Cell {
    fn index(self) -> Option<usize> {
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
    pub before: [Gem; CELLS],
    pub cleared: [bool; CELLS],
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
    score: u32,
    moves: u32,
    rng: u64,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut game = Self {
            board: [Gem::Ruby; CELLS],
            score: 0,
            moves: MOVE_LIMIT,
            rng: seed,
        };
        game.deal();
        game
    }

    pub fn gem(&self, cell: Cell) -> Option<Gem> {
        cell.index().map(|index| self.board[index])
    }

    pub fn score(&self) -> u32 {
        self.score
    }

    pub fn moves_left(&self) -> u32 {
        self.moves
    }

    pub fn status(&self) -> Status {
        if self.score >= GOAL {
            Status::Won
        } else if self.moves == 0 {
            Status::Lost
        } else {
            Status::Playing
        }
    }

    pub fn hint(&self) -> Option<(Cell, Cell)> {
        let mut board = self.board;
        for row in 0..SIDE {
            for col in 0..SIDE {
                let first = Cell { row, col };
                for second in [Cell { row: row + 1, col }, Cell { row, col: col + 1 }] {
                    let Some(second_index) = second.index() else {
                        continue;
                    };
                    let first_index = row * SIDE + col;
                    board.swap(first_index, second_index);
                    let playable = matched(&board).contains(&true);
                    board.swap(first_index, second_index);
                    if playable {
                        return Some((first, second));
                    }
                }
            }
        }
        None
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
            turn.cascades.push(Cascade {
                before: self.board,
                cleared,
            });
            let gems = cleared.iter().filter(|&&marked| marked).count() as u32;
            turn.gems += gems;
            turn.points += gems * 10 * turn.cascades.len() as u32;
            self.drop_and_refill(&cleared);
            cleared = matched(&self.board);
        }

        self.score += turn.points;
        if self.status() == Status::Playing && self.hint().is_none() {
            self.deal();
            turn.reshuffled = true;
        }
        Ok(turn)
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

    fn drop_and_refill(&mut self, cleared: &[bool; CELLS]) {
        for col in 0..SIDE {
            let mut write = SIDE;
            for row in (0..SIDE).rev() {
                let index = row * SIDE + col;
                if !cleared[index] {
                    write -= 1;
                    self.board[write * SIDE + col] = self.board[index];
                }
            }
            for row in 0..write {
                self.board[row * SIDE + col] = self.next_gem();
            }
        }
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
}
