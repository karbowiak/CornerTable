use std::collections::BTreeMap;

use corner_table_picture_logic::{Board, Cell, PUZZLES, Puzzle};
use serde::{Deserialize, Serialize};

use crate::{games::GameLaunch, storage::Session};

const UNDO_LIMIT: usize = 32;

pub(super) struct Notebook {
    session: Session,
    pub active: usize,
    pub plays: Vec<Play>,
}

#[derive(Default, Deserialize, Serialize)]
struct SavedNotebook {
    active: String,
    plates: BTreeMap<String, SavedPlate>,
}

#[derive(Deserialize, Serialize)]
struct SavedPlate {
    cells: String,
    cursor: usize,
    undo: Vec<String>,
}

pub(super) struct Play {
    pub board: Board,
    pub cursor: usize,
    pub solved: bool,
    undo: Vec<String>,
}

impl Notebook {
    pub fn load(launch: &GameLaunch) -> Self {
        let session = Session::new(launch);
        let saved = session.load::<SavedNotebook>().unwrap_or_default();
        let active = PUZZLES
            .iter()
            .position(|puzzle| puzzle.id == saved.active)
            .unwrap_or(0);
        let plays = PUZZLES
            .iter()
            .map(|puzzle| Play::load(puzzle, saved.plates.get(puzzle.id)))
            .collect();
        Self {
            session,
            active,
            plays,
        }
    }

    pub fn play(&self) -> &Play {
        &self.plays[self.active]
    }

    pub fn edit(&mut self) -> &mut Play {
        &mut self.plays[self.active]
    }

    pub fn save(&self) {
        self.session.save(&SavedNotebook {
            active: self.play().board.puzzle().id.to_owned(),
            plates: self
                .plays
                .iter()
                .filter(|play| play.started() || play.can_undo())
                .map(|play| (play.board.puzzle().id.to_owned(), play.saved()))
                .collect(),
        });
    }

    /// Look ahead in collection order and wrap to any earlier unfinished plate.
    pub fn next_unfinished(&self) -> Option<usize> {
        let count = self.plays.len();
        (1..count)
            .map(|offset| (self.active + offset) % count)
            .find(|index| !self.plays[*index].solved)
    }
}

impl Play {
    fn load(puzzle: &'static Puzzle, saved: Option<&SavedPlate>) -> Self {
        let mut board = Board::new(puzzle);
        let mut cursor = 0;
        let mut undo = Vec::new();
        if let Some(saved) = saved
            && board.restore(&saved.cells)
        {
            cursor = saved.cursor.min(board.cells().len() - 1);
            if !board.complete() {
                let mut scratch = board.clone();
                undo.extend(
                    saved.undo[saved.undo.len().saturating_sub(UNDO_LIMIT)..]
                        .iter()
                        .filter(|snapshot| scratch.restore(snapshot) && !scratch.complete())
                        .cloned(),
                );
            }
        }
        Self {
            solved: board.complete(),
            board,
            cursor,
            undo,
        }
    }

    fn saved(&self) -> SavedPlate {
        SavedPlate {
            cells: self.board.snapshot(),
            cursor: self.cursor,
            undo: self.undo.clone(),
        }
    }

    pub fn started(&self) -> bool {
        self.board.cells().iter().any(|cell| *cell != Cell::Blank)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn commit_turn(&mut self, before: String) -> bool {
        if before == self.board.snapshot() {
            return false;
        }
        self.solved = self.board.complete();
        if self.solved {
            self.undo.clear();
        } else {
            if self.undo.len() == UNDO_LIMIT {
                self.undo.remove(0);
            }
            self.undo.push(before);
        }
        true
    }

    pub fn undo(&mut self) -> bool {
        self.undo
            .pop()
            .is_some_and(|cells| self.board.restore(&cells))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_stroke_is_one_turn_after_reopening() {
        let mut play = Play::load(&PUZZLES[0], None);
        let before = play.board.snapshot();
        for index in 15..20 {
            assert!(play.board.paint(index, Cell::Filled));
        }
        play.cursor = 19;
        assert!(play.commit_turn(before.clone()));

        let mut resumed = Play::load(&PUZZLES[0], Some(&play.saved()));
        assert_eq!(resumed.cursor, 19);
        assert_eq!(resumed.board.snapshot(), play.board.snapshot());
        assert!(resumed.undo());
        assert_eq!(resumed.board.snapshot(), before);
        assert!(!resumed.undo());
    }

    #[test]
    fn solved_is_derived_from_the_saved_grid() {
        let puzzle = &PUZZLES[0];
        let mut play = Play::load(puzzle, None);
        let before = play.board.snapshot();
        for index in 0..play.board.cells().len() {
            if puzzle.filled(index) {
                play.board.paint(index, Cell::Filled);
            }
        }
        assert!(play.commit_turn(before));
        assert!(play.solved);
        assert!(!play.can_undo());
        assert!(Play::load(puzzle, Some(&play.saved())).solved);
    }
}
