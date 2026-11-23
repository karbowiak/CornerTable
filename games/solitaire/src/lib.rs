mod freecell;
mod klondike;
mod pyramid;
mod spider;
mod tripeaks;

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

#[derive(Clone, Debug)]
pub struct Game {
    pub variant: Variant,
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
        let mut game = Self {
            variant,
            tableau: Vec::new(),
            stock: Vec::new(),
            waste: Vec::new(),
            foundations: Default::default(),
            cells: [None; 4],
            layout: Vec::new(),
            runs: 0,
            moves: 0,
        };
        let deck = shuffled_deck(seed, variant == Variant::Spider);
        match variant {
            Variant::Klondike => klondike::deal(&mut game, deck),
            Variant::Spider => spider::deal(&mut game, deck),
            Variant::FreeCell => freecell::deal(&mut game, deck),
            Variant::Pyramid => pyramid::deal(&mut game, deck),
            Variant::TriPeaks => tripeaks::deal(&mut game, deck),
        }
        game
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

    pub fn foundation_count(&self) -> usize {
        self.foundations.iter().map(Vec::len).sum()
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

    fn draw_waste(&mut self, recycle: bool) -> bool {
        if let Some(card) = self.stock.pop() {
            self.waste.push(card);
        } else if recycle && !self.waste.is_empty() {
            self.stock.extend(self.waste.drain(..).rev());
        } else {
            return false;
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

fn shuffled_deck(mut seed: u64, spider: bool) -> Vec<Card> {
    let suits = if spider {
        &[Suit::Spades; 8][..]
    } else {
        &Suit::ALL
    };
    let mut deck: Vec<_> = suits
        .iter()
        .flat_map(|&suit| (1..=13).map(move |rank| Card { suit, rank }))
        .collect();
    for index in (1..deck.len()).rev() {
        seed = seed.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = seed;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^= value >> 31;
        deck.swap(index, (value % (index as u64 + 1)) as usize);
    }
    deck
}

fn foundation_accepts(game: &Game, index: usize, card: Card) -> bool {
    game.foundations
        .get(index)
        .is_some_and(|stack| Suit::ALL[index] == card.suit && card.rank as usize == stack.len() + 1)
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
}
