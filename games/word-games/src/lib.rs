//! Offline rules for Letter Grove, Letter Rack, and Mini Crossword.

use std::collections::BTreeSet;
use std::sync::LazyLock;

const ENGLISH: &str = include_str!("english.txt");
const SIDE: usize = 4;
const GROVES: [&[u8; 16]; 8] = [
    b"CRATSOPELINDMEGS",
    b"BASTEROMNIPEDCLR",
    b"SHORETIPNACELDUS",
    b"PLANTERCOISDMEAT",
    b"SLATEONIPERDSHUM",
    b"FROSTALENIPEMDUS",
    b"GRAINETOSLAPCREW",
    b"STONELIAPURNMEDS",
];
const RACKS: [&[u8; 7]; 8] = [
    b"GARDENS", b"FLOWERS", b"PAINTER", b"PLASTER", b"STAPLER", b"CANDLES", b"THREADS", b"MARBLES",
];

static WORDS: LazyLock<Vec<&'static str>> = LazyLock::new(|| ENGLISH.lines().collect());

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Grove,
    Rack,
    Crossword,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejection {
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
    answers: BTreeSet<String>,
    revealed: bool,
}

impl WordRound {
    pub fn new(mode: Mode, seed: u64) -> Self {
        assert!(matches!(mode, Mode::Grove | Mode::Rack));
        let letters = match mode {
            Mode::Grove => {
                let board = GROVES[(seed % GROVES.len() as u64) as usize];
                let symmetry = seed / GROVES.len() as u64 % 8;
                (0..SIDE * SIDE)
                    .map(|index| {
                        let (mut row, mut column) = (index / SIDE, index % SIDE);
                        if symmetry & 1 != 0 {
                            std::mem::swap(&mut row, &mut column);
                        }
                        if symmetry & 2 != 0 {
                            row = SIDE - 1 - row;
                        }
                        if symmetry & 4 != 0 {
                            column = SIDE - 1 - column;
                        }
                        board[row * SIDE + column]
                    })
                    .collect()
            }
            Mode::Rack => {
                let rack = RACKS[(seed % RACKS.len() as u64) as usize];
                let rotation = (seed / RACKS.len() as u64 % 7) as usize;
                (0..7).map(|index| rack[(index + rotation) % 7]).collect()
            }
            Mode::Crossword => unreachable!(),
        };
        let mut round = Self {
            mode,
            letters,
            selection: Vec::new(),
            found: Vec::new(),
            answers: BTreeSet::new(),
            revealed: false,
        };
        let mut candidate = String::with_capacity(round.letters.len());
        let mut answers = BTreeSet::new();
        round.search(0, None, &mut candidate, &mut answers);
        round.answers = answers;
        round
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

    pub fn revealed(&self) -> bool {
        self.revealed
    }

    pub fn remaining_words(&self) -> impl Iterator<Item = &str> {
        self.answers.iter().map(String::as_str).filter(|answer| {
            !self
                .found
                .iter()
                .any(|(found, _)| found.eq_ignore_ascii_case(answer))
        })
    }

    pub fn reveal(&mut self) {
        self.revealed = true;
        self.clear();
    }

    pub fn choose(&mut self, index: usize) -> bool {
        if self.revealed || index >= self.letters.len() {
            return false;
        }
        if self.selection.last() == Some(&index) {
            self.undo();
            return true;
        }
        if self.selection.contains(&index)
            || !self.can_follow(self.selection.last().copied(), index)
        {
            return false;
        }
        self.selection.push(index);
        true
    }

    pub fn undo(&mut self) {
        self.selection.pop();
    }

    pub fn clear(&mut self) {
        self.selection.clear();
    }

    pub fn submit(&mut self) -> Result<u32, Rejection> {
        let candidate = self.candidate();
        if candidate.len() < 3 {
            return Err(Rejection::TooShort);
        }
        if !self.answers.contains(&candidate.to_ascii_lowercase()) {
            return Err(Rejection::UnknownWord);
        }
        if self.found.iter().any(|(word, _)| word == &candidate) {
            return Err(Rejection::AlreadyFound);
        }
        let points = match candidate.len() {
            3 | 4 => 1,
            5 => 2,
            6 => 3,
            7 => 5,
            _ => 11,
        };
        self.found.push((candidate, points));
        self.clear();
        Ok(points)
    }

    fn can_follow(&self, previous: Option<usize>, next: usize) -> bool {
        self.mode == Mode::Rack
            || previous.is_none_or(|previous| {
                (previous / SIDE).abs_diff(next / SIDE) <= 1
                    && (previous % SIDE).abs_diff(next % SIDE) <= 1
            })
    }

    fn search(
        &self,
        used: u16,
        previous: Option<usize>,
        candidate: &mut String,
        answers: &mut BTreeSet<String>,
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
                if word == candidate {
                    answers.insert(candidate.clone());
                }
                self.search(used | flag, Some(index), candidate, answers);
            }
            candidate.pop();
        }
    }
}

#[derive(Clone, Copy)]
pub struct CrosswordPuzzle {
    solution: &'static [u8; 9],
    pub across: [&'static str; 3],
    pub down: [&'static str; 3],
}

const CROSSWORDS: [CrosswordPuzzle; 3] = [
    CrosswordPuzzle {
        solution: b"CATOREDEN",
        across: [
            "Common household feline",
            "Rock containing valuable minerals",
            "A fox's home",
        ],
        down: [
            "Fish often served with chips",
            "Plural form of ‘is’",
            "Number of fingers and thumbs combined",
        ],
    },
    CrosswordPuzzle {
        solution: b"TAPOREWET",
        across: ["Faucet", "Iron or copper source", "Soaked by the rain"],
        down: [
            "Pull a trailer",
            "You ___ here",
            "Animal that lives with you",
        ],
    },
    CrosswordPuzzle {
        solution: b"BAYICETEA",
        across: [
            "A sheltered part of the coast",
            "Frozen water",
            "Drink steeped in a pot",
        ],
        down: [
            "A tiny piece",
            "Playing card with one suit symbol",
            "An affirmative vote",
        ],
    },
];

pub const ACROSS_NUMBERS: [usize; 3] = [1, 4, 5];
pub const DOWN_NUMBERS: [usize; 3] = [1, 2, 3];

pub struct Crossword {
    puzzle: &'static CrosswordPuzzle,
    letters: [Option<u8>; 9],
    revealed: bool,
}

impl Crossword {
    pub fn new(seed: u64) -> Self {
        Self {
            puzzle: &CROSSWORDS[(seed % CROSSWORDS.len() as u64) as usize],
            letters: [None; 9],
            revealed: false,
        }
    }

    pub fn puzzle(&self) -> &CrosswordPuzzle {
        self.puzzle
    }

    pub fn letters(&self) -> &[Option<u8>; 9] {
        &self.letters
    }

    pub fn put(&mut self, index: usize, letter: Option<char>) -> bool {
        if self.revealed || index >= self.letters.len() {
            return false;
        }
        if let Some(letter) = letter {
            if !letter.is_ascii_alphabetic() {
                return false;
            }
            self.letters[index] = Some(letter.to_ascii_uppercase() as u8);
        } else {
            self.letters[index] = None;
        }
        true
    }

    pub fn incorrect_count(&self) -> usize {
        self.letters
            .iter()
            .zip(self.puzzle.solution)
            .filter(|(entered, correct)| entered.is_some_and(|letter| letter != **correct))
            .count()
    }

    pub fn complete(&self) -> bool {
        self.letters
            .iter()
            .zip(self.puzzle.solution)
            .all(|(entered, &correct)| *entered == Some(correct))
    }

    pub fn reveal(&mut self) {
        self.letters = self.puzzle.solution.map(Some);
        self.revealed = true;
    }

    pub fn revealed(&self) -> bool {
        self.revealed
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
    fn rack_scores_valid_words_only_once_and_stops_after_reveal() {
        let mut round = WordRound::new(Mode::Rack, 0);
        assert_eq!(round.submit(), Err(Rejection::TooShort));
        for index in 0..7 {
            assert!(round.choose(index));
        }
        assert_eq!(round.candidate(), "GARDENS");
        assert_eq!(round.submit(), Ok(5));
        for index in 0..7 {
            assert!(round.choose(index));
        }
        assert_eq!(round.submit(), Err(Rejection::AlreadyFound));
        round.reveal();
        assert!(round.revealed());
        assert!(!round.choose(2));
        assert!(round.selection().is_empty());
        assert!(!round.remaining_words().any(|word| word == "gardens"));
        assert_eq!(round.score(), 5);
    }

    #[test]
    fn all_seeded_word_sets_offer_many_answers() {
        for mode in [Mode::Grove, Mode::Rack] {
            let counts: Vec<_> = (0..8)
                .map(|seed| WordRound::new(mode, seed).answer_count())
                .collect();
            assert!(counts.iter().all(|&count| count >= 20), "{counts:?}");
            for (seed, count) in counts.into_iter().enumerate() {
                assert_eq!(WordRound::new(mode, seed as u64 + 8).answer_count(), count);
            }
        }
    }

    #[test]
    fn crossword_solutions_are_english_in_both_directions() {
        for puzzle in CROSSWORDS {
            for line in 0..3 {
                let across = &puzzle.solution[(line * 3)..(line * 3 + 3)];
                let down = [
                    puzzle.solution[line],
                    puzzle.solution[line + 3],
                    puzzle.solution[line + 6],
                ];
                for word in [across, &down] {
                    let word = String::from_utf8_lossy(word).to_ascii_lowercase();
                    assert!(WORDS.binary_search(&word.as_str()).is_ok(), "{word}");
                }
            }
        }
    }

    #[test]
    fn crossword_accepts_case_insensitive_entries_and_reports_mistakes() {
        let mut crossword = Crossword::new(0);
        assert!(!crossword.complete());
        assert!(!crossword.put(0, Some('9')));
        assert!(crossword.put(0, Some('x')));
        assert_eq!(crossword.incorrect_count(), 1);
        assert!(crossword.put(0, None));
        for (index, letter) in "catoreden".chars().enumerate() {
            assert!(crossword.put(index, Some(letter)));
        }
        assert!(crossword.complete());
        assert_eq!(crossword.incorrect_count(), 0);
        let mut revealed = Crossword::new(1);
        revealed.reveal();
        assert!(revealed.complete());
        assert!(!revealed.put(0, Some('a')));
    }
}
