use super::{Cell, Direction, Puzzle, Size, solver};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edit {
    pub cell: usize,
    /// Zero clears the entry and its pencil marks.
    pub digit: u8,
    pub pencil: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintKind {
    Clear,
    Forced,
    Reveal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hint {
    pub cell: usize,
    pub digit: u8,
    pub kind: HintKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunConflict {
    RepeatedDigit,
    UnreachableSum,
}

#[derive(Clone, Copy, Debug, Default)]
struct Entry {
    digit: u8,
    notes: u16,
}

#[derive(Debug)]
pub struct Game {
    puzzle: Puzzle,
    solution: Vec<u8>,
    values: Vec<u8>,
    notes: Vec<u16>,
    history: Vec<(Edit, Entry)>,
}

impl Game {
    /// Load a numbered board and certify that its clues have exactly one answer.
    pub fn new(size: Size, number: usize) -> Option<Self> {
        let puzzle = Puzzle::from_source(size, size.collection().lines().nth(number)?)?;
        let solution = solver::unique_solution(&puzzle)?;
        let cells = puzzle.cells.len();
        Some(Self {
            puzzle,
            solution,
            values: vec![0; cells],
            notes: vec![0; cells],
            history: Vec::new(),
        })
    }

    pub fn puzzle(&self) -> &Puzzle {
        &self.puzzle
    }

    pub fn value(&self, cell: usize) -> u8 {
        self.values[cell]
    }

    pub fn notes(&self, cell: usize) -> u16 {
        self.notes[cell]
    }

    pub fn empty_count(&self) -> usize {
        self.puzzle
            .entries
            .iter()
            .filter(|&&cell| self.value(cell) == 0)
            .count()
    }

    pub fn is_solved(&self) -> bool {
        self.values == self.solution
    }

    pub fn run_conflict(&self, index: usize) -> Option<RunConflict> {
        solver::remaining(self.puzzle.run(index), &self.values).err()
    }

    pub fn has_conflicts(&self) -> bool {
        (0..self.puzzle.runs.len()).any(|index| self.run_conflict(index).is_some())
    }

    pub fn edits(&self) -> impl Iterator<Item = Edit> + '_ {
        self.history.iter().map(|&(edit, _)| edit)
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn undo(&mut self) -> Option<usize> {
        let (edit, previous) = self.history.pop()?;
        self.values[edit.cell] = previous.digit;
        self.notes[edit.cell] = previous.notes;
        Some(edit.cell)
    }

    pub fn edit(&mut self, edit: Edit) -> bool {
        let Edit {
            cell,
            digit,
            pencil,
        } = edit;
        if self.puzzle.cells.get(cell) != Some(&Cell::Entry) || digit > 9 || self.is_solved() {
            return false;
        }
        let previous = Entry {
            digit: self.values[cell],
            notes: self.notes[cell],
        };
        if pencil {
            if digit == 0 || previous.digit != 0 {
                return false;
            }
            self.notes[cell] ^= 1 << digit;
        } else {
            if previous.digit == digit && previous.notes == 0 {
                return false;
            }
            self.values[cell] = digit;
            self.notes[cell] = 0;
        }
        self.history.push((edit, previous));
        true
    }

    pub fn hint(&self, focus: usize) -> Option<Hint> {
        let ordered = || {
            [focus]
                .into_iter()
                .filter(|&cell| self.puzzle.cells.get(cell) == Some(&Cell::Entry))
                .chain(
                    self.puzzle
                        .entries
                        .iter()
                        .copied()
                        .filter(|&cell| cell != focus),
                )
        };
        for cell in ordered() {
            let digit = self.value(cell);
            if digit != 0 && digit != self.solution[cell] {
                return Some(Hint {
                    cell,
                    digit: 0,
                    kind: HintKind::Clear,
                });
            }
        }

        let mut first_empty = None;
        for cell in ordered().filter(|&cell| self.value(cell) == 0) {
            first_empty.get_or_insert(cell);
            if self.candidates(cell).is_power_of_two() {
                return Some(Hint {
                    cell,
                    digit: self.solution[cell],
                    kind: HintKind::Forced,
                });
            }
        }
        let cell = first_empty?;
        Some(Hint {
            cell,
            digit: self.solution[cell],
            kind: HintKind::Reveal,
        })
    }

    fn candidates(&self, cell: usize) -> u16 {
        Direction::ALL
            .into_iter()
            .filter_map(|direction| self.puzzle.run_at(cell, direction))
            .map(|index| solver::remaining(self.puzzle.run(index), &self.values).unwrap_or(0))
            .fold(super::DIGITS, |left, right| left & right)
    }
}
