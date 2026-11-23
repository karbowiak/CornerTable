use std::collections::{HashMap, HashSet};

use super::*;

#[test]
fn curated_prints_have_unique_solutions_and_need_no_guesses() {
    let mut identifiers = HashSet::new();
    let mut failures = Vec::new();
    for puzzle in PUZZLES {
        assert!(identifiers.insert(puzzle.id), "Duplicate id {}", puzzle.id);
        assert!(
            puzzle.print.iter().all(|line| {
                line.len() == puzzle.width()
                    && line.bytes().all(|b| matches!(b, b'.' | b'a'..=b'd'))
            }),
            "Malformed print {}",
            puzzle.id
        );
        let mut board = Board::new(puzzle);
        let count = solution_count(board.clues());
        while !board.complete() {
            let Some(deduction) = board.deduction(0) else {
                break;
            };
            assert!(board.paint(deduction.index, deduction.cell));
            assert!(!board.has_conflict(deduction.index), "{}", puzzle.id);
        }
        if count != SolutionCount::Unique || !board.complete() {
            failures.push((puzzle.id, count, board.complete()));
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn solver_matches_exhaustive_counts_for_all_three_by_three_patterns() {
    let mut counts = HashMap::new();
    for bits in 0_u16..(1 << 9) {
        let row_key = (0..3)
            .map(|row| runs((0..3).map(|col| bits & (1 << (row * 3 + col)) != 0)))
            .collect::<Vec<_>>();
        let column_key = (0..3)
            .map(|col| runs((0..3).map(|row| bits & (1 << (row * 3 + col)) != 0)))
            .collect::<Vec<_>>();
        *counts.entry((row_key, column_key)).or_insert(0) += 1;
    }
    for ((rows, columns), count) in counts {
        let clues = Clues::new(3, 3, rows, columns).unwrap();
        assert_eq!(
            solution_count(&clues),
            if count == 1 {
                SolutionCount::Unique
            } else {
                SolutionCount::Multiple
            },
            "{clues:?}"
        );
    }
}

#[test]
fn solver_rejects_conflicting_crossings_with_the_same_total_ink() {
    let clues = Clues::new(3, 2, vec![vec![3], vec![]], vec![vec![2], vec![1], vec![]]).unwrap();
    assert_eq!(solution_count(&clues), SolutionCount::None);
    assert!(Clues::new(2, 1, vec![vec![1, 1]], vec![vec![], vec![]]).is_none());
    assert!(Clues::new(1, 1, vec![vec![0]], vec![vec![]]).is_none());
}

#[test]
fn restore_validates_all_cells_before_applying_changes() {
    let mut board = Board::new(&PUZZLES[0]);
    assert!(board.paint(0, Cell::Marked));
    assert!(board.paint(2, Cell::Filled));
    let snapshot = board.snapshot();
    let mut other = Board::new(&PUZZLES[0]);
    assert!(other.restore(&snapshot));
    assert_eq!(other.cells(), board.cells());
    assert!(!other.restore(&snapshot.replace('.', "*")));
    assert!(!other.restore(&snapshot[1..]));
    assert_eq!(other.snapshot(), snapshot);
}
