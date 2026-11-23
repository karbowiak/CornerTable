//! Rules for the 33-hole English cross, starting with an empty center.

use std::fmt;

pub const SIDE: u8 = 7;
pub const PEGS_AT_START: u32 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hole(u8);

impl Hole {
    pub const CENTER: Self = Self(24);
    const DIRECTIONS: [(i8, i8); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];

    pub const fn new(row: u8, column: u8) -> Option<Self> {
        let middle_rows = row >= 2 && row <= 4;
        let middle_columns = column >= 2 && column <= 4;
        if row < SIDE && column < SIDE && (middle_rows || middle_columns) {
            Some(Self(row * SIDE + column))
        } else {
            None
        }
    }

    pub fn from_index(index: u8) -> Option<Self> {
        Self::new(index / SIDE, index % SIDE)
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn row(self) -> u8 {
        self.0 / SIDE
    }

    pub const fn column(self) -> u8 {
        self.0 % SIDE
    }

    pub fn offset(self, rows: i8, columns: i8) -> Option<Self> {
        Self::new(
            self.row().checked_add_signed(rows)?,
            self.column().checked_add_signed(columns)?,
        )
    }

    pub fn all() -> impl Iterator<Item = Self> {
        (0..SIDE * SIDE).filter_map(Self::from_index)
    }

    const fn mask(self) -> u64 {
        1 << self.0
    }
}

impl fmt::Display for Hole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", char::from(b'A' + self.column()), self.row() + 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Jump {
    pub from: Hole,
    pub to: Hole,
}

impl Jump {
    fn over(self) -> Option<Hole> {
        let distance =
            self.from.row().abs_diff(self.to.row()) + self.from.column().abs_diff(self.to.column());
        if distance != 2
            || (self.from.row() != self.to.row() && self.from.column() != self.to.column())
        {
            return None;
        }
        Hole::new(
            (self.from.row() + self.to.row()) / 2,
            (self.from.column() + self.to.column()) / 2,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Playing,
    Won { center: bool },
    Stuck,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position(u64);

impl Default for Position {
    fn default() -> Self {
        Self(Hole::all().fold(0, |mask, hole| mask | hole.mask()) ^ Hole::CENTER.mask())
    }
}

impl Position {
    pub const fn occupied(self, hole: Hole) -> bool {
        self.0 & hole.mask() != 0
    }

    pub const fn peg_count(self) -> u32 {
        self.0.count_ones()
    }

    fn landing_mask(self, jump: Jump) -> Option<u64> {
        let over = jump.over()?;
        (self.occupied(jump.from) && self.occupied(over) && !self.occupied(jump.to))
            .then(|| jump.from.mask() | over.mask() | jump.to.mask())
    }

    pub fn jumps_from(self, from: Hole) -> impl Iterator<Item = Jump> {
        Hole::DIRECTIONS.into_iter().filter_map(move |(dr, dc)| {
            let to = from.offset(2 * dr, 2 * dc)?;
            let jump = Jump { from, to };
            self.landing_mask(jump)?;
            Some(jump)
        })
    }

    pub fn jumps(self) -> impl Iterator<Item = Jump> {
        Hole::all().flat_map(move |from| self.jumps_from(from))
    }

    pub fn status(self) -> Status {
        if self.peg_count() == 1 {
            Status::Won {
                center: self.occupied(Hole::CENTER),
            }
        } else if self.jumps().next().is_some() {
            Status::Playing
        } else {
            Status::Stuck
        }
    }
}

#[derive(Default)]
pub struct Round {
    position: Position,
    history: Vec<Jump>,
}

impl Round {
    pub fn restore(history: &[[u8; 2]]) -> Option<Self> {
        if history.len() >= PEGS_AT_START as usize {
            return None;
        }
        let mut round = Self::default();
        for &[from, to] in history {
            round.play(Jump {
                from: Hole::from_index(from)?,
                to: Hole::from_index(to)?,
            })?;
        }
        Some(round)
    }

    pub fn position(&self) -> Position {
        self.position
    }

    pub fn history(&self) -> &[Jump] {
        &self.history
    }

    pub fn play(&mut self, jump: Jump) -> Option<()> {
        self.position.0 ^= self.position.landing_mask(jump)?;
        self.history.push(jump);
        Some(())
    }

    pub fn undo(&mut self) -> Option<Jump> {
        let jump = self.history.pop()?;
        let over = jump.over().expect("history contains only valid jumps");
        self.position.0 ^= jump.from.mask() | over.mask() | jump.to.mask();
        Some(jump)
    }
}

#[cfg(test)]
mod tests;
