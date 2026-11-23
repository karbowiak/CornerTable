pub const SIDE: usize = 8;
pub const TRAY_SIZE: usize = 3;
pub const CHALLENGE_TRAYS: usize = 10;
pub const CHALLENGE_SEED: u64 = 0x7465_6e2d_7472_6179;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wood {
    Amber,
    Cherry,
    Juniper,
    Plum,
    Lagoon,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Piece {
    pub tiles: &'static [(usize, usize)],
    pub wood: Wood,
}

impl Piece {
    pub fn dimensions(self) -> (usize, usize) {
        self.tiles.iter().fold((0, 0), |(width, height), &(x, y)| {
            (width.max(x + 1), height.max(y + 1))
        })
    }
}

use Wood::*;

const PIECES: &[Piece] = &[
    Piece {
        tiles: &[(0, 0)],
        wood: Amber,
    },
    Piece {
        tiles: &[(0, 0), (1, 0)],
        wood: Cherry,
    },
    Piece {
        tiles: &[(0, 0), (0, 1)],
        wood: Cherry,
    },
    Piece {
        tiles: &[(0, 0), (1, 0), (2, 0)],
        wood: Juniper,
    },
    Piece {
        tiles: &[(0, 0), (0, 1), (0, 2)],
        wood: Juniper,
    },
    Piece {
        tiles: &[(0, 0), (0, 1), (1, 1)],
        wood: Plum,
    },
    Piece {
        tiles: &[(0, 0), (1, 0), (1, 1)],
        wood: Plum,
    },
    Piece {
        tiles: &[(0, 0), (1, 0), (0, 1)],
        wood: Plum,
    },
    Piece {
        tiles: &[(1, 0), (0, 1), (1, 1)],
        wood: Plum,
    },
    Piece {
        tiles: &[(0, 0), (1, 0), (0, 1), (1, 1)],
        wood: Lagoon,
    },
    Piece {
        tiles: &[(0, 0), (1, 0), (2, 0), (3, 0)],
        wood: Amber,
    },
    Piece {
        tiles: &[(0, 0), (0, 1), (0, 2), (0, 3)],
        wood: Amber,
    },
    Piece {
        tiles: &[(0, 0), (0, 1), (0, 2), (1, 2), (2, 2)],
        wood: Cherry,
    },
    Piece {
        tiles: &[(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)],
        wood: Cherry,
    },
    Piece {
        tiles: &[(0, 0), (1, 0), (2, 0), (1, 1)],
        wood: Juniper,
    },
    Piece {
        tiles: &[(0, 0), (0, 1), (1, 1), (0, 2)],
        wood: Juniper,
    },
    Piece {
        tiles: &[(0, 0), (1, 0), (1, 1), (2, 1)],
        wood: Lagoon,
    },
    Piece {
        tiles: &[(1, 0), (0, 1), (1, 1), (0, 2)],
        wood: Lagoon,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Move {
    pub slot: usize,
    pub column: usize,
    pub row: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Placement {
    pub lines: usize,
    pub points: u64,
    pub cleared_rows: [bool; SIDE],
    pub cleared_columns: [bool; SIDE],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Game {
    board: [[Option<Wood>; SIDE]; SIDE],
    tray: [Option<Piece>; TRAY_SIZE],
    score: u64,
    random: u64,
    remaining: Option<usize>,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Self::deal(seed, None)
    }

    /// A repeatable deal with a fixed budget of ten trays.
    pub fn ten_trays(seed: u64) -> Self {
        Self::deal(seed, Some(CHALLENGE_TRAYS * TRAY_SIZE))
    }

    fn deal(seed: u64, remaining: Option<usize>) -> Self {
        let mut game = Self {
            board: [[None; SIDE]; SIDE],
            tray: [None; TRAY_SIZE],
            score: 0,
            random: seed,
            remaining,
        };
        game.refill();
        game
    }

    pub fn board(&self) -> &[[Option<Wood>; SIDE]; SIDE] {
        &self.board
    }

    pub fn tray(&self) -> &[Option<Piece>; TRAY_SIZE] {
        &self.tray
    }

    pub fn score(&self) -> u64 {
        self.score
    }

    pub fn placements_left(&self) -> Option<usize> {
        self.remaining
    }

    pub fn can_place(&self, turn: Move) -> bool {
        let Some(Some(piece)) = self.tray.get(turn.slot) else {
            return false;
        };
        if turn.column >= SIDE || turn.row >= SIDE {
            return false;
        }
        piece.tiles.iter().all(|&(dx, dy)| {
            let (column, row) = (turn.column + dx, turn.row + dy);
            column < SIDE && row < SIDE && self.board[row][column].is_none()
        })
    }

    pub fn place(&mut self, turn: Move) -> Option<Placement> {
        if !self.can_place(turn) {
            return None;
        }

        let piece = self.tray[turn.slot].take().unwrap();
        for &(dx, dy) in piece.tiles {
            self.board[turn.row + dy][turn.column + dx] = Some(piece.wood);
        }

        // Detect both axes before removing any tiles so intersections count for both lines.
        let rows = self.board.map(|row| row.iter().all(Option::is_some));
        let columns: [bool; SIDE] =
            std::array::from_fn(|column| self.board.iter().all(|row| row[column].is_some()));
        let lines = rows.iter().chain(&columns).filter(|&&full| full).count();
        for (row, board_row) in self.board.iter_mut().enumerate() {
            for (column, cell) in board_row.iter_mut().enumerate() {
                if rows[row] || columns[column] {
                    *cell = None;
                }
            }
        }
        let points = piece.tiles.len() as u64 + 10 * lines as u64 * lines as u64;
        self.score = self.score.saturating_add(points);
        if let Some(remaining) = &mut self.remaining {
            *remaining -= 1;
        }
        if self.remaining != Some(0) && self.tray.iter().all(Option::is_none) {
            self.refill();
        }
        Some(Placement {
            lines,
            points,
            cleared_rows: rows,
            cleared_columns: columns,
        })
    }

    pub fn has_moves(&self) -> bool {
        (0..TRAY_SIZE).any(|slot| {
            (0..SIDE).any(|row| (0..SIDE).any(|column| self.can_place(Move { slot, column, row })))
        })
    }

    fn refill(&mut self) {
        for slot in &mut self.tray {
            // SplitMix64 gives a reproducible deal, including when the seed is zero.
            self.random = self.random.wrapping_add(0x9e3779b97f4a7c15);
            let mut roll = self.random;
            roll = (roll ^ (roll >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            roll = (roll ^ (roll >> 27)).wrapping_mul(0x94d049bb133111eb);
            roll ^= roll >> 31;
            *slot = Some(PIECES[(roll % PIECES.len() as u64) as usize]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_games_replay_through_multiple_deals() {
        let mut first = Game::new(0);
        let mut second = Game::new(0);
        assert_ne!(first.tray, Game::new(1).tray);
        let mut moves = 0;
        for _ in 0..24 {
            let turn = (0..TRAY_SIZE)
                .flat_map(|slot| {
                    (0..SIDE).flat_map(move |row| {
                        (0..SIDE).map(move |column| Move { slot, column, row })
                    })
                })
                .find(|&turn| first.can_place(turn));
            let Some(turn) = turn else { break };
            assert_eq!(first.place(turn), second.place(turn));
            assert_eq!(first, second);
            moves += 1;
        }
        assert!(moves > TRAY_SIZE);
    }

    #[test]
    fn a_single_tile_can_clear_a_cross() {
        let mut game = Game::new(7);
        game.tray = [Some(PIECES[0]); TRAY_SIZE];
        for index in 0..SIDE {
            if index != 3 {
                game.board[3][index] = Some(Cherry);
                game.board[index][3] = Some(Cherry);
            }
        }
        game.board[0][0] = Some(Juniper);
        let result = game.place(Move {
            slot: 0,
            column: 3,
            row: 3,
        });
        assert_eq!(
            result,
            Some(Placement {
                lines: 2,
                points: 41,
                cleared_rows: std::array::from_fn(|index| index == 3),
                cleared_columns: std::array::from_fn(|index| index == 3),
            })
        );
        assert_eq!(game.score(), 41);
        assert_eq!(
            game.board
                .iter()
                .flatten()
                .filter(|tile| tile.is_some())
                .count(),
            1
        );
        assert_eq!(game.board[0][0], Some(Juniper));
    }

    #[test]
    fn invalid_moves_are_atomic() {
        let mut game = Game::new(9);
        game.tray[0] = Some(PIECES[9]);
        game.board[0][0] = Some(Lagoon);
        let initial = game.clone();
        for (slot, column, row) in [(0, 0, 0), (0, SIDE - 1, 0), (0, usize::MAX, 0), (9, 0, 0)] {
            assert_eq!(game.place(Move { slot, column, row }), None);
            assert_eq!(game, initial);
        }
    }

    #[test]
    fn finds_any_remaining_piece_and_detects_a_blocked_tray() {
        let mut game = Game::new(11);
        game.board = [[Some(Plum); SIDE]; SIDE];
        game.board[7][7] = None;
        game.tray = [Some(PIECES[9]), None, Some(PIECES[9])];
        assert!(!game.has_moves());
        game.tray[1] = Some(PIECES[0]);
        assert!(game.has_moves());
    }

    #[test]
    fn ten_trays_can_be_completed_and_stops_at_thirty_placements() {
        let solution = [
            [(2, 0, 0), (1, 0, 4), (0, 0, 6)],
            [(0, 0, 0), (1, 3, 0), (2, 4, 0)],
            [(0, 1, 5), (1, 2, 3), (2, 1, 2)],
            [(1, 1, 6), (0, 0, 3), (2, 2, 0)],
            [(2, 1, 0), (0, 7, 0), (1, 5, 3)],
            [(1, 2, 1), (2, 5, 0), (0, 0, 0)],
            [(1, 4, 1), (0, 6, 5), (2, 5, 7)],
            [(0, 6, 0), (2, 0, 1), (1, 1, 0)],
            [(1, 6, 2), (2, 2, 1), (0, 1, 5)],
            [(0, 1, 2), (1, 1, 0), (2, 4, 0)],
        ];
        let mut game = Game::ten_trays(CHALLENGE_SEED);
        for (i, (slot, column, row)) in solution.into_iter().flatten().enumerate() {
            assert_eq!(
                game.placements_left(),
                Some(CHALLENGE_TRAYS * TRAY_SIZE - i)
            );
            assert!(game.place(Move { slot, column, row }).is_some());
        }
        assert_eq!(game.placements_left(), Some(0));
        assert_eq!(game.score(), 213);
        assert_eq!(game.tray(), &[None; TRAY_SIZE]);
        assert!(!game.has_moves());
    }
}
