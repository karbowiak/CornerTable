use super::game::RunConflict;
use super::{DIGITS, Puzzle, Run};

pub(super) fn combinations(length: usize, sum: u8) -> Box<[u16]> {
    (2..=DIGITS)
        .step_by(2)
        .filter(|&mask| {
            mask.count_ones() as usize == length
                && (1..=9).filter(|digit| mask & (1 << digit) != 0).sum::<u8>() == sum
        })
        .collect()
}

/// Digits that can still be used, taking this run's sum and its existing digits
/// into account. An error explains why the run has no legal completion.
pub(super) fn remaining(run: &Run, values: &[u8]) -> Result<u16, RunConflict> {
    let mut used = 0;
    for digit in run
        .cells
        .iter()
        .map(|&cell| values[cell])
        .filter(|&digit| digit != 0)
    {
        let bit = 1 << digit;
        if used & bit != 0 {
            return Err(RunConflict::RepeatedDigit);
        }
        used |= bit;
    }
    let possible = run
        .combinations
        .iter()
        .filter(|&&mask| mask & used == used)
        .fold(0, |union, &mask| union | mask);
    (possible != 0)
        .then_some(possible & !used)
        .ok_or(RunConflict::UnreachableSum)
}

/// Count up to two answers. Each branch fixes one digit. Run propagation only
/// removes a candidate if no assignment of the entire run can use that digit.
pub(super) fn unique_solution(puzzle: &Puzzle) -> Option<Vec<u8>> {
    let mut domains = vec![0; puzzle.cells.len()];
    for &cell in &puzzle.entries {
        domains[cell] = DIGITS;
    }
    let mut first = None;
    if search(puzzle, domains, &mut first, 2) == 1 {
        first
    } else {
        None
    }
}

fn search(puzzle: &Puzzle, mut domains: Vec<u16>, first: &mut Option<Vec<u8>>, limit: u8) -> u8 {
    if !propagate(puzzle, &mut domains) {
        return 0;
    }
    let next = puzzle
        .entries
        .iter()
        .copied()
        .filter(|&cell| !domains[cell].is_power_of_two())
        .min_by_key(|&cell| domains[cell].count_ones());
    let Some(cell) = next else {
        first.get_or_insert_with(|| {
            domains
                .iter()
                .map(|mask| {
                    if *mask == 0 {
                        0
                    } else {
                        mask.trailing_zeros() as u8
                    }
                })
                .collect()
        });
        return 1;
    };
    let mut found = 0;
    for digit in bits(domains[cell]) {
        let mut branch = domains.clone();
        branch[cell] = digit;
        found += search(puzzle, branch, first, limit - found);
        if found == limit {
            break;
        }
    }
    found
}

fn propagate(puzzle: &Puzzle, domains: &mut [u16]) -> bool {
    loop {
        let mut changed = false;
        for run in &puzzle.runs {
            let count = run.cells.len();
            let mut supports = [0; 9];
            let mut choices = [(0, 0); 9];
            for &combination in &run.combinations {
                for (offset, &cell) in run.cells.iter().enumerate() {
                    choices[offset] = (offset, domains[cell] & combination);
                }
                if choices[..count].iter().any(|&(_, mask)| mask == 0)
                    || choices[..count]
                        .iter()
                        .all(|&(offset, mask)| supports[offset] & mask == mask)
                {
                    continue;
                }
                choices[..count].sort_unstable_by_key(|&(_, mask)| mask.count_ones());
                assign(&choices[..count], combination, &mut [0; 9], &mut supports);
            }
            for (offset, &cell) in run.cells.iter().enumerate() {
                let possible = supports[offset];
                if possible == 0 {
                    return false;
                }
                changed |= domains[cell] != possible;
                domains[cell] = possible;
            }
        }
        if !changed {
            return true;
        }
    }
}

fn assign(choices: &[(usize, u16)], unused: u16, path: &mut [u16; 9], supports: &mut [u16; 9]) {
    let Some(((offset, mask), rest)) = choices.split_first() else {
        for (support, &digit) in supports.iter_mut().zip(path.iter()) {
            *support |= digit;
        }
        return;
    };
    for digit in bits(*mask & unused) {
        path[*offset] = digit;
        assign(rest, unused ^ digit, path, supports);
    }
    path[*offset] = 0;
}

fn bits(mut mask: u16) -> impl Iterator<Item = u16> {
    std::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let bit = 1 << mask.trailing_zeros();
        mask ^= bit;
        Some(bit)
    })
}
