//! Seeded, offline rules for Letter Grove, Letter Rack, Mini Crossword, and Word Search.

mod crossword;
mod search;
mod search_catalog;
mod search_generate;

pub use crossword::{Clue, Crossword, CrosswordPuzzle, GridSize};
pub use search::{SearchLine, SearchRejection, WordSearch};
pub use search_catalog::SearchTheme;

use std::collections::BTreeSet;
use std::sync::LazyLock;

const SIDE: usize = 4;
pub const WORD_GOAL: u32 = 12;
pub const WORD_BONUS: u32 = 24;

static WORDS: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| include_str!("english.txt").lines().collect());
static FAMILIAR: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| include_str!("familiar.txt").split_whitespace().collect());
static GROVES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| include_str!("groves.txt").lines().collect());
static RACKS: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| include_str!("racks.txt").lines().collect());

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Grove,
    Rack,
    Crossword,
}

impl Mode {
    pub fn puzzle_count(self) -> usize {
        match self {
            Self::Grove => GROVES.len(),
            Self::Rack => RACKS.len(),
            Self::Crossword => GridSize::Mini.puzzle_count(),
        }
    }

    fn legacy_pool_size(self) -> u64 {
        match self {
            Self::Grove => 72,
            Self::Rack => 64,
            Self::Crossword => unreachable!("crosswords have their own catalog"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejection {
    Finished,
    TooShort,
    UnknownWord,
    AlreadyFound,
}

#[derive(Debug)]
pub struct WordRound {
    mode: Mode,
    letters: Vec<u8>,
    selection: Vec<usize>,
    found: Vec<(String, u32)>,
    answers: BTreeSet<&'static str>,
    finished: bool,
}

impl WordRound {
    pub fn new(mode: Mode, seed: u64) -> Self {
        // Preserve existing daily and challenge deals when the collection grows.
        let size = mode.legacy_pool_size();
        Self::from_pool(mode, (seed % size) as usize, seed, seed / size)
    }

    pub fn collection(mode: Mode, index: usize) -> Self {
        Self::from_pool(mode, index, index as u64, 0)
    }

    fn from_pool(mode: Mode, index: usize, seed: u64, cycle: u64) -> Self {
        let letters = match mode {
            Mode::Grove => {
                let board = GROVES[index].as_bytes();
                (0..SIDE * SIDE)
                    .map(|index| {
                        let (mut row, mut column) = (index / SIDE, index % SIDE);
                        if cycle & 1 != 0 {
                            std::mem::swap(&mut row, &mut column);
                        }
                        if cycle & 2 != 0 {
                            row = SIDE - 1 - row;
                        }
                        if cycle & 4 != 0 {
                            column = SIDE - 1 - column;
                        }
                        board[row * SIDE + column]
                    })
                    .collect()
            }
            Mode::Rack => {
                let mut rack = RACKS[index].as_bytes().to_vec();
                let mut random = seed;
                for end in (1..rack.len()).rev() {
                    random = random
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    rack.swap(end, ((random >> 32) % (end as u64 + 1)) as usize);
                }
                rack
            }
            Mode::Crossword => unreachable!(),
        };
        let mut round = Self {
            mode,
            letters,
            selection: Vec::new(),
            found: Vec::new(),
            answers: BTreeSet::new(),
            finished: false,
        };
        let mut candidate = String::with_capacity(round.letters.len());
        let mut answers = BTreeSet::new();
        round.search(0, None, &mut candidate, &mut answers);
        round.answers = answers;
        round
    }

    pub fn restore(&mut self, found: Vec<String>, selection: &[usize], finished: bool) {
        let mut seen = BTreeSet::new();
        self.found = found
            .into_iter()
            .filter_map(|word| {
                let word = word.to_ascii_lowercase();
                (self.answers.contains(word.as_str()) && seen.insert(word.clone()))
                    .then(|| (word.to_ascii_uppercase(), points(word.len())))
            })
            .collect();
        self.clear();
        self.finished = finished;
        if !self.finished() {
            for &tile in selection {
                if self.selection.contains(&tile) || !self.choose(tile) {
                    self.clear();
                    break;
                }
            }
        }
    }

    pub fn letters(&self) -> &[u8] {
        &self.letters
    }

    pub fn selection(&self) -> &[usize] {
        &self.selection
    }

    pub fn candidate(&self) -> String {
        self.selection
            .iter()
            .map(|&index| char::from(self.letters[index]))
            .collect()
    }

    pub fn found(&self) -> &[(String, u32)] {
        &self.found
    }

    pub fn answer_count(&self) -> usize {
        self.answers.len()
    }

    pub fn score(&self) -> u32 {
        self.found.iter().map(|(_, points)| points).sum()
    }

    pub fn finished(&self) -> bool {
        self.finished || self.found.len() == self.answer_count()
    }

    /// Familiar answers for hints and post-game review. The full dictionary is valid in play.
    pub fn familiar_remaining(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.answers.iter().copied().filter(|answer| {
            FAMILIAR.contains(answer)
                && !self
                    .found
                    .iter()
                    .any(|(found, _)| found.eq_ignore_ascii_case(answer))
        })
    }

    pub fn hint_word(&self) -> Option<&'static str> {
        let preferred_length = if self.mode == Mode::Rack { 7 } else { 5 };
        self.familiar_remaining()
            .min_by_key(|word| (word.len().abs_diff(preferred_length), *word))
    }

    pub fn finish(&mut self) {
        self.finished = true;
        self.clear();
    }

    pub fn can_choose(&self, index: usize) -> bool {
        !self.finished()
            && index < self.letters.len()
            && (self.selection.last() == Some(&index)
                || (!self.selection.contains(&index)
                    && self.can_follow(self.selection.last().copied(), index)))
    }

    pub fn choose(&mut self, index: usize) -> bool {
        if !self.can_choose(index) {
            return false;
        }
        if self.selection.last() == Some(&index) {
            self.undo();
        } else {
            self.selection.push(index);
        }
        true
    }

    /// Preserve the current path when possible. Backtrack when identical tiles made
    /// an earlier key ambiguous, so a playable spelling always works by keyboard.
    pub fn type_letter(&mut self, letter: char) -> bool {
        if self.finished() || !letter.is_ascii_alphabetic() {
            return false;
        }
        let uppercase = letter.to_ascii_uppercase() as u8;
        let next = self.letters.iter().enumerate().position(|(index, &tile)| {
            tile == uppercase
                && !self.selection.contains(&index)
                && self.can_follow(self.selection.last().copied(), index)
        });
        if let Some(index) = next {
            self.selection.push(index);
            return true;
        }
        let mut input = self.candidate();
        input.push(letter);
        if let Some(path) = self.path_for(&input) {
            self.selection = path;
            return true;
        }
        false
    }

    pub fn path_for(&self, word: &str) -> Option<Vec<usize>> {
        if word.is_empty() || word.len() > self.letters.len() || !word.is_ascii() {
            return None;
        }
        let mut path = Vec::with_capacity(word.len());
        self.find_path(word.as_bytes(), 0, &mut path)
            .then_some(path)
    }

    pub fn undo(&mut self) {
        self.selection.pop();
    }

    pub fn clear(&mut self) {
        self.selection.clear();
    }

    pub fn submit(&mut self) -> Result<u32, Rejection> {
        if self.finished() {
            return Err(Rejection::Finished);
        }
        let candidate = self.candidate();
        if candidate.len() < 3 {
            return Err(Rejection::TooShort);
        }
        if !self
            .answers
            .contains(candidate.to_ascii_lowercase().as_str())
        {
            return Err(Rejection::UnknownWord);
        }
        if self.found.iter().any(|(word, _)| word == &candidate) {
            return Err(Rejection::AlreadyFound);
        }
        let awarded = points(candidate.len());
        self.found.push((candidate, awarded));
        self.clear();
        Ok(awarded)
    }

    fn can_follow(&self, previous: Option<usize>, next: usize) -> bool {
        self.mode == Mode::Rack
            || previous.is_none_or(|previous| {
                (previous / SIDE).abs_diff(next / SIDE) <= 1
                    && (previous % SIDE).abs_diff(next % SIDE) <= 1
            })
    }

    fn find_path(&self, word: &[u8], used: u16, path: &mut Vec<usize>) -> bool {
        let Some((&letter, remaining)) = word.split_first() else {
            return true;
        };
        for (index, tile) in self.letters.iter().enumerate() {
            let flag = 1 << index;
            if used & flag != 0
                || !tile.eq_ignore_ascii_case(&letter)
                || !self.can_follow(path.last().copied(), index)
            {
                continue;
            }
            path.push(index);
            if self.find_path(remaining, used | flag, path) {
                return true;
            }
            path.pop();
        }
        false
    }

    fn search(
        &self,
        used: u16,
        previous: Option<usize>,
        candidate: &mut String,
        answers: &mut BTreeSet<&'static str>,
    ) {
        for (index, letter) in self.letters.iter().enumerate() {
            let flag = 1 << index;
            if used & flag != 0 || !self.can_follow(previous, index) {
                continue;
            }
            candidate.push(char::from(letter.to_ascii_lowercase()));
            let lower_bound = WORDS.partition_point(|word| *word < candidate.as_str());
            if let Some(&word) = WORDS
                .get(lower_bound)
                .filter(|word| word.starts_with(candidate.as_str()))
            {
                if word == candidate && candidate.len() >= 3 {
                    answers.insert(word);
                }
                self.search(used | flag, Some(index), candidate, answers);
            }
            candidate.pop();
        }
    }
}

fn points(length: usize) -> u32 {
    match length {
        3 | 4 => 1,
        5 => 2,
        6 => 3,
        7 => 5,
        _ => 11,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grove_checks_neighbors_and_does_not_reuse_tiles() {
        let mut round = WordRound::new(Mode::Grove, 0);
        assert!(round.choose(0));
        assert!(!round.choose(15));
        assert!(round.choose(5));
        assert!(!round.choose(0));
        assert!(round.choose(5));
        assert_eq!(round.selection(), &[0]);
        round.clear();
        for index in [0, 1, 2, 3] {
            assert!(round.choose(index));
        }
        assert_eq!(round.candidate(), "CRAT");
        assert_eq!(round.submit(), Err(Rejection::UnknownWord));
    }

    #[test]
    fn grove_keyboard_can_revisit_an_ambiguous_tile_choice() {
        let mut round = WordRound::new(Mode::Grove, 0);
        for letter in "ELM".chars() {
            assert!(round.type_letter(letter), "could not add {letter}");
        }
        assert_eq!(round.selection(), &[13, 8, 12]);
        assert_eq!(round.submit(), Ok(1));
    }

    #[test]
    fn rack_scores_words_and_can_rehydrate_mid_word() {
        let mut round = WordRound::new(Mode::Rack, 0);
        assert_eq!(round.submit(), Err(Rejection::TooShort));
        for letter in "gardens".chars() {
            assert!(round.type_letter(letter));
        }
        assert_eq!(round.submit(), Ok(5));
        for letter in "GARDENS".chars() {
            assert!(round.type_letter(letter));
        }
        assert_eq!(round.submit(), Err(Rejection::AlreadyFound));
        let mut restored = WordRound::new(Mode::Rack, 0);
        restored.restore(
            vec!["gardens".into(), "GARDENS".into(), "plum".into()],
            round.selection(),
            false,
        );
        assert_eq!(restored.found(), round.found());
        assert_eq!(restored.candidate(), "GARDENS");
        restored.finish();
        assert!(restored.finished());
        assert!(!restored.choose(2));
        assert_eq!(restored.submit(), Err(Rejection::Finished));
        assert!(!restored.familiar_remaining().any(|word| word == "gardens"));
        assert_eq!(restored.score(), 5);
    }

    #[test]
    fn each_letter_puzzle_has_a_distinct_pool_with_room_to_exceed_the_goal() {
        assert!(WORDS.windows(2).all(|words| words[0] < words[1]));
        assert!(
            FAMILIAR
                .iter()
                .all(|word| WORDS.binary_search(word).is_ok())
        );
        for mode in [Mode::Grove, Mode::Rack] {
            assert!(mode.puzzle_count() >= 60);
            let mut signatures = BTreeSet::new();
            for index in 0..mode.puzzle_count() {
                let round = WordRound::collection(mode, index);
                let mut letters = round.letters().to_vec();
                assert_eq!(letters.len(), if mode == Mode::Grove { 16 } else { 7 });
                letters.sort_unstable();
                assert!(signatures.insert(letters), "{mode:?} {index}");
                let available: u32 = round
                    .familiar_remaining()
                    .map(|word| points(word.len()))
                    .sum();
                assert!(available >= WORD_BONUS, "{mode:?} {index}: {available}");
                if mode == Mode::Rack || index >= 8 {
                    assert!(
                        round.familiar_remaining().any(|word| word.len() >= 7),
                        "{mode:?} {index} is missing a long answer"
                    );
                }
                let hint = round.hint_word().unwrap();
                let path = round.path_for(hint).unwrap();
                assert_eq!(path.len(), hint.len());
                if (index as u64) < mode.legacy_pool_size() {
                    assert_eq!(
                        round.letters(),
                        WordRound::new(mode, index as u64).letters()
                    );
                }
            }
        }
    }
}
