#[path = "crossword_puzzles.rs"]
mod puzzles;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridSize {
    Mini,
    Large,
}

impl GridSize {
    pub fn side(self) -> usize {
        match self {
            Self::Mini => 3,
            Self::Large => 4,
        }
    }

    fn catalog(self) -> &'static [CrosswordPuzzle] {
        match self {
            Self::Mini => puzzles::MINIS,
            Self::Large => puzzles::LARGER,
        }
    }

    pub fn puzzle_count(self) -> usize {
        self.catalog().len()
    }

    pub fn clue_number(self, line: usize, down: bool) -> usize {
        match (down, line) {
            (true, _) => line + 1,
            (false, 0) => 1,
            (false, _) => self.side() + line,
        }
    }

    pub fn square_number(self, square: usize) -> Option<usize> {
        let (row, column) = (square / self.side(), square % self.side());
        if row == 0 {
            Some(self.clue_number(column, true))
        } else if column == 0 {
            Some(self.clue_number(row, false))
        } else {
            None
        }
    }

    pub fn line_at(self, square: usize, down: bool) -> usize {
        if down {
            square % self.side()
        } else {
            square / self.side()
        }
    }

    pub fn line(self, line: usize, down: bool) -> impl Iterator<Item = usize> {
        (0..self.side()).map(move |index| {
            if down {
                index * self.side() + line
            } else {
                line * self.side() + index
            }
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Clue {
    pub text: &'static str,
    pub(crate) answer: &'static str,
    #[cfg(test)]
    pub(crate) alternates: &'static [&'static str],
}

#[derive(Clone, Copy, Debug)]
pub struct CrosswordPuzzle {
    pub across: &'static [Clue],
    pub down: &'static [Clue],
}

impl CrosswordPuzzle {
    pub fn clues(self, down: bool) -> &'static [Clue] {
        if down { self.down } else { self.across }
    }

    fn answer(self, square: usize) -> u8 {
        let side = self.across.len();
        self.across[square / side].answer.as_bytes()[square % side]
    }
}

pub struct Crossword {
    size: GridSize,
    puzzle: &'static CrosswordPuzzle,
    letters: Vec<Option<u8>>,
    hinted: u16,
    revealed: bool,
}

impl Crossword {
    pub fn new(seed: u64) -> Self {
        Self::with_size(GridSize::Mini, seed)
    }

    pub fn with_size(size: GridSize, seed: u64) -> Self {
        // Existing dates and saved challenge seeds use the original rotation.
        let size_of_rotation = match size {
            GridSize::Mini => 27,
            GridSize::Large => 10,
        };
        Self::collection(size, (seed % size_of_rotation) as usize)
    }

    pub fn collection(size: GridSize, index: usize) -> Self {
        Self {
            size,
            puzzle: &size.catalog()[index],
            letters: vec![None; size.side() * size.side()],
            hinted: 0,
            revealed: false,
        }
    }

    pub fn restore(&mut self, letters: &[Option<u8>], hinted: u16, revealed: bool) {
        if letters.len() != self.letters.len() {
            return;
        }
        self.revealed = revealed;
        self.hinted = 0;
        for (square, &letter) in letters.iter().enumerate() {
            if hinted & (1 << square) != 0 {
                self.hinted |= 1 << square;
            }
            if revealed || self.hinted_at(square) {
                self.letters[square] = Some(self.puzzle.answer(square));
            } else {
                self.letters[square] = letter
                    .filter(u8::is_ascii_alphabetic)
                    .map(|letter| letter.to_ascii_uppercase());
            }
        }
    }

    pub fn size(&self) -> GridSize {
        self.size
    }

    pub fn puzzle(&self) -> &CrosswordPuzzle {
        self.puzzle
    }

    pub fn letters(&self) -> &[Option<u8>] {
        &self.letters
    }

    pub fn put(&mut self, index: usize, letter: Option<char>) -> bool {
        if self.complete()
            || index >= self.letters.len()
            || self.hinted_at(index)
            || letter.is_some_and(|letter| !letter.is_ascii_alphabetic())
        {
            return false;
        }
        self.letters[index] = letter.map(|letter| letter.to_ascii_uppercase() as u8);
        true
    }

    pub fn incorrect_squares(&self) -> u16 {
        self.letters
            .iter()
            .enumerate()
            .filter(|&(index, entered)| {
                entered.is_some_and(|letter| letter != self.puzzle.answer(index))
            })
            .fold(0, |mask, (index, _)| mask | (1 << index))
    }

    pub fn complete(&self) -> bool {
        self.letters
            .iter()
            .enumerate()
            .all(|(index, &entered)| entered == Some(self.puzzle.answer(index)))
    }

    pub fn hint(&mut self, index: usize) -> bool {
        if self.complete() || self.hinted_at(index) || index >= self.letters.len() {
            return false;
        }
        let answer = self.puzzle.answer(index);
        if self.letters[index] == Some(answer) {
            return false;
        }
        self.letters[index] = Some(answer);
        self.hinted |= 1 << index;
        true
    }

    pub fn hinted_at(&self, index: usize) -> bool {
        index < self.letters.len() && self.hinted & (1 << index) != 0
    }

    pub fn hinted_squares(&self) -> u16 {
        self.hinted
    }

    pub fn reveal(&mut self) {
        if !self.complete() {
            for (index, letter) in self.letters.iter_mut().enumerate() {
                *letter = Some(self.puzzle.answer(index));
            }
            self.revealed = true;
        }
    }

    pub fn revealed(&self) -> bool {
        self.revealed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WORDS;
    use std::collections::BTreeSet;

    fn candidates(clue: &Clue) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(clue.answer).chain(clue.alternates.iter().copied())
    }

    fn count_fills(puzzle: &CrosswordPuzzle, rows: &mut Vec<&str>) -> usize {
        if rows.len() == puzzle.across.len() {
            return usize::from(puzzle.down.iter().enumerate().all(|(column, clue)| {
                candidates(clue).any(|word| {
                    rows.iter()
                        .map(|row| row.as_bytes()[column])
                        .eq(word.bytes())
                })
            }));
        }
        candidates(&puzzle.across[rows.len()])
            .map(|answer| {
                rows.push(answer);
                let count = count_fills(puzzle, rows);
                rows.pop();
                count
            })
            .sum()
    }

    #[test]
    fn every_grid_has_distinct_dictionary_entries_and_one_fill_for_its_clues() {
        assert!(GridSize::Mini.puzzle_count() >= 20);
        assert!(GridSize::Large.puzzle_count() >= 8);
        for size in [GridSize::Mini, GridSize::Large] {
            let side = size.side();
            let mut boards = BTreeSet::new();
            for (index, puzzle) in size.catalog().iter().enumerate() {
                assert_eq!(puzzle.across.len(), side);
                assert_eq!(puzzle.down.len(), side);
                let mut words = BTreeSet::new();
                for clue in puzzle.across.iter().chain(puzzle.down) {
                    assert!(!clue.text.is_empty());
                    assert!(
                        words.insert(clue.answer),
                        "repeated entry in {size:?} {index}"
                    );
                    for answer in candidates(clue) {
                        assert_eq!(answer.len(), side, "{answer}");
                        assert!(answer.bytes().all(|letter| letter.is_ascii_uppercase()));
                        let lower = answer.to_ascii_lowercase();
                        assert!(WORDS.binary_search(&lower.as_str()).is_ok(), "{answer}");
                    }
                }
                assert!(boards.insert(words), "repeated grid in {size:?} {index}");
                assert_eq!(count_fills(puzzle, &mut Vec::new()), 1, "{size:?} {index}");
                for (column, clue) in puzzle.down.iter().enumerate() {
                    assert!(
                        size.line(column, true)
                            .map(|square| puzzle.answer(square))
                            .eq(clue.answer.bytes())
                    );
                }
            }
        }
    }

    #[test]
    fn checks_hints_and_completion_survive_a_round_trip() {
        let mut crossword = Crossword::new(0);
        assert!(!crossword.put(0, Some('9')));
        assert!(crossword.put(0, Some('x')));
        assert_eq!(crossword.incorrect_squares(), 1);
        assert!(crossword.hint(0));
        assert_eq!(crossword.incorrect_squares(), 0);
        assert!(!crossword.put(0, Some('z')));
        assert!(crossword.put(1, Some('a')));

        let mut restored = Crossword::new(0);
        restored.restore(
            crossword.letters(),
            crossword.hinted_squares(),
            crossword.revealed(),
        );
        assert!(restored.hinted_at(0));
        assert_eq!(restored.letters(), crossword.letters());
        for (index, letter) in "catoreden".chars().enumerate().skip(2) {
            assert!(restored.put(index, Some(letter)));
        }
        assert!(restored.complete());
        assert!(!restored.put(0, None));

        let mut revealed = Crossword::with_size(GridSize::Large, 0);
        revealed.reveal();
        assert!(revealed.complete());
        assert!(revealed.revealed());
        let mut restored = Crossword::with_size(GridSize::Large, 0);
        restored.restore(revealed.letters(), 0, true);
        assert!(restored.complete());
        assert!(restored.revealed());
        assert!(!restored.hint(1));
    }
}
