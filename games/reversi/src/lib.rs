//! Rules for an 8×8 Reversi match, independent of the desktop and storage.

mod computer;

pub use computer::{Difficulty, choose_move};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disc {
    Black,
    White,
}

impl Disc {
    pub const fn opposite(self) -> Self {
        match self {
            Self::Black => Self::White,
            Self::White => Self::Black,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Black => "Black",
            Self::White => "White",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Square(u8);

impl Square {
    pub const fn new(row: u8, column: u8) -> Option<Self> {
        if row < 8 && column < 8 {
            Some(Self(row * 8 + column))
        } else {
            None
        }
    }

    pub const fn from_index(index: u8) -> Option<Self> {
        if index < 64 { Some(Self(index)) } else { None }
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn row(self) -> u8 {
        self.0 / 8
    }

    pub const fn column(self) -> u8 {
        self.0 % 8
    }

    const fn bit(self) -> u64 {
        1 << self.0
    }
}

impl std::fmt::Display for Square {
    fn fmt(&self, format: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            format,
            "{}{}",
            char::from(b'A' + self.column()),
            self.row() + 1
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Squares(u64);

impl Squares {
    pub const fn contains(self, square: Square) -> bool {
        self.0 & square.bit() != 0
    }

    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl Iterator for Squares {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        let square = Square::from_index(self.0.trailing_zeros() as u8)?;
        self.0 &= self.0 - 1;
        Some(square)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.0.count_ones() as usize;
        (count, Some(count))
    }
}

impl ExactSizeIterator for Squares {}
impl std::iter::FusedIterator for Squares {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Won(Disc),
    Draw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    Play(Disc),
    Finished(Outcome),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Played {
    pub square: Square,
    pub disc: Disc,
    pub flipped: Squares,
    /// The opponent had no move and the current player goes again.
    pub passed: Option<Disc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    black: u64,
    white: u64,
    turn: Turn,
    legal: Squares,
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

impl Position {
    pub fn new() -> Self {
        let black = (1 << 28) | (1 << 35);
        let white = (1 << 27) | (1 << 36);
        Self {
            black,
            white,
            turn: Turn::Play(Disc::Black),
            legal: Squares(legal_moves(black, white)),
        }
    }

    pub const fn turn(self) -> Turn {
        self.turn
    }

    pub const fn legal_moves(self) -> Squares {
        self.legal
    }

    pub const fn score(self, disc: Disc) -> u32 {
        self.bits(disc).count_ones()
    }

    pub const fn empty_count(self) -> u32 {
        (self.black | self.white).count_zeros()
    }

    pub fn disc(self, square: Square) -> Option<Disc> {
        [Disc::Black, Disc::White]
            .into_iter()
            .find(|&disc| self.bits(disc) & square.bit() != 0)
    }

    pub fn flips(self, square: Square) -> Squares {
        if let Turn::Play(disc) = self.turn
            && self.legal.contains(square)
        {
            return Squares(flips(
                self.bits(disc),
                self.bits(disc.opposite()),
                square.bit(),
            ));
        }
        Squares::default()
    }

    /// Place a disc and resolve the opponent's forced pass or the end of the match.
    /// Illegal placements leave the board intact.
    pub fn play(&mut self, square: Square) -> Option<Played> {
        let Turn::Play(disc) = self.turn else {
            return None;
        };
        let flipped = self.flips(square);
        if flipped.is_empty() {
            return None;
        }

        let opponent = disc.opposite();
        let (mine, theirs) = place(
            self.bits(disc),
            self.bits(opponent),
            square.bit(),
            flipped.0,
        );
        (self.black, self.white) = match disc {
            Disc::Black => (mine, theirs),
            Disc::White => (theirs, mine),
        };
        self.legal = Squares(legal_moves(theirs, mine));
        self.turn = Turn::Play(opponent);
        let mut passed = None;
        if self.legal.is_empty() {
            self.legal = Squares(legal_moves(mine, theirs));
            if self.legal.is_empty() {
                let outcome = match self.black.count_ones().cmp(&self.white.count_ones()) {
                    std::cmp::Ordering::Greater => Outcome::Won(Disc::Black),
                    std::cmp::Ordering::Less => Outcome::Won(Disc::White),
                    std::cmp::Ordering::Equal => Outcome::Draw,
                };
                self.turn = Turn::Finished(outcome);
            } else {
                self.turn = Turn::Play(disc);
                passed = Some(opponent);
            }
        }

        Some(Played {
            square,
            disc,
            flipped,
            passed,
        })
    }

    const fn bits(self, disc: Disc) -> u64 {
        match disc {
            Disc::Black => self.black,
            Disc::White => self.white,
        }
    }
}

const A_FILE: u64 = 0x0101_0101_0101_0101;
const H_FILE: u64 = 0x8080_8080_8080_8080;

#[inline]
fn shift(bits: u64, direction: u8) -> u64 {
    match direction {
        0 => (bits & !A_FILE) >> 9,
        1 => bits >> 8,
        2 => (bits & !H_FILE) >> 7,
        3 => (bits & !A_FILE) >> 1,
        4 => (bits & !H_FILE) << 1,
        5 => (bits & !A_FILE) << 7,
        6 => bits << 8,
        _ => (bits & !H_FILE) << 9,
    }
}

fn legal_moves(mine: u64, theirs: u64) -> u64 {
    let empty = !(mine | theirs);
    let mut legal = 0;
    for direction in 0..8 {
        // An outflanked run has at most six discs between its endpoints.
        let mut run = shift(mine, direction) & theirs;
        for _ in 0..5 {
            run |= shift(run, direction) & theirs;
        }
        legal |= shift(run, direction) & empty;
    }
    legal
}

fn flips(mine: u64, theirs: u64, bit: u64) -> u64 {
    let mut flipped = 0;
    for direction in 0..8 {
        let mut cursor = shift(bit, direction);
        let mut run = 0;
        while cursor & theirs != 0 {
            run |= cursor;
            cursor = shift(cursor, direction);
        }
        if cursor & mine != 0 {
            flipped |= run;
        }
    }
    flipped
}

const fn place(mine: u64, theirs: u64, bit: u64, flipped: u64) -> (u64, u64) {
    (mine | bit | flipped, theirs & !flipped)
}
