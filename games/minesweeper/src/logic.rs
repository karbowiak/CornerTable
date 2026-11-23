use super::{Board, Difficulty, Status, neighbors, next_random};

const CANDIDATES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clue {
    Square { column: usize, row: usize },
    MinesRemaining,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintReason {
    Direct(Clue),
    Difference { larger: Clue, smaller: Clue },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    Square {
        column: usize,
        row: usize,
        mine: bool,
        reason: HintReason,
    },
    FlagConflict(Clue),
}

impl Board {
    /// Search a bounded, deterministic sequence for a field that can be cleared
    /// by deduction from this opening. Run the search off the UI thread.
    pub fn logical(difficulty: Difficulty, seed: u64, x: usize, y: usize) -> Option<Self> {
        let mut random = seed;
        for _ in 0..CANDIDATES {
            let mut board = Self::new(difficulty, next_random(&mut random));
            board.place_mines(board.index(x, y)?);
            if solves(board.clone(), x, y) {
                board.seed = seed;
                return Some(board);
            }
        }
        None
    }

    /// Suggest a move using only displayed clues and the player's current flags.
    /// A deduction assumes the flagged squares contain mines.
    pub fn hint(&self) -> Option<Hint> {
        if self.status != Status::Playing {
            return None;
        }
        let (width, height) = self.difficulty.dimensions();
        let constraints = match constraints(self, width, height) {
            Ok(constraints) => constraints,
            Err(clue) => return Some(Hint::FlagConflict(clue)),
        };
        let suggest = |index, mine, reason| Hint::Square {
            column: index % width,
            row: index / width,
            mine,
            reason,
        };
        for constraint in &constraints {
            if let Some(&index) = constraint.cells.first()
                && let Some(mine) = constraint.certain()
            {
                return Some(suggest(index, mine, HintReason::Direct(constraint.clue)));
            }
        }
        for larger in constraints.iter().rev() {
            for smaller in &constraints {
                if let Some((cells, mine)) = larger.difference(smaller) {
                    return Some(suggest(
                        cells[0],
                        mine,
                        HintReason::Difference {
                            larger: larger.clue,
                            smaller: smaller.clue,
                        },
                    ));
                }
            }
        }
        None
    }
}

struct Constraint {
    cells: Vec<usize>,
    mines: usize,
    clue: Clue,
}

impl Constraint {
    fn certain(&self) -> Option<bool> {
        if self.mines == 0 {
            Some(false)
        } else if self.mines == self.cells.len() {
            Some(true)
        } else {
            None
        }
    }

    // Subtract two clue counts when all unknown neighbors of one clue also
    // appear in the other. If the difference is full or empty, every cell in
    // that difference has a known value.
    fn difference(&self, subset: &Self) -> Option<(Vec<usize>, bool)> {
        let count = self.cells.len().checked_sub(subset.cells.len())?;
        let mines = self.mines.checked_sub(subset.mines)?;
        if count == 0 || (mines != 0 && mines != count) {
            return None;
        }
        if !subset.cells.iter().all(|cell| self.cells.contains(cell)) {
            return None;
        }
        let cells = self
            .cells
            .iter()
            .copied()
            .filter(|cell| !subset.cells.contains(cell))
            .collect();
        Some((cells, mines != 0))
    }
}

fn solves(mut board: Board, x: usize, y: usize) -> bool {
    board.reveal(x, y);
    let (width, height) = board.difficulty.dimensions();
    while board.status == Status::Playing {
        let Ok(constraints) = constraints(&board, width, height) else {
            return false;
        };
        let mut changed = false;
        for constraint in &constraints {
            if let Some(mine) = constraint.certain() {
                changed |= apply(&mut board, &constraint.cells, mine, width);
            }
        }
        if changed {
            continue;
        }
        let difference = constraints
            .iter()
            .rev()
            .find_map(|large| constraints.iter().find_map(|small| large.difference(small)));
        let Some((cells, mine)) = difference else {
            return false;
        };
        if !apply(&mut board, &cells, mine, width) {
            return false;
        }
    }
    board.status == Status::Won
}

fn constraints(board: &Board, width: usize, height: usize) -> Result<Vec<Constraint>, Clue> {
    let mut constraints = Vec::new();
    let mut covered = Vec::new();
    for (index, cell) in board.cells.iter().enumerate() {
        if !cell.open {
            if !cell.flagged {
                covered.push(index);
            }
            continue;
        }
        let clue = Clue::Square {
            column: index % width,
            row: index / width,
        };
        let mut cells = Vec::new();
        let mut mines = usize::from(cell.adjacent_mines);
        for neighbor in neighbors(index, width, height) {
            let cell = board.cells[neighbor];
            if cell.flagged {
                mines = mines.checked_sub(1).ok_or(clue)?;
            } else if !cell.open {
                cells.push(neighbor);
            }
        }
        if mines > cells.len() {
            return Err(clue);
        }
        if !cells.is_empty() {
            constraints.push(Constraint { cells, mines, clue });
        }
    }
    // The total number of mines remaining is another visible clue.
    let mines = board
        .difficulty
        .mines()
        .checked_sub(board.flags)
        .filter(|&mines| mines <= covered.len())
        .ok_or(Clue::MinesRemaining)?;
    constraints.push(Constraint {
        cells: covered,
        mines,
        clue: Clue::MinesRemaining,
    });
    Ok(constraints)
}

fn apply(board: &mut Board, cells: &[usize], mine: bool, width: usize) -> bool {
    let mut changed = false;
    for &index in cells {
        let (x, y) = (index % width, index / width);
        if mine {
            if !board.cells[index].flagged {
                changed |= board.toggle_flag(x, y);
            }
        } else {
            changed |= board.reveal(x, y);
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seeded_field_can_stall_without_a_deduction() {
        let mut board = Board::new(Difficulty::Summit, 0);
        board.place_mines(board.index(10, 7).unwrap());
        assert!(!solves(board, 10, 7));
    }

    #[test]
    fn logical_fields_are_reproducible_and_deducible_from_the_requested_opening() {
        for difficulty in Difficulty::ALL {
            let (width, height) = difficulty.dimensions();
            for (x, y) in [(0, 0), (width / 2, height / 2), (width - 1, height - 1)] {
                let board = Board::logical(difficulty, 827, x, y).expect("certified layout");
                assert_eq!(board.required_opening(), Some((x, y)));
                assert_eq!(board.status(), Status::Ready);
                assert_eq!(board.checkpoint().mines.len(), difficulty.mines());
                assert_eq!(
                    board.checkpoint(),
                    Board::logical(difficulty, 827, x, y).unwrap().checkpoint(),
                );
                assert!(solves(board, x, y));
            }
        }
        assert!(Board::logical(Difficulty::Meadow, 827, 9, 0).is_none());
    }

    #[test]
    fn visible_hints_can_finish_a_certified_field() {
        let mut board = Board::logical(Difficulty::Highlands, 827, 3, 4).unwrap();
        assert!(board.hint().is_none());
        board.reveal(3, 4);
        for _ in 0..board.cells.len() {
            if board.status() == Status::Won {
                break;
            }
            let before = board.checkpoint();
            let Some(Hint::Square {
                column, row, mine, ..
            }) = board.hint()
            else {
                panic!("certified field must have a deduction");
            };
            assert_eq!(board.checkpoint(), before);
            assert_eq!(board.cells[board.index(column, row).unwrap()].mine, mine);
            if mine {
                assert!(board.toggle_flag(column, row));
            } else {
                assert!(board.reveal(column, row));
            }
        }
        assert_eq!(board.status(), Status::Won);
    }

    #[test]
    fn flags_conflicting_with_a_visible_clue_are_reported() {
        let mut board = Board::new(Difficulty::Meadow, 7);
        board.status = Status::Playing;
        board.cells[0].open = true;
        assert!(board.toggle_flag(1, 0));
        assert_eq!(
            board.hint(),
            Some(Hint::FlagConflict(Clue::Square { column: 0, row: 0 }))
        );
    }
}
