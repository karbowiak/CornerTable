//! Killer Sudoku adds sum cages to the ordinary 9×9 constraints.

mod puzzles;
#[cfg(test)]
mod tests;

use std::sync::OnceLock;

use super::{Board, CELLS, Format, Game, Occupancy, SIZE};

const FORMAT: Format = Format::Standard;
const DIGITS: u16 = (1 << (SIZE + 1)) - 2;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u64)]
pub enum KillerPuzzle {
    FirstLight = 1,
    Crossings = 2,
    Patchwork = 3,
}

impl KillerPuzzle {
    pub const ALL: [Self; 3] = [Self::FirstLight, Self::Crossings, Self::Patchwork];

    /// Stable identity for saved progress. Keep the ID and layout together.
    pub const fn id(self) -> u64 {
        self as u64
    }

    pub fn from_id(id: u64) -> Option<Self> {
        Self::ALL.into_iter().find(|puzzle| puzzle.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FirstLight => "First Light",
            Self::Crossings => "Crossings",
            Self::Patchwork => "Patchwork",
        }
    }

    fn rules(self) -> &'static Rules {
        static FIRST_LIGHT: OnceLock<Rules> = OnceLock::new();
        static CROSSINGS: OnceLock<Rules> = OnceLock::new();
        static PATCHWORK: OnceLock<Rules> = OnceLock::new();
        let (cache, cages) = match self {
            Self::FirstLight => (&FIRST_LIGHT, puzzles::FIRST_LIGHT),
            Self::Crossings => (&CROSSINGS, puzzles::CROSSINGS),
            Self::Patchwork => (&PATCHWORK, puzzles::PATCHWORK),
        };
        cache.get_or_init(|| Rules::new(cages))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Cage {
    sum: u8,
    cells: &'static [usize],
}

impl Cage {
    pub fn sum(&self) -> u8 {
        self.sum
    }

    pub fn cells(&self) -> &'static [usize] {
        self.cells
    }

    /// The first cell in reading order, where the sum is printed.
    pub fn anchor(&self) -> usize {
        self.cells[0]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CageIssue {
    Repeated(u8),
    OverTotal,
    WrongTotal,
    ImpossibleTotal,
}

impl Game {
    pub fn killer(puzzle: KillerPuzzle) -> Self {
        let rules = puzzle.rules();
        let solution = *rules.solution.get_or_init(|| {
            let (count, solution) = rules.solve(2);
            assert_eq!(count, 1, "bundled Killer Sudoku must have one solution");
            solution
        });
        Self {
            format: FORMAT,
            killer: Some(rules),
            givens: [0; CELLS],
            solution,
            board: Board {
                values: [0; CELLS],
                notes: [0; CELLS],
            },
            undo: Vec::new(),
        }
    }

    pub fn cages(&self) -> &'static [Cage] {
        self.killer.map_or(&[], |rules| rules.cages)
    }

    pub fn cage(&self, index: usize) -> Option<&'static Cage> {
        let rules = self.killer?;
        Some(&rules.cages[*rules.by_cell.get(index)?])
    }

    /// A cage issue is separate from ordinary row, column and box conflicts.
    pub fn cage_issue(&self, index: usize) -> Option<CageIssue> {
        self.killer?.issue(index, &self.board.values)
    }
}

pub(super) struct Rules {
    cages: &'static [Cage],
    by_cell: [usize; CELLS],
    combinations: Vec<Vec<u16>>,
    solution: OnceLock<[u8; CELLS]>,
}

impl Rules {
    fn new(cages: &'static [Cage]) -> Self {
        let mut by_cell = [usize::MAX; CELLS];
        let combinations = cages
            .iter()
            .enumerate()
            .map(|(index, cage)| {
                for &cell in cage.cells {
                    assert_eq!(by_cell[cell], usize::MAX, "overlapping cages");
                    by_cell[cell] = index;
                }
                (0..=DIGITS)
                    .step_by(2)
                    .filter(|&mask| {
                        mask.count_ones() as usize == cage.cells.len()
                            && (1..=9).filter(|digit| mask & (1 << digit) != 0).sum::<u8>()
                                == cage.sum
                    })
                    .collect()
            })
            .collect();
        assert!(!by_cell.contains(&usize::MAX), "cages must cover the grid");
        Self {
            cages,
            by_cell,
            combinations,
            solution: OnceLock::new(),
        }
    }

    pub(super) fn share_cage(&self, index: usize, other: usize) -> bool {
        self.by_cell[index] == self.by_cell[other]
    }

    fn issue(&self, index: usize, values: &[u8; CELLS]) -> Option<CageIssue> {
        let &cage_index = self.by_cell.get(index)?;
        let cage = &self.cages[cage_index];
        let mut fixed = 0u16;
        let mut sum = 0;
        for &cell in cage.cells {
            let digit = values[cell];
            if digit == 0 {
                continue;
            }
            let bit = 1 << digit;
            if fixed & bit != 0 {
                return Some(CageIssue::Repeated(digit));
            }
            fixed |= bit;
            sum += digit;
        }

        if sum > cage.sum {
            Some(CageIssue::OverTotal)
        } else if fixed.count_ones() as usize == cage.cells.len() && sum != cage.sum {
            Some(CageIssue::WrongTotal)
        } else if !self.combinations[cage_index]
            .iter()
            .any(|&mask| mask & fixed == fixed)
        {
            Some(CageIssue::ImpossibleTotal)
        } else {
            None
        }
    }

    /// Keep only digits that participate in a complete assignment for their cage.
    /// An assignment must honor both the sum and each square's Sudoku candidates.
    pub(super) fn candidates(
        &self,
        values: &[u8; CELLS],
        used: &Occupancy,
    ) -> Option<[u16; CELLS]> {
        let mut candidates = std::array::from_fn(|index| {
            if values[index] == 0 {
                used.allowed(FORMAT, index)
            } else {
                0
            }
        });
        for (cage, combinations) in self.cages.iter().zip(&self.combinations) {
            let mut fixed = 0;
            let mut empty = [0; SIZE];
            let mut len = 0;
            for &index in cage.cells {
                let value = values[index];
                if value == 0 {
                    empty[len] = index;
                    len += 1;
                } else {
                    let bit = 1 << value;
                    if fixed & bit != 0 {
                        return None;
                    }
                    fixed |= bit;
                }
            }
            let empty = &mut empty[..len];
            empty.sort_unstable_by_key(|&index| candidates[index].count_ones());
            let mut supported = [0; SIZE];
            let mut feasible = false;
            for &mask in combinations {
                if mask & fixed != fixed {
                    continue;
                }
                feasible |= assign(empty, mask & !fixed, &candidates, &mut supported);
                if feasible
                    && empty
                        .iter()
                        .zip(&supported)
                        .all(|(&index, &mask)| candidates[index] == mask)
                {
                    break;
                }
            }
            if !feasible {
                return None;
            }
            for (&index, mask) in empty.iter().zip(supported) {
                candidates[index] = mask;
            }
        }
        Some(candidates)
    }

    fn solve(&self, limit: usize) -> (usize, [u8; CELLS]) {
        let mut solution = [0; CELLS];
        let count = self.search(
            &mut [0; CELLS],
            &mut Occupancy::default(),
            limit,
            &mut solution,
        );
        (count, solution)
    }

    fn search(
        &self,
        values: &mut [u8; CELLS],
        used: &mut Occupancy,
        limit: usize,
        solution: &mut [u8; CELLS],
    ) -> usize {
        let Some(candidates) = self.candidates(values, used) else {
            return 0;
        };
        let Some(index) = (0..CELLS)
            .filter(|&index| values[index] == 0)
            .min_by_key(|&index| candidates[index].count_ones())
        else {
            *solution = *values;
            return 1;
        };
        let mut allowed = candidates[index];
        let mut count = 0;
        while allowed != 0 && count < limit {
            let digit = allowed.trailing_zeros() as u8;
            let bit = 1 << digit;
            allowed &= !bit;
            values[index] = digit;
            used.toggle(FORMAT, index, bit);
            count += self.search(values, used, limit - count, solution);
            used.toggle(FORMAT, index, bit);
        }
        values[index] = 0;
        count
    }
}

fn assign(empty: &[usize], available: u16, candidates: &[u16; CELLS], support: &mut [u16]) -> bool {
    let Some((&index, rest)) = empty.split_first() else {
        return available == 0;
    };
    let (current, remaining) = support
        .split_first_mut()
        .expect("support for every empty cell");
    let mut allowed = available & candidates[index];
    let mut feasible = false;
    while allowed != 0 {
        let bit = 1 << allowed.trailing_zeros();
        allowed &= !bit;
        if assign(rest, available & !bit, candidates, remaining) {
            *current |= bit;
            feasible = true;
        }
    }
    feasible
}
