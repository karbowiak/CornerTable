use super::{Board, Difficulty, Status, neighbors, next_random};

const CANDIDATES: usize = 64;

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
}

struct Constraint {
    cells: Vec<usize>,
    mines: usize,
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
        let Some(constraints) = constraints(&board, width, height) else {
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

fn constraints(board: &Board, width: usize, height: usize) -> Option<Vec<Constraint>> {
    let mut constraints = Vec::new();
    let mut covered = Vec::new();
    for (index, cell) in board.cells.iter().enumerate() {
        if !cell.open {
            if !cell.flagged {
                covered.push(index);
            }
            continue;
        }
        let mut cells = Vec::new();
        let mut mines = usize::from(cell.adjacent_mines);
        for neighbor in neighbors(index, width, height) {
            let cell = board.cells[neighbor];
            if cell.flagged {
                mines = mines.checked_sub(1)?;
            } else if !cell.open {
                cells.push(neighbor);
            }
        }
        if !cells.is_empty() {
            constraints.push(Constraint { cells, mines });
        }
    }
    // The total number of mines remaining is another visible clue.
    constraints.push(Constraint {
        cells: covered,
        mines: board.difficulty.mines().checked_sub(board.flags)?,
    });
    Some(constraints)
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
}
