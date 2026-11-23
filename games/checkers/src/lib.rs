//! English draughts (American checkers) on the 32 dark squares of an 8×8 board.
//!
//! Draws are automatic on the third occurrence of a position or after 40 quiet turns per side.

mod computer;

pub use computer::Difficulty;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Square(u8);

impl Square {
    pub const fn from_index(index: u8) -> Option<Self> {
        if index < 32 { Some(Self(index)) } else { None }
    }

    pub const fn from_coords(row: u8, column: u8) -> Option<Self> {
        if row < 8 && column < 8 && (row + column) % 2 == 1 {
            Some(Self(row * 4 + column / 2))
        } else {
            None
        }
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn row(self) -> u8 {
        self.0 / 4
    }

    pub const fn column(self) -> u8 {
        (self.0 % 4) * 2 + (1 - self.row() % 2)
    }

    fn offset(self, row: i8, column: i8) -> Option<Self> {
        Self::from_coords(
            self.row().checked_add_signed(row)?,
            self.column().checked_add_signed(column)?,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Red,
    Light,
}

impl Side {
    pub const fn other(self) -> Self {
        match self {
            Self::Red => Self::Light,
            Self::Light => Self::Red,
        }
    }

    const fn forward(self) -> i8 {
        match self {
            Self::Red => -1,
            Self::Light => 1,
        }
    }

    const fn king_row(self) -> u8 {
        match self {
            Self::Red => 0,
            Self::Light => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub side: Side,
    pub king: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub from: Square,
    pub to: Square,
}

impl Step {
    pub fn captured(self) -> Option<Square> {
        if self.from.row().abs_diff(self.to.row()) != 2
            || self.from.column().abs_diff(self.to.column()) != 2
        {
            return None;
        }
        Square::from_coords(
            u8::midpoint(self.from.row(), self.to.row()),
            u8::midpoint(self.from.column(), self.to.column()),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepEffect {
    pub captured: Option<Square>,
    pub crowned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draw {
    Repetition,
    NoProgress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Playing,
    Won(Side),
    Draw(Draw),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Position {
    board: [Option<Piece>; 32],
    side: Side,
    // Count completed, non-capturing king turns. 80 is 40 per player.
    quiet_turns: u8,
}

impl Position {
    fn opening() -> Self {
        let board = std::array::from_fn(|i| match i {
            0..12 => Some(Piece {
                side: Side::Light,
                king: false,
            }),
            20..32 => Some(Piece {
                side: Side::Red,
                king: false,
            }),
            _ => None,
        });
        Self {
            board,
            side: Side::Red,
            quiet_turns: 0,
        }
    }

    fn piece(&self, square: Square) -> Option<Piece> {
        self.board[square.0 as usize]
    }

    fn side_pieces(&self) -> impl Iterator<Item = Square> + '_ {
        (0..32).map(Square).filter(|&square| {
            self.piece(square)
                .is_some_and(|piece| piece.side == self.side)
        })
    }

    fn piece_steps(&self, from: Square, jump: bool) -> impl Iterator<Item = Step> + '_ {
        [(-1, -1), (-1, 1), (1, -1), (1, 1)]
            .into_iter()
            .filter_map(move |(row, column)| {
                let piece = self.piece(from)?;
                if !piece.king && row != piece.side.forward() {
                    return None;
                }
                let neighbor = from.offset(row, column)?;
                let to = if jump {
                    let enemy = self.piece(neighbor)?;
                    if enemy.side == piece.side {
                        return None;
                    }
                    neighbor.offset(row, column)?
                } else {
                    neighbor
                };
                self.piece(to).is_none().then_some(Step { from, to })
            })
    }

    fn legal_steps(&self, continuation: Option<Square>) -> Vec<Step> {
        if let Some(from) = continuation {
            return self.piece_steps(from, true).collect();
        }
        let jumps: Vec<_> = self
            .side_pieces()
            .flat_map(|from| self.piece_steps(from, true))
            .collect();
        if jumps.is_empty() {
            return self
                .side_pieces()
                .flat_map(|from| self.piece_steps(from, false))
                .collect();
        }
        jumps
    }

    fn has_move(&self) -> bool {
        self.side_pieces().any(|from| {
            self.piece_steps(from, false).next().is_some()
                || self.piece_steps(from, true).next().is_some()
        })
    }

    /// Update one validated step, keeping the side to move on a compulsory continuation.
    fn advance(&mut self, step: Step) -> (StepEffect, Option<Square>) {
        let mut piece = self.board[step.from.0 as usize].take().unwrap();
        let captured = step.captured();
        if let Some(square) = captured {
            // Jump landings keep the same row parity, so a captured square can never become
            // a landing later in this chain. Removing it now also prevents a repeat capture.
            self.board[square.0 as usize] = None;
        }

        self.quiet_turns = if piece.king && captured.is_none() {
            self.quiet_turns + 1
        } else {
            0
        };
        let crowned = !piece.king && step.to.row() == piece.side.king_row();
        piece.king |= crowned;
        self.board[step.to.0 as usize] = Some(piece);

        let effect = StepEffect { captured, crowned };
        if captured.is_some() && !crowned && self.piece_steps(step.to, true).next().is_some() {
            return (effect, Some(step.to));
        }
        self.side = self.side.other();
        (effect, None)
    }

    /// Call only at a turn boundary. Irreversible moves reset the repetition window.
    fn outcome<'a>(&self, earlier: impl Iterator<Item = &'a Self>) -> Outcome {
        if !self.has_move() {
            return Outcome::Won(self.side.other());
        }
        if earlier
            .take(self.quiet_turns as usize)
            .filter(|p| p.side == self.side && p.board == self.board)
            .nth(1)
            .is_some()
        {
            return Outcome::Draw(Draw::Repetition);
        }
        if self.quiet_turns >= 80 {
            return Outcome::Draw(Draw::NoProgress);
        }
        Outcome::Playing
    }
}

#[derive(Clone, Copy)]
struct Boundary {
    position: Position,
    step_count: usize,
}

/// A replayable match. A turn can contain one slide or an entire chain of jumps.
#[derive(Clone)]
pub struct Game {
    position: Position,
    continuation: Option<Square>,
    outcome: Outcome,
    steps: Vec<Step>,
    boundaries: Vec<Boundary>,
}

impl Default for Game {
    fn default() -> Self {
        Self::from_position(Position::opening())
    }
}

impl Game {
    fn from_position(position: Position) -> Self {
        Self {
            position,
            continuation: None,
            outcome: position.outcome(std::iter::empty()),
            steps: Vec::new(),
            boundaries: vec![Boundary {
                position,
                step_count: 0,
            }],
        }
    }

    pub fn piece(&self, square: Square) -> Option<Piece> {
        self.position.piece(square)
    }

    pub fn material(&self, side: Side) -> (usize, usize) {
        self.position
            .board
            .iter()
            .flatten()
            .filter(|piece| piece.side == side)
            .fold((0, 0), |(pieces, kings), piece| {
                (pieces + 1, kings + usize::from(piece.king))
            })
    }

    pub fn side(&self) -> Side {
        self.position.side
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    pub fn continuation(&self) -> Option<Square> {
        self.continuation
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// The jumps so far, or the latest complete turn when between moves.
    pub fn last_steps(&self) -> &[Step] {
        if self.continuation.is_some() {
            &self.steps[self.boundaries.last().unwrap().step_count..]
        } else {
            self.boundaries
                .iter()
                .rev()
                .nth(1)
                .map_or(&[], |boundary| &self.steps[boundary.step_count..])
        }
    }

    pub fn legal_steps(&self) -> Vec<Step> {
        if self.outcome == Outcome::Playing {
            self.position.legal_steps(self.continuation)
        } else {
            Vec::new()
        }
    }

    /// Returns `None` for an illegal action and leaves the match unchanged.
    pub fn play(&mut self, step: Step) -> Option<StepEffect> {
        if !self.legal_steps().contains(&step) {
            return None;
        }
        let (effect, continuation) = self.position.advance(step);
        self.continuation = continuation;
        self.steps.push(step);
        if continuation.is_none() {
            self.outcome = self
                .position
                .outcome(self.boundaries.iter().rev().map(|b| &b.position));
            self.boundaries.push(Boundary {
                position: self.position,
                step_count: self.steps.len(),
            });
        }
        Some(effect)
    }

    /// Rewind a partial jump chain or the last completed turn as one action.
    pub fn undo_turn(&mut self) -> bool {
        if self.steps.is_empty() {
            return false;
        }
        if self.continuation.is_none() {
            self.boundaries.pop();
        }
        let boundary = self.boundaries.last().unwrap();
        self.position = boundary.position;
        self.steps.truncate(boundary.step_count);
        self.continuation = None;
        self.outcome = Outcome::Playing;
        true
    }
}

#[cfg(test)]
mod tests;
