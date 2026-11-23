use corner_table_reversi::{Difficulty, Disc, Outcome, Played, Position, Square, Turn};
use serde::{Deserialize, Serialize};

use crate::storage::{AttemptId, Outcome as RecordOutcome, ResultMetric, Session};

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
    #[serde(default)]
    attempts: [Option<AttemptId>; 3],
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
            attempts: std::array::from_fn(|i| rounds[i].attempt),
        }
    }

    pub(super) fn restore(self) -> Option<(Opponent, DisplayOptions, [Round; 3])> {
        if self.version != SAVE_VERSION {
            return None;
        }
        let rounds = std::array::from_fn(|i| {
            Round::replay(&self.moves[i])
                .map(|mut round| {
                    round.attempt = self.attempts[i];
                    round
                })
                .unwrap_or_default()
        });
        Some((self.opponent, self.display, rounds))
    }
}

#[derive(Default)]
pub(super) struct Round {
    pub(super) position: Position,
    history: Vec<Ply>,
    attempt: Option<AttemptId>,
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

    pub(super) fn remember_attempt(&mut self) {
        self.attempt.get_or_insert_with(AttemptId::default);
    }

    pub(super) fn record_result(&self, session: &Session, opponent: Opponent) {
        let (Some(attempt), Turn::Finished(result)) = (self.attempt, self.position.turn()) else {
            return;
        };
        if opponent == Opponent::Local {
            return;
        }
        let outcome = match result {
            Outcome::Won(Disc::Black) => RecordOutcome::Won,
            Outcome::Won(Disc::White) => RecordOutcome::Lost,
            Outcome::Draw => RecordOutcome::Draw,
        };
        session.record_result(
            attempt,
            opponent.label(),
            outcome,
            ResultMetric::Discs(self.position.score(Disc::Black).into()),
        );
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
        for round in &mut rounds {
            round.remember_attempt();
        }
        let sv = Saved::capture(Opponent::Strategist, DisplayOptions::default(), &rounds);
        let json = serde_json::to_string(&sv).unwrap();
        let de: Saved = serde_json::from_str(&json).unwrap();
        let (opponent, display, mut restored) = de.restore().unwrap();
        assert_eq!(opponent, Opponent::Strategist);
        assert!(display.coordinates && display.hints);
        for (source, target) in rounds.iter().zip(&restored) {
            assert_eq!(source.position, target.position);
            assert_eq!(source.last(), target.last());
            assert!(source.attempt.is_some() && source.attempt == target.attempt);
        }
        assert!(restored[2].undo(Opponent::Strategist));
        assert_eq!(restored[2].history.len(), 8);
        assert!(restored[2].attempt == rounds[2].attempt);
        restored[2] = Round::default();
        restored[2].remember_attempt();
        assert!(restored[2].attempt != rounds[2].attempt);
    }

    #[test]
    fn older_rounds_resume_without_an_attempt_id() {
        let saved: Saved = serde_json::from_value(serde_json::json!({
            "version": 1,
            "opponent": "casual",
            "display": {"hints": true, "coordinates": true},
            "moves": [[], [19], []]
        }))
        .unwrap();
        let (_, _, rounds) = saved.restore().unwrap();
        let casual = &rounds[Opponent::Casual.slot()];
        assert_eq!(casual.position.turn(), Turn::Play(Disc::White));
        assert!(casual.attempt.is_none());
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
