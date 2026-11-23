use super::*;

pub(super) fn deal(game: &mut Game, mut deck: Vec<Card>) {
    game.layout = deck.drain(..28).map(Some).collect();
    game.stock = deck;
}

pub(super) fn available(game: &Game, place: Place) -> Option<Card> {
    if place == Place::Waste {
        return game.waste.last().copied();
    }
    let Place::Layout(index) = place else {
        return None;
    };
    let card = game.layout.get(index).copied().flatten()?;
    let row = (0..7).find(|row| index < (row + 1) * (row + 2) / 2)?;
    let child = index + row + 1;
    (row == 6 || (game.layout[child].is_none() && game.layout[child + 1].is_none())).then_some(card)
}

pub(super) fn play(game: &mut Game, action: Move) -> bool {
    match action {
        Move::Draw => game.draw_waste(1, false),
        Move::Clear(place) if available(game, place).is_some_and(|card| card.rank == 13) => {
            remove(game, place);
            true
        }
        Move::Transfer { from, to } if from != to => {
            let (Some(first), Some(second)) = (available(game, from), available(game, to)) else {
                return false;
            };
            if first.rank + second.rank != 13 {
                return false;
            }
            remove(game, from);
            remove(game, to);
            true
        }
        _ => false,
    }
}

fn remove(game: &mut Game, place: Place) {
    match place {
        Place::Layout(index) => game.layout[index] = None,
        Place::Waste => {
            game.waste.pop();
        }
        _ => unreachable!(),
    }
}
