use crate::{Move, Place};

mod catalog {
    include!("deals_v1.rs");
}

fn deal(seed: u64) -> (u64, &'static str) {
    // Catalog order and size are part of the persisted seed mapping.
    let deals = &catalog::DEALS_V1;
    deals[(seed % deals.len() as u64) as usize]
}

pub(crate) fn shuffle_seed(seed: u64) -> u64 {
    deal(seed).0
}

pub(crate) fn solution(seed: u64) -> impl Iterator<Item = Move> {
    // A dot draws. Transfers encode the source (waste, foundation A–D, or
    // column and base-36 card index) and destination (column or foundation a–d).
    let mut bytes = deal(seed).1.bytes();
    std::iter::from_fn(move || {
        let from = match bytes.next()? {
            b'.' => return Some(Move::Draw),
            b'w' => Place::Waste,
            suit @ b'A'..=b'D' => Place::Foundation(usize::from(suit - b'A')),
            column @ b'0'..=b'6' => {
                let index = char::from(bytes.next().unwrap()).to_digit(36).unwrap() as usize;
                Place::Tableau(usize::from(column - b'0'), index)
            }
            byte => panic!("invalid Klondike witness source: {byte}"),
        };
        let to = match bytes.next().unwrap() {
            suit @ b'a'..=b'd' => Place::Foundation(usize::from(suit - b'a')),
            column @ b'0'..=b'6' => Place::Tableau(usize::from(column - b'0'), 0),
            byte => panic!("invalid Klondike witness destination: {byte}"),
        };
        Some(Move::Transfer { from, to })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DealMode, Game};

    const MODE: DealMode = DealMode::KlondikeDrawOneVerified;

    #[test]
    fn every_catalog_deal_is_a_distinct_shuffle_with_a_winning_replay() {
        let mut openings = std::collections::HashSet::new();
        for (index, &(raw_seed, _)) in catalog::DEALS_V1.iter().enumerate() {
            let seed = index as u64;
            let opening = Game::with_mode(MODE, seed);
            let classic = Game::with_mode(DealMode::KlondikeDrawOne, raw_seed);
            assert_eq!(opening.tableau, classic.tableau, "deal {seed}");
            assert_eq!(opening.stock, classic.stock, "deal {seed}");
            assert!(
                openings.insert(
                    opening
                        .tableau
                        .iter()
                        .flatten()
                        .map(|entry| entry.card)
                        .chain(opening.stock.iter().copied())
                        .map(|card| 13 * card.suit as u8 + card.rank)
                        .collect::<Vec<_>>()
                )
            );

            let turns = Game::verified_klondike_solution(seed).collect::<Vec<_>>();
            assert!(turns.iter().any(|turn| matches!(
                turn,
                Move::Transfer {
                    from: Place::Tableau(..),
                    to: Place::Tableau(..)
                }
            )));
            let game = Game::replay(MODE, seed, turns)
                .unwrap_or_else(|| panic!("illegal witness move for deal {seed}"));
            assert!(game.won(), "incomplete witness for deal {seed}");
        }
    }

    #[test]
    fn seeds_wrap_without_changing_draw_one_rules_or_the_witness() {
        let seed = u64::MAX;
        let opening = Game::with_mode(MODE, seed);
        assert_ne!(
            opening.stock,
            Game::with_mode(MODE, seed.wrapping_add(1)).stock
        );
        let mut drawing = opening.clone();
        let mut stock = opening.stock;
        for _ in 0..stock.len() {
            assert!(drawing.play(Move::Draw));
            assert_eq!(drawing.waste.last().copied(), stock.pop());
            assert_eq!(drawing.stock, stock);
        }
        assert!(drawing.play(Move::Draw));
        assert_eq!(drawing.stock, Game::with_mode(MODE, seed).stock);
        assert!(drawing.waste.is_empty());
        assert!(
            Game::replay(MODE, seed, Game::verified_klondike_solution(seed))
                .unwrap()
                .won()
        );
    }
}
