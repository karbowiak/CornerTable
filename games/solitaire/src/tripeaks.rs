use super::*;

const LAYOUT_SIZE: usize = 28;
const CHAIN_LENGTH: usize = 7;

pub(super) fn deal(game: &mut Game, mut deck: Vec<Card>) {
    game.layout = deck.drain(..LAYOUT_SIZE).map(Some).collect();
    game.waste.push(deck.pop().unwrap());
    game.stock = deck;
}

pub(super) fn deal_solvable(game: &mut Game, mut deck: Vec<Card>, mut seed: u64) {
    let order = removal_order(next_random(&mut seed));
    let (chains, _) = order.as_chunks::<CHAIN_LENGTH>();
    let mut draws = Vec::new();
    game.layout = vec![None; LAYOUT_SIZE];

    // Each of the four chains uses eight distinct ranks including its starter.
    // The deck therefore has enough copies of every rank, regardless of the starts.
    for (chain, positions) in chains.iter().enumerate() {
        let start = (next_random(&mut seed) % 13) as i32;
        let step = if next_random(&mut seed) & 1 == 0 {
            1
        } else {
            -1
        };
        let starter = take_rank(&mut deck, (start + 1) as u8);
        if chain == 0 {
            game.waste.push(starter);
        } else {
            draws.push(starter);
        }
        for (offset, &index) in positions.iter().enumerate() {
            let rank = (start + step * (offset as i32 + 1)).rem_euclid(13) + 1;
            game.layout[index] = Some(take_rank(&mut deck, rank as u8));
        }
    }

    game.stock = deck;
    game.stock.extend(draws.into_iter().rev());
}

fn take_rank(deck: &mut Vec<Card>, rank: u8) -> Card {
    let index = deck.iter().position(|card| card.rank == rank).unwrap();
    deck.remove(index)
}

fn removal_order(seed: u64) -> [usize; LAYOUT_SIZE] {
    let mut positions: [usize; LAYOUT_SIZE] = std::array::from_fn(|index| index);
    shuffle(&mut positions, seed);
    let mut remaining = [true; LAYOUT_SIZE];
    std::array::from_fn(|_| {
        let index = *positions
            .iter()
            .find(|&&index| {
                remaining[index]
                    && supports(index)
                        .is_none_or(|[left, right]| !remaining[left] && !remaining[right])
            })
            .unwrap();
        remaining[index] = false;
        index
    })
}

fn supports(index: usize) -> Option<[usize; 2]> {
    let left = match index {
        0..3 => 3 + index * 2,
        3..9 => 9 + (index - 3) / 2 * 3 + (index - 3) % 2,
        9..18 => index + 9,
        _ => return None,
    };
    Some([left, left + 1])
}

pub(super) fn available(game: &Game, place: Place) -> Option<Card> {
    let Place::Layout(index) = place else {
        return None;
    };
    let card = game.layout.get(index).copied().flatten()?;
    supports(index)
        .is_none_or(|[left, right]| game.layout[left].is_none() && game.layout[right].is_none())
        .then_some(card)
}

pub(super) fn play(game: &mut Game, action: Move) -> bool {
    match action {
        Move::Draw => game.draw_waste(1, false),
        Move::Clear(place @ Place::Layout(index)) => {
            let (Some(card), Some(top)) = (available(game, place), game.waste.last()) else {
                return false;
            };
            if !matches!(card.rank.abs_diff(top.rank), 1 | 12) {
                return false;
            }
            game.layout[index] = None;
            game.waste.push(card);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solvable_deal_replays_its_construction_with_only_three_draws() {
        for mut seed in [0, 1, 18, u64::MAX] {
            let mut game = Game::with_mode(DealMode::TriPeaksSolvable, seed);
            let order = removal_order(next_random(&mut seed));
            for (step, index) in order.into_iter().enumerate() {
                if step != 0 && step % CHAIN_LENGTH == 0 {
                    assert!(game.play(Move::Draw));
                }
                assert!(game.play(Move::Clear(Place::Layout(index))));
            }
            assert!(game.won());
            assert_eq!(game.moves, 31);
        }
    }
}
