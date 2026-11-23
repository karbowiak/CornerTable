//! Picture Logic rules and the original print collection. The solver uses only clues.

mod prints;
mod solver;

pub use prints::PUZZLES;
pub use solver::{SolutionCount, solution_count};

const MAX_SIDE: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Series {
    Pocket,
    Postcard,
    FullSheet,
}

impl Series {
    pub const ALL: [Self; 3] = [Self::Pocket, Self::Postcard, Self::FullSheet];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pocket => "Pocket",
            Self::Postcard => "Postcard",
            Self::FullSheet => "Full sheet",
        }
    }

    pub const fn sizes(self) -> &'static str {
        match self {
            Self::Pocket => "5 × 5 and 7 × 7",
            Self::Postcard => "10 × 10 and 12 × 12",
            Self::FullSheet => "15 × 15",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Puzzle {
    pub id: &'static str,
    pub title: &'static str,
    pub caption: &'static str,
    pub series: Series,
    // Periods are unprinted paper. Letters index the inks chosen for the print
    // and all count as filled.
    ink: [u32; 4],
    print: &'static [&'static str],
}

impl Puzzle {
    pub fn width(&self) -> usize {
        self.print[0].len()
    }

    pub fn height(&self) -> usize {
        self.print.len()
    }

    pub fn color(&self, index: usize) -> Option<u32> {
        let width = self.width();
        let byte = self.print.get(index / width)?.as_bytes()[index % width];
        match byte {
            b'a'..=b'd' => Some(self.ink[usize::from(byte - b'a')]),
            _ => None,
        }
    }

    pub fn filled(&self, index: usize) -> bool {
        self.color(index).is_some()
    }

    pub fn clues(&self) -> Clues {
        let width = self.width();
        let height = self.height();
        Clues::new(
            width,
            height,
            (0..height)
                .map(|row| runs((0..width).map(|col| self.filled(row * width + col))))
                .collect(),
            (0..width)
                .map(|col| runs((0..height).map(|row| self.filled(row * width + col))))
                .collect(),
        )
        .expect("curated print has valid dimensions and clues")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clues {
    width: usize,
    height: usize,
    rows: Vec<Vec<u8>>,
    columns: Vec<Vec<u8>>,
}

impl Clues {
    /// Line clues are positive run lengths in reading order. Empty lines use `[]`.
    /// Validates shape and line lengths. Cross-line consistency is checked by the solver.
    pub fn new(
        width: usize,
        height: usize,
        rows: Vec<Vec<u8>>,
        columns: Vec<Vec<u8>>,
    ) -> Option<Self> {
        ((1..=MAX_SIDE).contains(&width)
            && (1..=MAX_SIDE).contains(&height)
            && rows.len() == height
            && columns.len() == width
            && rows.iter().all(|row| valid_line(row, width))
            && columns.iter().all(|col| valid_line(col, height)))
        .then_some(Self {
            width,
            height,
            rows,
            columns,
        })
    }

    pub fn rows(&self) -> &[Vec<u8>] {
        &self.rows
    }

    pub fn columns(&self) -> &[Vec<u8>] {
        &self.columns
    }
}

fn valid_line(clues: &[u8], length: usize) -> bool {
    clues.iter().all(|run| *run > 0)
        && clues.iter().map(|run| usize::from(*run)).sum::<usize>() + clues.len().saturating_sub(1)
            <= length
}

fn runs(cells: impl IntoIterator<Item = bool>) -> Vec<u8> {
    let mut lengths = Vec::new();
    let mut count = 0;
    for filled in cells.into_iter().chain([false]) {
        if filled {
            count += 1;
        } else if count > 0 {
            lengths.push(count);
            count = 0;
        }
    }
    lengths
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cell {
    #[default]
    Blank,
    Filled,
    Marked,
}

impl Cell {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Blank => "Blank",
            Self::Filled => "Filled",
            Self::Marked => "Marked empty",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
}

impl Axis {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Row => "Row",
            Self::Column => "Column",
        }
    }

    fn index(self, line: usize, offset: usize, width: usize) -> usize {
        match self {
            Self::Row => line * width + offset,
            Self::Column => offset * width + line,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deduction {
    pub index: usize,
    pub cell: Cell,
    pub axis: Axis,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct Board {
    puzzle: &'static Puzzle,
    clues: Clues,
    cells: Vec<Cell>,
    needed: usize,
}

impl Board {
    pub fn new(puzzle: &'static Puzzle) -> Self {
        let clues = puzzle.clues();
        let needed = clues.rows.iter().flatten().map(|n| usize::from(*n)).sum();
        Self {
            puzzle,
            cells: vec![Cell::Blank; clues.width * clues.height],
            clues,
            needed,
        }
    }

    pub fn puzzle(&self) -> &'static Puzzle {
        self.puzzle
    }

    pub fn clues(&self) -> &Clues {
        &self.clues
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Restores a snapshot only if its length and every cell are valid.
    pub fn restore(&mut self, snapshot: &str) -> bool {
        if snapshot.len() != self.cells.len() {
            return false;
        }
        let decoded = snapshot
            .bytes()
            .map(|byte| match byte {
                b'.' => Some(Cell::Blank),
                b'#' => Some(Cell::Filled),
                b'x' => Some(Cell::Marked),
                _ => None,
            })
            .collect::<Option<Vec<_>>>();
        if let Some(decoded) = decoded {
            self.cells = decoded;
            return true;
        }
        false
    }

    pub fn snapshot(&self) -> String {
        self.cells
            .iter()
            .map(|cell| match cell {
                Cell::Blank => '.',
                Cell::Filled => '#',
                Cell::Marked => 'x',
            })
            .collect()
    }

    pub fn paint(&mut self, index: usize, cell: Cell) -> bool {
        match self.cells.get_mut(index) {
            Some(previous) if *previous != cell => {
                *previous = cell;
                true
            }
            _ => false,
        }
    }

    pub fn complete(&self) -> bool {
        self.cells
            .iter()
            .enumerate()
            .all(|(index, cell)| (*cell == Cell::Filled) == self.puzzle.filled(index))
    }

    pub fn has_conflict(&self, index: usize) -> bool {
        match self.cells.get(index) {
            Some(Cell::Filled) => !self.puzzle.filled(index),
            Some(Cell::Marked) => self.puzzle.filled(index),
            _ => false,
        }
    }

    /// Returns placed and required ink counts. Placed can exceed the target.
    pub fn ink(&self) -> (usize, usize) {
        (
            self.cells
                .iter()
                .filter(|cell| **cell == Cell::Filled)
                .count(),
            self.needed,
        )
    }

    /// One result per row and column. A line is met when its filled runs match its clues.
    pub fn matched(&self) -> (Vec<bool>, Vec<bool>) {
        let (width, height) = (self.clues.width, self.clues.height);
        let matched =
            |axis: Axis, length: usize, clues: &[Vec<u8>]| {
                clues
                    .iter()
                    .enumerate()
                    .map(|(line, clue)| {
                        runs((0..length).map(|offset| {
                            self.cells[axis.index(line, offset, width)] == Cell::Filled
                        })) == *clue
                    })
                    .collect()
            };
        (
            matched(Axis::Row, width, &self.clues.rows),
            matched(Axis::Column, height, &self.clues.columns),
        )
    }

    /// Finds a cell that every valid placement of a single row or column agrees on.
    pub fn deduction(&self, preferred: usize) -> Option<Deduction> {
        solver::deductions(&self.clues, &self.cells)
            .into_iter()
            .max_by_key(|d| {
                let distance = (d.index / self.clues.width).abs_diff(preferred / self.clues.width)
                    + (d.index % self.clues.width).abs_diff(preferred % self.clues.width);
                (
                    d.index == preferred,
                    d.cell == Cell::Filled,
                    std::cmp::Reverse(distance),
                    std::cmp::Reverse(d.index),
                )
            })
    }
}

#[cfg(test)]
mod tests;
