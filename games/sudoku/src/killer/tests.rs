use super::*;
use crate::{HintReason, Unit};

#[test]
fn each_original_grid_has_a_unique_solution_from_its_cages_alone() {
    let mut solutions = Vec::new();
    for puzzle in KillerPuzzle::ALL {
        let game = Game::killer(puzzle);
        assert_eq!(game.format(), Format::Standard);
        assert_eq!(game.filled(), 0);
        assert!(game.givens.iter().all(|&given| given == 0));
        let (count, digits) = puzzle.rules().solve(2);
        assert_eq!(count, 1, "{}", puzzle.label());
        assert_eq!(digits, game.solution);
        assert!(!solutions.contains(&digits));
        solutions.push(digits);

        for index in 0..SIZE {
            for unit in [Unit::Row(index), Unit::Column(index), Unit::Box(index)] {
                let mask = unit
                    .cells(FORMAT)
                    .fold(0, |mask, cell| mask | (1 << digits[cell]));
                assert_eq!(mask, DIGITS);
            }
        }

        let mut covered = [false; CELLS];
        for cage in game.cages() {
            assert!(cage.cells().is_sorted());
            let mut connected = vec![cage.anchor()];
            while connected.len() < cage.cells().len() {
                let next = cage.cells().iter().copied().find(|cell| {
                    !connected.contains(cell)
                        && connected.iter().any(|neighbor| {
                            (cell / SIZE).abs_diff(neighbor / SIZE)
                                + (cell % SIZE).abs_diff(neighbor % SIZE)
                                == 1
                        })
                });
                connected.push(next.expect("cages must be orthogonally connected"));
            }
            let mut mask = 0;
            let mut sum = 0;
            for &cell in cage.cells() {
                assert!(!covered[cell]);
                covered[cell] = true;
                assert_eq!(game.cage(cell), Some(cage));
                let digit = digits[cell];
                assert_eq!(mask & (1 << digit), 0);
                mask |= 1 << digit;
                sum += digit;
            }
            assert_eq!(sum, cage.sum());
        }
        assert!(covered.iter().all(|&cell| cell));
    }
}

#[test]
fn solution_count_distinguishes_a_two_solution_trade_from_an_impossible_sum() {
    let solution = Game::killer(KillerPuzzle::Patchwork).solution;
    let indices = Box::leak(Box::new(std::array::from_fn::<_, CELLS, _>(|index| index)));
    let cages = |override_sum| {
        let mut cages: Vec<_> = (0..CELLS)
            .filter(|index| ![68, 69, 77, 78].contains(index))
            .map(|index| Cage {
                sum: solution[index],
                cells: &indices[index..index + 1],
            })
            .collect();
        // The last two rows can swap the same pair across a box boundary.
        cages.push(Cage {
            sum: override_sum,
            cells: &[68, 69],
        });
        cages.push(Cage {
            sum: solution[77] + solution[78],
            cells: &[77, 78],
        });
        Rules::new(Box::leak(cages.into_boxed_slice()))
    };
    assert_eq!(cages(solution[68] + solution[69]).solve(3).0, 2);
    assert_eq!(cages(18).solve(2).0, 0);
}

#[test]
fn cage_sums_detect_overflow_wrong_totals_and_partial_impossibilities() {
    let mut game = Game::killer(KillerPuzzle::FirstLight);
    let cage = game.cage(0).unwrap();
    assert_eq!(cage.cells(), &[0, 9]);
    assert_eq!(cage.sum(), 8);
    assert!(game.enter(0, 9, false));
    assert_eq!(game.cage_issue(9), Some(CageIssue::OverTotal));
    assert!(game.enter(0, 4, false));
    // Only a second 4 would reach 8.
    assert_eq!(game.cage_issue(0), Some(CageIssue::ImpossibleTotal));
    assert!(game.enter(0, 5, false));
    assert_eq!(game.cage_issue(0), None);
    assert!(game.enter(9, 2, false));
    assert_eq!(game.cage_issue(9), Some(CageIssue::WrongTotal));
    assert_eq!(
        game.hint(9).unwrap().reason,
        HintReason::CageConflict {
            issue: CageIssue::WrongTotal
        }
    );
    assert!(!game.has_conflict(9));
    assert!(game.enter(9, 3, false));
    assert_eq!(game.cage_issue(9), None);
}

#[test]
fn a_cage_extends_duplicate_checks_and_note_pruning_across_boxes() {
    let mut game = Game::killer(KillerPuzzle::FirstLight);
    let (first, diagonal) = (1, 12);
    let digit = game.solution[first];
    assert!(!FORMAT.related(first, diagonal));
    assert_eq!(game.cage(first), game.cage(diagonal));
    assert!(game.enter(diagonal, digit, true));
    assert!(game.enter(first, digit, false));
    assert_eq!(game.notes(diagonal), 0);
    assert!(game.undo());
    assert_eq!(game.notes(diagonal), 1 << digit);

    assert!(game.enter(first, digit, false));
    assert!(game.enter(diagonal, digit, false));
    assert_eq!(game.cage_issue(first), Some(CageIssue::Repeated(digit)));
    assert!(!game.has_conflict(first));
    assert!(!game.has_conflict(diagonal));
    let hint = game.hint(first).unwrap();
    assert_eq!(hint.index, diagonal);
    assert_eq!(
        hint.reason,
        HintReason::CageConflict {
            issue: CageIssue::Repeated(digit)
        }
    );
    assert_eq!(hint.entry(), 0);
}

#[test]
fn killer_hints_are_sound_and_the_intro_can_finish_with_deductions() {
    for puzzle in KillerPuzzle::ALL {
        let mut game = Game::killer(puzzle);
        if puzzle == KillerPuzzle::FirstLight {
            let hint = game.hint(4).unwrap();
            assert_eq!(hint.index, 4);
            assert_eq!(hint.reason, HintReason::CageCandidate { digit: 9 });
        }
        for entered in 0..CELLS {
            let hint = game.hint(0).expect("hint until the grid is full");
            assert_eq!(game.filled(), entered);
            assert_eq!(hint.entry(), game.solution[hint.index]);
            if puzzle == KillerPuzzle::FirstLight {
                assert!(!matches!(hint.reason, HintReason::Forced { .. }));
            }
            assert!(game.enter(hint.index, hint.entry(), false));
        }
        assert!(game.is_solved());
        assert_eq!(game.hint(0), None);
    }
}
