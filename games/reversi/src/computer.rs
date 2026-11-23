use super::{Position, Square, Squares, Turn, flips, legal_moves, place, shift};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Casual,
    Strategist,
}

const CORNERS: u64 = 0x8100_0000_0000_0081;
const EDGES: u64 = 0xff81_8181_8181_81ff;
const CORNER_NEIGHBORS: [(u64, u64); 4] = [
    (1, 0x0000_0000_0000_0302),
    (1 << 7, 0x0000_0000_0000_c040),
    (1 << 56, 0x0203_0000_0000_0000),
    (1 << 63, 0x40c0_0000_0000_0000),
];
const WIN: i32 = 10_000;
const LIMIT: i32 = WIN * 2;
const NODE_BUDGET: u32 = 100_000;

/// Select a legal move offline. Strategist should be called on a background executor.
pub fn choose_move(position: Position, difficulty: Difficulty) -> Option<Square> {
    let Turn::Play(disc) = position.turn() else {
        return None;
    };
    let mut legal = position.legal_moves();
    if legal.count() == 1 {
        return legal.next();
    }
    let mine = position.bits(disc);
    let theirs = position.bits(disc.opposite());
    match difficulty {
        Difficulty::Casual => legal.max_by_key(|square| {
            let bit = square.bit();
            let gain = flips(mine, theirs, bit).count_ones();
            let corner = if bit & CORNERS != 0 { 16 } else { 0 };
            (corner + gain, std::cmp::Reverse(*square))
        }),
        Difficulty::Strategist => deeper_move(mine, theirs, legal),
    }
}

fn deeper_move(mine: u64, theirs: u64, legal: Squares) -> Option<Square> {
    let empty = (mine | theirs).count_zeros();
    let max_depth = if empty <= 10 { empty } else { 7 };
    let mut best = ordered(legal.0).next()?;
    let mut search = Search {
        remaining: NODE_BUDGET,
    };
    // Keep the last completed iteration if a particularly broad position uses the budget.
    for depth in 1..=max_depth {
        let mut choice = best;
        let mut score = -LIMIT;
        let moves = std::iter::once(best).chain(ordered(legal.0 & !best.bit()));
        for square in moves {
            let bit = square.bit();
            let (mine, theirs) = place(mine, theirs, bit, flips(mine, theirs, bit));
            let Some(reply) = search.evaluate(theirs, mine, depth - 1, -LIMIT, -score) else {
                return Some(best);
            };
            if -reply > score {
                score = -reply;
                choice = square;
            }
        }
        best = choice;
    }
    Some(best)
}

struct Search {
    remaining: u32,
}

impl Search {
    fn evaluate(
        &mut self,
        mine: u64,
        theirs: u64,
        depth: u32,
        mut alpha: i32,
        beta: i32,
    ) -> Option<i32> {
        self.remaining = self.remaining.checked_sub(1)?;
        let legal = legal_moves(mine, theirs);
        if legal == 0 {
            if legal_moves(theirs, mine) == 0 {
                let margin = count(mine) - count(theirs);
                return Some(margin.signum() * WIN + margin);
            }
            return self
                .evaluate(theirs, mine, depth, -beta, -alpha)
                .map(|score| -score);
        }
        if depth == 0 {
            return Some(heuristic(mine, theirs, legal));
        }

        let mut best = -LIMIT;
        for square in ordered(legal) {
            let bit = square.bit();
            let (mine, theirs) = place(mine, theirs, bit, flips(mine, theirs, bit));
            let score = -self.evaluate(theirs, mine, depth - 1, -beta, -alpha)?;
            best = best.max(score);
            alpha = alpha.max(best);
            if alpha >= beta {
                break;
            }
        }
        Some(best)
    }
}

fn ordered(legal: u64) -> impl Iterator<Item = Square> {
    Squares(legal & CORNERS)
        .chain(Squares(legal & EDGES & !CORNERS))
        .chain(Squares(legal & !EDGES))
}

fn heuristic(mine: u64, theirs: u64, legal: u64) -> i32 {
    let empty = !(mine | theirs);
    let corners = difference(mine, theirs, CORNERS);
    let edge = difference(mine, theirs, EDGES & !CORNERS);
    let mobility = count(legal) - count(legal_moves(theirs, mine));
    let frontier = (0..8).fold(0, |bits, direction| bits | shift(empty, direction));
    let exposed = difference(mine, theirs, frontier);
    let unsafe_neighbors = CORNER_NEIGHBORS
        .into_iter()
        .filter(|&(corner, _)| corner & empty != 0)
        .fold(0, |mask, (_, neighbors)| mask | neighbors);
    let corner_risk = difference(mine, theirs, unsafe_neighbors);
    let disc_weight = if empty.count_ones() <= 16 { 2 } else { -1 };

    60 * corners + 4 * edge + 11 * mobility - 4 * exposed - 15 * corner_risk
        + disc_weight * (count(mine) - count(theirs))
}

fn difference(mine: u64, theirs: u64, mask: u64) -> i32 {
    count(mine & mask) - count(theirs & mask)
}

fn count(bits: u64) -> i32 {
    bits.count_ones() as i32
}
