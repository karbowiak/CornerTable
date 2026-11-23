//! Four-player Hearts with a deterministic deal and computer opponents.

mod computer;

pub use computer::Difficulty;

pub const MATCH_LIMIT: u16 = 100;
pub const TRICKS_PER_HAND: usize = 13;
const POINTS_IN_HAND: u8 = 26;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Suit {
    Clubs,
    Diamonds,
    Spades,
    Hearts,
}

impl Suit {
    pub const ALL: [Self; 4] = [Self::Clubs, Self::Diamonds, Self::Spades, Self::Hearts];
}

/// A card's stable ID runs from 0 (two of clubs) to 51 (ace of hearts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Card(u8);

impl Card {
    pub const TWO_OF_CLUBS: Self = Self(0);
    pub const QUEEN_OF_SPADES: Self = Self(36);

    pub fn from_id(id: u8) -> Option<Self> {
        (id < 52).then_some(Self(id))
    }

    pub fn id(self) -> u8 {
        self.0
    }

    pub fn suit(self) -> Suit {
        Suit::ALL[(self.0 / 13) as usize]
    }

    /// Ordered from two (2) to ace (14).
    pub fn rank(self) -> u8 {
        self.0 % 13 + 2
    }

    pub fn points(self) -> u8 {
        match self {
            Self::QUEEN_OF_SPADES => 13,
            _ if self.suit() == Suit::Hearts => 1,
            _ => 0,
        }
    }
}

/// Seats in clockwise order, starting at the bottom of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seat {
    You,
    West,
    North,
    East,
}

impl Seat {
    pub const ALL: [Self; 4] = [Self::You, Self::West, Self::North, Self::East];

    pub const fn index(self) -> usize {
        self as usize
    }

    fn advance(self, clockwise: usize) -> Self {
        Self::ALL[(self.index() + clockwise) % 4]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    Left,
    Right,
    Across,
    Keep,
}

impl Pass {
    fn for_hand(hand: usize) -> Self {
        [Self::Left, Self::Right, Self::Across, Self::Keep][(hand - 1) % 4]
    }

    pub fn recipient(self, from: Seat) -> Seat {
        from.advance(match self {
            Self::Left => 1,
            Self::Right => 3,
            Self::Across => 2,
            Self::Keep => 0,
        })
    }

    pub fn sender(self, to: Seat) -> Seat {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            direction => direction,
        }
        .recipient(to)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayedCard {
    pub seat: Seat,
    pub card: Card,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrickResult {
    pub winner: Seat,
    pub points: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandResult {
    pub points: [u8; 4],
    pub moon: Option<Seat>,
    pub winner: Option<Seat>,
    pub tied_at_limit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Passing,
    Playing(Seat),
    TrickComplete(TrickResult),
    HandComplete(HandResult),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Pass([Card; 3]),
    Play(Card),
    Collect,
    Deal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    seed: u64,
    hand_number: usize,
    hands: [Vec<Card>; 4],
    phase: Phase,
    trick: Vec<PlayedCard>,
    taken: Vec<PlayedCard>,
    last_trick: Option<TrickResult>,
    received: Option<[Card; 3]>,
    hearts_broken: bool,
    hand_points: [u8; 4],
    scores: [u16; 4],
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let mut game = Self {
            seed,
            hand_number: 1,
            hands: Default::default(),
            phase: Phase::Passing,
            trick: Vec::with_capacity(4),
            taken: Vec::with_capacity(52),
            last_trick: None,
            received: None,
            hearts_broken: false,
            hand_points: [0; 4],
            scores: [0; 4],
        };
        game.deal();
        game
    }

    pub fn hand_number(&self) -> usize {
        self.hand_number
    }

    pub fn pass_direction(&self) -> Pass {
        Pass::for_hand(self.hand_number)
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn hand(&self, seat: Seat) -> &[Card] {
        &self.hands[seat.index()]
    }

    pub fn trick(&self) -> &[PlayedCard] {
        &self.trick
    }

    pub fn trick_number(&self) -> usize {
        (self.taken.len() / 4 + 1).min(TRICKS_PER_HAND)
    }

    pub fn last_trick(&self) -> Option<TrickResult> {
        self.last_trick
    }

    pub fn received(&self) -> Option<[Card; 3]> {
        self.received
    }

    pub fn hearts_broken(&self) -> bool {
        self.hearts_broken
    }

    pub fn hand_points(&self) -> [u8; 4] {
        self.hand_points
    }

    pub fn scores(&self) -> [u16; 4] {
        self.scores
    }

    pub fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Pass(cards) => self.pass(cards),
            Action::Play(card) => self.play(card),
            Action::Collect => self.collect(),
            Action::Deal if matches!(self.phase, Phase::HandComplete(result) if result.winner.is_none()) =>
            {
                self.hand_number += 1;
                self.deal();
                true
            }
            Action::Deal => false,
        }
    }

    pub fn legal_plays(&self) -> Vec<Card> {
        let Phase::Playing(seat) = self.phase else {
            return Vec::new();
        };
        let hand = self.hand(seat);
        let first_trick = self.taken.is_empty();
        if let Some(lead) = self.trick.first() {
            let following: Vec<_> = hand
                .iter()
                .copied()
                .filter(|card| card.suit() == lead.card.suit())
                .collect();
            if !following.is_empty() {
                return following;
            }
            if first_trick {
                let without_points: Vec<_> = hand
                    .iter()
                    .copied()
                    .filter(|card| card.points() == 0)
                    .collect();
                if !without_points.is_empty() {
                    return without_points;
                }
            }
        } else if first_trick {
            return vec![Card::TWO_OF_CLUBS];
        } else if !self.hearts_broken {
            let other_suits: Vec<_> = hand
                .iter()
                .copied()
                .filter(|card| card.suit() != Suit::Hearts)
                .collect();
            if !other_suits.is_empty() {
                return other_suits;
            }
        }
        hand.to_vec()
    }

    /// Rates the current player's legal cards using their hand and public play.
    pub fn suggested_play(&self) -> Option<Card> {
        self.suggested_play_at(Difficulty::Medium)
    }

    pub fn suggested_play_at(&self, difficulty: Difficulty) -> Option<Card> {
        computer::play(self, difficulty)
    }

    fn deal(&mut self) {
        let mut deck = std::array::from_fn::<_, 52, _>(|id| Card(id as u8));
        for index in (1..deck.len()).rev() {
            let other = (next_random(&mut self.seed) % (index as u64 + 1)) as usize;
            deck.swap(index, other);
        }
        for hand in &mut self.hands {
            hand.clear();
        }
        for (index, card) in deck.into_iter().enumerate() {
            self.hands[index % 4].push(card);
        }
        self.hands.iter_mut().for_each(|hand| hand.sort_unstable());
        self.trick.clear();
        self.taken.clear();
        self.last_trick = None;
        self.received = None;
        self.hearts_broken = false;
        self.hand_points = [0; 4];
        self.phase = if self.pass_direction() == Pass::Keep {
            Phase::Playing(self.opener())
        } else {
            Phase::Passing
        };
    }

    fn opener(&self) -> Seat {
        Seat::ALL
            .into_iter()
            .find(|&seat| self.hand(seat).contains(&Card::TWO_OF_CLUBS))
            .expect("the opening card belongs to one of the four hands")
    }

    fn pass(&mut self, cards: [Card; 3]) -> bool {
        let [a, b, c] = cards;
        if self.phase != Phase::Passing
            || a == b
            || a == c
            || b == c
            || cards
                .iter()
                .any(|card| !self.hand(Seat::You).contains(card))
        {
            return false;
        }

        // All four players choose from their original hands.
        let outgoing = Seat::ALL.map(|seat| {
            if seat == Seat::You {
                cards
            } else {
                computer::pass(self.hand(seat))
            }
        });
        for (hand, sent) in self.hands.iter_mut().zip(outgoing) {
            hand.retain(|card| !sent.contains(card));
        }
        let direction = self.pass_direction();
        for seat in Seat::ALL {
            let hand = &mut self.hands[direction.recipient(seat).index()];
            hand.extend(outgoing[seat.index()]);
            hand.sort_unstable();
        }
        self.received = Some(outgoing[direction.sender(Seat::You).index()]);
        self.phase = Phase::Playing(self.opener());
        true
    }

    fn play(&mut self, card: Card) -> bool {
        let Phase::Playing(seat) = self.phase else {
            return false;
        };
        if !self.legal_plays().contains(&card) {
            return false;
        }
        self.hands[seat.index()].retain(|&held| held != card);
        self.hearts_broken |= card.suit() == Suit::Hearts;
        self.trick.push(PlayedCard { seat, card });
        self.phase = if self.trick.len() == 4 {
            Phase::TrickComplete(TrickResult {
                winner: winning_card(&self.trick)
                    .expect("four cards fill a trick")
                    .seat,
                points: self.trick.iter().map(|play| play.card.points()).sum(),
            })
        } else {
            Phase::Playing(seat.advance(1))
        };
        true
    }

    fn collect(&mut self) -> bool {
        let Phase::TrickComplete(result) = self.phase else {
            return false;
        };
        self.hand_points[result.winner.index()] += result.points;
        self.last_trick = Some(result);
        self.taken.append(&mut self.trick);
        if self.taken.len() == 52 {
            let result = score_hand(self.hand_points, &mut self.scores);
            self.phase = Phase::HandComplete(result);
        } else {
            self.phase = Phase::Playing(result.winner);
        }
        true
    }
}

fn score_hand(captured: [u8; 4], scores: &mut [u16; 4]) -> HandResult {
    let moon = Seat::ALL
        .into_iter()
        .find(|seat| captured[seat.index()] == POINTS_IN_HAND);
    let points = match moon {
        Some(shooter) => Seat::ALL.map(|seat| if seat == shooter { 0 } else { POINTS_IN_HAND }),
        None => captured,
    };
    for (score, points) in scores.iter_mut().zip(points) {
        *score += u16::from(points);
    }
    let at_limit = scores.iter().any(|&score| score >= MATCH_LIMIT);
    let winner = at_limit
        .then(|| {
            let low_score = scores.iter().copied().min().expect("four scores");
            let mut lows = Seat::ALL
                .into_iter()
                .filter(|seat| scores[seat.index()] == low_score);
            let low = lows.next();
            if lows.next().is_none() { low } else { None }
        })
        .flatten();
    HandResult {
        points,
        moon,
        winner,
        tied_at_limit: at_limit && winner.is_none(),
    }
}

fn winning_card(trick: &[PlayedCard]) -> Option<PlayedCard> {
    let suit = trick.first()?.card.suit();
    trick
        .iter()
        .copied()
        .filter(|play| play.card.suit() == suit)
        .max_by_key(|play| play.card.rank())
}

// SplitMix64 provides the deterministic deal sequence.
fn next_random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = *seed;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests;
