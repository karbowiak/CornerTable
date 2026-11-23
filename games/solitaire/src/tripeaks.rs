use super::*;

pub(super) fn deal(game: &mut Game, mut deck: Vec<Card>) {
    game.layout = deck.drain(..28).map(Some).collect();
    game.waste.push(deck.pop().unwrap());
    game.stock = deck;
}

pub(super) fn available(game: &Game, place: Place) -> Option<Card> {
    let Place::Layout(index) = place else {
        return None;
    };
    let card = game.layout.get(index).copied().flatten()?;
    let child = match index {
        0..3 => 3 + index * 2,
        3..9 => 9 + (index - 3) / 2 * 3 + (index - 3) % 2,
        9..18 => 18 + index - 9,
        _ => return Some(card),
    };
    (game.layout[child].is_none() && game.layout[child + 1].is_none()).then_some(card)
}

pub(super) fn play(game: &mut Game, action: Move) -> bool {
    match action {
        Move::Draw => game.draw_waste(false),
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
