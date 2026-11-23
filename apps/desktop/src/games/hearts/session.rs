use corner_table_hearts::{Action, Card, Difficulty, Game, Pass, Phase, Seat};
use serde::{Deserialize, Serialize};

use crate::{
    games::GameLaunch,
    storage::{AttemptId, Outcome, ResultMetric, Session},
};

// A change to deals, computer passes or the move format needs a new version.
const VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "move")]
pub(super) enum Turn {
    Pass { cards: [u8; 3] },
    Begin,
    Play { card: u8 },
    Collect,
    Deal,
}

#[derive(Serialize, Deserialize)]
struct SavedMatch {
    version: u32,
    seed: u64,
    turns: Vec<Turn>,
    selected: Vec<u8>,
    pause_tricks: bool,
    #[serde(default)]
    difficulty: Difficulty,
    #[serde(default)]
    attempt: Option<AttemptId>,
}

impl SavedMatch {
    fn new(seed: u64, pause_tricks: bool) -> Self {
        Self {
            version: VERSION,
            seed,
            turns: Vec::new(),
            selected: Vec::new(),
            pause_tricks,
            difficulty: Difficulty::default(),
            attempt: Some(AttemptId::default()),
        }
    }

    fn restore(mut self) -> Option<(Round, Self)> {
        if self.version != VERSION {
            return None;
        }
        let mut round = Round::new(self.seed);
        for &turn in &self.turns {
            if !round.apply(turn) {
                return None;
            }
        }
        // A damaged draft selection should not invalidate a playable match.
        let mut selected = Vec::new();
        if round.game.phase() == Phase::Passing {
            for code in self.selected {
                if let Some(card) = Card::from_id(code)
                    && !selected.contains(&code)
                    && round.game.hand(Seat::You).contains(&card)
                    && selected.len() < 3
                {
                    selected.push(code);
                }
            }
        }
        self.selected = selected;
        Some((round, self))
    }
}

struct Round {
    game: Game,
    ready_to_open: bool,
}

impl Round {
    fn new(seed: u64) -> Self {
        Self {
            game: Game::new(seed),
            ready_to_open: false,
        }
    }

    fn apply(&mut self, turn: Turn) -> bool {
        if self.ready_to_open {
            if matches!(turn, Turn::Begin) {
                self.ready_to_open = false;
                return true;
            }
            return false;
        }
        let action = match turn {
            Turn::Pass { cards: [a, b, c] } => {
                let [Some(a), Some(b), Some(c)] = [a, b, c].map(Card::from_id) else {
                    return false;
                };
                Action::Pass([a, b, c])
            }
            Turn::Begin => return false,
            Turn::Play { card } => {
                let Some(card) = Card::from_id(card) else {
                    return false;
                };
                Action::Play(card)
            }
            Turn::Collect => Action::Collect,
            Turn::Deal => Action::Deal,
        };
        if !self.game.apply(action) {
            return false;
        }
        self.ready_to_open = matches!(action, Action::Pass(_))
            || (action == Action::Deal && self.game.pass_direction() == Pass::Keep);
        true
    }

    fn completed_result(&self) -> Option<(Outcome, ResultMetric)> {
        let Phase::HandComplete(result) = self.game.phase() else {
            return None;
        };
        // A tie for the lowest score at the match limit requires another hand.
        let outcome = if result.winner? == Seat::You {
            Outcome::Won
        } else {
            Outcome::Lost
        };
        let score = self.game.scores()[Seat::You.index()];
        Some((outcome, ResultMetric::PenaltyPoints(score.into())))
    }
}

pub(super) struct TableSession {
    store: Session,
    saved: SavedMatch,
    round: Round,
}

impl TableSession {
    pub(super) fn open(launch: &GameLaunch) -> Self {
        let store = Session::new(launch);
        let (round, saved) = store
            .load::<SavedMatch>()
            .and_then(SavedMatch::restore)
            .unwrap_or_else(|| {
                let seed = launch.seed();
                (Round::new(seed), SavedMatch::new(seed, false))
            });
        let table = Self {
            store,
            saved,
            round,
        };
        table.save();
        table.record_result();
        table
    }

    pub(super) fn game(&self) -> &Game {
        &self.round.game
    }

    pub(super) fn ready_to_open(&self) -> bool {
        self.round.ready_to_open
    }

    pub(super) fn selected(&self) -> Vec<Card> {
        self.saved
            .selected
            .iter()
            .filter_map(|&code| Card::from_id(code))
            .collect()
    }

    pub(super) fn toggle(&mut self, card: Card) {
        if self.game().phase() != Phase::Passing || !self.game().hand(Seat::You).contains(&card) {
            return;
        }
        let code = card.id();
        if let Some(index) = self.saved.selected.iter().position(|&held| held == code) {
            self.saved.selected.remove(index);
        } else if self.saved.selected.len() < 3 {
            self.saved.selected.push(code);
        } else {
            return;
        }
        self.save();
    }

    pub(super) fn pass_turn(&self) -> Option<Turn> {
        Some(Turn::Pass {
            cards: self.saved.selected.as_slice().try_into().ok()?,
        })
    }

    pub(super) fn pause_tricks(&self) -> bool {
        self.saved.pause_tricks
    }

    pub(super) fn difficulty(&self) -> Difficulty {
        self.saved.difficulty
    }

    pub(super) fn set_difficulty(&mut self, difficulty: Difficulty) {
        self.saved.difficulty = difficulty;
        self.save();
    }

    pub(super) fn toggle_pause(&mut self) {
        self.saved.pause_tricks = !self.saved.pause_tricks;
        self.save();
    }

    pub(super) fn apply(&mut self, turn: Turn) -> bool {
        if !self.round.apply(turn) {
            return false;
        }
        self.saved.attempt.get_or_insert_with(AttemptId::default);
        self.saved.turns.push(turn);
        self.saved.selected.clear();
        self.save();
        self.record_result();
        true
    }

    pub(super) fn restart(&mut self, seed: u64) {
        self.round = Round::new(seed);
        self.saved = SavedMatch {
            difficulty: self.saved.difficulty,
            ..SavedMatch::new(seed, self.saved.pause_tricks)
        };
        self.save();
    }

    fn save(&self) {
        self.store.save(&self.saved);
    }

    fn record_result(&self) {
        if let (Some(attempt), Some((outcome, metric))) =
            (self.saved.attempt, self.round.completed_result())
        {
            self.store
                .record_result(attempt, "Classic", outcome, metric);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_match_restores_draft_received_cards_and_the_completed_trick() {
        let seed = 212;
        let mut round = Round::new(seed);
        let hand = round.game.hand(Seat::You);
        let cards = [hand[0], hand[3], hand[8]].map(Card::id);
        let mut saved = SavedMatch::new(seed, true);
        saved.selected = vec![cards[0], 255, cards[1], cards[0]];
        let (_, mut saved) = round_trip(&saved);
        assert_eq!(saved.selected, cards[..2]);
        assert!(saved.pause_tricks);

        let pass = Turn::Pass { cards };
        assert!(round.apply(pass));
        saved.turns.push(pass);
        let (restored, mut saved) = round_trip(&saved);
        assert!(restored.ready_to_open);
        assert!(saved.selected.is_empty());
        assert_eq!(restored.game, round.game);
        assert!(saved.attempt.is_some());
        assert!(round_trip(&saved).1.attempt == saved.attempt);
        assert!(SavedMatch::new(seed, true).attempt != saved.attempt);

        saved.turns.push(Turn::Begin);
        assert!(round.apply(Turn::Begin));
        for _ in 0..4 {
            let turn = Turn::Play {
                card: round.game.suggested_play().unwrap().id(),
            };
            assert!(round.apply(turn));
            saved.turns.push(turn);
        }
        let (restored, _) = round_trip(&saved);
        assert!(!restored.ready_to_open);
        assert!(matches!(restored.game.phase(), Phase::TrickComplete(_)));
        assert_eq!(restored.game, round.game);
    }

    #[test]
    fn saved_match_rejects_out_of_order_or_invalid_turns() {
        let mut saved = SavedMatch::new(3, false);
        saved.turns.push(Turn::Play { card: 52 });
        assert!(saved.restore().is_none());
        let mut saved = SavedMatch::new(3, false);
        saved.turns.push(Turn::Begin);
        assert!(saved.restore().is_none());
    }

    #[test]
    fn older_matches_resume_without_an_attempt_id() {
        let saved: SavedMatch = serde_json::from_value(serde_json::json!({
            "version": 1,
            "seed": 212,
            "turns": [],
            "selected": [],
            "pause_tricks": false
        }))
        .unwrap();
        let (round, saved) = saved.restore().unwrap();
        assert_eq!(round.game.phase(), Phase::Passing);
        assert!(saved.attempt.is_none());
        assert_eq!(saved.difficulty, Difficulty::Medium);
    }

    #[test]
    fn only_the_final_hand_records_the_players_match_score() {
        let mut saw_tie = false;
        for seed in [1, 77, 94] {
            let mut round = Round::new(seed);
            loop {
                let turn = if round.ready_to_open {
                    Turn::Begin
                } else {
                    match round.game.phase() {
                        Phase::Passing => {
                            let hand = round.game.hand(Seat::You);
                            Turn::Pass {
                                cards: [hand[0], hand[1], hand[2]].map(Card::id),
                            }
                        }
                        Phase::Playing(_) => Turn::Play {
                            card: round.game.suggested_play().unwrap().id(),
                        },
                        Phase::TrickComplete(_) => Turn::Collect,
                        Phase::HandComplete(hand) if hand.winner.is_none() => {
                            saw_tie |= hand.tied_at_limit;
                            Turn::Deal
                        }
                        Phase::HandComplete(_) => break,
                    }
                };
                assert_eq!(round.completed_result(), None);
                assert!(round.apply(turn));
            }
            match seed {
                1 => assert_eq!(
                    round.completed_result(),
                    Some((Outcome::Lost, ResultMetric::PenaltyPoints(107)))
                ),
                94 => assert_eq!(
                    round.completed_result(),
                    Some((Outcome::Won, ResultMetric::PenaltyPoints(13)))
                ),
                _ => assert!(saw_tie),
            }
        }
    }

    fn round_trip(saved: &SavedMatch) -> (Round, SavedMatch) {
        serde_json::from_slice::<SavedMatch>(&serde_json::to_vec(saved).unwrap())
            .unwrap()
            .restore()
            .unwrap()
    }
}
