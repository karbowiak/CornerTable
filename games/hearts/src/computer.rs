use serde::{Deserialize, Serialize};

use super::{Card, Game, Phase, Suit, winning_card};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Difficulty {
    Easy,
    #[default]
    Medium,
    Hard,
}

pub(super) fn pass(hand: &[Card]) -> [Card; 3] {
    let mut ranked = hand.to_vec();
    ranked.sort_by_cached_key(|&card| {
        let length = hand
            .iter()
            .filter(|held| held.suit() == card.suit())
            .count();
        let void_bonus = match card.suit() {
            Suit::Diamonds | Suit::Clubs if length <= 3 => 5,
            _ => 0,
        };
        (discard_risk(card) + void_bonus, card)
    });
    let len = ranked.len();
    [ranked[len - 1], ranked[len - 2], ranked[len - 3]]
}

fn discard_risk(card: Card) -> u8 {
    let rank = card.rank();
    match card.suit() {
        Suit::Spades if rank >= 12 => rank + 18,
        Suit::Hearts => rank + 3,
        _ => rank,
    }
}

pub(super) fn play(game: &Game, difficulty: Difficulty) -> Option<Card> {
    let Phase::Playing(seat) = game.phase() else {
        return None;
    };
    let legal = game.legal_plays();
    if difficulty == Difficulty::Easy {
        return legal.into_iter().min_by_key(|card| card.rank());
    }
    let held = game.hand(seat);
    let queen_live = !game
        .taken
        .iter()
        .chain(game.trick())
        .any(|play| play.card == Card::QUEEN_OF_SPADES);
    let Some(leader) = winning_card(game.trick()) else {
        return legal.into_iter().min_by_key(|&card| {
            let short_suit = held
                .iter()
                .filter(|held| held.suit() == card.suit())
                .count();
            let probes_queen = queen_live
                && card.suit() == Suit::Spades
                && card.rank() < 12
                && !held.contains(&Card::QUEEN_OF_SPADES);
            let voids = if difficulty == Difficulty::Hard {
                known_voids(game, card.suit())
            } else {
                0
            };
            (voids, !probes_queen, discard_risk(card), short_suit)
        });
    };

    let suit = leader.card.suit();
    if legal.first()?.suit() != suit {
        // With no card in the led suit, discard penalty cards first.
        return legal
            .into_iter()
            .max_by_key(|&card| (card.points(), discard_risk(card)));
    }
    let points_on_table: u8 = game.trick().iter().map(|play| play.card.points()).sum();
    if game.trick().len() == 3 {
        if points_on_table == 0 {
            // The last player can safely unload a high card on a clean trick.
            return legal
                .into_iter()
                .filter(|&card| card.points() == 0 || card.rank() < leader.card.rank())
                .max_by_key(|card| card.rank())
                .or_else(|| weakest_take(game));
        }
        let captured = game.hand_points();
        let threat = captured[leader.seat.index()];
        // One opponent has every penalty so far. Take this trick if possible.
        let threshold = if difficulty == Difficulty::Hard && points_on_table <= 3 {
            10
        } else {
            16
        };
        if threat >= threshold
            && captured.iter().sum::<u8>() == threat
            && let Some(card) = legal
                .iter()
                .copied()
                .filter(|card| card.rank() > leader.card.rank())
                .min_by_key(|card| (card.points(), card.rank()))
        {
            return Some(card);
        }
    }
    legal
        .into_iter()
        .filter(|card| card.rank() < leader.card.rank())
        .max_by_key(|card| card.rank())
        .or_else(|| weakest_take(game))
}

fn weakest_take(game: &Game) -> Option<Card> {
    game.legal_plays()
        .into_iter()
        .min_by_key(|card| (card.points(), card.rank()))
}

fn known_voids(game: &Game, suit: Suit) -> usize {
    let mut seats = [false; 4];
    for trick in game.taken.as_chunks::<4>().0 {
        if trick[0].card.suit() == suit {
            for play in &trick[1..] {
                if play.card.suit() != suit {
                    seats[play.seat.index()] = true;
                }
            }
        }
    }
    seats.into_iter().filter(|&void| void).count()
}
