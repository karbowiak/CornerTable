//! Seeded deals and turn rules for an offline matching game.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    Pocket,
    #[default]
    Classic,
    Grand,
}

impl Layout {
    pub const ALL: [Self; 3] = [Self::Pocket, Self::Classic, Self::Grand];
    pub const MAX_CARDS: usize = Self::Grand.card_count();

    pub const fn name(self) -> &'static str {
        match self {
            Self::Pocket => "Pocket",
            Self::Classic => "Classic",
            Self::Grand => "Grand",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Pocket => "pocket",
            Self::Classic => "classic",
            Self::Grand => "grand",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layout| layout.id() == id)
    }

    pub const fn slot(self) -> usize {
        match self {
            Self::Pocket => 0,
            Self::Classic => 1,
            Self::Grand => 2,
        }
    }

    pub const fn dimensions(self) -> (usize, usize) {
        match self {
            Self::Pocket => (4, 3),
            Self::Classic => (4, 4),
            Self::Grand => (6, 4),
        }
    }

    pub const fn card_count(self) -> usize {
        let (columns, rows) = self.dimensions();
        columns * rows
    }

    pub const fn pair_count(self) -> usize {
        self.card_count() / 2
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Face {
    FanShell,
    Crab,
    Jellyfish,
    Kelp,
    SeaStar,
    Lanternfish,
    Urchin,
    SandDollar,
    Seahorse,
    Anemone,
    Snail,
    Mussel,
}

impl Face {
    pub const ALL: [Self; Layout::MAX_CARDS / 2] = [
        Self::FanShell,
        Self::Crab,
        Self::Jellyfish,
        Self::Kelp,
        Self::SeaStar,
        Self::Lanternfish,
        Self::Urchin,
        Self::SandDollar,
        Self::Seahorse,
        Self::Anemone,
        Self::Snail,
        Self::Mussel,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Card {
    Hidden,
    Revealed(Face),
    Matched(Face),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Turn {
    Ready,
    One { first: usize },
    Mismatch { first: usize, second: usize },
}

impl Turn {
    fn includes(self, index: usize) -> bool {
        match self {
            Self::Ready => false,
            Self::One { first } => index == first,
            Self::Mismatch { first, second } => index == first || index == second,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flip {
    First(Face),
    Mismatch(Face, Face),
    Match { face: Face, complete: bool },
}

/// The deck is regenerated from its seed and layout on restore.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Checkpoint {
    pub layout: Layout,
    seed: u64,
    matched: u32,
    attempts: u64,
    turn: Turn,
}

pub struct Board {
    deck: Vec<Face>,
    state: Checkpoint,
}

impl Board {
    pub fn new(layout: Layout, seed: u64) -> Self {
        let mut rng = DealRng(seed);
        let mut faces = Face::ALL;
        rng.shuffle(&mut faces);
        let mut deck = Vec::with_capacity(layout.card_count());
        for &face in &faces[..layout.pair_count()] {
            deck.extend([face, face]);
        }
        rng.shuffle(&mut deck);
        Self {
            deck,
            state: Checkpoint {
                layout,
                seed,
                matched: 0,
                attempts: 0,
                turn: Turn::Ready,
            },
        }
    }

    pub fn restore(state: Checkpoint) -> Option<Self> {
        let mut board = Self::new(state.layout, state.seed);
        if state.matched >> board.deck.len() != 0 {
            return None;
        }

        for face in Face::ALL {
            let copies = board
                .deck
                .iter()
                .enumerate()
                .filter(|&(index, &candidate)| candidate == face && state.matched & bit(index) != 0)
                .count();
            if copies == 1 {
                return None;
            }
        }

        board.state = state;
        let pending = u64::from(matches!(state.turn, Turn::Mismatch { .. }));
        if board.attempts() < board.pairs() as u64 + pending {
            return None;
        }

        let revealed = |index| match board.card(index) {
            Some(Card::Revealed(face)) => Some(face),
            _ => None,
        };
        match state.turn {
            Turn::Ready => {}
            Turn::One { first } => {
                revealed(first)?;
            }
            Turn::Mismatch { first, second } => {
                if revealed(first)? == revealed(second)? {
                    return None;
                }
            }
        }
        Some(board)
    }

    pub fn layout(&self) -> Layout {
        self.state.layout
    }

    pub fn checkpoint(&self) -> Checkpoint {
        self.state
    }

    pub fn attempts(&self) -> u64 {
        self.state.attempts
    }

    pub fn pairs(&self) -> usize {
        self.state.matched.count_ones() as usize / 2
    }

    pub fn is_complete(&self) -> bool {
        self.pairs() == self.layout().pair_count()
    }

    pub fn turn(&self) -> Turn {
        self.state.turn
    }

    pub fn card(&self, index: usize) -> Option<Card> {
        let &face = self.deck.get(index)?;
        Some(if self.state.matched & bit(index) != 0 {
            Card::Matched(face)
        } else if self.state.turn.includes(index) {
            Card::Revealed(face)
        } else {
            Card::Hidden
        })
    }

    pub fn can_flip(&self, index: usize) -> bool {
        !matches!(self.turn(), Turn::Mismatch { .. }) && self.card(index) == Some(Card::Hidden)
    }

    /// An attempt is counted exactly once, when a second distinct card turns up.
    /// A mismatch holds both faces until the caller explicitly conceals them.
    pub fn flip(&mut self, index: usize) -> Option<Flip> {
        if !self.can_flip(index) {
            return None;
        }
        let face = self.deck[index];
        let Turn::One { first } = self.turn() else {
            self.state.turn = Turn::One { first: index };
            return Some(Flip::First(face));
        };
        self.state.attempts = self.state.attempts.saturating_add(1);
        let previous = self.deck[first];
        if previous != face {
            self.state.turn = Turn::Mismatch {
                first,
                second: index,
            };
            return Some(Flip::Mismatch(previous, face));
        }

        self.state.matched |= bit(first) | bit(index);
        self.state.turn = Turn::Ready;
        Some(Flip::Match {
            face,
            complete: self.is_complete(),
        })
    }

    pub fn conceal_mismatch(&mut self) -> bool {
        if matches!(self.turn(), Turn::Mismatch { .. }) {
            self.state.turn = Turn::Ready;
            true
        } else {
            false
        }
    }
}

fn bit(index: usize) -> u32 {
    1 << index
}

/// SplitMix64 with Fisher–Yates. Keep the sequence stable for saved deals.
struct DealRng(u64);

impl DealRng {
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for last in (1..items.len()).rev() {
            let other = (self.next() % (last + 1) as u64) as usize;
            items.swap(last, other);
        }
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn other(board: &Board, first: usize, matching: bool) -> usize {
        board
            .deck
            .iter()
            .enumerate()
            .find(|&(index, face)| index != first && (*face == board.deck[first]) == matching)
            .unwrap()
            .0
    }

    #[test]
    fn every_layout_has_exactly_two_copies_of_each_dealt_face() {
        for layout in Layout::ALL {
            for seed in 0..80 {
                let board = Board::new(layout, seed);
                assert_eq!(board.deck.len(), layout.card_count());
                assert_eq!(board.deck, Board::new(layout, seed).deck);
                for face in Face::ALL {
                    assert!(matches!(
                        board.deck.iter().filter(|&&card| card == face).count(),
                        0 | 2
                    ));
                }
                assert!(
                    (0..layout.card_count()).all(|index| board.card(index) == Some(Card::Hidden))
                );
            }
            assert_ne!(Board::new(layout, 0).deck, Board::new(layout, 1).deck);
        }
    }

    #[test]
    fn mismatch_holds_both_faces_and_rejects_duplicate_or_third_flips() {
        let mut board = Board::new(Layout::Pocket, 32);
        let second = other(&board, 0, false);
        let third = (1..board.deck.len())
            .find(|&index| index != second)
            .unwrap();

        assert_eq!(board.flip(usize::MAX), None);
        assert_eq!(board.flip(0), Some(Flip::First(board.deck[0])));
        assert_eq!(board.flip(0), None);
        assert_eq!(board.attempts(), 0);
        assert_eq!(
            board.flip(second),
            Some(Flip::Mismatch(board.deck[0], board.deck[second]))
        );
        let before = board.checkpoint();
        assert_eq!(board.flip(0), None);
        assert_eq!(board.flip(second), None);
        assert_eq!(board.flip(third), None);
        assert_eq!(board.checkpoint(), before);
        assert_eq!(board.card(0), Some(Card::Revealed(board.deck[0])));
        assert_eq!(board.card(second), Some(Card::Revealed(board.deck[second])));
        assert!(board.conceal_mismatch());
        assert!(!board.conceal_mismatch());
        assert_eq!(board.card(0), Some(Card::Hidden));
        assert_eq!(board.card(second), Some(Card::Hidden));
        assert_eq!(board.attempts(), 1);
        assert!(board.can_flip(third));
    }

    #[test]
    fn two_distinct_cards_count_once_and_the_last_pair_completes() {
        let mut board = Board::new(Layout::Grand, 94);
        for expected in 1..=board.layout().pair_count() {
            let first = (0..board.deck.len())
                .find(|&index| board.can_flip(index))
                .unwrap();
            let second = other(&board, first, true);
            let face = board.deck[first];
            board.flip(first);
            assert_eq!(
                board.flip(second),
                Some(Flip::Match {
                    face,
                    complete: expected == board.layout().pair_count(),
                })
            );
            assert_eq!(board.card(first), Some(Card::Matched(face)));
            assert_eq!(board.card(second), Some(Card::Matched(face)));
            assert_eq!(board.flip(first), None);
            assert_eq!(board.pairs(), expected);
            assert_eq!(board.attempts(), expected as u64);
        }
        assert!(board.is_complete());
        assert!(Board::restore(board.checkpoint()).unwrap().is_complete());
    }

    #[test]
    fn restore_preserves_open_cards_matches_and_the_mismatch_lock() {
        let mut board = Board::new(Layout::Classic, 176);
        let mate = other(&board, 0, true);
        board.flip(0);
        board.flip(mate);
        let next = (1..board.deck.len()).find(|&index| index != mate).unwrap();
        board.flip(next);
        let mut restored = Board::restore(board.checkpoint()).unwrap();
        assert_eq!(restored.card(mate), board.card(mate));
        assert_eq!(restored.card(next), board.card(next));
        let different = (1..board.deck.len())
            .find(|&index| index != mate && board.deck[index] != board.deck[next])
            .unwrap();
        restored.flip(different);
        let mut restored = Board::restore(restored.checkpoint()).unwrap();
        assert_eq!(restored.attempts(), 2);
        assert_eq!(restored.pairs(), 1);
        assert_eq!(
            restored.turn(),
            Turn::Mismatch {
                first: next,
                second: different
            }
        );
        assert_eq!(restored.flip(next), None);
        assert!(restored.conceal_mismatch());
        assert_eq!(restored.card(next), Some(Card::Hidden));
    }

    #[test]
    fn impossible_checkpoints_are_rejected() {
        let board = Board::new(Layout::Pocket, 2);
        let empty = board.checkpoint();
        for damaged in [
            Checkpoint {
                matched: bit(24),
                ..empty
            },
            Checkpoint {
                matched: bit(0),
                ..empty
            },
            Checkpoint {
                turn: Turn::One { first: 17 },
                ..empty
            },
            Checkpoint {
                turn: Turn::Mismatch {
                    first: 0,
                    second: 0,
                },
                attempts: 1,
                ..empty
            },
            Checkpoint {
                turn: Turn::Mismatch {
                    first: 0,
                    second: other(&board, 0, true),
                },
                attempts: 1,
                ..empty
            },
            Checkpoint {
                turn: Turn::Mismatch {
                    first: 0,
                    second: other(&board, 0, false),
                },
                ..empty
            },
            Checkpoint {
                matched: bit(0) | bit(other(&board, 0, true)),
                ..empty
            },
        ] {
            assert!(Board::restore(damaged).is_none());
        }
    }
}
