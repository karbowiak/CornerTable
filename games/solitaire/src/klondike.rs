use super::*;

pub(super) mod verified;

pub(super) fn deal(game: &mut Game, mut deck: Vec<Card>) {
    for column in 0..7 {
        game.tableau.push(
            (0..=column)
                .map(|index| TableCard {
                    card: deck.pop().unwrap(),
                    face_up: index == column,
                })
                .collect(),
        );
    }
    game.stock = deck;
}

pub(super) fn play(game: &mut Game, action: Move) -> bool {
    let (from, to) = match action {
        Move::Draw => {
            let count = if game.mode() == DealMode::KlondikeDrawThree {
                3
            } else {
                1
            };
            return game.draw_waste(count, true);
        }
        Move::Transfer { from, to } => (from, to),
        _ => return false,
    };
    let Some((card, count)) = game.card_run(from) else {
        return false;
    };
    if !accepts(game, from, to, card, count) {
        return false;
    }
    let run = game.remove_run(from);
    match to {
        Place::Foundation(index) => game.foundations[index].push(card),
        Place::Tableau(column, _) => game.tableau[column].extend(run),
        _ => unreachable!(),
    }
    true
}

pub(super) fn accepts(game: &Game, from: Place, to: Place, card: Card, count: usize) -> bool {
    match to {
        Place::Foundation(index) => {
            from != to && count == 1 && foundation_accepts(game, index, card)
        }
        Place::Tableau(column, _) if !matches!(from, Place::Tableau(source, _) if source == column) => {
            game.tableau.get(column).is_some_and(|stack| {
                stack
                    .last()
                    .map_or(card.rank == 13, |top| alternating_on(top.card, card))
            })
        }
        _ => false,
    }
}
