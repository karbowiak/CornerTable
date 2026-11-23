use super::*;

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
        Move::Draw => return game.draw_waste(true),
        Move::Transfer { from, to } => (from, to),
        _ => return false,
    };
    let Some((card, count)) = game.card_run(from) else {
        return false;
    };
    match to {
        Place::Foundation(index)
            if from != to && count == 1 && foundation_accepts(game, index, card) =>
        {
            game.remove_run(from);
            game.foundations[index].push(card);
        }
        Place::Tableau(column, _) if !matches!(from, Place::Tableau(source, _) if source == column) =>
        {
            let Some(stack) = game.tableau.get(column) else {
                return false;
            };
            if !stack
                .last()
                .map_or(card.rank == 13, |top| alternating_on(top.card, card))
            {
                return false;
            }
            let run = game.remove_run(from);
            game.tableau[column].extend(run);
        }
        _ => return false,
    }
    true
}
