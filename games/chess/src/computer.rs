use std::sync::atomic::{AtomicBool, Ordering};

use cozy_chess::{Board, Color, Move, Piece};
use serde::{Deserialize, Serialize};

use super::{Game, Position, captured_piece, claim_on, insufficient_material, raw_moves, repeated};

const MATE: i32 = 30_000;
const BOUND: i32 = MATE + 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Difficulty {
    Easy,
    #[default]
    Medium,
    Hard,
    Expert,
}

impl Difficulty {
    fn search_budget(self) -> (usize, u8, u8) {
        match self {
            Self::Easy => (600, 1, 0),
            Self::Medium => (28_000, 4, 3),
            Self::Hard => (120_000, 5, 3),
            Self::Expert => (450_000, 7, 4),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputerTurn {
    Play(Move),
    ClaimDraw { intended: Option<Move> },
}

/// Search a fixed number of nodes and honor cancellation between nodes. Call
/// this on a background thread. Identical positions and histories play alike.
pub fn choose_turn(game: &Game, cancel: &AtomicBool) -> Option<ComputerTurn> {
    choose_turn_with_difficulty(game, Difficulty::Medium, cancel)
}

pub fn choose_turn_with_difficulty(
    game: &Game,
    difficulty: Difficulty,
    cancel: &AtomicBool,
) -> Option<ComputerTurn> {
    if game.ending().is_some() || cancel.load(Ordering::Relaxed) {
        return None;
    }
    let position = &game.position;
    let board = &position.board;
    let mut moves = raw_moves(board);
    order_moves(board, &mut moves);
    let mut best = *moves.first()?;
    let mut score = -BOUND;
    let (node_limit, max_depth, capture_depth) = difficulty.search_budget();
    let mut search = Search {
        remaining: node_limit,
        cancel,
        // Keep the newest board at the end for inexpensive push/pop in search.
        ancestors: game
            .previous_boards()
            .rev()
            .cloned()
            .chain(std::iter::once(board.clone()))
            .collect(),
    };
    for depth in 1..=max_depth {
        let mut alpha = -BOUND;
        let mut best_index = 0;
        for (index, &movement) in moves.iter().enumerate() {
            let child = position.after(movement);
            let Some(value) = search.score(&child, depth - 1, capture_depth, 1, -BOUND, -alpha)
            else {
                break;
            };
            let value = -value;
            if value > alpha {
                alpha = value;
                best_index = index;
            }
        }
        if search.remaining == 0 || cancel.load(Ordering::Relaxed) {
            break;
        }
        score = alpha;
        moves.swap(0, best_index);
        best = moves[0];
        if score >= MATE - i32::from(depth) {
            break;
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    if score <= 0 {
        if game.claim(None).is_some() {
            return Some(ComputerTurn::ClaimDraw { intended: None });
        }
        if let Some(intended) = game.claim_moves().into_iter().next() {
            return Some(ComputerTurn::ClaimDraw {
                intended: Some(intended),
            });
        }
    }
    Some(ComputerTurn::Play(super::standard_move(board, best)))
}

struct Search<'a> {
    remaining: usize,
    cancel: &'a AtomicBool,
    ancestors: Vec<Board>,
}

impl Search<'_> {
    fn score(
        &mut self,
        position: &Position,
        depth: u8,
        capture_depth: u8,
        ply: u8,
        mut alpha: i32,
        beta: i32,
    ) -> Option<i32> {
        if self.remaining == 0 || self.cancel.load(Ordering::Relaxed) {
            return None;
        }
        self.remaining -= 1;
        let board = &position.board;
        let in_check = !board.checkers().is_empty();
        let mut moves = raw_moves(board);
        if moves.is_empty() {
            return Some(if in_check { i32::from(ply) - MATE } else { 0 });
        }
        if position.halfmoves >= 150
            || insufficient_material(board)
            || repeated(position, self.ancestors.iter().rev(), 4)
        {
            return Some(0);
        }

        let mut best = if claim_on(position, self.ancestors.iter().rev()).is_some() {
            0
        } else {
            -BOUND
        };
        if depth == 0 {
            if capture_depth == 0 {
                return Some(best.max(evaluate(board)));
            }
            if !in_check {
                best = best.max(evaluate(board));
                moves.retain(|&movement| {
                    movement.promotion.is_some() || captured_piece(board, movement).is_some()
                });
            }
        }
        alpha = alpha.max(best);
        if alpha >= beta {
            return Some(best);
        }

        order_moves(board, &mut moves);
        self.ancestors.push(board.clone());
        let score = (|| {
            for movement in moves {
                let child = position.after(movement);
                let captures = if depth == 0 {
                    capture_depth - 1
                } else {
                    capture_depth
                };
                let value = -self.score(
                    &child,
                    depth.saturating_sub(1),
                    captures,
                    ply + 1,
                    -beta,
                    -alpha,
                )?;
                best = best.max(value);
                alpha = alpha.max(best);
                if alpha >= beta {
                    break;
                }
            }
            Some(best)
        })();
        self.ancestors.pop();
        score
    }
}

fn order_moves(board: &Board, moves: &mut [Move]) {
    moves.sort_by_cached_key(|&movement| {
        let attacker = board.piece_on(movement.from).unwrap();
        let capture =
            captured_piece(board, movement).map_or(0, |victim| 8 * worth(victim) - worth(attacker));
        let promotion = movement.promotion.map_or(0, |piece| 8 * worth(piece));
        let file = movement.to.file() as i32;
        let rank = movement.to.rank() as i32;
        let center = 14 - (2 * file - 7).abs() - (2 * rank - 7).abs();
        -(capture + promotion + center)
    });
}

fn evaluate(board: &Board) -> i32 {
    let endgame = board.pieces(Piece::Queen).is_empty() || board.occupied().len() < 13;
    let mut score = [0; 2];
    for color in [Color::White, Color::Black] {
        let total = &mut score[color as usize];
        for piece in Piece::ALL {
            let squares = board.colored_pieces(color, piece);
            *total += worth(piece) * squares.len() as i32;
            for square in squares {
                let file = square.file() as i32;
                let rank = square.rank().relative_to(color) as i32;
                let center_file = 7 - (2 * file - 7).abs();
                let center = center_file + 7 - (2 * rank - 7).abs();
                *total += match piece {
                    Piece::Pawn => rank * (if endgame { 11 } else { 6 }) + center_file * 2,
                    Piece::Knight => 5 * center - 25,
                    Piece::Bishop => 3 * center + 5 * i32::from(rank > 0),
                    Piece::Rook => center_file + 16 * i32::from(rank == 6),
                    Piece::Queen => center - 12 * i32::from(!endgame && rank > 2),
                    Piece::King if endgame => 4 * center,
                    Piece::King => {
                        24 * i32::from(rank == 0 && matches!(file, 1 | 2 | 6)) - rank * 16
                    }
                };
            }
        }
        if board.colored_pieces(color, Piece::Bishop).len() >= 2 {
            *total += 27;
        }
    }
    score[board.side_to_move() as usize] - score[(!board.side_to_move()) as usize]
}

const fn worth(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 320,
        Piece::Bishop => 335,
        Piece::Rook => 510,
        Piece::Queen => 940,
        Piece::King => 0,
    }
}
