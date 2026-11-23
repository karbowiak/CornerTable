use super::*;

pub(super) fn deal(game: &mut Game, deck: Vec<Card>) {
    game.tableau = vec![Vec::new(); 8];
    for (index, card) in deck.into_iter().enumerate() {
        game.tableau[index % 8].push(face_up(card));
    }
}

pub(super) fn play(game: &mut Game, action: Move) -> bool {
    let Move::Transfer { from, to } = action else {
        return false;
    };
    let Some((card, count)) = game.card_run(from) else {
        return false;
    };
    match to {
        Place::Foundation(index) if count == 1 && foundation_accepts(game, index, card) => {
            game.remove_run(from);
            game.foundations[index].push(card);
        }
        Place::Cell(index) if count == 1 && game.cells.get(index) == Some(&None) => {
            game.remove_run(from);
            game.cells[index] = Some(card);
        }
        Place::Tableau(column, _) if !matches!(from, Place::Tableau(source, _) if source == column) =>
        {
            let Some(stack) = game.tableau.get(column) else {
                return false;
            };
            let empty_cells = game.cells.iter().filter(|cell| cell.is_none()).count();
            let empty_columns = game
                .tableau
                .iter()
                .enumerate()
                .filter(|(index, stack)| *index != column && stack.is_empty())
                .count();
            let capacity = (empty_cells + 1) << empty_columns;
            if count > capacity
                || stack
                    .last()
                    .is_some_and(|top| !alternating_on(top.card, card))
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
