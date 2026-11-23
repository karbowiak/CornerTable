use corner_table_match_three::{Cell, Game, STAGES, Status};
use serde::{Deserialize, Serialize};

use crate::storage::AttemptId;

#[derive(Default, Deserialize, Serialize)]
struct StageSave {
    layout: u64,
    #[serde(default)]
    attempt: AttemptId,
    swaps: Vec<[usize; 2]>,
    cleared: bool,
    best_score: u32,
    best_chain: u32,
}

#[derive(Deserialize, Serialize)]
pub struct Journal {
    seed: u64,
    pub active: usize,
    stages: [StageSave; STAGES.len()],
}

impl Journal {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            active: 0,
            stages: Default::default(),
        }
    }

    fn deal(&self) -> Game {
        let stage = &self.stages[self.active];
        let seed = self.seed
            ^ (self.active as u64).wrapping_mul(0xa0761d6478bd642f)
            ^ stage.layout.wrapping_mul(0xe7037ed1a0b428db);
        Game::for_stage(seed, STAGES[self.active])
    }

    pub fn resume(&mut self) -> Game {
        if !self.is_unlocked(self.active) {
            self.active = 0;
        }
        let mut game = self.deal();
        let stage = &mut self.stages[self.active];
        if stage
            .swaps
            .iter()
            .any(|&[a, b]| game.swap(Cell::from_index(a), Cell::from_index(b)).is_err())
        {
            // A changed ruleset or a damaged log can always return to a fresh deal.
            stage.swaps.clear();
            stage.attempt = AttemptId::default();
            return self.deal();
        }
        game
    }

    pub fn resume_challenge(&mut self, seed: u64) -> Game {
        if self.seed != seed || self.stages[0].layout != 0 {
            *self = Self::new(seed);
        }
        self.active = 0;
        self.resume()
    }

    pub fn restart(&mut self, new_layout: bool) -> Game {
        let stage = &mut self.stages[self.active];
        if new_layout {
            stage.layout = stage.layout.wrapping_add(1);
        }
        stage.swaps.clear();
        stage.attempt = AttemptId::default();
        self.deal()
    }

    pub fn attempt(&self) -> AttemptId {
        self.stages[self.active].attempt
    }

    pub fn add_move(&mut self, first: usize, second: usize, game: &Game) {
        let stage = &mut self.stages[self.active];
        stage.swaps.push([first, second]);
        stage.cleared |= game.status() == Status::Won;
        stage.best_score = stage.best_score.max(game.score());
        stage.best_chain = stage.best_chain.max(game.progress().best_chain);
    }

    pub fn is_unlocked(&self, stage: usize) -> bool {
        stage < STAGES.len() && (stage == 0 || self.stages[stage - 1].cleared)
    }

    pub fn is_cleared(&self, stage: usize) -> bool {
        self.stages[stage].cleared
    }

    pub fn record(&self) -> (u32, u32) {
        let stage = &self.stages[self.active];
        (stage.best_score, stage.best_chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_daily_challenge_rejects_a_redealt_layout() {
        let mut journal = Journal::new(42);
        let original = *journal.resume().board();
        assert_ne!(journal.restart(true).board(), &original);
        assert_eq!(journal.resume_challenge(42).board(), &original);
    }

    #[test]
    fn serde_round_trip_restores_board_progress_and_new_layouts() {
        let mut journal = Journal::new(42);
        let mut game = journal.resume();
        let first_layout = *game.board();
        let (a, b) = game.suggestion().unwrap();
        game.swap(a, b).unwrap();
        journal.add_move(a.index().unwrap(), b.index().unwrap(), &game);

        let bytes = serde_json::to_vec(&journal).unwrap();
        let mut restored: Journal = serde_json::from_slice(&bytes).unwrap();
        let resumed = restored.resume();
        assert_eq!(resumed.board(), game.board());
        assert_eq!(resumed.progress(), game.progress());
        assert_eq!(resumed.moves_left(), game.moves_left());
        let record = restored.record();
        assert_eq!(restored.restart(false).board(), &first_layout);
        assert_ne!(restored.restart(true).board(), &first_layout);
        assert_eq!(restored.record(), record);
    }

    #[test]
    fn clearing_a_stage_unlocks_the_next_even_after_replaying_it() {
        let mut journal = Journal::new(3);
        assert!(!journal.is_unlocked(1));
        let mut game = journal.resume();
        while let Some((a, b)) = game.suggestion() {
            game.swap(a, b).unwrap();
            journal.add_move(a.index().unwrap(), b.index().unwrap(), &game);
        }
        assert_eq!(game.status(), Status::Won);
        assert!(journal.is_unlocked(1));
        assert!(!journal.is_unlocked(2));
        journal.restart(false);
        assert!(journal.is_unlocked(1));
    }
}
