use corner_table_word_games::{SearchLine, SearchRejection, WordSearch};

use super::state::{PuzzleKey, Saved, Source};
use crate::{audio::Effect, storage::AttemptId};

#[derive(Clone, Copy)]
pub(super) struct Pointer {
    pub start: usize,
    pub end: Option<usize>,
    moved: bool,
}

pub(super) struct SearchPlay {
    pub board: WordSearch,
    pub cursor: usize,
    pub anchor: Option<usize>,
    pub pointer: Option<Pointer>,
    pub hint: Option<usize>,
    pub hints_used: u32,
    pub moves: u64,
    pub notice: String,
    pub attempt: AttemptId,
    pub latest_mark: Option<(usize, SearchLine)>,
}

impl SearchPlay {
    pub fn load(key: PuzzleKey, saved: Option<Saved>) -> Self {
        let board = match key.source {
            Source::Collection(index) => WordSearch::collection(index),
            Source::Daily(_) | Source::Challenge(_) => WordSearch::seeded(key.seed()),
            Source::Generated { theme, seed } => WordSearch::generated(theme, seed),
        };
        let mut play = Self {
            board,
            cursor: 0,
            anchor: None,
            pointer: None,
            hint: None,
            hints_used: 0,
            moves: 0,
            notice: String::new(),
            attempt: AttemptId::default(),
            latest_mark: None,
        };
        if let Some(Saved::Search {
            found,
            hint,
            hints_used,
            moves,
            attempt,
        }) = saved
        {
            play.board.restore(&found);
            let words = play.board.theme().words;
            play.hint = hint.and_then(|word| {
                words
                    .iter()
                    .enumerate()
                    .position(|(index, &name)| name == word && !play.board.found(index))
            });
            play.hints_used = hints_used;
            play.moves = moves.max(play.board.found_count() as u64 + u64::from(hints_used));
            play.attempt = attempt;
        }
        play
    }

    pub fn save(&self) -> Saved {
        let words = self.board.theme().words;
        Saved::Search {
            found: words
                .iter()
                .enumerate()
                .filter(|(index, _)| self.board.found(*index))
                .map(|(_, &word)| word.to_owned())
                .collect(),
            hint: self.hint.map(|index| words[index].to_owned()),
            hints_used: self.hints_used,
            moves: self.moves,
            attempt: self.attempt,
        }
    }

    pub fn hint(&mut self) -> Option<Effect> {
        if self.hint.is_some() {
            return None;
        }
        self.hint = self.board.hint();
        self.hint.map(|_| {
            self.cancel_selection();
            self.hints_used = self.hints_used.saturating_add(1);
            self.moves = self.moves.saturating_add(1);
            Effect::Move
        })
    }

    pub fn cancel_selection(&mut self) {
        self.pointer = None;
        self.anchor = None;
        self.notice.clear();
        self.latest_mark = None;
    }

    pub fn move_cursor(&mut self, dx: isize, dy: isize) {
        self.pointer = None;
        self.notice.clear();
        let side = self.board.theme().side;
        let column = (self.cursor % side).saturating_add_signed(dx).min(side - 1);
        let row = (self.cursor / side).saturating_add_signed(dy).min(side - 1);
        self.cursor = row * side + column;
    }

    pub fn click(&mut self, cell: usize) -> Option<Effect> {
        if self.board.complete() {
            return None;
        }
        self.cursor = cell;
        self.notice.clear();
        match self.anchor.take() {
            Some(start) if start != cell => self.submit(start, cell),
            Some(_) => None,
            None => {
                self.anchor = Some(cell);
                None
            }
        }
    }

    pub fn pointer_down(&mut self, cell: usize) {
        self.cursor = cell;
        self.notice.clear();
        self.pointer = Some(Pointer {
            start: cell,
            end: Some(cell),
            moved: false,
        });
    }

    pub fn pointer_move(&mut self, cell: Option<usize>) -> bool {
        let Some(pointer) = self.pointer.as_mut() else {
            return false;
        };
        if pointer.end == cell {
            return false;
        }
        pointer.end = cell;
        pointer.moved |= cell != Some(pointer.start);
        true
    }

    pub fn pointer_up(&mut self, cell: Option<usize>) -> Option<Effect> {
        let pointer = self.pointer.take()?;
        let Some(cell) = cell else {
            self.anchor = None;
            return None;
        };
        if !pointer.moved && cell == pointer.start {
            return self.click(cell);
        }
        self.cursor = cell;
        self.anchor = None;
        if cell == pointer.start {
            return None;
        }
        self.submit(pointer.start, cell)
    }

    fn submit(&mut self, start: usize, end: usize) -> Option<Effect> {
        let result = self.board.submit(start, end);
        if result == Err(SearchRejection::Complete) {
            return None;
        }
        self.moves = self.moves.saturating_add(1);
        let (notice, effect) = match result {
            Ok(index) => {
                self.latest_mark = Some((index, SearchLine { start, end }));
                if self.hint == Some(index) {
                    self.hint = None;
                }
                (
                    format!("Found {}.", self.board.theme().words[index]),
                    Effect::Clear,
                )
            }
            Err(SearchRejection::NotStraight) => (
                "Choose two letters on the same row, column or diagonal.".into(),
                Effect::InvalidMove,
            ),
            Err(SearchRejection::NoMatch) => (
                "That line doesn’t spell a listed word.".into(),
                Effect::InvalidMove,
            ),
            Err(SearchRejection::AlreadyFound) => (
                "You’ve already found that word.".into(),
                Effect::InvalidMove,
            ),
            Err(SearchRejection::Complete) => unreachable!(),
        };
        self.notice = notice;
        Some(effect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::word_games::state::Variant;

    #[test]
    fn dragging_commits_on_release_and_can_override_an_armed_click() {
        let mut play = SearchPlay::load(
            PuzzleKey {
                variant: Variant::Search,
                source: Source::Collection(0),
            },
            None,
        );
        let line = play.board.line(0);
        assert!(play.click(0).is_none());
        play.pointer_down(line.end);
        assert!(play.pointer_move(Some(line.start)));
        assert_eq!(play.board.found_count(), 0);
        assert_eq!(play.moves, 0);
        assert!(matches!(
            play.pointer_up(Some(line.start)),
            Some(Effect::Clear)
        ));
        assert_eq!(play.board.found_count(), 1);
        assert_eq!(play.anchor, None);

        let line = play.board.line(1);
        play.pointer_down(line.start);
        play.pointer_move(Some(line.end));
        assert!(play.pointer_up(None).is_none());
        assert!(play.pointer.is_none());
        assert_eq!(play.board.found_count(), 1);
        assert_eq!(play.moves, 1);
    }

    #[test]
    fn arrow_navigation_can_join_diagonal_endpoints() {
        let mut play = SearchPlay::load(
            PuzzleKey {
                variant: Variant::Search,
                source: Source::Collection(0),
            },
            None,
        );
        let side = play.board.theme().side;
        let line = play.board.line(2);
        play.cursor = line.start;
        play.click(play.cursor);
        while play.cursor % side != line.end % side {
            let dx = if play.cursor % side < line.end % side {
                1
            } else {
                -1
            };
            play.move_cursor(dx, 0);
        }
        while play.cursor / side != line.end / side {
            let dy = if play.cursor / side < line.end / side {
                1
            } else {
                -1
            };
            play.move_cursor(0, dy);
        }
        assert!(matches!(play.click(play.cursor), Some(Effect::Clear)));
        assert!(play.board.found(2));
    }
}
