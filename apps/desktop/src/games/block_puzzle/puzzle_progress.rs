use std::collections::BTreeMap;

use corner_table_block_puzzle::{PUZZLE_BOARDS, Puzzle};
use serde::{Deserialize, Serialize};

use super::{AttemptId, Round, SavedJournal};

#[derive(Default, Deserialize, Serialize)]
pub(super) struct SavedPuzzleBook {
    #[serde(default)]
    active: String,
    #[serde(default)]
    boards: BTreeMap<String, SavedPuzzle>,
}

#[derive(Deserialize, Serialize)]
struct SavedPuzzle {
    #[serde(flatten)]
    journal: SavedJournal,
    #[serde(default)]
    attempt: AttemptId,
    #[serde(default)]
    best_moves: Option<usize>,
}

pub(super) struct PuzzleRound {
    pub definition: &'static Puzzle,
    pub round: Round,
    pub best_moves: Option<usize>,
}

impl PuzzleRound {
    fn restore(definition: &'static Puzzle, saved: Option<SavedPuzzle>) -> Self {
        let mut entry = Self {
            definition,
            round: Round::new(definition.play()),
            best_moves: None,
        };
        if let Some(saved) = saved {
            let total = entry
                .round
                .game
                .placements_left()
                .expect("puzzles have a fixed tray");
            entry.round = entry
                .round
                .replay(saved.journal, saved.attempt)
                .unwrap_or_else(|| Round::new(definition.play()));
            entry.best_moves = saved.best_moves.filter(|moves| (1..=total).contains(moves));
            entry.update_record();
        }
        entry
    }

    fn saved(&self) -> Option<SavedPuzzle> {
        (!self.round.turns.is_empty() || self.best_moves.is_some()).then(|| SavedPuzzle {
            journal: self.round.journal(),
            attempt: self.round.attempt,
            best_moves: self.best_moves,
        })
    }

    pub fn restart(&mut self) {
        self.round = Round::new(self.definition.play());
    }

    pub fn update_record(&mut self) -> bool {
        if !self.round.game.puzzle_solved() {
            return false;
        }
        let moves = self.round.turns.len();
        self.best_moves = Some(self.best_moves.map_or(moves, |best| best.min(moves)));
        true
    }
}

pub(super) struct PuzzleBook {
    pub boards: Vec<PuzzleRound>,
    pub active: usize,
}

impl PuzzleBook {
    pub fn restore(mut saved: SavedPuzzleBook) -> Self {
        let active = PUZZLE_BOARDS
            .iter()
            .position(|definition| definition.id == saved.active)
            .unwrap_or(0);
        let boards = PUZZLE_BOARDS
            .iter()
            .map(|definition| PuzzleRound::restore(definition, saved.boards.remove(definition.id)))
            .collect();
        Self { boards, active }
    }

    pub fn current(&self) -> &PuzzleRound {
        &self.boards[self.active]
    }

    pub fn current_mut(&mut self) -> &mut PuzzleRound {
        &mut self.boards[self.active]
    }

    pub fn choose(&mut self, index: usize) -> bool {
        if index == self.active || index >= self.boards.len() {
            return false;
        }
        self.active = index;
        true
    }

    pub fn saved(&self) -> SavedPuzzleBook {
        SavedPuzzleBook {
            active: self.current().definition.id.to_owned(),
            boards: self
                .boards
                .iter()
                .filter_map(|board| {
                    board
                        .saved()
                        .map(|saved| (board.definition.id.to_owned(), saved))
                })
                .collect(),
        }
    }

    pub fn solved_count(&self) -> usize {
        self.boards
            .iter()
            .filter(|board| board.best_moves.is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_board_journals_restore_the_selected_board_undo_and_records() {
        let saved = serde_json::json!({
            "active": "inlay",
            "boards": {
                "first-crossing": {
                    "turns": [[0, 4, 3]],
                    "can_undo": true
                },
                "inlay": {
                    "turns": [[0, 1, 1]],
                    "can_undo": true
                },
                "square-key": {
                    "turns": [[42, 99, 0]],
                    "can_undo": true,
                    "best_moves": 2
                }
            }
        });
        let book = PuzzleBook::restore(serde_json::from_value(saved).unwrap());
        let inlay_attempt = book.current().round.attempt;
        let encoded = serde_json::to_value(book.saved()).unwrap();
        assert_eq!(
            encoded["boards"]["inlay"]["turns"],
            serde_json::json!([[0, 1, 1]])
        );
        let mut book = PuzzleBook::restore(serde_json::from_value(encoded).unwrap());
        let inlay = book.current();
        assert_eq!(inlay.definition.id, "inlay");
        assert_eq!(inlay.definition.mark_progress(&inlay.round.game), (4, 7));
        assert_eq!(inlay.round.undo, Some(inlay.definition.play()));
        assert!(inlay.round.attempt == inlay_attempt);

        assert!(book.choose(0));
        assert!(book.current().round.game.puzzle_solved());
        assert_eq!(book.current().best_moves, Some(1));
        let attempt = book.current().round.attempt;
        book.current_mut().restart();
        assert!(book.current().round.attempt != attempt);
        assert_eq!(book.current().round.game, book.current().definition.play());
        assert!(book.current().round.undo.is_none());
        assert_eq!(book.solved_count(), 2);

        assert!(book.choose(2));
        assert!(book.current().round.turns.is_empty());
        assert_eq!(book.current().round.game, book.current().definition.play());
        assert_eq!(book.current().best_moves, Some(2));
    }
}
