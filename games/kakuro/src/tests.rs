use std::collections::HashSet;

use super::*;

#[test]
fn each_bundled_board_is_distinct_and_has_one_legal_solution() {
    for size in Size::ALL {
        let mut layouts = HashSet::new();
        assert_eq!(size.puzzle_count(), 48);
        for (number, source) in size.collection().lines().enumerate() {
            let layout: Vec<_> = source.bytes().map(|byte| byte == b'#').collect();
            assert!(layouts.insert(layout));

            let mut game = Game::new(size, number).expect("board must have one solution");
            let entries = game.puzzle().entries.clone();
            for &cell in &entries {
                for direction in Direction::ALL {
                    let run = game
                        .puzzle()
                        .run(game.puzzle().run_at(cell, direction).unwrap());
                    assert!(run.cells.contains(&cell));
                    assert!((2..=9).contains(&run.cells.len()));
                }
                let digit = source.as_bytes()[cell] - b'0';
                assert!(game.edit(Edit {
                    cell,
                    digit,
                    pencil: false
                }));
            }
            assert_eq!(game.empty_count(), 0);
            assert!(!game.has_conflicts());
            assert!(game.is_solved());
        }
        assert!(Game::new(size, size.puzzle_count()).is_none());
    }
}

#[test]
fn solver_rejects_an_ambiguous_board_and_an_impossible_board() {
    let mut puzzle = Puzzle::from_source(
        Size::Compact,
        concat!("######", "#12###", "#21###", "######", "######", "######"),
    )
    .unwrap();
    // Both the listed board and the one with 1 and 2 swapped meet every clue.
    assert!(solver::unique_solution(&puzzle).is_none());
    // The top across run now requires a 3 that neither down clue can admit.
    let across = puzzle.run_at(7, Direction::Across).unwrap();
    puzzle.runs[across].sum = 4;
    puzzle.runs[across].combinations = solver::combinations(2, 4);
    assert!(solver::unique_solution(&puzzle).is_none());
}

#[test]
fn template_rejects_runs_without_a_clue_or_with_one_square() {
    for source in [
        concat!("######", "#1####", "#2####", "######", "######", "######"),
        concat!("######", "12####", "21####", "######", "######", "######"),
        concat!("#12###", "#21###", "######", "######", "######", "######"),
    ] {
        assert!(Puzzle::from_source(Size::Compact, source).is_none());
    }
}

#[test]
fn pencil_marks_and_undo_never_change_run_totals() {
    let mut game = Game::new(Size::Compact, 0).unwrap();
    let cell = game.puzzle().entries()[0];
    let first = Edit {
        cell,
        digit: 3,
        pencil: true,
    };
    assert!(game.edit(first));
    assert_eq!(game.notes(cell), 1 << 3);
    assert_eq!(game.value(cell), 0);
    assert!(!game.has_conflicts());
    assert!(!game.edit(Edit {
        cell: 0,
        digit: 3,
        pencil: true
    }));
    assert!(!game.edit(Edit {
        cell,
        digit: 12,
        pencil: false
    }));

    assert!(game.edit(Edit {
        cell,
        digit: 1,
        pencil: false
    }));
    assert_eq!(game.notes(cell), 0);
    assert!(!game.edit(first));
    assert_eq!(game.undo(), Some(cell));
    assert_eq!(game.notes(cell), 1 << 3);
    assert_eq!(game.edits().collect::<Vec<_>>(), vec![first]);
}

#[test]
fn conflict_feedback_distinguishes_repeats_from_unreachable_sums() {
    let mut game = Game::new(Size::Compact, 0).unwrap();
    let first = game.puzzle().entries()[0];
    let run = game.puzzle().run_at(first, Direction::Across).unwrap();
    let second = game.puzzle().run(run).cells[1];
    assert_eq!(game.puzzle().run(run).sum, 7);
    assert!(game.edit(Edit {
        cell: first,
        digit: 9,
        pencil: false
    }));
    assert_eq!(game.run_conflict(run), Some(RunConflict::UnreachableSum));
    assert!(game.edit(Edit {
        cell: second,
        digit: 9,
        pencil: false
    }));
    assert_eq!(game.run_conflict(run), Some(RunConflict::RepeatedDigit));

    let hint = game.hint(second).unwrap();
    assert_eq!(
        hint,
        Hint {
            cell: second,
            digit: 0,
            kind: HintKind::Clear
        }
    );
    assert!(game.edit(Edit {
        cell: hint.cell,
        digit: hint.digit,
        pencil: false
    }));
    let hint = game.hint(first).unwrap();
    assert_eq!(hint.kind, HintKind::Clear);
    assert!(game.edit(Edit {
        cell: hint.cell,
        digit: hint.digit,
        pencil: false
    }));
    assert!(!game.has_conflicts());
}

#[test]
fn repeated_valid_hints_can_complete_a_puzzle() {
    for size in Size::ALL {
        let mut game = Game::new(size, 1).unwrap();
        let focus = game.puzzle().entries()[0];
        for remaining in (1..=game.empty_count()).rev() {
            let hint = game.hint(focus).unwrap();
            assert_ne!(hint.kind, HintKind::Clear);
            assert!(game.edit(Edit {
                cell: hint.cell,
                digit: hint.digit,
                pencil: false
            }));
            assert_eq!(game.empty_count(), remaining - 1);
            assert!(!game.has_conflicts());
        }
        assert!(game.is_solved());
        assert_eq!(game.hint(focus), None);
    }
}
