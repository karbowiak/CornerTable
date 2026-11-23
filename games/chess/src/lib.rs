//! Standard chess with a replayable move journal and a bounded offline opponent.

mod computer;
mod notation;

pub use computer::{ComputerTurn, choose_turn};
pub use cozy_chess::{Color, File, Move, Piece, Rank, Square};

use cozy_chess::{BitBoard, Board};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawClaim {
    ThreefoldRepetition,
    FiftyMoveRule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    ClaimDraw { intended: Option<Move> },
    AgreeDraw,
    Resign,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    Checkmate { winner: Color },
    Resignation { winner: Color },
    ResignationDraw,
    Stalemate,
    InsufficientMaterial,
    FivefoldRepetition,
    SeventyFiveMoveRule,
    ClaimedDraw(DrawClaim),
    AgreedDraw,
}

#[derive(Clone, Debug)]
pub struct Played {
    /// Castles use the king's destination, as in standard UCI notation.
    pub movement: Move,
    pub mover: Color,
    pub captured: Option<Piece>,
    pub san: String,
}

#[derive(Clone, Default)]
pub struct Game {
    position: Position,
    history: Vec<Ply>,
    ending: Option<Ending>,
    decision: Option<Decision>,
}

#[derive(Clone)]
struct Ply {
    before: Position,
    played: Played,
}

#[derive(Clone, Default)]
struct Position {
    board: Board,
    // cozy-chess caps its own clock at 100. A claim is optional, and the
    // automatic draw only happens at 150, so track the real value here.
    halfmoves: u8,
}

impl Game {
    pub fn side_to_move(&self) -> Color {
        self.position.board.side_to_move()
    }

    pub fn piece(&self, square: Square) -> Option<(Color, Piece)> {
        let board = &self.position.board;
        Some((board.color_on(square)?, board.piece_on(square)?))
    }

    pub fn checked_king(&self) -> Option<Square> {
        let board = &self.position.board;
        (!board.checkers().is_empty()).then(|| board.king(board.side_to_move()))
    }

    pub fn ending(&self) -> Option<Ending> {
        self.ending
    }

    pub fn decision(&self) -> Option<Decision> {
        self.decision
    }

    pub fn moves(&self) -> impl ExactSizeIterator<Item = &Played> + DoubleEndedIterator {
        self.history.iter().map(|ply| &ply.played)
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        if self.ending.is_some() {
            return Vec::new();
        }
        let board = &self.position.board;
        raw_moves(board)
            .into_iter()
            .map(|movement| standard_move(board, movement))
            .collect()
    }

    pub fn play(&mut self, movement: Move) -> Option<&Played> {
        let raw = self.legal_raw_move(movement)?;
        let board = &self.position.board;
        let mover = board.side_to_move();
        let captured = captured_piece(board, raw);
        let next = self.position.after(raw);
        let san = notation::san(board, &next.board, raw, movement, captured.is_some());
        let before = std::mem::replace(&mut self.position, next);
        self.history.push(Ply {
            before,
            played: Played {
                movement,
                mover,
                captured,
                san,
            },
        });
        self.ending = self.automatic_ending();
        self.history.last().map(|ply| &ply.played)
    }

    /// A valid claim can use the current position or an announced move.
    /// Announced moves do not enter the journal or change the board.
    pub fn claim(&self, intended: Option<Move>) -> Option<DrawClaim> {
        if self.ending.is_some() {
            return None;
        }
        match intended {
            Some(movement) => {
                let raw = self.legal_raw_move(movement)?;
                self.claim_after(raw)
            }
            None => claim_on(&self.position, self.previous_boards()),
        }
    }

    pub fn claim_moves(&self) -> Vec<Move> {
        if self.ending.is_some() || self.position.halfmoves < 7 {
            return Vec::new();
        }
        let board = &self.position.board;
        raw_moves(board)
            .into_iter()
            .filter(|&movement| self.claim_after(movement).is_some())
            .map(|movement| standard_move(board, movement))
            .collect()
    }

    pub fn decide(&mut self, decision: Decision) -> bool {
        if self.ending.is_some() {
            return false;
        }
        let ending = match decision {
            Decision::ClaimDraw { intended } => {
                let Some(basis) = self.claim(intended) else {
                    return false;
                };
                Ending::ClaimedDraw(basis)
            }
            Decision::AgreeDraw if self.history.len() >= 2 => Ending::AgreedDraw,
            Decision::AgreeDraw => return false,
            Decision::Resign => {
                let winner = !self.side_to_move();
                if self.position.board.colors(winner).len() == 1 {
                    Ending::ResignationDraw
                } else {
                    Ending::Resignation { winner }
                }
            }
        };
        self.decision = Some(decision);
        self.ending = Some(ending);
        true
    }

    /// Undo one action in hot-seat, or return to the human's last decision in solo play.
    pub fn undo(&mut self, return_to: Option<Color>) -> bool {
        let decision = self.decision.take();
        if decision.is_some() {
            self.ending = None;
            if return_to.is_none() || return_to == Some(self.side_to_move()) {
                return true;
            }
        }
        let Some(last) = self.history.pop() else {
            return decision.is_some();
        };
        self.position = last.before;
        self.ending = None;
        while return_to.is_some_and(|color| color != self.side_to_move()) {
            let Some(last) = self.history.pop() else {
                break;
            };
            self.position = last.before;
        }
        true
    }

    fn legal_raw_move(&self, movement: Move) -> Option<Move> {
        let board = &self.position.board;
        let raw = raw_move(board, movement);
        // Reject king-to-own-rook notation at our public API boundary.
        (self.ending.is_none() && board.is_legal(raw) && standard_move(board, raw) == movement)
            .then_some(raw)
    }

    fn claim_after(&self, movement: Move) -> Option<DrawClaim> {
        let next = self.position.after(movement);
        claim_on(
            &next,
            std::iter::once(&self.position.board).chain(self.previous_boards()),
        )
    }

    fn previous_boards(&self) -> impl DoubleEndedIterator<Item = &Board> {
        self.history.iter().rev().map(|ply| &ply.before.board)
    }

    fn automatic_ending(&self) -> Option<Ending> {
        let position = &self.position;
        let board = &position.board;
        if !board.generate_moves(|_| true) {
            return Some(if board.checkers().is_empty() {
                Ending::Stalemate
            } else {
                Ending::Checkmate {
                    winner: !board.side_to_move(),
                }
            });
        }
        if insufficient_material(board) {
            return Some(Ending::InsufficientMaterial);
        }
        if position.halfmoves >= 150 {
            return Some(Ending::SeventyFiveMoveRule);
        }
        repeated(position, self.previous_boards(), 4).then_some(Ending::FivefoldRepetition)
    }
}

impl Position {
    fn after(&self, raw: Move) -> Self {
        let mut board = self.board.clone();
        board.play_unchecked(raw);
        let halfmoves = if board.halfmove_clock() == 0 {
            0
        } else {
            self.halfmoves + 1
        };
        Self { board, halfmoves }
    }
}

fn raw_moves(board: &Board) -> Vec<Move> {
    let mut result = Vec::with_capacity(36);
    board.generate_moves(|moves| {
        result.extend(moves);
        false
    });
    result
}

fn raw_move(board: &Board, movement: Move) -> Move {
    let home_rank = Rank::First.relative_to(board.side_to_move());
    if movement.from == Square::new(File::E, home_rank)
        && board.piece_on(movement.from) == Some(Piece::King)
        && movement.to.rank() == home_rank
    {
        let file = match movement.to.file() {
            File::C => File::A,
            File::G => File::H,
            file => file,
        };
        return Move {
            to: Square::new(file, home_rank),
            ..movement
        };
    }
    movement
}

fn standard_move(board: &Board, movement: Move) -> Move {
    if board.colors(board.side_to_move()).has(movement.to) {
        let file = if movement.to.file() == File::H {
            File::G
        } else {
            File::C
        };
        Move {
            to: Square::new(file, movement.to.rank()),
            ..movement
        }
    } else {
        movement
    }
}

fn captured_piece(board: &Board, raw: Move) -> Option<Piece> {
    if board.colors(!board.side_to_move()).has(raw.to) {
        return board.piece_on(raw.to);
    }
    (board.piece_on(raw.from) == Some(Piece::Pawn) && raw.from.file() != raw.to.file())
        .then_some(Piece::Pawn)
}

fn insufficient_material(board: &Board) -> bool {
    let knights = board.pieces(Piece::Knight);
    let bishops = board.pieces(Piece::Bishop);
    let minors = knights | bishops;
    if board.occupied() != (board.pieces(Piece::King) | minors) {
        return false;
    }
    minors.len() <= 1
        || (knights.is_empty()
            && (bishops.is_subset(BitBoard::LIGHT_SQUARES)
                || bishops.is_subset(BitBoard::DARK_SQUARES)))
}

fn claim_on<'a>(
    position: &Position,
    history: impl Iterator<Item = &'a Board>,
) -> Option<DrawClaim> {
    if position.halfmoves >= 100 {
        Some(DrawClaim::FiftyMoveRule)
    } else if repeated(position, history, 2) {
        Some(DrawClaim::ThreefoldRepetition)
    } else {
        None
    }
}

fn repeated<'a>(
    position: &Position,
    history: impl Iterator<Item = &'a Board>,
    prior_occurrences: usize,
) -> bool {
    // Pawns and captures cannot be undone by later moves. Only the positions
    // since the most recent such move, with the same side to play, can count.
    history
        .take(usize::from(position.halfmoves))
        .skip(1)
        .step_by(2)
        .filter(|previous| position.board.same_position(previous))
        .nth(prior_occurrences - 1)
        .is_some()
}

#[cfg(test)]
mod tests;
