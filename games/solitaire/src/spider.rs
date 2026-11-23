use super::*;

pub(super) fn deal(game: &mut Game, mut deck: Vec<Card>) {
    for column in 0..10 {
        let hidden = if column < 4 { 5 } else { 4 };
        game.tableau.push(
            (0..=hidden)
                .map(|index| TableCard {
                    card: deck.pop().unwrap(),
                    face_up: index == hidden,
                })
                .collect(),
        );
    }
    game.stock = deck;
}

pub(super) fn play(game: &mut Game, action: Move) -> bool {
    match action {
        Move::Draw => {
            if !can_deal(game) {
                return false;
            }
            for column in 0..game.tableau.len() {
                game.tableau[column].push(face_up(game.stock.pop().unwrap()));
                collect_runs(game, column);
            }
        }
        Move::Transfer {
            from: Place::Tableau(source, index),
            to: Place::Tableau(target, _),
        } if source != target => {
            let Some((card, _)) = game.run(source, index, true) else {
                return false;
            };
            let Some(stack) = game.tableau.get(target) else {
                return false;
            };
            if !accepts(stack, card) {
                return false;
            }
            let run = game.remove_run(Place::Tableau(source, index));
            game.tableau[target].extend(run);
            collect_runs(game, target);
        }
        _ => return false,
    }
    true
}

pub(super) fn has_move(game: &Game) -> bool {
    can_deal(game)
        || game.tableau.iter().enumerate().any(|(source, stack)| {
            // Every face-up, suited suffix is movable, including a whole column.
            (0..stack.len())
                .rev()
                .map_while(|index| game.run(source, index, true))
                .any(|(card, _)| {
                    game.tableau
                        .iter()
                        .enumerate()
                        .any(|(target, stack)| target != source && accepts(stack, card))
                })
        })
}

fn can_deal(game: &Game) -> bool {
    game.stock.len() >= game.tableau.len() && game.tableau.iter().all(|stack| !stack.is_empty())
}

fn accepts(stack: &[TableCard], card: Card) -> bool {
    stack
        .last()
        .is_none_or(|entry| entry.card.rank == card.rank + 1)
}

fn collect_runs(game: &mut Game, column: usize) {
    while let Some(start) = game.tableau[column].len().checked_sub(13) {
        if game.run(column, start, true).is_none() {
            break;
        }
        game.tableau[column].truncate(start);
        game.flip(column);
        game.runs += 1;
    }
}
