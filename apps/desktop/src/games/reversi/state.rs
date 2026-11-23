use corner_table_reversi::{Difficulty, Disc, Played, Position, Square};
use serde::{Deserialize, Serialize};

const SAVE_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Opponent {
    #[default]
    Local,
    Casual,
    Strategist,
}

impl Opponent {
    pub(super) const ALL: [Self; 3] = [Self::Local, Self::Casual, Self::Strategist];

    pub(super) const fn slot(self) -> usize {
        match self {
            Self::Local => 0,
            Self::Casual => 1,
            Self::Strategist => 2,
        }
    }

    pub(super) fn from_id(id: &str) -> Self {
        match id {
            "casual" => Self::Casual,
            "strategist" => Self::Strategist,
            _ => Self::Local,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Local => "Two players",
            Self::Casual => "Casual",
            Self::Strategist => "Strategist",
        }
    }

    pub(super) const fn difficulty(self) -> Option<Difficulty> {
        match self {
            Self::Local => None,
            Self::Casual => Some(Difficulty::Casual),
            Self::Strategist => Some(Difficulty::Strategist),
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(super) struct DisplayOptions {
    pub(super) hints: bool,
    pub(super) coordinates: bool,
}

impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            hints: true,
            coordinates: true,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub(super) struct Saved {
    version: u8,
    opponent: Opponent,
    display: DisplayOptions,
    moves: [Vec<u8>; 3],
}

impl Saved {
    pub(super) fn capture(
        opponent: Opponent,
        display: DisplayOptions,
        rounds: &[Round; 3],
    ) -> Self {
        Self {
            version: SAVE_VERSION,
            opponent,
            display,
            moves: std::array::from_fn(|i| {
                rounds[i]
                    .history
                    .iter()
                    .map(|ply| ply.played.square.index())
                    .collect()
            }),
        }
    }

    pub(super) fn restore(self) -> Option<(Opponent, DisplayOptions, [Round; 3])> {
        if self.version != SAVE_VERSION {
            return None;
        }
        Some((
            self.opponent,
            self.display,
            self.moves
                .map(|moves| Round::replay(&moves).unwrap_or_default()),
        ))
    }
}

#[derive(Default)]
pub(super) struct Round {
    pub(super) position: Position,
    history: Vec<Ply>,
}

struct Ply {
    before: Position,
    played: Played,
}

impl Round {
    fn replay(moves: &[u8]) -> Option<Self> {
        if moves.len() > 60 {
            return None;
        }
        let mut round = Self::default();
        for &index in moves {
            round.play(Square::from_index(index)?)?;
        }
        Some(round)
    }

    pub(super) fn play(&mut self, square: Square) -> Option<Played> {
        let before = self.position;
        let played = self.position.play(square)?;
        self.history.push(Ply { before, played });
        Some(played)
    }

    pub(super) fn last(&self) -> Option<Played> {
        self.history.last().map(|ply| ply.played)
    }

    pub(super) fn preceding_pass(&self) -> Option<Disc> {
        self.history.iter().rev().nth(1)?.played.passed
    }

    pub(super) fn undo(&mut self, opponent: Opponent) -> bool {
        let index = match opponent {
            Opponent::Local => self.history.len().checked_sub(1),
            _ => self
                .history
                .iter()
                .rposition(|ply| ply.played.disc == Disc::Black),
        };
        let Some(index) = index else {
            return false;
        };
        self.position = self.history[index].before;
        self.history.truncate(index);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corner_table_reversi::Turn;

    fn place_first(round: &mut Round, count: usize) {
        for _ in 0..count {
            let square = round.position.legal_moves().next().unwrap();
            round.play(square).unwrap();
        }
    }

    #[test]
    fn local_undo_restores_one_ply_and_solo_restores_the_last_human_turn() {
        let mut round = Round::default();
        place_first(&mut round, 2);
        assert!(round.undo(Opponent::Local));
        assert_eq!(round.position.turn(), Turn::Play(Disc::White));
        place_first(&mut round, 1);
        assert!(round.undo(Opponent::Strategist));
        assert_eq!(round.position, Position::new());
        assert!(!round.undo(Opponent::Strategist));
    }

    #[test]
    fn solo_undo_also_works_while_waiting_for_the_computer() {
        let mut round = Round::default();
        place_first(&mut round, 1);
        assert!(round.undo(Opponent::Casual));
        assert_eq!(round.position, Position::new());
    }

    #[test]
    fn saved_matches_replay_independently_and_keep_undo() {
        let mut rounds = std::array::from_fn(|_| Round::default());
        place_first(&mut rounds[0], 3);
        place_first(&mut rounds[1], 5);
        place_first(&mut rounds[2], 10);
        let sv = Saved::capture(Opponent::Strategist, DisplayOptions::default(), &rounds);
        let json = serde_json::to_string(&sv).unwrap();
        let de: Saved = serde_json::from_str(&json).unwrap();
        let (opponent, display, mut restored) = de.restore().unwrap();
        assert_eq!(opponent, Opponent::Strategist);
        assert!(display.coordinates && display.hints);
        for (source, target) in rounds.iter().zip(&restored) {
            assert_eq!(source.position, target.position);
            assert_eq!(source.last(), target.last());
        }
        assert!(restored[2].undo(Opponent::Strategist));
        assert_eq!(restored[2].history.len(), 8);
    }

    #[test]
    fn an_invalid_save_slot_leaves_other_rounds_intact() {
        let mut rounds = std::array::from_fn(|_| Round::default());
        place_first(&mut rounds[1], 5);
        let mut save = Saved::capture(Opponent::Local, DisplayOptions::default(), &rounds);
        save.moves[0].push(255);
        let (_, _, restored) = save.restore().unwrap();
        assert_eq!(restored[0].position, Position::new());
        assert_eq!(restored[1].position, rounds[1].position);
    }
}
