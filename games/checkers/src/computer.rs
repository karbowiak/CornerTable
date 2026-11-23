use super::{Game, Outcome, Position, Square, Step};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    #[default]
    Standard,
    Expert,
}

impl Game {
    /// Choose a complete legal turn with a deterministic search and a fixed work budget.
    pub fn computer_turn(&self, difficulty: Difficulty) -> Vec<Step> {
        if self.outcome != Outcome::Playing {
            return Vec::new();
        }
        let mut turns = turns(&self.position, self.legal_steps());
        if turns.len() <= 1 {
            return turns.pop().map_or_else(Vec::new, |turn| turn.steps);
        }

        let (max_depth, nodes, extensions) = match difficulty {
            Difficulty::Easy => (1, 2000, 0),
            Difficulty::Standard => (4, 6000, 2),
            Difficulty::Expert => (8, 40_000, 4),
        };
        let start = self.boundaries.len().saturating_sub(81);
        let mut search = Search {
            remaining: nodes,
            extensions,
            history: self.boundaries[start..]
                .iter()
                .map(|boundary| boundary.position)
                .collect(),
        };

        // Keep the deepest completed iteration if the next one exhausts its budget.
        for depth in 1..=max_depth {
            let Some(best) = search.best(&turns, depth) else {
                break;
            };
            turns.swap(0, best);
        }
        turns.swap_remove(0).steps
    }
}

struct Turn {
    steps: Vec<Step>,
    position: Position,
    crowned: bool,
}

fn turns(position: &Position, first_steps: Vec<Step>) -> Vec<Turn> {
    let mut turns = Vec::new();
    let mut path = Vec::new();
    for step in first_steps {
        extend(*position, step, &mut path, &mut turns);
    }
    // Inspect longer jumps and promotions first to find cutoffs sooner.
    turns.sort_by_key(|turn| (std::cmp::Reverse(turn.steps.len()), !turn.crowned));
    turns
}

fn extend(mut position: Position, step: Step, path: &mut Vec<Step>, turns: &mut Vec<Turn>) {
    let (effect, continuation) = position.advance(step);
    path.push(step);
    if let Some(from) = continuation {
        for next in position.piece_steps(from, true) {
            extend(position, next, path, turns);
        }
    } else {
        turns.push(Turn {
            steps: path.clone(),
            position,
            crowned: effect.crowned,
        });
    }
    path.pop();
}

const MATE: i32 = 20_000;
const INFINITY: i32 = MATE + 1;

struct Search {
    remaining: usize,
    extensions: u8,
    history: Vec<Position>,
}

impl Search {
    fn best(&mut self, turns: &[Turn], depth: i8) -> Option<usize> {
        let mut alpha = -INFINITY;
        let mut best = 0;
        for (index, turn) in turns.iter().enumerate() {
            let value = -self.score(turn.position, depth - 1, 1, -INFINITY, -alpha)?;
            if value > alpha {
                best = index;
                alpha = value;
            }
        }
        Some(best)
    }

    fn score(
        &mut self,
        position: Position,
        depth: i8,
        ply: i32,
        mut alpha: i32,
        beta: i32,
    ) -> Option<i32> {
        self.remaining = self.remaining.checked_sub(1)?;
        match position.outcome(self.history.iter().rev()) {
            Outcome::Won(_) => return Some(-MATE + ply),
            Outcome::Draw(_) => return Some(0),
            Outcome::Playing => (),
        }
        if depth <= -(self.extensions as i8) {
            return Some(evaluate(&position));
        }
        let steps = position.legal_steps(None);
        if depth <= 0 && steps[0].captured().is_none() {
            return Some(evaluate(&position));
        }

        self.history.push(position);
        let mut best = -INFINITY;
        for turn in turns(&position, steps) {
            let Some(reply) = self.score(turn.position, depth - 1, ply + 1, -beta, -alpha) else {
                self.history.pop();
                return None;
            };
            let score = -reply;
            best = best.max(score);
            alpha = alpha.max(score);
            if alpha >= beta {
                break;
            }
        }
        self.history.pop();
        Some(best)
    }
}

fn evaluate(position: &Position) -> i32 {
    (0..32)
        .map(Square)
        .filter_map(|square| position.piece(square).map(|piece| (square, piece)))
        .map(|(square, piece)| {
            let row = square.row();
            let column = square.column();
            let centered = (2..=5).contains(&row) && (2..=5).contains(&column);
            let advance = row.abs_diff(piece.side.other().king_row());
            let value = if piece.king {
                178 + i32::from(centered) * 10
            } else {
                100 + i32::from(advance) * 3 + i32::from(centered) * 7
            };
            if piece.side == position.side {
                value
            } else {
                -value
            }
        })
        .sum()
}
