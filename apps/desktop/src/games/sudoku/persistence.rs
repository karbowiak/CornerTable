use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_sudoku::{Difficulty, Edit, Format};
use serde::{Deserialize, Serialize};

use super::{GameEntry, GameLaunch, Puzzle, Round, Session, utc_day};

const SAVED_UNDO_LIMIT: usize = 128;

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    #[default]
    Standard,
    Mini,
    Daily(u64),
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "Difficulty", rename_all = "kebab-case")]
enum DifficultyName {
    Easy,
    Medium,
    Hard,
}

#[derive(Deserialize, Serialize)]
pub(super) struct PracticeModes {
    mode: Mode,
    #[serde(with = "DifficultyName")]
    standard: Difficulty,
    #[serde(with = "DifficultyName")]
    mini: Difficulty,
}

impl Default for PracticeModes {
    fn default() -> Self {
        Self {
            mode: Mode::Standard,
            standard: Difficulty::Easy,
            mini: Difficulty::Easy,
        }
    }
}

impl PracticeModes {
    pub(super) fn practice(&self, format: Format) -> Puzzle {
        let difficulty = match format {
            Format::Standard => self.standard,
            Format::Mini => self.mini,
        };
        Puzzle::Practice(format, difficulty)
    }

    pub(super) fn selected_puzzle(&self) -> Puzzle {
        match self.mode {
            Mode::Mini => self.practice(Format::Mini),
            Mode::Daily(day) if day <= utc_day() => Puzzle::Daily(day),
            Mode::Standard | Mode::Daily(_) => self.practice(Format::Standard),
        }
    }

    pub(super) fn select(&mut self, puzzle: Puzzle) {
        match puzzle {
            Puzzle::Practice(Format::Standard, difficulty) => {
                self.standard = difficulty;
                self.mode = Mode::Standard;
            }
            Puzzle::Practice(Format::Mini, difficulty) => {
                self.mini = difficulty;
                self.mode = Mode::Mini;
            }
            Puzzle::Daily(day) => self.mode = Mode::Daily(day),
            Puzzle::Challenge(_) => {}
        }
    }
}

impl Puzzle {
    pub(super) fn session(self, launch: &GameLaunch) -> Session {
        let key = match self {
            Self::Practice(Format::Standard, difficulty) => format!("9x9/{}", difficulty.label()),
            Self::Practice(Format::Mini, difficulty) => format!("6x6/{}", difficulty.label()),
            Self::Daily(day) => format!("daily/{day}"),
            Self::Challenge(_) => return Session::new(launch),
        };
        let origin = match &launch.entry {
            GameEntry::Mode(id) => format!("mode/{}:{id}", id.len()),
            _ => "classic".to_owned(),
        };
        Session::new(&GameLaunch {
            game: launch.game.clone(),
            entry: GameEntry::Mode(format!("sudoku/v1/{origin}/{key}")),
        })
    }
}

// The stored grid is the position before the retained undo edits.
#[derive(Deserialize, Serialize)]
struct SavedRound<Values, Notes> {
    seed: u64,
    values: Values,
    notes: Notes,
    edits: Vec<(usize, u8, bool)>,
    selected: usize,
    note_mode: bool,
    hint_cell: Option<usize>,
    hint_open: bool,
    hints_used: u32,
    best: Option<u32>,
}

impl Round {
    pub(super) fn load(puzzle: Puzzle, session: &Session) -> Self {
        if let Some(saved) = session.load()
            && let Some(round) = Self::restore(puzzle, saved)
        {
            return round;
        }
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos() as u64);
        let round = Self::new(puzzle, seed);
        round.save(session);
        round
    }

    fn restore(puzzle: Puzzle, saved: SavedRound<Vec<u8>, Vec<u16>>) -> Option<Self> {
        if puzzle.fixed_seed().is_some_and(|seed| seed != saved.seed)
            || saved.edits.len() > SAVED_UNDO_LIMIT
        {
            return None;
        }
        let mut round = Self::new(puzzle, saved.seed);
        if !round.game.restore(
            &saved.values,
            &saved.notes,
            saved
                .edits
                .into_iter()
                .map(|(index, digit, note)| Edit { index, digit, note }),
        ) {
            return None;
        }
        let cells = round.game.format().cell_count();
        if saved.selected < cells {
            round.selected = saved.selected;
        }
        round.note_mode = saved.note_mode;
        round.hints_used = saved.hints_used;
        round.best = saved.best;
        round.record_result();
        round.hint = saved
            .hint_cell
            .filter(|&index| index < cells && round.hints_used > 0)
            .and_then(|index| round.game.hint(index).filter(|hint| hint.index == index));
        round.hint_open = saved.hint_open && round.hint.is_some();
        Some(round)
    }

    fn saved(&self) -> SavedRound<&[u8], &[u16]> {
        let checkpoint = self.game.checkpoint(SAVED_UNDO_LIMIT);
        SavedRound {
            seed: self.seed,
            values: checkpoint.values,
            notes: checkpoint.notes,
            edits: checkpoint
                .edits()
                .map(|Edit { index, digit, note }| (index, digit, note))
                .collect(),
            selected: self.selected,
            note_mode: self.note_mode,
            hint_cell: self.hint.map(|hint| hint.index),
            hint_open: self.hint_open,
            hints_used: self.hints_used,
            best: self.best,
        }
    }

    pub(super) fn save(&self, session: &Session) {
        session.save(&self.saved());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoded(round: &Round) -> SavedRound<Vec<u8>, Vec<u16>> {
        serde_json::from_slice(&serde_json::to_vec(&round.saved()).unwrap()).unwrap()
    }

    #[test]
    fn restores_notes_selection_and_undo_without_charging_for_a_cached_hint() {
        let puzzle = Puzzle::Practice(Format::Mini, Difficulty::Hard);
        let mut round = Round::new(puzzle, 173);
        let index = round.selected;
        round.note_mode = true;
        round.edit(6, true);
        round.show_hint();
        let hint = round.visible_hint().unwrap();
        round.hint_open = false;
        round.select_blank(false);

        let mut restored = Round::restore(puzzle, decoded(&round)).unwrap();
        assert_eq!(restored.selected, round.selected);
        assert!(restored.note_mode);
        assert_eq!(restored.game.notes(index), 1 << 6);
        assert!(restored.visible_hint().is_none());
        restored.show_hint();
        assert_eq!(restored.visible_hint(), Some(hint));
        assert_eq!(restored.hints_used, 1);
        restored.undo();
        assert_eq!(restored.game.notes(index), 0);
        assert_eq!(restored.hints_used, 1);
    }

    #[test]
    fn keeps_a_clean_daily_result_after_reload_and_undo() {
        let puzzle = Puzzle::Daily(20_000);
        let mut round = Round::new(puzzle, 0);
        while let Some(hint) = round.game.hint(round.selected) {
            round.selected = hint.index;
            round.edit(hint.entry(), false);
        }
        assert!(round.game.is_solved());

        let mut restored = Round::restore(puzzle, decoded(&round)).unwrap();
        assert!(restored.game.is_solved());
        assert_eq!(restored.best, Some(0));
        restored.undo();
        assert!(!restored.game.is_solved());
        assert_eq!(restored.best, Some(0));
        assert!(Round::restore(Puzzle::Daily(20_001), decoded(&round)).is_none());
        assert!(Round::restore(Puzzle::Challenge(20_001), decoded(&round)).is_none());
    }

    #[test]
    fn repairs_a_bad_selection_and_rejects_corrupt_moves_and_seeds() {
        let puzzle = Puzzle::Challenge(132);
        let round = Round::new(puzzle, 0);
        let mut saved = decoded(&round);
        saved.selected = usize::MAX;
        saved.hint_cell = Some(usize::MAX);
        saved.hint_open = true;
        saved.hints_used = 1;
        let restored = Round::restore(puzzle, saved).unwrap();
        assert_eq!(restored.selected, round.selected);
        assert!(restored.visible_hint().is_none());
        assert_eq!(restored.hints_used, 1);

        let mut saved = decoded(&round);
        saved.edits.push((81, 2, false));
        assert!(Round::restore(puzzle, saved).is_none());
        let mut saved = decoded(&round);
        saved.seed = 0;
        assert!(Round::restore(puzzle, saved).is_none());
    }

    #[test]
    fn remembers_each_practice_difficulty_when_resuming_a_daily() {
        let mut modes = PracticeModes::default();
        modes.select(Puzzle::Practice(Format::Mini, Difficulty::Hard));
        modes.select(Puzzle::Practice(Format::Standard, Difficulty::Medium));
        modes.select(Puzzle::Daily(20_000));
        let restored: PracticeModes =
            serde_json::from_slice(&serde_json::to_vec(&modes).unwrap()).unwrap();
        assert_eq!(restored.selected_puzzle(), Puzzle::Daily(20_000));
        assert_eq!(
            restored.practice(Format::Standard),
            Puzzle::Practice(Format::Standard, Difficulty::Medium)
        );
        assert_eq!(
            restored.practice(Format::Mini),
            Puzzle::Practice(Format::Mini, Difficulty::Hard)
        );
    }
}
