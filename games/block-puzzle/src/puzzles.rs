use super::{Game, Move, PIECES, Piece, Placement, SIDE, TRAY_SIZE, Wood};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PuzzleGoal {
    /// Clear at least one row and one column with a single placement.
    Crossing,
    /// Place every piece in the fixed tray.
    FitTray,
    /// Occupy every marked cell at once, after line clears have resolved.
    CoverMarks,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PuzzleStatus {
    Playing,
    Solved,
    OutOfMoves,
    Blocked,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Puzzle {
    pub id: &'static str,
    pub title: &'static str,
    pub goal: PuzzleGoal,
    layout: [&'static [u8; SIDE]; SIDE],
    tray: [Option<Piece>; TRAY_SIZE],
    solution: &'static [Move],
}

impl Puzzle {
    pub fn play(&'static self) -> Game {
        Game {
            board: self.layout.map(|row| {
                row.map(|cell| match cell {
                    b'#' => Some(Wood::Juniper),
                    b'.' | b'+' => None,
                    _ => unreachable!("invalid puzzle cell"),
                })
            }),
            tray: self.tray,
            score: 0,
            random: 0,
            remaining: Some(self.tray.iter().flatten().count()),
            puzzle: Some(PuzzleState {
                definition: self,
                solved: false,
            }),
        }
    }

    /// A known solution using zero-based tray slots, columns and rows.
    pub fn solution(&self) -> &'static [Move] {
        self.solution
    }

    pub fn is_marked(&self, column: usize, row: usize) -> bool {
        self.layout.get(row).and_then(|row| row.get(column)) == Some(&b'+')
    }

    pub fn mark_progress(&self, game: &Game) -> (usize, usize) {
        self.count_marks(game.board())
    }

    fn count_marks(&self, board: &[[Option<Wood>; SIDE]; SIDE]) -> (usize, usize) {
        let mut filled = 0;
        let mut total = 0;
        for (pattern, row) in self.layout.iter().zip(board) {
            for (&mark, tile) in pattern.iter().zip(row) {
                if mark == b'+' {
                    total += 1;
                    filled += usize::from(tile.is_some());
                }
            }
        }
        (filled, total)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PuzzleState {
    definition: &'static Puzzle,
    pub(super) solved: bool,
}

impl PuzzleState {
    pub(super) fn update(
        &mut self,
        board: &[[Option<Wood>; SIDE]; SIDE],
        remaining: Option<usize>,
        placement: Placement,
    ) {
        self.solved = match self.definition.goal {
            PuzzleGoal::Crossing => {
                placement.cleared_rows.contains(&true) && placement.cleared_columns.contains(&true)
            }
            PuzzleGoal::FitTray => remaining == Some(0),
            PuzzleGoal::CoverMarks => {
                let (filled, total) = self.definition.count_marks(board);
                filled == total
            }
        };
    }
}

impl Game {
    pub fn puzzle_solved(&self) -> bool {
        self.puzzle.is_some_and(|puzzle| puzzle.solved)
    }

    pub fn puzzle_status(&self) -> Option<PuzzleStatus> {
        self.puzzle.map(|puzzle| {
            if puzzle.solved {
                PuzzleStatus::Solved
            } else if self.remaining == Some(0) {
                PuzzleStatus::OutOfMoves
            } else if self.has_moves() {
                PuzzleStatus::Playing
            } else {
                PuzzleStatus::Blocked
            }
        })
    }
}

// Keep IDs stable for saved journals. The diagrams use # for wood, + for a mark
// to cover, and . for an unmarked empty cell. Moves are replayed by Game::place.
pub static PUZZLE_BOARDS: &[Puzzle] = &[
    Puzzle {
        id: "first-crossing",
        title: "First Crossing",
        goal: PuzzleGoal::Crossing,
        layout: [
            b"....#...",
            b"....#...",
            b"....#...",
            b"####.###",
            b"....#...",
            b"....#...",
            b"....#...",
            b"....#...",
        ],
        tray: tray([DOT]),
        solution: &[at(0, 4, 3)],
    },
    Puzzle {
        id: "inlay",
        title: "Inlay",
        goal: PuzzleGoal::CoverMarks,
        layout: [
            b".#......",
            b"#++++.#.",
            b"......#.",
            b"...#....",
            b"....#...",
            b"...#+...",
            b"...#++..",
            b".....#..",
        ],
        tray: tray([BAR_FOUR, EL_DOWN_RIGHT]),
        solution: &[at(0, 1, 1), at(1, 4, 5)],
    },
    Puzzle {
        id: "square-key",
        title: "Square Key",
        goal: PuzzleGoal::FitTray,
        layout: [
            b".###.##.",
            b"##...##.",
            b"#.###..#",
            b"..#.#..#",
            b"####..##",
            b"#######.",
            b"#####.#.",
            b"#.#...##",
        ],
        tray: tray([EL_UPPER_LEFT, SQUARE]),
        solution: &[at(1, 5, 2), at(0, 6, 0)],
    },
    Puzzle {
        id: "two-notches",
        title: "Two Notches",
        goal: PuzzleGoal::Crossing,
        layout: [
            b"...#....",
            b"........",
            b"........",
            b"........",
            b"##...###",
            b"...#....",
            b"...#....",
            b"...#....",
        ],
        tray: tray([BAR_THREE, POST_THREE]),
        solution: &[at(1, 3, 1), at(0, 2, 4)],
    },
    Puzzle {
        id: "pocket-lock",
        title: "Pocket Lock",
        goal: PuzzleGoal::FitTray,
        layout: [
            b"#######.",
            b"#.##.##.",
            b".#######",
            b".##..#..",
            b"#.##.###",
            b"#.#.#.#.",
            b"..#.#..#",
            b"#..##..#",
        ],
        tray: tray([BAR_FOUR, EL_DOWN_RIGHT, SQUARE]),
        solution: &[at(2, 5, 6), at(1, 1, 6), at(0, 2, 7)],
    },
    Puzzle {
        id: "split-inlay",
        title: "Split Inlay",
        goal: PuzzleGoal::CoverMarks,
        layout: [
            b"###.##.#",
            b"#####..#",
            b"..######",
            b"+..+..##",
            b"#######.",
            b"#..##+++",
            b".####.##",
            b"###.##..",
        ],
        tray: tray([EL_UPPER_LEFT, BAR_FOUR, SQUARE]),
        solution: &[at(0, 0, 2), at(2, 3, 2), at(1, 4, 5)],
    },
    Puzzle {
        id: "double-take",
        title: "Double Take",
        goal: PuzzleGoal::Crossing,
        layout: [
            b"....#...",
            b"....#...",
            b"....#...",
            b"...#####",
            b"#..#.###",
            b"....#...",
            b"....#...",
            b"....#...",
        ],
        tray: tray([BAR_THREE, T_DOWN, BAR_TWO]),
        solution: &[at(0, 0, 3), at(2, 1, 4), at(1, 3, 3)],
    },
    Puzzle {
        id: "narrow-passage",
        title: "Narrow Passage",
        goal: PuzzleGoal::FitTray,
        layout: [
            b".###.##.",
            b"##.##.#.",
            b"###.####",
            b".#..##..",
            b"##..##..",
            b"##.#####",
            b"#.######",
            b"#.#####.",
        ],
        tray: tray([EL_UPPER_RIGHT, SQUARE, ZIGZAG]),
        solution: &[at(1, 6, 3), at(0, 5, 1), at(2, 5, 6)],
    },
    Puzzle {
        id: "last-inlay",
        title: "Last Inlay",
        goal: PuzzleGoal::CoverMarks,
        layout: [
            b"..#.###.",
            b"..###...",
            b".###.#..",
            b"#.#.####",
            b"###..#..",
            b"#++###..",
            b"#+++####",
            b".###..#.",
        ],
        tray: tray([T_RIGHT, SQUARE, BAR_FOUR]),
        solution: &[at(1, 1, 5), at(0, 2, 5), at(2, 1, 6)],
    },
];

const DOT: Piece = PIECES[0];
const BAR_TWO: Piece = PIECES[1];
const BAR_THREE: Piece = PIECES[3];
const POST_THREE: Piece = PIECES[4];
const EL_DOWN_RIGHT: Piece = PIECES[5];
const EL_UPPER_RIGHT: Piece = PIECES[6];
const EL_UPPER_LEFT: Piece = PIECES[7];
const SQUARE: Piece = PIECES[9];
const BAR_FOUR: Piece = PIECES[10];
const T_DOWN: Piece = PIECES[14];
const T_RIGHT: Piece = PIECES[15];
const ZIGZAG: Piece = PIECES[16];

const fn tray<const N: usize>(pieces: [Piece; N]) -> [Option<Piece>; TRAY_SIZE] {
    let mut slots = [None; TRAY_SIZE];
    let mut slot = 0;
    while slot < N {
        slots[slot] = Some(pieces[slot]);
        slot += 1;
    }
    slots
}

const fn at(slot: usize, column: usize, row: usize) -> Move {
    Move { slot, column, row }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_solution_replays_with_live_rules() {
        let mut ids = std::collections::HashSet::new();
        for puzzle in PUZZLE_BOARDS {
            assert!(ids.insert(puzzle.id), "duplicate puzzle ID {}", puzzle.id);
            let mut game = puzzle.play();
            let (_, marks) = puzzle.mark_progress(&game);
            assert_eq!(
                marks > 0,
                puzzle.goal == PuzzleGoal::CoverMarks,
                "{}",
                puzzle.id
            );
            assert!(game.board().iter().all(|row| row.contains(&None)));
            assert!((0..SIDE).all(|col| game.board().iter().any(|row| row[col].is_none())));
            for (index, &turn) in puzzle.solution().iter().enumerate() {
                assert_eq!(
                    game.puzzle_status(),
                    Some(PuzzleStatus::Playing),
                    "{} at move {}",
                    puzzle.id,
                    index + 1
                );
                let placement = game.place(turn).unwrap_or_else(|| {
                    panic!("{} has an invalid move {}", puzzle.id, index + 1);
                });
                if index + 1 == puzzle.solution().len() {
                    match puzzle.goal {
                        PuzzleGoal::Crossing => {
                            assert!(placement.cleared_rows.contains(&true));
                            assert!(placement.cleared_columns.contains(&true));
                        }
                        PuzzleGoal::FitTray => assert_eq!(game.tray(), &[None; TRAY_SIZE]),
                        PuzzleGoal::CoverMarks => {
                            assert_eq!(puzzle.mark_progress(&game), (marks, marks))
                        }
                    }
                }
            }
            assert_eq!(
                game.puzzle_status(),
                Some(PuzzleStatus::Solved),
                "{}",
                puzzle.id
            );
            assert!(!game.has_moves());
        }
    }

    #[test]
    fn an_exhausted_tray_can_fail_its_goal() {
        let mut game = PUZZLE_BOARDS[0].play();
        assert!(game.place(at(0, 0, 0)).is_some());
        assert_eq!(game.placements_left(), Some(0));
        assert_eq!(game.puzzle_status(), Some(PuzzleStatus::OutOfMoves));
        assert_eq!(game.tray(), &[None; TRAY_SIZE]);
    }

    #[test]
    fn a_tray_can_be_blocked_with_pieces_left() {
        let mut game = PUZZLE_BOARDS[2].play();
        assert!(game.place(at(0, 5, 2)).is_some());
        assert_eq!(game.placements_left(), Some(1));
        assert_eq!(game.puzzle_status(), Some(PuzzleStatus::Blocked));
    }

    #[test]
    fn a_row_clear_can_open_space_but_cannot_satisfy_the_crossing_goal() {
        let puzzle = &PUZZLE_BOARDS[6];
        let mut game = puzzle.play();
        let placement = game.place(puzzle.solution()[0]).unwrap();
        assert_eq!(placement.lines, 1);
        assert_eq!(game.puzzle_status(), Some(PuzzleStatus::Playing));
    }

    #[test]
    fn cleared_marks_must_be_filled_again() {
        let puzzle = &PUZZLE_BOARDS[8];
        let mut game = puzzle.play();
        let outcome = game.place(puzzle.solution()[0]).unwrap();
        assert!(outcome.cleared_columns[2]);
        assert_eq!(puzzle.mark_progress(&game), (2, 5));
        assert_eq!(game.puzzle_status(), Some(PuzzleStatus::Playing));
    }
}
