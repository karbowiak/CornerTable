//! Rules for a cross-sums puzzle. Every white cell belongs to one run in each direction.

mod game;
mod solver;
#[cfg(test)]
mod tests;

pub use game::{Edit, Game, Hint, HintKind, RunConflict};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Size {
    #[default]
    Compact,
    Classic,
}

impl Size {
    pub const ALL: [Self; 2] = [Self::Compact, Self::Classic];

    pub const fn side(self) -> usize {
        match self {
            Self::Compact => 6,
            Self::Classic => 9,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact 6×6",
            Self::Classic => "Classic 9×9",
        }
    }

    pub fn puzzle_count(self) -> usize {
        self.collection().lines().count()
    }

    fn collection(self) -> &'static str {
        // Board numbers are save identifiers. Keep the existing lines in order.
        match self {
            Self::Compact => include_str!("compact.txt"),
            Self::Classic => include_str!("classic.txt"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Across,
    Down,
}

impl Direction {
    const ALL: [Self; 2] = [Self::Across, Self::Down];

    fn index(self) -> usize {
        match self {
            Self::Across => 0,
            Self::Down => 1,
        }
    }

    fn next(self, index: usize, side: usize) -> Option<usize> {
        match self {
            Self::Across => (index % side + 1 < side).then_some(index + 1),
            Self::Down => (index / side + 1 < side).then_some(index + side),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cell {
    Entry,
    Block {
        across: Option<usize>,
        down: Option<usize>,
    },
}

#[derive(Debug)]
pub struct Run {
    pub clue: usize,
    pub sum: u8,
    pub cells: Vec<usize>,
    combinations: Box<[u16]>,
}

#[derive(Debug)]
pub struct Puzzle {
    size: Size,
    cells: Vec<Cell>,
    links: Vec<[usize; 2]>,
    runs: Vec<Run>,
    entries: Vec<usize>,
}

const DIGITS: u16 = 0b11_1111_1110;
const NO_RUN: usize = usize::MAX;

impl Puzzle {
    pub fn size(&self) -> Size {
        self.size
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn entries(&self) -> &[usize] {
        &self.entries
    }

    pub fn run(&self, index: usize) -> &Run {
        &self.runs[index]
    }

    pub fn run_at(&self, cell: usize, direction: Direction) -> Option<usize> {
        let run = self.links.get(cell)?[direction.index()];
        (run != NO_RUN).then_some(run)
    }

    // The digits in a source board specify the totals. The solver independently
    // derives the answer from those totals and the white-cell pattern.
    fn from_source(size: Size, source: &str) -> Option<Self> {
        let side = size.side();
        if source.len() != side * side {
            return None;
        }
        let template: Vec<_> = source
            .bytes()
            .map(|byte| match byte {
                b'#' => Some(0),
                b'1'..=b'9' => Some(byte - b'0'),
                _ => None,
            })
            .collect::<Option<_>>()?;
        let entries: Vec<_> = (0..template.len())
            .filter(|&index| template[index] != 0)
            .collect();
        if entries.is_empty() {
            return None;
        }

        let mut puzzle = Self {
            size,
            cells: Vec::with_capacity(template.len()),
            links: vec![[NO_RUN; 2]; template.len()],
            runs: Vec::new(),
            entries,
        };
        for (index, digit) in template.iter().enumerate() {
            if *digit != 0 {
                puzzle.cells.push(Cell::Entry);
                continue;
            }
            let mut clues = [None; 2];
            for direction in Direction::ALL {
                let cells: Vec<_> = std::iter::successors(direction.next(index, side), |&cell| {
                    direction.next(cell, side)
                })
                .take_while(|&cell| template[cell] != 0)
                .collect();
                if cells.is_empty() {
                    continue;
                }
                if !(2..=9).contains(&cells.len()) {
                    return None;
                }
                let used = cells
                    .iter()
                    .fold(0u16, |mask, &cell| mask | 1 << template[cell]);
                if used.count_ones() as usize != cells.len() {
                    return None;
                }
                let sum = cells.iter().map(|&cell| template[cell]).sum();
                let run = puzzle.runs.len();
                clues[direction.index()] = Some(run);
                for &cell in &cells {
                    puzzle.links[cell][direction.index()] = run;
                }
                puzzle.runs.push(Run {
                    clue: index,
                    sum,
                    combinations: solver::combinations(cells.len(), sum),
                    cells,
                });
            }
            let [across, down] = clues;
            puzzle.cells.push(Cell::Block { across, down });
        }
        puzzle
            .entries
            .iter()
            .all(|&index| !puzzle.links[index].contains(&NO_RUN))
            .then_some(puzzle)
    }
}
