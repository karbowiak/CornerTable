//! Sudoku generation, editing, and validation independent of the desktop UI.

mod hints;

pub use hints::{Hint, HintReason};

pub const SIZE: usize = 9;
pub const CELLS: usize = SIZE * SIZE;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Format {
    Standard,
    Mini,
}

impl Format {
    pub const fn size(self) -> usize {
        match self {
            Self::Standard => SIZE,
            Self::Mini => 6,
        }
    }

    pub const fn cell_count(self) -> usize {
        self.size() * self.size()
    }

    pub const fn box_rows(self) -> usize {
        match self {
            Self::Standard => 3,
            Self::Mini => 2,
        }
    }

    pub const fn box_columns(self) -> usize {
        3
    }

    fn digit_mask(self) -> u16 {
        (1 << (self.size() + 1)) - 2
    }

    fn coordinates(self, index: usize) -> (usize, usize, usize) {
        let row = index / self.size();
        let column = index % self.size();
        let boxes_per_row = self.size() / self.box_columns();
        let region = row / self.box_rows() * boxes_per_row + column / self.box_columns();
        (row, column, region)
    }

    pub fn related(self, first: usize, second: usize) -> bool {
        let (row, column, region) = self.coordinates(first);
        let (other_row, other_column, other_region) = self.coordinates(second);
        row == other_row || column == other_column || region == other_region
    }

    fn units(self, index: usize) -> [Unit; 3] {
        let (row, column, region) = self.coordinates(index);
        [Unit::Row(row), Unit::Column(column), Unit::Box(region)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unit {
    Row(usize),
    Column(usize),
    Box(usize),
}

impl Unit {
    pub fn contains(self, format: Format, index: usize) -> bool {
        format.units(index).contains(&self)
    }

    fn cells(self, format: Format) -> impl Iterator<Item = usize> {
        let size = format.size();
        let columns = format.box_columns();
        let boxes_per_row = size / columns;
        (0..size).map(move |offset| match self {
            Self::Row(row) => row * size + offset,
            Self::Column(column) => offset * size + column,
            Self::Box(region) => {
                let row = region / boxes_per_row * format.box_rows() + offset / columns;
                let column = region % boxes_per_row * columns + offset % columns;
                row * size + column
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

    fn clues(self, format: Format) -> usize {
        match format {
            Format::Standard => match self {
                Self::Easy => 44,
                Self::Medium => 36,
                Self::Hard => 28,
            },
            Format::Mini => match self {
                Self::Easy => 21,
                Self::Medium => 17,
                Self::Hard => 13,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Board {
    values: [u8; CELLS],
    notes: [u16; CELLS],
}

pub struct Game {
    format: Format,
    givens: [u8; CELLS],
    solution: [u8; CELLS],
    board: Board,
    undo: Vec<Board>,
}

impl Game {
    pub fn new(seed: u64, difficulty: Difficulty) -> Self {
        Self::with_format(seed, difficulty, Format::Standard)
    }

    pub fn with_format(seed: u64, difficulty: Difficulty, format: Format) -> Self {
        let size = format.size();
        let cells = format.cell_count();
        let mut random = Shuffle(seed);
        let mut digits: [u8; SIZE] = std::array::from_fn(|index| index as u8 + 1);
        random.shuffle(&mut digits[..size]);
        let rows = random.axis(size, format.box_rows());
        let columns = random.axis(size, format.box_columns());
        let mut solution = [0; CELLS];
        for (index, value) in solution[..cells].iter_mut().enumerate() {
            let row = rows[index / size];
            let column = columns[index % size];
            *value = digits[(row * format.box_columns() + row / format.box_rows() + column) % size];
        }

        let mut givens = solution;
        let mut order: [usize; CELLS] = std::array::from_fn(|index| index);
        random.shuffle(&mut order[..cells]);
        let mut remaining = cells;
        for &index in &order[..cells] {
            let original = std::mem::take(&mut givens[index]);
            if count_solutions(format, &mut givens, 2) == 1 {
                remaining -= 1;
                if remaining == difficulty.clues(format) {
                    break;
                }
            } else {
                givens[index] = original;
            }
        }

        Self {
            format,
            givens,
            solution,
            board: Board {
                values: givens,
                notes: [0; CELLS],
            },
            undo: Vec::new(),
        }
    }

    /// The same puzzle for everyone on a given UTC day, keyed by days since the Unix epoch.
    pub fn daily(utc_day: u64) -> Self {
        Self::new(utc_day ^ 0x434f_524e_4552_0001, Difficulty::Medium)
    }

    pub fn format(&self) -> Format {
        self.format
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

    /// Whether two cells share a unit in the standard 9×9 format.
    pub fn related(first: usize, second: usize) -> bool {
        Format::Standard.related(first, second)
    }

    fn conflict_peer(&self, index: usize) -> Option<usize> {
        let value = self.value(index);
        if value == 0 {
            return None;
        }
        self.board.values[..self.format.cell_count()]
            .iter()
            .enumerate()
            .find_map(|(other, &candidate)| {
                (other != index && candidate == value && self.format.related(index, other))
                    .then_some(other)
            })
    }

    pub fn has_conflict(&self, index: usize) -> bool {
        self.conflict_peer(index).is_some()
    }

    pub fn filled(&self) -> usize {
        self.board.values[..self.format.cell_count()]
            .iter()
            .filter(|&&value| value != 0)
            .count()
    }

    pub fn is_solved(&self) -> bool {
        self.board.values == self.solution
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(board) = self.undo.pop() else {
            return false;
        };
        self.board = board;
        true
    }

    /// A zero clears both the value and notes. A note toggles only in an empty cell.
    /// Returns whether the board changed.
    pub fn enter(&mut self, index: usize, digit: u8, note: bool) -> bool {
        if index >= self.format.cell_count()
            || usize::from(digit) > self.format.size()
            || self.is_given(index)
            || self.is_solved()
        {
            return false;
        }

        if digit != 0 && note {
            if self.value(index) != 0 {
                return false;
            }
            self.undo.push(self.board);
            self.board.notes[index] ^= 1 << digit;
            return true;
        }

        if self.value(index) == digit && self.notes(index) == 0 {
            return false;
        }
        self.undo.push(self.board);
        self.board.values[index] = digit;
        self.board.notes[index] = 0;
        if digit != 0 && !self.has_conflict(index) {
            for (other, notes) in self.board.notes[..self.format.cell_count()]
                .iter_mut()
                .enumerate()
            {
                if self.format.related(index, other) {
                    *notes &= !(1 << digit);
                }
            }
        }
        true
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

    fn axis(&mut self, size: usize, group_size: usize) -> [usize; SIZE] {
        let mut groups: [usize; SIZE] = std::array::from_fn(|index| index);
        let group_count = size / group_size;
        self.shuffle(&mut groups[..group_count]);
        let mut axis = [0; SIZE];
        for (index, &group) in groups[..group_count].iter().enumerate() {
            let mut members: [usize; SIZE] =
                std::array::from_fn(|member| group * group_size + member);
            self.shuffle(&mut members[..group_size]);
            let start = index * group_size;
            axis[start..start + group_size].copy_from_slice(&members[..group_size]);
        }
        axis
    }
}

#[derive(Default)]
struct Occupancy {
    rows: [u16; SIZE],
    columns: [u16; SIZE],
    boxes: [u16; SIZE],
}

impl Occupancy {
    fn from_values(format: Format, values: &[u8; CELLS]) -> Self {
        let mut used = Self::default();
        for (index, &digit) in values[..format.cell_count()].iter().enumerate() {
            if digit != 0 {
                used.toggle(format, index, 1 << digit);
            }
        }
        used
    }

    fn allowed(&self, format: Format, index: usize) -> u16 {
        let (row, column, region) = format.coordinates(index);
        format.digit_mask() & !(self.rows[row] | self.columns[column] | self.boxes[region])
    }

    fn toggle(&mut self, format: Format, index: usize, digit: u16) {
        let (row, column, region) = format.coordinates(index);
        self.rows[row] ^= digit;
        self.columns[column] ^= digit;
        self.boxes[region] ^= digit;
    }
}

// Used only for conflict-free boards during generation.
fn count_solutions(format: Format, values: &mut [u8; CELLS], limit: usize) -> usize {
    let mut used = Occupancy::from_values(format, values);
    search(format, values, &mut used, limit)
}

fn search(format: Format, values: &mut [u8; CELLS], used: &mut Occupancy, limit: usize) -> usize {
    let mut best = None;
    let mut candidates = format.digit_mask();
    for (index, &value) in values[..format.cell_count()].iter().enumerate() {
        if value != 0 {
            continue;
        }
        let allowed = used.allowed(format, index);
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
        let bit = 1 << digit;
        candidates &= !bit;
        values[index] = digit as u8;
        used.toggle(format, index, bit);
        total += search(format, values, used, limit - total);
        used.toggle(format, index, bit);
    }
    values[index] = 0;
    total
}

#[cfg(test)]
mod tests;
