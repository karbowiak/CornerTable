use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hint {
    pub action: Move,
    pub reason: HintReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintReason {
    Reveal,
    SafeFoundation,
    Foundation,
    EmptyColumn,
    FreeCell,
    JoinRun,
    UseWaste,
    SetUp,
    OpenDeal,
    CompleteRun,
    Pair,
    King,
    Adjacent,
    Draw,
    Recycle,
    DealRow,
}

impl Game {
    /// Suggest one legal action without changing the board. Recent positions
    /// let the caller avoid sending the player back through the same moves.
    pub fn hint(&self, recent: &[Game]) -> Option<Hint> {
        if self.won() {
            return None;
        }
        match self.variant {
            Variant::Pyramid | Variant::TriPeaks => layout_hint(self),
            _ => tableau_hint(self, recent),
        }
    }

    /// Return a complete, legal foundation collection from a face-up board
    /// when every remaining card can be collected without drawing or rearranging.
    pub fn collection_moves(&self) -> Option<Vec<Move>> {
        if !matches!(self.variant, Variant::Klondike | Variant::FreeCell)
            || self.won()
            || !self.stock.is_empty()
            || self.tableau.iter().flatten().any(|entry| !entry.face_up)
        {
            return None;
        }

        let mut board = self.clone();
        let mut moves = Vec::with_capacity(52 - board.foundation_count());
        while !board.won() {
            let (action, next) = tableau_tops(&board)
                .chain(rack_sources(&board))
                .filter_map(|from| {
                    let card = board.card_run(from)?.0;
                    Some(transfer(from, foundation(card)))
                })
                .find_map(|action| played(&board, action).map(|next| (action, next)))?;
            moves.push(action);
            board = next;
        }
        Some(moves)
    }
}

fn layout_hint(game: &Game) -> Option<Hint> {
    let pyramid = game.variant == Variant::Pyramid;
    let clear = if pyramid {
        HintReason::King
    } else {
        HintReason::Adjacent
    };
    let places: Vec<_> = (0..game.layout.len())
        .map(Place::Layout)
        .chain(pyramid.then_some(Place::Waste))
        .filter(|&place| game.selectable(place))
        .collect();
    let mut best = None;
    for (index, &from) in places.iter().enumerate() {
        let candidates = [(Move::Clear(from), clear)];
        let pairs = places[index + 1..]
            .iter()
            .copied()
            .filter(|_| pyramid)
            .map(|to| (transfer(from, to), HintReason::Pair));
        for (action, reason) in candidates.into_iter().chain(pairs) {
            let Some(next) = played(game, action) else {
                continue;
            };
            let score = (
                next.won(),
                match action {
                    Move::Transfer {
                        to: Place::Layout(_),
                        ..
                    } => 2,
                    Move::Clear(Place::Waste) => 0,
                    _ => 1,
                },
                (0..next.layout.len())
                    .map(Place::Layout)
                    .filter(|&place| {
                        if pyramid {
                            next.selectable(place)
                        } else {
                            next.clone().play(Move::Clear(place))
                        }
                    })
                    .count(),
            );
            keep_best(&mut best, score, Hint { action, reason });
        }
    }
    best.map(|(_, hint)| hint)
        .or_else(|| draw_hint(game, false))
}

fn tableau_hint(game: &Game, recent: &[Game]) -> Option<Hint> {
    let mut best = None;
    let mut foundation = None;
    let mut setups = Vec::new();
    for (action, next) in legal_transfers(game) {
        if recent.iter().any(|board| same_position(&next, board)) {
            continue;
        }
        match progress(game, action, &next) {
            Some((_, reason @ HintReason::Foundation)) => {
                foundation.get_or_insert(Hint { action, reason });
            }
            Some((score, reason)) => keep_best(&mut best, score, Hint { action, reason }),
            None => setups.push((action, next)),
        }
    }
    if let Some((_, hint)) = best {
        return Some(hint);
    }

    // Offer a temporary move if it opens a productive move or helps make a
    // pending Spider deal possible.
    let empty = game.tableau.iter().filter(|stack| stack.is_empty()).count();
    let spider_waiting = game.variant == Variant::Spider
        && game.stock.len() >= game.tableau.len()
        && empty > 0
        && game
            .tableau
            .iter()
            .map(|stack| stack.len().saturating_sub(1))
            .sum::<usize>()
            >= empty;
    for (action, setup) in setups {
        let fills_column = matches!(action, Move::Transfer { to: Place::Tableau(column, _), .. }
            if game.tableau[column].is_empty());
        if spider_waiting && fills_column {
            keep_best(
                &mut best,
                65,
                Hint {
                    action,
                    reason: HintReason::OpenDeal,
                },
            );
        }
        for (followup, next) in legal_transfers(&setup) {
            if let Some((score, reason)) = progress(&setup, followup, &next)
                && reason != HintReason::Foundation
                && !same_position(&next, game)
                && !recent.iter().any(|board| same_position(&next, board))
            {
                keep_best(
                    &mut best,
                    score,
                    Hint {
                        action,
                        reason: HintReason::SetUp,
                    },
                );
            }
        }
    }
    best.map(|(_, hint)| hint)
        .or_else(|| draw_hint(game, foundation.is_some()))
        .or(foundation)
}

fn legal_transfers(game: &Game) -> impl Iterator<Item = (Move, Game)> {
    let sources = game
        .tableau
        .iter()
        .enumerate()
        .flat_map(|(column, stack)| {
            (0..stack.len()).map(move |index| Place::Tableau(column, index))
        })
        .chain(rack_sources(game));
    sources
        .filter(|&from| game.selectable(from))
        .flat_map(|from| transfers_from(game, from))
        .filter_map(|action| played(game, action).map(|next| (action, next)))
}

fn rack_sources(game: &Game) -> impl Iterator<Item = Place> {
    (game.variant == Variant::Klondike && !game.waste.is_empty())
        .then_some(Place::Waste)
        .into_iter()
        .chain(
            game.cells
                .iter()
                .enumerate()
                .filter_map(|(index, cell)| cell.map(|_| Place::Cell(index))),
        )
}

fn tableau_tops(game: &Game) -> impl Iterator<Item = Place> {
    game.tableau
        .iter()
        .enumerate()
        .filter_map(|(column, stack)| Some(Place::Tableau(column, stack.len().checked_sub(1)?)))
}

fn transfers_from(game: &Game, from: Place) -> impl Iterator<Item = Move> {
    let foundation = game.card_run(from).and_then(|(card, count)| {
        (game.variant != Variant::Spider && count == 1).then(|| foundation(card))
    });
    let cell = if game.variant == Variant::FreeCell {
        game.cells.iter().position(Option::is_none).map(Place::Cell)
    } else {
        None
    };
    let empty = game.tableau.iter().position(Vec::is_empty);
    let column = match (from, empty) {
        // Moving an entire column to an empty space only swaps the columns.
        (Place::Tableau(_, 0), _) => None,
        (_, Some(index)) => Some(Place::Tableau(index, 0)),
        _ => None,
    };
    foundation
        .into_iter()
        .chain(tableau_tops(game))
        .chain(column)
        .chain(cell)
        .map(move |to| transfer(from, to))
}

fn progress(game: &Game, action: Move, next: &Game) -> Option<(u16, HintReason)> {
    use HintReason::*;
    let Move::Transfer { from, to } = action else {
        return None;
    };
    if next.runs > game.runs {
        return Some((120, CompleteRun));
    }
    let before = match from {
        Place::Tableau(column, index) => index
            .checked_sub(1)
            .map(|index| game.tableau[column][index]),
        _ => None,
    };
    if before.is_some_and(|entry| !entry.face_up) {
        let unsafe_foundation = matches!(to, Place::Foundation(index)
            if !safe_foundation(game, *next.foundations[index].last().unwrap()));
        return Some((100 - 5 * u16::from(unsafe_foundation), Reveal));
    }
    if let Place::Foundation(index) = to {
        let card = *next.foundations[index].last().unwrap();
        return Some(if safe_foundation(game, card) {
            (90, SafeFoundation)
        } else {
            (0, Foundation)
        });
    }
    if from == Place::Waste {
        return Some((70, UseWaste));
    }
    if let Place::Tableau(column, _) = to
        && let Some(target) = game.tableau[column].last()
    {
        if matches!(from, Place::Cell(_)) {
            return Some((75, FreeCell));
        }
        let Place::Tableau(source, index) = from else {
            return None;
        };
        let card = game.tableau[source][index].card;
        let connects = |parent: Card| {
            if game.variant == Variant::Spider {
                parent.suit == card.suit
            } else {
                alternating_on(parent, card)
            }
        };
        if index == 0 {
            return Some((if connects(target.card) { 80 } else { 60 }, EmptyColumn));
        }
        if connects(target.card) && before.is_none_or(|entry| !connects(entry.card)) {
            return Some((68, JoinRun));
        }
    }
    None
}

fn safe_foundation(game: &Game, card: Card) -> bool {
    card.rank <= 2
        || Suit::ALL
            .into_iter()
            .zip(&game.foundations)
            .filter(|(suit, _)| suit.red() != card.suit.red())
            .all(|(_, stack)| stack.len() >= usize::from(card.rank - 1))
}

fn draw_hint(game: &Game, require_safe_foundation: bool) -> Option<Hint> {
    let mut next = played(game, Move::Draw)?;
    let reason = match game.variant {
        Variant::Klondike => {
            // Check whether the stock cycle can bring any usable card to the
            // waste. The hint only identifies the stock, never a hidden card.
            let limit = 2 * (game.stock.len() + game.waste.len()) + 2;
            let mut useful = false;
            for _ in 0..limit {
                if same_position(&next, game) {
                    break;
                }
                if !next.waste.is_empty()
                    && transfers_from(&next, Place::Waste).any(|action| {
                        played(&next, action).is_some_and(|after| {
                            !require_safe_foundation
                                || !matches!(
                                    progress(&next, action, &after),
                                    Some((_, HintReason::Foundation))
                                )
                        })
                    })
                {
                    useful = true;
                    break;
                }
                next.play(Move::Draw);
            }
            if !useful {
                return None;
            }
            if game.stock.is_empty() {
                HintReason::Recycle
            } else {
                HintReason::Draw
            }
        }
        Variant::Spider => HintReason::DealRow,
        Variant::Pyramid if game.can_recycle_stock() => HintReason::Recycle,
        _ => HintReason::Draw,
    };
    Some(Hint {
        action: Move::Draw,
        reason,
    })
}

fn played(game: &Game, action: Move) -> Option<Game> {
    let mut next = game.clone();
    next.play(action).then_some(next)
}

fn transfer(from: Place, to: Place) -> Move {
    Move::Transfer { from, to }
}

fn keep_best<S: Ord>(best: &mut Option<(S, Hint)>, score: S, hint: Hint) {
    if best.as_ref().is_none_or(|(previous, _)| score > *previous) {
        *best = Some((score, hint));
    }
}

fn same_position(first: &Game, second: &Game) -> bool {
    first.mode() == second.mode()
        && first.tableau == second.tableau
        && first.stock == second.stock
        && first.waste == second.waste
        && first.cells == second.cells
        && first.foundations == second.foundations
        && first.runs == second.runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(suit: Suit, rank: u8) -> Card {
        Card { suit, rank }
    }

    fn empty_board(mode: DealMode) -> Game {
        let mut game = Game::with_mode(mode, 0);
        game.tableau.iter_mut().for_each(Vec::clear);
        game.stock.clear();
        game.waste.clear();
        game.layout.fill(None);
        game
    }

    #[test]
    fn seeded_hints_are_legal_and_leave_the_game_unchanged() {
        for variant in [
            Variant::Klondike,
            Variant::Spider,
            Variant::FreeCell,
            Variant::Pyramid,
            Variant::TriPeaks,
        ] {
            for &mode in variant.deal_modes() {
                for seed in [0, 1, 18, u64::MAX] {
                    let mut game = Game::with_mode(mode, seed);
                    assert!(game.hint(&[]).is_some(), "{mode:?}, seed {seed}");
                    let mut recent = Vec::new();
                    for _ in 0..16 {
                        let before = game.clone();
                        let Some(hint) = game.hint(&recent) else {
                            break;
                        };
                        assert_eq!(game.hint(&recent), Some(hint));
                        assert_eq!(game, before);
                        assert!(game.play(hint.action), "{mode:?}, {hint:?}");
                        if hint.action != Move::Draw && game.layout.is_empty() {
                            assert!(!recent.iter().any(|board| same_position(&game, board)));
                        }
                        recent.push(before);
                    }
                }
            }
        }
    }

    #[test]
    fn klondike_prefers_turning_a_card_over_and_avoids_unproductive_draw_cycles() {
        let mut game = empty_board(DealMode::KlondikeDrawOne);
        game.tableau[0] = vec![
            TableCard {
                card: card(Suit::Clubs, 9),
                face_up: false,
            },
            face_up(card(Suit::Spades, 6)),
        ];
        game.tableau[1] = vec![face_up(card(Suit::Hearts, 7))];
        game.tableau[2] = vec![face_up(card(Suit::Diamonds, 1))];
        game.stock = vec![card(Suit::Spades, 1)];
        assert_eq!(
            game.hint(&[]),
            Some(Hint {
                action: transfer(Place::Tableau(0, 1), Place::Tableau(1, 0)),
                reason: HintReason::Reveal,
            })
        );

        for mode in [DealMode::KlondikeDrawOne, DealMode::KlondikeDrawThree] {
            let mut game = empty_board(mode);
            game.stock = vec![
                card(Suit::Spades, 6),
                card(Suit::Clubs, 7),
                card(Suit::Hearts, 8),
            ];
            assert!(game.hint(&[]).is_none());
            assert!(game.play(Move::Draw));
            assert!(game.hint(&[]).is_none());
        }
    }

    #[test]
    fn draw_three_recommends_recycling_if_it_opens_a_waste_play() {
        let mut game = empty_board(DealMode::KlondikeDrawThree);
        // The ace becomes the first exposed waste card after the recycle.
        game.waste = vec![
            card(Suit::Spades, 8),
            card(Suit::Clubs, 8),
            card(Suit::Hearts, 1),
            card(Suit::Diamonds, 8),
        ];
        assert_eq!(
            game.hint(&[]),
            Some(Hint {
                action: Move::Draw,
                reason: HintReason::Recycle,
            })
        );
        assert!(game.play(Move::Draw));
        assert_eq!(game.hint(&[]).unwrap().action, Move::Draw);
        assert!(game.play(Move::Draw));
        assert_eq!(
            game.hint(&[]),
            Some(Hint {
                action: transfer(Place::Waste, Place::Foundation(1)),
                reason: HintReason::SafeFoundation,
            })
        );
    }

    #[test]
    fn spider_prioritizes_a_completed_suited_run_and_can_set_up_a_deal() {
        let mut game = empty_board(DealMode::SpiderTwoSuits);
        game.tableau[0] = vec![face_up(card(Suit::Spades, 1))];
        game.tableau[1] = (2..=13)
            .rev()
            .map(|rank| face_up(card(Suit::Spades, rank)))
            .collect();
        game.tableau[2] = vec![face_up(card(Suit::Hearts, 2))];
        assert_eq!(
            game.hint(&[]),
            Some(Hint {
                action: transfer(Place::Tableau(0, 0), Place::Tableau(1, 11)),
                reason: HintReason::CompleteRun,
            })
        );

        let mut game = empty_board(DealMode::SpiderOneSuit);
        for column in game.tableau.iter_mut().take(9) {
            *column = vec![face_up(card(Suit::Spades, 13))];
        }
        game.tableau[0].push(face_up(card(Suit::Spades, 2)));
        game.stock = vec![card(Suit::Spades, 6); 10];
        assert!(!game.clone().play(Move::Draw));
        let hint = game.hint(&[]).unwrap();
        assert_eq!(
            hint,
            Hint {
                action: transfer(Place::Tableau(0, 1), Place::Tableau(9, 0)),
                reason: HintReason::OpenDeal,
            }
        );
        assert!(game.play(hint.action));
        assert_eq!(game.hint(&[]).unwrap().reason, HintReason::DealRow);
    }

    #[test]
    fn freecell_uses_a_cell_to_open_a_move_and_skips_recent_positions() {
        let mut game = empty_board(DealMode::FreeCell);
        game.tableau.fill(vec![face_up(card(Suit::Hearts, 13))]);
        game.tableau[0] = vec![
            face_up(card(Suit::Spades, 12)),
            face_up(card(Suit::Hearts, 7)),
        ];
        let action = transfer(Place::Tableau(0, 1), Place::Cell(0));
        assert_eq!(
            game.hint(&[]),
            Some(Hint {
                action,
                reason: HintReason::SetUp
            })
        );
        assert!(game.play(action));
        assert_eq!(game.hint(&[]).unwrap().reason, HintReason::EmptyColumn);

        let mut game = empty_board(DealMode::FreeCell);
        game.tableau[0] = vec![face_up(card(Suit::Clubs, 8))];
        game.cells[0] = Some(card(Suit::Hearts, 7));
        let action = transfer(Place::Cell(0), Place::Tableau(0, 0));
        assert_eq!(
            game.hint(&[]),
            Some(Hint {
                action,
                reason: HintReason::FreeCell
            })
        );
        let mut seen = game.clone();
        assert!(seen.play(action));
        assert!(game.hint(&[seen]).is_none());
    }

    #[test]
    fn layout_hints_prefer_exposed_pairs_and_adjacent_ranks_to_drawing() {
        let mut pyramid = empty_board(DealMode::Pyramid);
        pyramid.layout[15] = Some(card(Suit::Clubs, 13));
        pyramid.layout[21] = Some(card(Suit::Spades, 3));
        pyramid.layout[22] = Some(card(Suit::Hearts, 10));
        pyramid.stock = vec![card(Suit::Diamonds, 6)];
        assert_eq!(
            pyramid.hint(&[]),
            Some(Hint {
                action: transfer(Place::Layout(21), Place::Layout(22)),
                reason: HintReason::Pair,
            })
        );

        let mut tripeaks = empty_board(DealMode::TriPeaks);
        tripeaks.layout[18] = Some(card(Suit::Spades, 13));
        tripeaks.stock = vec![card(Suit::Diamonds, 10)];
        tripeaks.waste = vec![card(Suit::Hearts, 1)];
        assert_eq!(
            tripeaks.hint(&[]),
            Some(Hint {
                action: Move::Clear(Place::Layout(18)),
                reason: HintReason::Adjacent,
            })
        );
        tripeaks.waste = vec![card(Suit::Hearts, 6)];
        assert_eq!(tripeaks.hint(&[]).unwrap().action, Move::Draw);
        tripeaks.stock.clear();
        assert!(tripeaks.hint(&[]).is_none());
    }

    #[test]
    fn collection_requires_a_complete_legal_foundation_replay() {
        for mode in [DealMode::KlondikeDrawOne, DealMode::FreeCell] {
            let mut game = empty_board(mode);
            for (index, suit) in Suit::ALL.into_iter().enumerate() {
                game.foundations[index] = (1..=11).map(|rank| card(suit, rank)).collect();
                game.tableau[index] = vec![face_up(card(suit, 13)), face_up(card(suit, 12))];
            }
            let remaining = game.tableau[0].pop().unwrap().card;
            if game.variant == Variant::Klondike {
                game.waste.push(remaining);
            } else {
                game.cells[0] = Some(remaining);
            }
            let moves = game.collection_moves().unwrap();
            assert_eq!(moves.len(), 8);
            let mut finished = game.clone();
            assert!(moves.into_iter().all(|action| finished.play(action)));
            assert!(finished.won());
            assert!(finished.collection_moves().is_none());

            game.tableau[1].swap(0, 1);
            assert!(game.collection_moves().is_none());
        }
    }
}
