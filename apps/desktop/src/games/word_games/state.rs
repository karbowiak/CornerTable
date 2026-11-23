use std::time::{SystemTime, UNIX_EPOCH};

use chrono::NaiveDate;
use corner_table_word_games::{
    Crossword, GridSize, Mode, Rejection, WORD_GOAL, WordRound, WordSearch,
};
use serde::{Deserialize, Serialize};

use super::search_play::SearchPlay;
use crate::{
    audio::Effect,
    games::GameEntry,
    storage::{AttemptId, Outcome, ResultMetric},
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Variant {
    #[default]
    Grove,
    Rack,
    Mini,
    Large,
    Search,
}

impl Variant {
    pub const TABS: [Self; 4] = [Self::Grove, Self::Rack, Self::Mini, Self::Search];

    pub fn title(self) -> &'static str {
        match self {
            Self::Grove => "Letter Grove",
            Self::Rack => "Letter Rack",
            Self::Mini | Self::Large => "Mini Crossword",
            Self::Search => "Word Search",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Grove => "grove",
            Self::Rack => "rack",
            Self::Mini => "mini",
            Self::Large => "large",
            Self::Search => "search",
        }
    }

    fn slot(self) -> usize {
        match self {
            Self::Grove => 0,
            Self::Rack => 1,
            Self::Mini => 2,
            Self::Large => 3,
            Self::Search => 4,
        }
    }

    pub fn grid(self) -> Option<GridSize> {
        match self {
            Self::Mini => Some(GridSize::Mini),
            Self::Large => Some(GridSize::Large),
            Self::Grove | Self::Rack | Self::Search => None,
        }
    }

    pub fn word_mode(self) -> Mode {
        match self {
            Self::Grove => Mode::Grove,
            Self::Rack => Mode::Rack,
            _ => unreachable!("grid puzzles use their own boards"),
        }
    }

    pub fn puzzle_count(self) -> usize {
        match self {
            Self::Search => WordSearch::themes().len(),
            _ => self
                .grid()
                .map_or_else(|| self.word_mode().puzzle_count(), GridSize::puzzle_count),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Shelf {
    #[default]
    Collection,
    Daily,
    Challenge,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Source {
    Collection(usize),
    Daily(u64),
    Challenge(u64),
}

impl Source {
    pub fn daily_date(self) -> Option<NaiveDate> {
        let Self::Daily(day) = self else { return None };
        NaiveDate::from_epoch_days(day.try_into().ok()?)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct PuzzleKey {
    pub variant: Variant,
    pub source: Source,
}

impl PuzzleKey {
    pub fn storage_id(self) -> String {
        let source = match self.source {
            Source::Collection(index) => format!("collection-{index}"),
            Source::Daily(day) => format!("daily-{day}"),
            Source::Challenge(seed) => format!("challenge-{seed}"),
        };
        format!("word-games-v1-{}-{source}", self.variant.slug())
    }

    pub(super) fn seed(self) -> u64 {
        match self.source {
            Source::Collection(index) => index as u64,
            Source::Daily(day) => day.wrapping_add(19 * (self.variant.slot() as u64 + 1)),
            Source::Challenge(seed) => seed,
        }
    }

    pub fn result_mode(self) -> String {
        let title = if let Some(grid) = self.variant.grid() {
            let side = grid.side();
            format!("{} {side}×{side}", self.variant.title())
        } else {
            self.variant.title().to_owned()
        };
        let source = match self.source {
            Source::Collection(index) if self.variant == Variant::Search => {
                WordSearch::themes()[index].title.to_owned()
            }
            Source::Collection(index) => format!("Puzzle {}", index + 1),
            Source::Daily(_) => self
                .source
                .daily_date()
                .map_or_else(|| "Daily".to_owned(), |date| format!("Daily {date}")),
            Source::Challenge(seed) if self.variant == Variant::Search => {
                format!("Challenge {seed:016x}")
            }
            Source::Challenge(_) => "Challenge".to_owned(),
        };
        format!("{title} · {source}")
    }
}

pub(super) fn utc_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since_epoch| since_epoch.as_secs() / 86_400)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub(super) struct Cursor {
    pub index: usize,
    pub shelf: Shelf,
    #[serde(default)]
    day: Option<u64>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct Navigation {
    pub variant: Variant,
    large: bool,
    cursors: [Cursor; 4],
    #[serde(default)]
    search: Cursor,
}

impl Navigation {
    pub fn initial(entry: &GameEntry) -> Self {
        let mut nav = Self::default();
        match entry {
            GameEntry::Mode(id) => match id.as_str() {
                "rack" | "letter-rack" => nav.select(Variant::Rack),
                "crossword" | "mini-crossword" => nav.select(Variant::Mini),
                "large-crossword" => nav.select(Variant::Large),
                "search" | "word-search" => nav.select(Variant::Search),
                _ => {}
            },
            GameEntry::Challenge(_) => nav.edit().shelf = Shelf::Challenge,
            GameEntry::Classic => {}
        }
        nav
    }

    pub fn select(&mut self, variant: Variant) {
        self.variant = variant;
        if variant.grid().is_some() {
            self.large = variant == Variant::Large;
        }
    }

    pub fn crossword(&self) -> Variant {
        if self.large {
            Variant::Large
        } else {
            Variant::Mini
        }
    }

    pub fn cursor(&self) -> Cursor {
        let mut cursor = if self.variant == Variant::Search {
            self.search
        } else {
            self.cursors[self.variant.slot()]
        };
        cursor.index = cursor.index.min(self.variant.puzzle_count() - 1);
        cursor
    }

    pub fn edit(&mut self) -> &mut Cursor {
        if self.variant == Variant::Search {
            &mut self.search
        } else {
            &mut self.cursors[self.variant.slot()]
        }
    }

    pub fn select_shelf(&mut self, shelf: Shelf, today: u64) {
        let cursor = self.edit();
        cursor.shelf = shelf;
        if shelf == Shelf::Daily {
            cursor.day = Some(today);
        }
    }

    pub fn resume(&mut self, today: u64, challenge: Option<u64>) -> PuzzleKey {
        let cursor = self.edit();
        if cursor.shelf == Shelf::Daily {
            cursor.day.get_or_insert(today);
        }
        self.key(today, challenge)
    }

    pub fn key(&self, day: u64, challenge: Option<u64>) -> PuzzleKey {
        let cursor = self.cursor();
        let source = match (cursor.shelf, challenge) {
            (Shelf::Daily, _) => Source::Daily(cursor.day.unwrap_or(day)),
            (Shelf::Challenge, Some(seed)) => Source::Challenge(seed),
            _ => Source::Collection(cursor.index),
        };
        PuzzleKey {
            variant: self.variant,
            source,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub(super) enum Saved {
    Words {
        found: Vec<String>,
        selection: Vec<usize>,
        finished: bool,
        hint: Option<WordHint>,
        hints_used: u32,
        shuffle: u32,
        #[serde(default)]
        attempt: AttemptId,
    },
    Crossword {
        letters: Vec<Option<u8>>,
        hints: u16,
        revealed: bool,
        cursor: usize,
        down: bool,
        checked: u16,
        #[serde(default)]
        attempt: AttemptId,
        #[serde(default)]
        moves: Option<u64>,
    },
    Search {
        found: Vec<String>,
        hint: Option<String>,
        hints_used: u32,
        moves: u64,
        attempt: AttemptId,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct WordHint {
    pub word: String,
    pub show_path: bool,
}

pub(super) struct WordPlay {
    pub round: WordRound,
    pub hint: Option<WordHint>,
    pub hints_used: u32,
    pub shuffle: u32,
    pub notice: String,
    pub confirm_finish: bool,
    attempt: AttemptId,
}

impl WordPlay {
    pub fn submit(&mut self) -> Option<Effect> {
        let candidate = self.round.candidate();
        let points = self.round.submit();
        self.notice = match points {
            Ok(points) => {
                if self
                    .hint
                    .as_ref()
                    .is_some_and(|hint| candidate.eq_ignore_ascii_case(&hint.word))
                {
                    self.hint = None;
                }
                format!(
                    "{candidate} · +{points} {}",
                    plural(points, "point", "points")
                )
            }
            Err(Rejection::TooShort) => "Use at least three letters.".into(),
            Err(Rejection::UnknownWord) => {
                "That spelling isn’t in the game dictionary. Try another word.".into()
            }
            Err(Rejection::AlreadyFound) => "You’ve already found that word.".into(),
            Err(Rejection::Finished) => String::new(),
        };
        match points {
            Ok(_) => Some(Effect::Clear),
            Err(Rejection::Finished) => None,
            Err(_) => Some(Effect::InvalidMove),
        }
    }

    pub fn hint(&mut self) {
        if let Some(hint) = &mut self.hint {
            hint.show_path = true;
            self.round.clear();
        } else if let Some(word) = self.round.hint_word() {
            self.hint = Some(WordHint {
                word: word.into(),
                show_path: false,
            });
            self.hints_used = self.hints_used.saturating_add(1);
        }
        self.notice.clear();
    }

    pub fn finish(&mut self) {
        self.round.finish();
        self.hint = None;
        self.notice.clear();
        self.confirm_finish = false;
    }

    pub fn display_order(&self) -> Vec<usize> {
        let mut order: Vec<_> = (0..self.round.letters().len()).collect();
        if self.shuffle > 0 {
            let mut random = u64::from(self.shuffle);
            for end in (1..order.len()).rev() {
                random = random
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                order.swap(end, ((random >> 32) % (end as u64 + 1)) as usize);
            }
        }
        order
    }
}

pub(super) struct CrosswordPlay {
    pub board: Crossword,
    pub cursor: usize,
    pub down: bool,
    pub checked: u16,
    pub notice: String,
    pub confirm_reveal: bool,
    attempt: AttemptId,
    moves: u64,
}

impl CrosswordPlay {
    pub fn check(&mut self) {
        if self.board.complete() {
            return;
        }
        self.checked = self.board.incorrect_squares();
        let count = self.checked.count_ones();
        self.notice = match count {
            0 if self.board.letters().iter().all(Option::is_none) => {
                "Enter a few letters to check.".into()
            }
            0 => "Every letter entered so far is correct.".into(),
            _ => format!("{count} {} to revisit.", plural(count, "letter", "letters")),
        };
    }

    pub fn select_clue(&mut self, line: usize, down: bool) {
        let mut squares = self.board.size().line(line, down);
        let first = squares.next().expect("every clue has squares");
        self.cursor = std::iter::once(first)
            .chain(squares)
            .find(|&square| self.board.letters()[square].is_none())
            .unwrap_or(first);
        self.down = down;
    }

    pub fn step_clue(&mut self, backward: bool, prefer_empty: bool) {
        let size = self.board.size();
        let side = size.side();
        let total = side * 2;
        let from = usize::from(self.down) * side + size.line_at(self.cursor, self.down);
        for offset in 1..total {
            let step = if backward { total - offset } else { offset };
            let to = (from + step) % total;
            let (line, down) = (to % side, to >= side);
            if !prefer_empty
                || size
                    .line(line, down)
                    .any(|i| self.board.letters()[i].is_none())
            {
                self.select_clue(line, down);
                break;
            }
        }
    }

    pub fn move_cursor(&mut self, x: isize, y: isize) {
        let side = self.board.size().side();
        let column = self.cursor % side;
        let row = self.cursor / side;
        self.cursor = row.saturating_add_signed(y).min(side - 1) * side
            + column.saturating_add_signed(x).min(side - 1);
        self.down = y != 0;
    }

    pub fn type_letter(&mut self, letter: char) -> Option<Effect> {
        if self.board.hinted_at(self.cursor) {
            self.notice = "That square is locked by a hint. Move to another square.".into();
            return None;
        }
        let previous = self.board.letters()[self.cursor];
        if !self.board.put(self.cursor, Some(letter)) {
            return None;
        }
        self.entered_letter(previous)
    }

    pub fn erase(&mut self, backward: bool) {
        if self.board.complete() {
            return;
        }
        if backward
            && (self.board.letters()[self.cursor].is_none() || self.board.hinted_at(self.cursor))
        {
            let size = self.board.size();
            let clue = size.line_at(self.cursor, self.down);
            if let Some(previous) = size
                .line(clue, self.down)
                .take_while(|&square| square != self.cursor)
                .filter(|&square| !self.board.hinted_at(square))
                .last()
            {
                self.cursor = previous;
            }
        }
        let had_letter = self.board.letters()[self.cursor].is_some();
        if self.board.put(self.cursor, None) {
            if had_letter {
                self.moves = self.moves.saturating_add(1);
            }
            self.edited();
        }
    }

    pub fn hint(&mut self) -> Option<Effect> {
        let previous = self.board.letters()[self.cursor];
        if self.board.hint(self.cursor) {
            let effect = self.entered_letter(previous);
            self.notice = "Hinted letters are locked.".into();
            effect.or(Some(Effect::Move))
        } else {
            self.notice = "This square already has the right letter.".into();
            None
        }
    }

    fn entered_letter(&mut self, previous: Option<u8>) -> Option<Effect> {
        let mut effect = None;
        if previous != self.board.letters()[self.cursor] {
            self.moves = self.moves.saturating_add(1);
            let size = self.board.size();
            if previous.is_none()
                && [false, true].into_iter().any(|down| {
                    size.line(size.line_at(self.cursor, down), down)
                        .all(|square| self.board.letters()[square].is_some())
                })
            {
                effect = Some(Effect::Clear);
            }
        }
        self.edited();
        self.advance();
        effect
    }

    fn edited(&mut self) {
        self.checked &= !(1 << self.cursor);
        self.notice.clear();
        self.confirm_reveal = false;
    }

    fn advance(&mut self) {
        if self.board.complete() {
            return;
        }
        let size = self.board.size();
        let line = size.line_at(self.cursor, self.down);
        if let Some(next) = size
            .line(line, self.down)
            .skip_while(|&square| square <= self.cursor)
            .find(|&square| !self.board.hinted_at(square))
        {
            self.cursor = next;
        } else {
            self.step_clue(false, true);
        }
    }
}

pub(super) enum Play {
    Words(WordPlay),
    Crossword(CrosswordPlay),
    Search(SearchPlay),
}

impl Play {
    pub fn load(key: PuzzleKey, saved: Option<Saved>) -> Self {
        if key.variant == Variant::Search {
            Self::Search(SearchPlay::load(key, saved))
        } else if let Some(size) = key.variant.grid() {
            let mut play = CrosswordPlay {
                board: Crossword::with_size(size, key.seed()),
                cursor: 0,
                down: false,
                checked: 0,
                notice: String::new(),
                confirm_reveal: false,
                attempt: AttemptId::default(),
                moves: 0,
            };
            if let Some(Saved::Crossword {
                letters,
                hints,
                revealed,
                cursor,
                down,
                checked,
                attempt,
                moves,
            }) = saved
            {
                play.board.restore(&letters, hints, revealed);
                play.cursor = cursor.min(play.board.letters().len() - 1);
                play.down = down;
                play.checked = checked & play.board.incorrect_squares();
                play.attempt = attempt;
                play.moves =
                    moves.unwrap_or_else(|| play.board.letters().iter().flatten().count() as u64);
            }
            Self::Crossword(play)
        } else {
            let mut play = WordPlay {
                round: WordRound::new(key.variant.word_mode(), key.seed()),
                hint: None,
                hints_used: 0,
                shuffle: 0,
                notice: String::new(),
                confirm_finish: false,
                attempt: AttemptId::default(),
            };
            if let Some(Saved::Words {
                found,
                selection,
                finished,
                hint,
                hints_used,
                shuffle,
                attempt,
            }) = saved
            {
                play.round.restore(found, &selection, finished);
                play.hint = hint.filter(|hint| {
                    !play.round.finished()
                        && play
                            .round
                            .familiar_remaining()
                            .any(|word| word == hint.word)
                });
                play.hints_used = hints_used;
                play.shuffle = shuffle;
                play.attempt = attempt;
            }
            Self::Words(play)
        }
    }

    pub fn is_finished(&self) -> bool {
        match self {
            Self::Words(play) => play.round.finished(),
            Self::Crossword(play) => play.board.complete(),
            Self::Search(play) => play.board.complete(),
        }
    }

    pub fn result(&self) -> Option<(AttemptId, Outcome, ResultMetric)> {
        match self {
            Self::Words(play) if play.round.finished() && play.round.score() > 0 => {
                let score = play.round.score();
                let won =
                    score >= WORD_GOAL || play.round.found().len() == play.round.answer_count();
                let outcome = if won { Outcome::Won } else { Outcome::Finished };
                Some((play.attempt, outcome, ResultMetric::Points(score.into())))
            }
            Self::Crossword(play) if play.board.complete() && !play.board.revealed() => {
                Some((play.attempt, Outcome::Won, ResultMetric::Moves(play.moves)))
            }
            Self::Search(play) if play.board.complete() => {
                Some((play.attempt, Outcome::Won, ResultMetric::Moves(play.moves)))
            }
            _ => None,
        }
    }

    pub fn save(&self) -> Saved {
        match self {
            Self::Words(play) => Saved::Words {
                found: play
                    .round
                    .found()
                    .iter()
                    .map(|(word, _)| word.clone())
                    .collect(),
                selection: play.round.selection().to_vec(),
                finished: play.round.finished(),
                hint: play.hint.clone(),
                hints_used: play.hints_used,
                shuffle: play.shuffle,
                attempt: play.attempt,
            },
            Self::Crossword(play) => Saved::Crossword {
                letters: play.board.letters().to_vec(),
                hints: play.board.hinted_squares(),
                revealed: play.board.revealed(),
                cursor: play.cursor,
                down: play.down,
                checked: play.checked,
                attempt: play.attempt,
                moves: Some(play.moves),
            },
            Self::Search(play) => play.save(),
        }
    }
}

pub(super) fn plural<'a>(count: u32, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_navigation_keeps_the_four_cursors_and_adds_an_independent_search_cursor() {
        let old = r#"{
            "variant":"large", "large":true,
            "cursors":[
                {"index":7,"shelf":"collection"},
                {"index":5,"shelf":"daily","day":300},
                {"index":3,"shelf":"collection"},
                {"index":2,"shelf":"collection"}
            ]
        }"#;
        let mut nav: Navigation = serde_json::from_str(old).unwrap();
        assert_eq!(nav.crossword(), Variant::Large);
        assert_eq!(nav.resume(301, None).source, Source::Collection(2));
        nav.select(Variant::Search);
        assert_eq!(nav.resume(301, None).source, Source::Collection(0));
        nav.edit().index = 9;
        nav.select(Variant::Rack);
        assert_eq!(nav.resume(301, None).source, Source::Daily(300));
        nav.select(Variant::Grove);
        assert_eq!(nav.resume(301, None).source, Source::Collection(7));
        nav.select(Variant::Search);
        let saved = serde_json::to_string(&nav).unwrap();
        let mut restored: Navigation = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.resume(301, None).source, Source::Collection(9));
        assert_eq!(
            Navigation::initial(&GameEntry::Mode("word-search".into())).variant,
            Variant::Search,
        );
    }

    #[test]
    fn search_resumes_words_and_hint_and_records_the_same_completed_attempt() {
        let key = PuzzleKey {
            variant: Variant::Search,
            source: Source::Collection(0),
        };
        let Play::Search(mut play) = Play::load(key, None) else {
            panic!("expected Word Search");
        };
        let attempt = play.attempt;
        let line = play.board.line(1);
        play.click(line.end);
        assert!(matches!(play.click(line.start), Some(Effect::Clear)));
        assert!(matches!(play.hint(), Some(Effect::Move)));
        assert!(play.hint().is_none());
        let saved = serde_json::to_string(&play.save()).unwrap();
        let Play::Search(mut restored) = Play::load(key, serde_json::from_str(&saved).unwrap())
        else {
            panic!("expected Word Search");
        };
        assert!(restored.board.found(1));
        assert_eq!(restored.board.line(1), line);
        assert_eq!(restored.hint, Some(0));
        assert_eq!(restored.hints_used, 1);
        assert_eq!(restored.moves, 2);
        let total = restored.board.theme().words.len();
        for index in (0..total).filter(|&i| i != 1) {
            let line = restored.board.line(index);
            restored.click(line.start);
            assert!(matches!(restored.click(line.end), Some(Effect::Clear)));
        }
        assert_eq!(restored.hint, None);

        let play = Play::Search(restored);
        let result = play.result().unwrap();
        assert!(result.0 == attempt);
        assert_eq!(result.1, Outcome::Won);
        assert_eq!(result.2, ResultMetric::Moves(total as u64 + 1));
        let reopened = Play::load(key, Some(play.save()));
        assert!(reopened.is_finished());
        assert!(reopened.result().unwrap().0 == attempt);
        assert!(Play::load(key, None).result().is_none());
    }

    #[test]
    fn switching_variants_and_daily_play_remembers_collection_choices() {
        let mut nav = Navigation::initial(&GameEntry::Classic);
        nav.edit().index = 12;
        let grove = nav.key(100, None);
        nav.select(Variant::Rack);
        nav.edit().shelf = Shelf::Daily;
        let rack = nav.key(100, None);
        assert_ne!(rack.storage_id(), nav.key(101, None).storage_id());
        assert_ne!(rack.seed(), nav.key(101, None).seed());
        nav.select(Variant::Large);
        nav.edit().index = 2;
        let crossword = nav.key(100, None);
        nav.select(Variant::Grove);
        assert_eq!(nav.key(101, None), grove);
        nav.select(nav.crossword());
        assert_eq!(nav.key(101, None), crossword);
        nav.select(Variant::Rack);
        assert_eq!(nav.key(100, None), rack);
    }

    #[test]
    fn resumes_the_saved_daily_after_midnight_until_today_is_selected() {
        let mut nav = Navigation::initial(&GameEntry::Classic);
        nav.select(Variant::Large);
        nav.select_shelf(Shelf::Daily, 20_000);
        let previous = nav.resume(20_000, None);
        let saved = serde_json::to_string(&nav).unwrap();
        let mut resumed: Navigation = serde_json::from_str(&saved).unwrap();
        assert_eq!(resumed.resume(20_001, None), previous);

        resumed.select(Variant::Grove);
        assert_eq!(resumed.resume(20_001, None).source, Source::Collection(0));
        resumed.select(resumed.crossword());
        assert_eq!(resumed.resume(20_001, None), previous);
        resumed.select_shelf(Shelf::Daily, 20_001);
        assert_eq!(resumed.resume(20_001, None).source, Source::Daily(20_001));
    }

    #[test]
    fn serialization_preserves_an_in_progress_word_and_hint() {
        let key = PuzzleKey {
            variant: Variant::Rack,
            source: Source::Collection(0),
        };
        let Play::Words(mut play) = Play::load(key, None) else {
            panic!("expected a rack");
        };
        for letter in "garden".chars() {
            assert!(play.round.type_letter(letter));
        }
        assert!(matches!(play.submit(), Some(Effect::Clear)));
        play.hint();
        for letter in "sand".chars() {
            assert!(play.round.type_letter(letter));
        }
        let saved = serde_json::to_string(&Play::Words(play).save()).unwrap();
        let Play::Words(restored) = Play::load(key, serde_json::from_str(&saved).unwrap()) else {
            panic!("expected a rack");
        };
        assert_eq!(restored.round.found()[0].0, "GARDEN");
        assert_eq!(restored.round.candidate(), "SAND");
        assert!(restored.hint.is_some());
        assert_eq!(restored.hints_used, 1);
    }

    #[test]
    fn word_results_use_the_played_score_and_ignore_an_empty_finish() {
        let key = PuzzleKey {
            variant: Variant::Rack,
            source: Source::Collection(0),
        };
        let Play::Words(mut play) = Play::load(key, None) else {
            panic!("expected a rack");
        };
        play.finish();
        assert!(Play::Words(play).result().is_none());

        let Play::Words(mut play) = Play::load(key, None) else {
            panic!("expected a rack");
        };
        for letter in "garden".chars() {
            assert!(play.round.type_letter(letter));
        }
        assert!(matches!(play.submit(), Some(Effect::Clear)));
        let score = play.round.score();
        let attempt = play.attempt;
        assert!(score < WORD_GOAL);
        play.finish();
        let saved = serde_json::to_value(Play::Words(play).save()).unwrap();
        let result = Play::load(key, Some(serde_json::from_value(saved).unwrap()));
        let (restored, outcome, metric) = result.result().unwrap();
        assert!(restored == attempt);
        assert_eq!(outcome, Outcome::Finished);
        assert_eq!(metric, ResultMetric::Points(score.into()));
    }

    #[test]
    fn crossword_typing_steps_to_the_next_clue_and_keeps_marks() {
        let key = PuzzleKey {
            variant: Variant::Mini,
            source: Source::Collection(0),
        };
        let Play::Crossword(mut play) = Play::load(key, None) else {
            panic!("expected a grid");
        };
        assert!(play.type_letter('c').is_none());
        assert!(play.type_letter('a').is_none());
        assert!(matches!(play.type_letter('r'), Some(Effect::Clear)));
        assert_eq!(play.cursor, 3);
        play.check();
        assert_eq!(play.checked, 1 << 2);
        let saved = serde_json::to_string(&Play::Crossword(play).save()).unwrap();
        let Play::Crossword(mut restored) = Play::load(key, serde_json::from_str(&saved).unwrap())
        else {
            panic!("expected a grid");
        };
        assert_eq!(restored.cursor, 3);
        assert_eq!(restored.checked, 1 << 2);
        restored.hint();
        assert_eq!(restored.board.hinted_squares(), 1 << 3);
        assert_eq!(restored.checked, 1 << 2);
    }

    #[test]
    fn crossword_records_letter_moves_and_skips_the_revealed_solution() {
        let key = PuzzleKey {
            variant: Variant::Mini,
            source: Source::Collection(0),
        };
        let Play::Crossword(mut play) = Play::load(key, None) else {
            panic!("expected a grid");
        };
        for letter in "CATOREDE".chars() {
            play.type_letter(letter);
        }
        let attempt = play.attempt;
        let saved = serde_json::to_value(Play::Crossword(play).save()).unwrap();
        let Play::Crossword(mut restored) =
            Play::load(key, Some(serde_json::from_value(saved).unwrap()))
        else {
            panic!("expected a grid");
        };
        restored.type_letter('N');
        let (restored_attempt, outcome, metric) = Play::Crossword(restored).result().unwrap();
        assert!(restored_attempt == attempt);
        assert_eq!(outcome, Outcome::Won);
        assert_eq!(metric, ResultMetric::Moves(9));

        let Play::Crossword(mut replay) = Play::load(key, None) else {
            panic!("expected a grid");
        };
        assert!(replay.attempt != attempt);
        replay.board.reveal();
        assert!(Play::Crossword(replay).result().is_none());
    }
}
