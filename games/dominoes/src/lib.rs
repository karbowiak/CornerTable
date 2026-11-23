//! Two-player double-six Draw Dominoes.
//!
//! Deal seven tiles to each player. The highest dealt double opens automatically.
//! If neither hand contains a double, use the highest pip total, breaking ties
//! with the larger individual pip. Play then alternates. The chain has two ends
//! throughout the hand, including when a double is played.
//!
//! A player without a match draws until a tile fits or the boneyard is empty.
//! A player with no match and an empty boneyard passes. The hand ends when a
//! player goes out or both pass in succession. On a block, the lower pip total
//! wins. Either kind of win scores the difference in the remaining pip totals.

use std::{collections::VecDeque, fmt};

pub const MAX_PIP: u8 = 6;
pub const TILE_COUNT: usize = 28;
pub const HAND_SIZE: usize = 7;

/// Copy for Corner Table's shared How to Play guide.
pub const GUIDE_SECTIONS: &[(&str, &str)] = &[
    (
        "Deal and open",
        "Deal seven tiles to each player from a 28-tile double-six set. The remaining fourteen form the boneyard. The highest dealt double opens automatically. If neither hand has a double, the tile with the most pips opens, with the higher single face breaking a tie. The other player takes the next turn.",
    ),
    (
        "Match an end",
        "Select a tile and choose either exposed end with a matching face. The table turns the tile to join the matching pips. Doubles extend the same chain. Use the arrow keys to browse, Enter to select, and L or R to play at the chosen end.",
    ),
    (
        "Draw or pass",
        "If no tile in your hand fits, choose Draw tiles or press D. Draw until the first tile fits or the boneyard is empty. Play the matching tile on the same turn. Pass with P only when the boneyard is empty and you still have no match.",
    ),
    (
        "Score a round",
        "Play your last tile to win. Two consecutive passes block the round, and the player with fewer pips left wins. The winner scores the difference between the hands' remaining pip totals. A tied block awards zero. Scores carry across rounds in each mode.",
    ),
    (
        "Share the table",
        "In Two players mode, both hands are covered between turns. The next player reveals their own hand when they are ready. Each mode saves its own round and scores.",
    ),
];

/// A physical tile, stored with the lower value first.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Tile {
    low: u8,
    high: u8,
}

impl Tile {
    pub const fn new(a: u8, b: u8) -> Option<Self> {
        if a > MAX_PIP || b > MAX_PIP {
            return None;
        }
        let (low, high) = if a <= b { (a, b) } else { (b, a) };
        Some(Self { low, high })
    }

    pub const fn pips(self) -> [u8; 2] {
        [self.low, self.high]
    }

    pub const fn total(self) -> u8 {
        self.low + self.high
    }

    pub const fn is_double(self) -> bool {
        self.low == self.high
    }

    pub const fn other(self, pip: u8) -> Option<u8> {
        if self.low == pip {
            Some(self.high)
        } else if self.high == pip {
            Some(self.low)
        } else {
            None
        }
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}–{}", self.low, self.high)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Player {
    One,
    Two,
}

impl Player {
    pub const ALL: [Self; 2] = [Self::One, Self::Two];

    pub const fn index(self) -> usize {
        match self {
            Self::One => 0,
            Self::Two => 1,
        }
    }

    pub const fn opponent(self) -> Self {
        match self {
            Self::One => Self::Two,
            Self::Two => Self::One,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum End {
    Left,
    Right,
}

impl End {
    pub const ALL: [Self; 2] = [Self::Left, Self::Right];

    pub const fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

/// The tile's faces in logical chain order. A displayed row can reverse the
/// visual order, while these faces always read from the left end to the right.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Placed {
    pub tile: Tile,
    pub faces: [u8; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Play(Tile, End),
    Draw,
    Pass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Play,
    Draw,
    Pass,
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Opened {
        player: Player,
        tile: Tile,
    },
    Played {
        player: Player,
        tile: Tile,
        end: End,
    },
    Drew {
        player: Player,
        count: usize,
    },
    Passed {
        player: Player,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Finish {
    WentOut,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Outcome {
    pub finish: Finish,
    pub winner: Option<Player>,
    pub pips: [u16; 2],
    pub points: u16,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Round {
    hands: [Vec<Tile>; 2],
    boneyard: Vec<Tile>,
    chain: VecDeque<Placed>,
    player: Player,
    passes: u8,
    outcome: Option<Outcome>,
    last_placement: Tile,
    last_event: Event,
}

impl Round {
    /// Start a reproducible deal. The shuffle is part of the saved game format.
    pub fn new(seed: u64) -> Self {
        let mut deck = Vec::with_capacity(TILE_COUNT);
        for high in 0..=MAX_PIP {
            for low in 0..=high {
                deck.push(Tile { low, high });
            }
        }
        shuffle(&mut deck, seed);
        Self::deal(deck)
    }

    fn deal(mut deck: Vec<Tile>) -> Self {
        let mut hands = std::array::from_fn(|_| Vec::with_capacity(HAND_SIZE));
        for _ in 0..HAND_SIZE {
            for hand in &mut hands {
                hand.push(deck.pop().unwrap());
            }
        }
        let (opener, tile) = Player::ALL
            .into_iter()
            .flat_map(|player| {
                hands[player.index()]
                    .iter()
                    .map(move |&tile| (player, tile))
            })
            .max_by_key(|&(_, tile)| (tile.is_double(), tile.total(), tile.high))
            .unwrap();
        hands[opener.index()].retain(|&held| held != tile);
        Self {
            hands,
            boneyard: deck,
            chain: VecDeque::from([Placed {
                tile,
                faces: tile.pips(),
            }]),
            player: opener.opponent(),
            passes: 0,
            outcome: None,
            last_placement: tile,
            last_event: Event::Opened {
                player: opener,
                tile,
            },
        }
    }

    pub fn hand(&self, player: Player) -> &[Tile] {
        &self.hands[player.index()]
    }

    pub fn boneyard_count(&self) -> usize {
        self.boneyard.len()
    }

    pub fn chain(&self) -> &VecDeque<Placed> {
        &self.chain
    }

    pub fn ends(&self) -> [u8; 2] {
        [
            self.chain[0].faces[0],
            self.chain[self.chain.len() - 1].faces[1],
        ]
    }

    pub const fn player(&self) -> Player {
        self.player
    }

    pub const fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    pub const fn last_placement(&self) -> Tile {
        self.last_placement
    }

    pub const fn last_event(&self) -> Event {
        self.last_event
    }

    pub fn phase(&self) -> Phase {
        if self.outcome.is_some() {
            Phase::Finished
        } else if self
            .hand(self.player)
            .iter()
            .any(|tile| self.fits(*tile).contains(&true))
        {
            Phase::Play
        } else if self.boneyard.is_empty() {
            Phase::Pass
        } else {
            Phase::Draw
        }
    }

    /// Match the tile against the left and right exposed pips respectively.
    pub fn fits(&self, tile: Tile) -> [bool; 2] {
        self.ends().map(|pip| tile.other(pip).is_some())
    }

    pub fn preview(&self, tile: Tile, end: End) -> Option<Placed> {
        let matching = self.ends()[end.index()];
        let other = tile.other(matching)?;
        let faces = match end {
            End::Left => [other, matching],
            End::Right => [matching, other],
        };
        Some(Placed { tile, faces })
    }

    /// Apply a legal action for the current player. Invalid actions leave the
    /// entire state unchanged.
    pub fn act(&mut self, action: Action) -> Option<Event> {
        let player = self.player;
        let event = match action {
            Action::Play(tile, end) if self.outcome.is_none() => {
                let placed = self.preview(tile, end)?;
                let hand = &mut self.hands[player.index()];
                let index = hand.iter().position(|&held| held == tile)?;
                hand.remove(index);
                match end {
                    End::Left => self.chain.push_front(placed),
                    End::Right => self.chain.push_back(placed),
                }
                self.passes = 0;
                self.last_placement = tile;
                if hand.is_empty() {
                    self.finish(Finish::WentOut);
                } else {
                    self.player = player.opponent();
                }
                Event::Played { player, tile, end }
            }
            Action::Draw if self.phase() == Phase::Draw => {
                let mut count = 0;
                while let Some(tile) = self.boneyard.pop() {
                    self.hands[player.index()].push(tile);
                    count += 1;
                    if self.fits(tile).contains(&true) {
                        break;
                    }
                }
                Event::Drew { player, count }
            }
            Action::Pass if self.phase() == Phase::Pass => {
                self.passes += 1;
                if self.passes == 2 {
                    self.finish(Finish::Blocked);
                } else {
                    self.player = player.opponent();
                }
                Event::Passed { player }
            }
            _ => return None,
        };
        self.last_event = event;
        Some(event)
    }

    fn finish(&mut self, finish: Finish) {
        let pips = self
            .hands
            .each_ref()
            .map(|hand| hand.iter().map(|tile| u16::from(tile.total())).sum::<u16>());
        let winner = if pips[0] < pips[1] {
            Some(Player::One)
        } else if pips[1] < pips[0] {
            Some(Player::Two)
        } else if finish == Finish::WentOut {
            // The other player might still hold only the blank double.
            Some(self.player)
        } else {
            None
        };
        self.outcome = Some(Outcome {
            finish,
            winner,
            pips,
            points: pips[0].abs_diff(pips[1]),
        });
    }
}

/// Choose a legal play using only the active player's hand and public ends.
/// Favors shedding pips and keeping at least one useful matching tile.
pub fn computer_action(round: &Round) -> Option<Action> {
    match round.phase() {
        Phase::Finished => None,
        Phase::Draw => Some(Action::Draw),
        Phase::Pass => Some(Action::Pass),
        Phase::Play => {
            let hand = round.hand(round.player());
            let ends = round.ends();
            hand.iter()
                .flat_map(|&tile| {
                    End::ALL.into_iter().filter_map(move |end| {
                        let mut next = ends;
                        next[end.index()] = tile.other(ends[end.index()])?;
                        let options = hand
                            .iter()
                            .filter(|&&held| {
                                held != tile && next.iter().any(|&pip| held.other(pip).is_some())
                            })
                            .count();
                        let weight = usize::from(tile.total()) * 3 + options.min(5) * 2;
                        Some(((weight, options, tile.is_double()), Action::Play(tile, end)))
                    })
                })
                .max_by_key(|&(rank, _)| rank)
                .map(|(_, action)| action)
        }
    }
}

fn shuffle<T>(values: &mut [T], mut state: u64) {
    for i in (1..values.len()).rev() {
        // Rejection avoids modulo bias. SplitMix64 also accepts the zero seed.
        let bound = (i + 1) as u64;
        let threshold = bound.wrapping_neg() % bound;
        let j = loop {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            let sample = z ^ (z >> 31);
            if sample >= threshold {
                break (sample % bound) as usize;
            }
        };
        values.swap(i, j);
    }
}

#[cfg(test)]
mod tests;
