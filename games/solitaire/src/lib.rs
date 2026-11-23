mod freecell;
mod hint;
mod klondike;
mod pyramid;
mod spider;
mod tripeaks;

pub use hint::{Hint, HintReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Variant {
    Klondike,
    Spider,
    FreeCell,
    Pyramid,
    TriPeaks,
}

impl Variant {
    pub fn from_id(id: &str) -> Self {
        match id {
            "spider" => Self::Spider,
            "freecell" => Self::FreeCell,
            "pyramid" => Self::Pyramid,
            "tripeaks" => Self::TriPeaks,
            _ => Self::Klondike,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Klondike => "Klondike",
            Self::Spider => "Spider",
            Self::FreeCell => "FreeCell",
            Self::Pyramid => "Pyramid",
            Self::TriPeaks => "TriPeaks",
        }
    }

    pub fn deal_modes(self) -> &'static [DealMode] {
        use DealMode::*;
        match self {
            Self::Klondike => &[KlondikeDrawOne, KlondikeDrawThree, KlondikeDrawOneVerified],
            Self::Spider => &[SpiderOneSuit, SpiderTwoSuits, SpiderFourSuits],
            Self::FreeCell => &[FreeCell],
            Self::Pyramid => &[Pyramid],
            Self::TriPeaks => &[TriPeaks, TriPeaksSolvable],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealMode {
    KlondikeDrawOne,
    KlondikeDrawThree,
    /// Draw 1 rules with a seed selecting a shuffle from a fixed catalog of wins.
    KlondikeDrawOneVerified,
    SpiderOneSuit,
    SpiderTwoSuits,
    SpiderFourSuits,
    FreeCell,
    Pyramid,
    TriPeaks,
    TriPeaksSolvable,
}

impl DealMode {
    fn variant(self) -> Variant {
        match self {
            Self::KlondikeDrawOne | Self::KlondikeDrawThree | Self::KlondikeDrawOneVerified => {
                Variant::Klondike
            }
            Self::SpiderOneSuit | Self::SpiderTwoSuits | Self::SpiderFourSuits => Variant::Spider,
            Self::FreeCell => Variant::FreeCell,
            Self::Pyramid => Variant::Pyramid,
            Self::TriPeaks | Self::TriPeaksSolvable => Variant::TriPeaks,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Suit {
    Spades,
    Hearts,
    Clubs,
    Diamonds,
}

impl Suit {
    pub const ALL: [Self; 4] = [Self::Spades, Self::Hearts, Self::Clubs, Self::Diamonds];

    pub fn red(self) -> bool {
        matches!(self, Self::Hearts | Self::Diamonds)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Card {
    pub suit: Suit,
    pub rank: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableCard {
    pub card: Card,
    pub face_up: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Place {
    Stock,
    Waste,
    Tableau(usize, usize),
    Foundation(usize),
    Cell(usize),
    Layout(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Move {
    Draw,
    Transfer { from: Place, to: Place },
    Clear(Place),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Game {
    pub variant: Variant,
    mode: DealMode,
    pub tableau: Vec<Vec<TableCard>>,
    pub stock: Vec<Card>,
    pub waste: Vec<Card>,
    pub foundations: [Vec<Card>; 4],
    pub cells: [Option<Card>; 4],
    pub layout: Vec<Option<Card>>,
    pub runs: usize,
    pub moves: usize,
}

impl Game {
    pub fn new(variant: Variant, seed: u64) -> Self {
        Self::with_mode(variant.deal_modes()[0], seed)
    }

    pub fn with_mode(mode: DealMode, seed: u64) -> Self {
        let mut game = Self {
            variant: mode.variant(),
            mode,
            tableau: Vec::new(),
            stock: Vec::new(),
            waste: Vec::new(),
            foundations: Default::default(),
            cells: [None; 4],
            layout: Vec::new(),
            runs: 0,
            moves: 0,
        };
        let deck_seed = if mode == DealMode::KlondikeDrawOneVerified {
            klondike::verified::shuffle_seed(seed)
        } else {
            seed
        };
        let deck = shuffled_deck(deck_seed, mode);
        match game.variant {
            Variant::Klondike => klondike::deal(&mut game, deck),
            Variant::Spider => spider::deal(&mut game, deck),
            Variant::FreeCell => freecell::deal(&mut game, deck),
            Variant::Pyramid => pyramid::deal(&mut game, deck),
            Variant::TriPeaks if mode == DealMode::TriPeaksSolvable => {
                tripeaks::deal_solvable(&mut game, deck, seed);
            }
            Variant::TriPeaks => tripeaks::deal(&mut game, deck),
        }
        game
    }

    pub fn mode(&self) -> DealMode {
        self.mode
    }

    /// Replay witness that wins the opening [`DealMode::KlondikeDrawOneVerified`] deal at `seed`.
    pub fn verified_klondike_solution(seed: u64) -> impl Iterator<Item = Move> {
        klondike::verified::solution(seed)
    }

    /// Reconstruct a seeded deal and reject a replay containing any illegal move.
    pub fn replay(
        mode: DealMode,
        seed: u64,
        turns: impl IntoIterator<Item = Move>,
    ) -> Option<Self> {
        let mut game = Self::with_mode(mode, seed);
        turns
            .into_iter()
            .all(|turn| game.play(turn))
            .then_some(game)
    }

    pub fn play(&mut self, action: Move) -> bool {
        if self.won() {
            return false;
        }
        let applied = match self.variant {
            Variant::Klondike => klondike::play(self, action),
            Variant::Spider => spider::play(self, action),
            Variant::FreeCell => freecell::play(self, action),
            Variant::Pyramid => pyramid::play(self, action),
            Variant::TriPeaks => tripeaks::play(self, action),
        };
        self.moves += usize::from(applied);
        applied
    }

    pub fn selectable(&self, place: Place) -> bool {
        match self.variant {
            Variant::Pyramid => pyramid::available(self, place).is_some(),
            Variant::TriPeaks => tripeaks::available(self, place).is_some(),
            Variant::Spider => matches!(place, Place::Tableau(column, index)
                if self.run(column, index, true).is_some()),
            _ => self.card_run(place).is_some(),
        }
    }

    pub fn won(&self) -> bool {
        match self.variant {
            Variant::Klondike | Variant::FreeCell => self.foundation_count() == 52,
            Variant::Spider => self.runs == 8,
            Variant::Pyramid | Variant::TriPeaks => self.layout.iter().all(Option::is_none),
        }
    }

    /// A live deal is stalled only when neither drawing nor any transfer or clear is legal.
    pub fn stalled(&self) -> bool {
        if self.won() {
            return false;
        }
        match self.variant {
            Variant::Pyramid | Variant::TriPeaks => self.layout_stalled(),
            Variant::Spider => !spider::has_move(self),
            Variant::Klondike => {
                self.stock.is_empty()
                    && self.waste.is_empty()
                    && !self.has_foundation_game_transfer()
            }
            Variant::FreeCell => !self.has_foundation_game_transfer(),
        }
    }

    /// The stock is exhausted and neither a clear nor a pair is possible.
    /// Only Pyramid and TriPeaks have these layout rules.
    pub fn layout_stalled(&self) -> bool {
        self.stock.is_empty()
            && !self.won()
            && match self.variant {
                Variant::Pyramid => !pyramid::has_clear(self),
                Variant::TriPeaks => !tripeaks::has_clear(self),
                _ => false,
            }
    }

    pub fn foundation_count(&self) -> usize {
        self.foundations.iter().map(Vec::len).sum()
    }

    fn has_foundation_game_transfer(&self) -> bool {
        let accepts = match self.variant {
            Variant::Klondike => klondike::accepts,
            Variant::FreeCell => freecell::accepts,
            _ => unreachable!(),
        };
        let rack = (0..4).map(|index| match self.variant {
            Variant::Klondike => Place::Foundation(index),
            _ => Place::Cell(index),
        });
        let tableau = self.tableau.iter().enumerate().flat_map(|(column, stack)| {
            (0..stack.len())
                .rev()
                .map(move |index| Place::Tableau(column, index))
        });
        rack.chain(tableau).any(|from| {
            self.card_run(from).is_some_and(|(card, count)| {
                std::iter::once(foundation(card))
                    .chain((0..self.tableau.len()).map(|column| Place::Tableau(column, 0)))
                    .chain(self.cells.iter().position(Option::is_none).map(Place::Cell))
                    .any(|to| accepts(self, from, to, card, count))
            })
        })
    }

    fn card_run(&self, place: Place) -> Option<(Card, usize)> {
        let card = match place {
            Place::Tableau(column, index) => return self.run(column, index, false),
            Place::Foundation(index) if self.variant == Variant::Klondike => {
                self.foundations.get(index)?.last().copied()
            }
            Place::Waste if self.variant == Variant::Klondike => self.waste.last().copied(),
            Place::Cell(index) if self.variant == Variant::FreeCell => *self.cells.get(index)?,
            _ => None,
        };
        Some((card?, 1))
    }

    fn run(&self, column: usize, index: usize, same_suit: bool) -> Option<(Card, usize)> {
        let stack = self.tableau.get(column)?.get(index..)?;
        let top = stack.first()?;
        if stack.iter().all(|entry| entry.face_up)
            && stack.windows(2).all(|pair| {
                let [base, next] = [pair[0].card, pair[1].card];
                base.rank == next.rank + 1
                    && if same_suit {
                        base.suit == next.suit
                    } else {
                        base.suit.red() != next.suit.red()
                    }
            })
        {
            Some((top.card, stack.len()))
        } else {
            None
        }
    }

    fn remove_run(&mut self, from: Place) -> Vec<TableCard> {
        let card = match from {
            Place::Tableau(column, index) => {
                let removed = self.tableau[column].split_off(index);
                self.flip(column);
                return removed;
            }
            Place::Waste => self.waste.pop(),
            Place::Cell(index) => self.cells[index].take(),
            Place::Foundation(index) => self.foundations[index].pop(),
            _ => None,
        };
        card.into_iter().map(face_up).collect()
    }

    fn flip(&mut self, column: usize) {
        if let Some(last) = self.tableau[column].last_mut() {
            last.face_up = true;
        }
    }

    fn draw_waste(&mut self, count: usize, recycle: bool) -> bool {
        if self.stock.is_empty() {
            if !recycle || self.waste.is_empty() {
                return false;
            }
            self.stock.extend(self.waste.drain(..).rev());
        } else {
            for _ in 0..count.min(self.stock.len()) {
                self.waste.push(self.stock.pop().unwrap());
            }
        }
        true
    }
}

fn face_up(card: Card) -> TableCard {
    TableCard {
        card,
        face_up: true,
    }
}

fn shuffled_deck(seed: u64, mode: DealMode) -> Vec<Card> {
    let (suits, copies) = match mode {
        DealMode::SpiderOneSuit => (&Suit::ALL[..1], 8),
        DealMode::SpiderTwoSuits => (&Suit::ALL[..2], 4),
        DealMode::SpiderFourSuits => (&Suit::ALL[..], 2),
        _ => (&Suit::ALL[..], 1),
    };
    let mut deck: Vec<_> = (0..copies)
        .flat_map(|_| suits)
        .flat_map(|&suit| (1..=13).map(move |rank| Card { suit, rank }))
        .collect();
    shuffle(&mut deck, seed);
    deck
}

fn shuffle<T>(values: &mut [T], mut seed: u64) {
    for index in (1..values.len()).rev() {
        values.swap(
            index,
            (next_random(&mut seed) % (index as u64 + 1)) as usize,
        );
    }
}

fn next_random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = *seed;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn foundation_accepts(game: &Game, index: usize, card: Card) -> bool {
    game.foundations
        .get(index)
        .is_some_and(|stack| Suit::ALL[index] == card.suit && card.rank as usize == stack.len() + 1)
}

fn foundation(card: Card) -> Place {
    Place::Foundation(
        Suit::ALL
            .iter()
            .position(|&suit| suit == card.suit)
            .unwrap(),
    )
}

fn alternating_on(destination: Card, card: Card) -> bool {
    destination.rank == card.rank + 1 && destination.suit.red() != card.suit.red()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spade(rank: u8) -> Card {
        Card {
            suit: Suit::Spades,
            rank,
        }
    }

    fn heart(rank: u8) -> Card {
        Card {
            suit: Suit::Hearts,
            rank,
        }
    }

    fn transfer(game: &mut Game, from: Place, to: Place) -> bool {
        game.play(Move::Transfer { from, to })
    }

    #[test]
    fn klondike_cycles_stock_and_flips_exposed_cards() {
        let mut game = Game::new(Variant::Klondike, 18);
        let stock = game.stock.clone();
        for _ in 0..stock.len() {
            assert!(game.play(Move::Draw));
        }
        assert!(game.play(Move::Draw));
        assert_eq!(game.stock, stock);
        assert!(game.waste.is_empty());

        game.tableau[0] = vec![
            TableCard {
                card: heart(6),
                face_up: false,
            },
            face_up(spade(1)),
        ];
        assert!(!transfer(
            &mut game,
            Place::Tableau(0, 0),
            Place::Foundation(1)
        ));
        assert!(transfer(
            &mut game,
            Place::Tableau(0, 1),
            Place::Foundation(0)
        ));
        assert!(game.tableau[0][0].face_up);
        assert_eq!(game.foundations[0], [spade(1)]);
    }

    #[test]
    fn klondike_draw_three_uses_the_same_deck_and_recycles_in_draw_order() {
        let mut game = Game::with_mode(DealMode::KlondikeDrawThree, 18);
        let standard = Game::new(Variant::Klondike, 18);
        assert_eq!(game.tableau, standard.tableau);
        assert_eq!(game.stock, standard.stock);

        game.stock = vec![spade(1), spade(2), heart(5), heart(6), heart(7)];
        assert!(game.play(Move::Draw));
        assert_eq!(game.waste, [heart(7), heart(6), heart(5)]);
        assert!(!transfer(&mut game, Place::Waste, Place::Foundation(0)));
        assert!(game.play(Move::Draw));
        assert!(game.stock.is_empty());
        assert_eq!(
            game.waste,
            [heart(7), heart(6), heart(5), spade(2), spade(1)]
        );
        assert!(transfer(&mut game, Place::Waste, Place::Foundation(0)));
        assert!(game.play(Move::Draw));
        assert!(game.waste.is_empty());
        assert_eq!(game.stock, [spade(2), heart(5), heart(6), heart(7)]);
        assert!(game.play(Move::Draw));
        assert!(game.play(Move::Draw));
        assert!(transfer(&mut game, Place::Waste, Place::Foundation(0)));
    }

    #[test]
    fn spider_removes_completed_sequence_and_requires_filled_columns_to_deal() {
        let mut game = Game::new(Variant::Spider, 18);
        game.tableau[0] = vec![face_up(spade(1))];
        game.tableau[1] = (2..=13).rev().map(|rank| face_up(spade(rank))).collect();
        assert!(transfer(
            &mut game,
            Place::Tableau(0, 0),
            Place::Tableau(1, 11)
        ));
        assert_eq!(game.runs, 1);
        assert!(game.tableau[1].is_empty());
        assert!(!game.play(Move::Draw));
    }

    #[test]
    fn spider_stalls_only_without_a_legal_row_or_transfer() {
        let mut game = Game::new(Variant::Spider, 266);
        for _ in 0..5 {
            assert!(!game.stalled());
            assert!(game.play(Move::Draw));
        }
        assert!(game.stalled());
        assert!(!game.won());

        game.tableau = vec![vec![face_up(heart(8))]; 10];
        game.tableau[0] = vec![face_up(heart(7)), face_up(spade(6))];
        assert!(game.stalled(), "a mixed-suit run cannot move together");
        game.tableau[0][0].card.suit = Suit::Spades;
        assert!(!game.stalled(), "a suited run can move onto another suit");
        assert!(transfer(
            &mut game,
            Place::Tableau(0, 0),
            Place::Tableau(1, 0)
        ));
        assert!(!game.stalled(), "the empty column accepts a legal transfer");

        game.tableau = vec![vec![face_up(heart(8))]; 10];
        game.stock = vec![spade(3); 9];
        assert!(game.stalled(), "a partial row cannot be dealt");
        game.stock.push(spade(3));
        assert!(!game.stalled(), "dealing a full row is a legal move");

        game.tableau[9].clear();
        assert!(!game.play(Move::Draw));
        assert!(
            !game.stalled(),
            "even a whole column can move into an empty slot"
        );
        game.tableau[0].push(face_up(heart(5)));
        assert!(transfer(
            &mut game,
            Place::Tableau(0, 1),
            Place::Tableau(9, 0)
        ));
        assert!(game.play(Move::Draw));
    }

    #[test]
    fn foundation_games_count_their_legal_moves_before_declaring_a_stall() {
        let mut klondike = Game::new(Variant::Klondike, 1);
        klondike.tableau = vec![vec![face_up(spade(7))]; 7];
        klondike.stock.clear();
        assert!(klondike.stalled());
        klondike.waste.push(spade(9));
        assert!(!klondike.stalled(), "recycling the waste is still legal");
        klondike.waste.clear();
        klondike.foundations[1] = (1..=6).map(heart).collect();
        assert!(!klondike.stalled(), "a card can return from a foundation");

        let mut freecell = Game::new(Variant::FreeCell, 1);
        freecell.tableau = vec![vec![face_up(spade(7))]; 8];
        freecell.cells = [Some(spade(8)); 4];
        assert!(freecell.stalled());
        freecell.cells[0] = None;
        assert!(!freecell.stalled(), "a card can move into a free cell");
        freecell.cells[0] = Some(heart(6));
        assert!(!freecell.stalled(), "a card can move out of a free cell");
    }

    #[test]
    fn spider_uses_two_full_decks_and_moves_only_same_suit_runs() {
        for (mode, copies) in [
            (DealMode::SpiderOneSuit, [8, 0, 0, 0]),
            (DealMode::SpiderTwoSuits, [4, 4, 0, 0]),
            (DealMode::SpiderFourSuits, [2, 2, 2, 2]),
        ] {
            let game = Game::with_mode(mode, 18);
            let cards: Vec<_> = game
                .tableau
                .iter()
                .flatten()
                .map(|entry| entry.card)
                .chain(game.stock)
                .collect();
            assert_eq!(cards.len(), 104);
            for (suit, count) in Suit::ALL.into_iter().zip(copies) {
                for rank in 1..=13 {
                    assert_eq!(
                        cards
                            .iter()
                            .filter(|&&card| card == Card { suit, rank })
                            .count(),
                        count
                    );
                }
            }
        }

        let mut game = Game::with_mode(DealMode::SpiderTwoSuits, 18);
        game.tableau[0] = vec![face_up(heart(1))];
        game.tableau[1] = (2..=13).rev().map(|rank| face_up(spade(rank))).collect();
        game.tableau[2].clear();
        assert!(transfer(
            &mut game,
            Place::Tableau(0, 0),
            Place::Tableau(1, 11)
        ));
        assert_eq!(game.runs, 0);
        assert!(!transfer(
            &mut game,
            Place::Tableau(1, 0),
            Place::Tableau(2, 0)
        ));
        assert!(transfer(
            &mut game,
            Place::Tableau(1, 12),
            Place::Tableau(2, 0)
        ));
    }

    #[test]
    fn freecell_limits_run_length_to_available_workspaces() {
        let mut game = Game::new(Variant::FreeCell, 18);
        game.cells = [Some(heart(8)); 4];
        game.tableau[0] = vec![face_up(spade(4)), face_up(heart(3))];
        game.tableau[1] = vec![face_up(heart(5))];
        assert!(!transfer(
            &mut game,
            Place::Tableau(0, 0),
            Place::Tableau(1, 0)
        ));
        game.cells[0] = None;
        assert!(transfer(
            &mut game,
            Place::Tableau(0, 0),
            Place::Tableau(1, 0)
        ));
        assert_eq!(game.tableau[1].len(), 3);
    }

    #[test]
    fn pyramid_checks_both_supports_and_pair_total() {
        let mut game = Game::new(Variant::Pyramid, 18);
        game.layout[15] = Some(spade(13));
        game.layout[21] = Some(spade(3));
        game.layout[22] = Some(heart(10));
        assert!(!game.play(Move::Clear(Place::Layout(15))));
        assert!(transfer(&mut game, Place::Layout(21), Place::Layout(22)));
        assert!(game.play(Move::Clear(Place::Layout(15))));
    }

    #[test]
    fn tripeaks_wraps_ranks_and_unblocks_the_correct_cards() {
        let mut game = Game::new(Variant::TriPeaks, 18);
        game.layout[9] = Some(spade(11));
        game.layout[18] = Some(heart(13));
        game.layout[19] = Some(heart(12));
        game.waste = vec![spade(1)];
        assert!(!game.play(Move::Clear(Place::Layout(9))));
        for index in [18, 19, 9] {
            assert!(game.play(Move::Clear(Place::Layout(index))));
        }
        assert_eq!(game.waste.last(), Some(&spade(11)));
    }

    #[test]
    fn pyramid_stalls_only_without_a_draw_king_or_available_pair() {
        let mut game = Game::new(Variant::Pyramid, 18);
        game.layout.fill(None);
        game.layout[21] = Some(spade(6));
        assert!(!game.layout_stalled());

        game.stock.clear();
        assert!(game.layout_stalled());

        game.layout[22] = Some(heart(7));
        assert!(!game.layout_stalled());
        game.layout[22] = None;
        game.waste.push(heart(7));
        assert!(!game.layout_stalled());

        game.waste.clear();
        game.layout[15] = Some(spade(13));
        assert!(game.layout_stalled());
        game.layout[21] = None;
        assert!(!game.layout_stalled());
    }

    #[test]
    fn tripeaks_stalls_only_without_a_draw_or_available_adjacent_rank() {
        let mut game = Game::new(Variant::TriPeaks, 18);
        game.layout.fill(None);
        game.layout[0] = Some(heart(13));
        game.layout[3] = Some(spade(4));
        game.waste = vec![spade(1)];
        assert!(!game.layout_stalled());

        game.stock.clear();
        assert!(game.layout_stalled());
        game.layout[3] = None;
        assert!(!game.layout_stalled());
        assert!(game.play(Move::Clear(Place::Layout(0))));
        assert!(game.won());
        assert!(!game.layout_stalled());
    }
}
