use super::{Axis, Cell, Clues, Deduction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolutionCount {
    None,
    Unique,
    Multiple,
}

/// Counts solutions to the clues alone, stopping as soon as a second one is found.
/// Propagation proves forced cells. A stalled search splits on the possible
/// patterns of one line, which partitions every remaining grid into disjoint cases.
pub fn solution_count(clues: &Clues) -> SolutionCount {
    let lines = lines(clues);
    let mut count = 0;
    search(&lines, vec![Known::default(); clues.height], &mut count);
    match count {
        0 => SolutionCount::None,
        1 => SolutionCount::Unique,
        _ => SolutionCount::Multiple,
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Known {
    filled: u32,
    empty: u32,
}

impl Known {
    fn add(&mut self, offset: usize, cell: Cell) {
        match cell {
            Cell::Filled => self.filled |= 1 << offset,
            Cell::Marked => self.empty |= 1 << offset,
            Cell::Blank => {}
        }
    }

    fn cell(self, offset: usize) -> Cell {
        let bit = 1 << offset;
        if self.filled & bit != 0 {
            Cell::Filled
        } else if self.empty & bit != 0 {
            Cell::Marked
        } else {
            Cell::Blank
        }
    }

    fn agrees(self, pattern: u32) -> bool {
        pattern & self.filled == self.filled && pattern & self.empty == 0
    }
}

struct Line {
    axis: Axis,
    index: usize,
    span: usize,
    patterns: Vec<u32>,
}

impl Line {
    fn known(&self, rows: &[Known]) -> Known {
        match self.axis {
            Axis::Row => rows[self.index],
            Axis::Column => {
                let mut known = Known::default();
                for (offset, row) in rows.iter().enumerate() {
                    known.add(offset, row.cell(self.index));
                }
                known
            }
        }
    }

    fn update(&self, rows: &mut [Known], known: Known) {
        match self.axis {
            Axis::Row => rows[self.index] = known,
            Axis::Column => {
                for (offset, row) in rows.iter_mut().enumerate() {
                    row.add(self.index, known.cell(offset));
                }
            }
        }
    }

    fn mask(&self) -> u32 {
        low_bits(self.span)
    }

    fn fits(&self, known: Known) -> Vec<u32> {
        self.patterns
            .iter()
            .copied()
            .filter(|pattern| known.agrees(*pattern))
            .collect()
    }
}

fn low_bits(length: usize) -> u32 {
    (1 << length) - 1
}

fn lines(clues: &Clues) -> Vec<Line> {
    [
        (Axis::Row, &clues.rows, clues.width),
        (Axis::Column, &clues.columns, clues.height),
    ]
    .into_iter()
    .flat_map(|(axis, clues, span)| {
        clues.iter().enumerate().map(move |(index, clue)| {
            let mut patterns = Vec::new();
            place_runs(span, clue, 0, 0, &mut patterns);
            Line {
                axis,
                index,
                span,
                patterns,
            }
        })
    })
    .collect()
}

fn place_runs(span: usize, clue: &[u8], earliest: usize, pattern: u32, patterns: &mut Vec<u32>) {
    let Some((&first, rest)) = clue.split_first() else {
        patterns.push(pattern);
        return;
    };
    let space_required = clue.iter().map(|n| usize::from(*n)).sum::<usize>() + rest.len();
    let run = usize::from(first);
    for start in earliest..=span - space_required {
        place_runs(
            span,
            rest,
            start + run + 1,
            pattern | (low_bits(run) << start),
            patterns,
        );
    }
}

fn consensus(patterns: &[u32], mask: u32) -> Known {
    let mut all = mask;
    let mut any = 0;
    for pattern in patterns {
        all &= *pattern;
        any |= *pattern;
    }
    Known {
        filled: all,
        empty: mask ^ any,
    }
}

fn search(lines: &[Line], mut rows: Vec<Known>, count: &mut usize) {
    loop {
        let mut changed = false;
        let mut split: Option<(&Line, Vec<u32>)> = None;
        for line in lines {
            let known = line.known(&rows);
            let patterns = line.fits(known);
            if patterns.is_empty() {
                return;
            }
            let forced = consensus(&patterns, line.mask());
            if forced != known {
                line.update(&mut rows, forced);
                changed = true;
            }
            if patterns.len() > 1
                && split
                    .as_ref()
                    .is_none_or(|(_, previous)| patterns.len() < previous.len())
            {
                split = Some((line, patterns));
            }
        }
        // Check another full pass after propagation. A late column deduction
        // can leave a fully assigned row that still needs checking.
        if changed {
            continue;
        }
        let Some((line, patterns)) = split else {
            *count += 1;
            return;
        };
        for pattern in patterns {
            let mut branch = rows.clone();
            line.update(
                &mut branch,
                Known {
                    filled: pattern,
                    empty: line.mask() ^ pattern,
                },
            );
            search(lines, branch, count);
            if *count >= 2 {
                return;
            }
        }
        return;
    }
}

pub(super) fn deductions(clues: &Clues, cells: &[Cell]) -> Vec<Deduction> {
    let rows = cells
        .chunks_exact(clues.width)
        .map(|cells| {
            let mut known = Known::default();
            for (offset, cell) in cells.iter().enumerate() {
                known.add(offset, *cell);
            }
            known
        })
        .collect::<Vec<_>>();

    let mut deductions = Vec::new();
    for line in lines(clues) {
        let known = line.known(&rows);
        let patterns = line.fits(known);
        if patterns.is_empty() {
            continue;
        }
        let forced = consensus(&patterns, line.mask());
        for offset in 0..line.span {
            let cell = forced.cell(offset);
            if cell != Cell::Blank && known.cell(offset) == Cell::Blank {
                deductions.push(Deduction {
                    index: line.axis.index(line.index, offset, clues.width),
                    cell,
                    axis: line.axis,
                    line: line.index,
                });
            }
        }
    }
    deductions
}
