use std::collections::BTreeMap;

use corner_table_kakuro::{Cell, Edit, Game, Size};
use serde::{Deserialize, Serialize};

use super::Round;
use crate::storage::AttemptId;

#[derive(Default, Deserialize, Serialize)]
pub(super) struct SavedSession {
    size: Option<usize>,
    numbers: [usize; 2],
    boards: [BTreeMap<usize, SavedRound>; 2],
}

impl SavedSession {
    pub fn size(&self) -> Option<Size> {
        Size::ALL.get(self.size?).copied()
    }

    pub fn number(&self, size: Size) -> usize {
        let number = self.numbers[size_index(size)];
        if number < size.puzzle_count() {
            number
        } else {
            0
        }
    }

    pub fn open(&self, size: Size, number: usize) -> Round {
        let round = Round::new(size, number);
        let Some(saved) = self.boards[size_index(size)].get(&number) else {
            return round;
        };
        if let Some((game, selected)) = saved.replay(round.game) {
            return Round {
                number,
                game,
                selected,
                pencil: saved.pencil,
                hint: None,
                moves: saved.moves,
                attempt: saved.attempt,
            };
        }
        // A corrupt replay may have partly edited its board. Start a clean attempt.
        Round::new(size, number)
    }

    pub fn remember(&mut self, round: &Round) {
        let size = size_index(round.game.puzzle().size());
        self.size = Some(size);
        self.numbers[size] = round.number;
        self.boards[size].insert(round.number, SavedRound::capture(round));
    }
}

fn size_index(size: Size) -> usize {
    match size {
        Size::Compact => 0,
        Size::Classic => 1,
    }
}

#[derive(Deserialize, Serialize)]
struct SavedRound {
    selected: usize,
    pencil: bool,
    edits: Vec<(usize, u8, bool)>,
    moves: u64,
    attempt: AttemptId,
}

impl SavedRound {
    fn capture(round: &Round) -> Self {
        Self {
            selected: round.selected,
            pencil: round.pencil,
            edits: round
                .game
                .edits()
                .map(|edit| (edit.cell, edit.digit, edit.pencil))
                .collect(),
            moves: round.moves,
            attempt: round.attempt,
        }
    }

    fn replay(&self, mut game: Game) -> Option<(Game, usize)> {
        for &(cell, digit, pencil) in &self.edits {
            if !game.edit(Edit {
                cell,
                digit,
                pencil,
            }) {
                return None;
            }
        }
        let puzzle = game.puzzle();
        let selected = if puzzle.cells().get(self.selected) == Some(&Cell::Entry) {
            self.selected
        } else {
            puzzle.entries()[0]
        };
        Some((game, selected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_keeps_each_board_selection_pencil_marks_entries_and_undo() {
        let mut progress = SavedSession::default();
        let mut compact = progress.open(Size::Compact, 7);
        let compact_cell = compact.selected;
        compact.pencil = true;
        assert!(compact.edit(8, true).is_some());
        assert!(compact.edit(4, false).is_some());
        progress.remember(&compact);
        let mut classic = progress.open(Size::Classic, 4);
        classic.selected = classic.game.puzzle().entries()[1];
        assert!(classic.edit(9, false).is_some());
        progress.remember(&classic);

        let encoded = serde_json::to_vec(&progress).unwrap();
        let restored: SavedSession = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored.size(), Some(Size::Classic));
        assert_eq!(restored.number(Size::Classic), 4);
        assert_eq!(restored.number(Size::Compact), 7);
        let larger = restored.open(Size::Classic, 4);
        assert_eq!(larger.selected, classic.selected);
        assert_eq!(larger.game.value(larger.selected), 9);
        let mut smaller = restored.open(Size::Compact, 7);
        assert_eq!(smaller.game.value(compact_cell), 4);
        assert!(smaller.pencil);
        assert_eq!(smaller.moves, 1);
        assert!(smaller.undo().is_some());
        assert_eq!(smaller.game.notes(compact_cell), 1 << 8);
        assert_eq!(smaller.game.value(compact_cell), 0);
        assert_eq!(restored.open(Size::Compact, 8).game.value(compact_cell), 0);
    }

    #[test]
    fn malformed_save_recovers_only_the_affected_board() {
        let mut progress = SavedSession::default();
        let mut invalid = progress.open(Size::Compact, 0);
        assert!(invalid.edit(3, false).is_some());
        progress.remember(&invalid);
        let mut valid = progress.open(Size::Compact, 1);
        assert!(valid.edit(2, false).is_some());
        progress.remember(&valid);
        progress.boards[0]
            .get_mut(&0)
            .unwrap()
            .edits
            .push((0, 3, false));
        let fresh = progress.open(Size::Compact, 0);
        assert_eq!(
            fresh.game.empty_count(),
            fresh.game.puzzle().entries().len()
        );
        assert_eq!(
            progress.open(Size::Compact, 1).game.value(valid.selected),
            2
        );
    }
}
