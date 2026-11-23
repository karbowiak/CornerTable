//! Sudoku generation, editing, and validation independent of the desktop UI.

pub const SIZE: usize = 9;
pub const CELLS: usize = SIZE * SIZE;
const DIGITS: u16 = 0b11_1111_1110;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    pub const ALL: [Self; 3] = [Self::Easy, Self::Medium, Self::Hard];

    pub fn label(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Medium => "Medium",
            Self::Hard => "Hard",
        }
    }

    fn clues(self) -> usize {
        match self {
            Self::Easy => 44,
            Self::Medium => 36,
            Self::Hard => 28,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Board {
    values: [u8; CELLS],
    notes: [u16; CELLS],
}

pub struct Game {
    givens: [u8; CELLS],
    board: Board,
    undo: Vec<Board>,
}

impl Game {
    pub fn new(seed: u64, difficulty: Difficulty) -> Self {
        let mut random = Shuffle(seed);
        let mut digits = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        random.shuffle(&mut digits);
        let rows = random.axis();
        let columns = random.axis();
        let mut givens = std::array::from_fn(|index| {
            let row = rows[index / SIZE];
            let column = columns[index % SIZE];
            digits[(row * 3 + row / 3 + column) % SIZE]
        });

        let mut order: [usize; CELLS] = std::array::from_fn(|index| index);
        random.shuffle(&mut order);
        let mut remaining = CELLS;
        for index in order {
            let original = std::mem::take(&mut givens[index]);
            if count_solutions(&mut givens, 2) == 1 {
                remaining -= 1;
                if remaining == difficulty.clues() {
                    break;
                }
            } else {
                givens[index] = original;
            }
        }

        Self {
            givens,
            board: Board {
                values: givens,
                notes: [0; CELLS],
            },
            undo: Vec::new(),
        }
    }

    pub fn value(&self, index: usize) -> u8 {
        self.board.values[index]
    }

    pub fn is_given(&self, index: usize) -> bool {
        self.givens[index] != 0
    }

    pub fn notes(&self, index: usize) -> u16 {
        self.board.notes[index]
    }

    pub fn related(first: usize, second: usize) -> bool {
        let (row, column) = (first / SIZE, first % SIZE);
        let (other_row, other_column) = (second / SIZE, second % SIZE);
        row == other_row
            || column == other_column
            || (row / 3 == other_row / 3 && column / 3 == other_column / 3)
    }

    pub fn has_conflict(&self, index: usize) -> bool {
        let value = self.value(index);
        value != 0
            && self
                .board
                .values
                .iter()
                .enumerate()
                .any(|(other, &candidate)| {
                    other != index && candidate == value && Self::related(index, other)
                })
    }

    pub fn filled(&self) -> usize {
        self.board
            .values
            .iter()
            .filter(|&&value| value != 0)
            .count()
    }

    pub fn is_solved(&self) -> bool {
        self.filled() == CELLS && !(0..CELLS).any(|index| self.has_conflict(index))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(board) = self.undo.pop() {
            self.board = board;
        }
    }

    /// A zero clears both the value and notes. A note toggles only in an empty cell.
    pub fn enter(&mut self, index: usize, digit: u8, note: bool) {
        if index >= CELLS || digit > 9 || self.is_given(index) || self.is_solved() {
            return;
        }

        if digit != 0 && note {
            if self.value(index) == 0 {
                self.undo.push(self.board);
                self.board.notes[index] ^= 1 << digit;
            }
            return;
        }

        if self.value(index) == digit && self.notes(index) == 0 {
            return;
        }
        self.undo.push(self.board);
        self.board.values[index] = digit;
        self.board.notes[index] = 0;
        if digit != 0 && !self.has_conflict(index) {
            for other in 0..CELLS {
                if Self::related(index, other) {
                    self.board.notes[other] &= !(1 << digit);
                }
            }
        }
    }
}

struct Shuffle(u64);

impl Shuffle {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mixed = (self.0 ^ (self.0 >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    fn shuffle<T>(&mut self, slice: &mut [T]) {
        for end in (1..slice.len()).rev() {
            slice.swap(end, (self.next() % (end as u64 + 1)) as usize);
        }
    }

    fn axis(&mut self) -> [usize; SIZE] {
        let mut groups = [0, 1, 2];
        self.shuffle(&mut groups);
        let mut axis = [0; SIZE];
        for (index, group) in groups.into_iter().enumerate() {
            let mut members = [group * 3, group * 3 + 1, group * 3 + 2];
            self.shuffle(&mut members);
            axis[index * 3..index * 3 + 3].copy_from_slice(&members);
        }
        axis
    }
}

fn count_solutions(values: &mut [u8; CELLS], limit: usize) -> usize {
    let mut best = None;
    let mut candidates = DIGITS;
    for (index, &value) in values.iter().enumerate() {
        if value != 0 {
            continue;
        }
        let mut allowed = DIGITS;
        for (other, &digit) in values.iter().enumerate() {
            if digit != 0 && Game::related(index, other) {
                allowed &= !(1 << digit);
            }
        }
        if allowed == 0 {
            return 0;
        }
        if best.is_none() || allowed.count_ones() < candidates.count_ones() {
            best = Some(index);
            candidates = allowed;
        }
        if candidates.count_ones() == 1 {
            break;
        }
    }

    let Some(index) = best else {
        return 1;
    };
    let mut total = 0;
    while candidates != 0 && total < limit {
        let digit = candidates.trailing_zeros();
        candidates &= !(1 << digit);
        values[index] = digit as u8;
        total += count_solutions(values, limit - total);
    }
    values[index] = 0;
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_repeatable_unique_and_orders_difficulty_by_clue_count() {
        for seed in [0, 1, 173, u64::MAX] {
            let mut previous = CELLS;
            for difficulty in Difficulty::ALL {
                let mut game = Game::new(seed, difficulty);
                assert_eq!(game.givens, Game::new(seed, difficulty).givens);
                assert_eq!(count_solutions(&mut game.givens, 2), 1);
                assert!(game.filled() < previous);
                previous = game.filled();
            }
        }
    }

    #[test]
    fn givens_are_locked_and_notes_and_values_are_undoable() {
        let mut game = Game::new(0, Difficulty::Medium);
        let given = (0..CELLS).find(|&index| game.is_given(index)).unwrap();
        let digit = game.value(given);
        let empty = (0..CELLS)
            .find(|&index| !game.is_given(index) && Game::related(given, index))
            .unwrap();
        game.enter(given, 0, false);
        assert_eq!(game.value(given), digit);
        assert!(!game.can_undo());
        game.enter(empty, digit, true);
        assert_eq!(game.notes(empty), 1 << digit);
        game.enter(empty, digit, false);
        assert!(game.has_conflict(empty));
        assert!(game.has_conflict(given));
        assert_eq!(game.notes(empty), 0);
        game.undo();
        assert_eq!(game.notes(empty), 1 << digit);
        game.enter(empty, 0, false);
        assert_eq!(game.notes(empty), 0);
    }

    #[test]
    fn completing_the_board_unlocks_a_win_and_undo_reopens_play() {
        let mut game = Game::new(0, Difficulty::Easy);
        let mut solution = game.givens;
        for index in 0..CELLS {
            if solution[index] != 0 {
                continue;
            }
            let digit = (1..=9)
                .find(|&digit| {
                    if solution.iter().enumerate().any(|(other, &value)| {
                        value == digit && other != index && Game::related(index, other)
                    }) {
                        return false;
                    }
                    solution[index] = digit;
                    count_solutions(&mut solution, 1) == 1
                })
                .unwrap();
            solution[index] = digit;
            game.enter(index, digit, false);
        }
        assert!(game.is_solved());
        game.undo();
        assert!(!game.is_solved());
    }
}
