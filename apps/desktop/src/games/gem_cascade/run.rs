use corner_table_gem_cascade::{Clear, Direction, Fall, Game, Rotation};
use serde::{Deserialize, Serialize};

use crate::storage::AttemptId;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Mode {
    #[default]
    Timed,
    Practice,
}

impl Mode {
    pub fn index(self) -> usize {
        match self {
            Self::Timed => 0,
            Self::Practice => 1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Timed => "Timed",
            Self::Practice => "Practice",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Move {
    Left,
    Right,
    TurnLeft,
    TurnRight,
    Fall,
    Drop,
}

impl Move {
    fn apply(self, game: &mut Game) -> Option<Fall> {
        match self {
            Self::Left => game.shift(Direction::Left).then_some(Fall::Moved),
            Self::Right => game.shift(Direction::Right).then_some(Fall::Moved),
            Self::TurnLeft => game
                .rotate(Rotation::Counterclockwise)
                .then_some(Fall::Moved),
            Self::TurnRight => game.rotate(Rotation::Clockwise).then_some(Fall::Moved),
            Self::Fall => game.fall_one(),
            Self::Drop => game.lock_pair().map(Fall::Locked),
        }
    }
}

pub(super) const SAVE_VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
pub(super) struct SavedSession<Record> {
    pub version: u32,
    pub mode: Mode,
    pub runs: [Record; 2],
}

#[derive(Deserialize, Serialize)]
pub(super) struct Record {
    seed: u64,
    best: u32,
    moves: Vec<Move>,
    #[serde(default)]
    pub attempt: AttemptId,
}

pub(super) struct Run {
    pub game: Game,
    pub record: Record,
    pub last_scoring: Clear,
}

impl Run {
    pub fn new(seed: u64) -> Self {
        Self {
            game: Game::new(seed),
            record: Record {
                seed,
                best: 0,
                moves: Vec::new(),
                attempt: AttemptId::default(),
            },
            last_scoring: Clear::default(),
        }
    }

    pub fn restore(mut record: Record) -> Self {
        let mut game = Game::new(record.seed);
        let mut last_scoring = Clear::default();
        // Reconstruct the last valid position even if a save contains an invalid move.
        for (index, &movement) in record.moves.iter().enumerate() {
            match movement.apply(&mut game) {
                Some(Fall::Locked(resolution)) if resolution.clear.points > 0 => {
                    last_scoring = resolution.clear;
                }
                Some(_) => {}
                None => {
                    record.moves.truncate(index);
                    break;
                }
            }
        }
        record.best = record.best.max(game.score());
        Self {
            game,
            record,
            last_scoring,
        }
    }

    pub fn has_moves(&self) -> bool {
        !self.record.moves.is_empty()
    }

    pub fn best(&self) -> u32 {
        self.record.best
    }

    pub fn play(&mut self, movement: Move) -> Option<Fall> {
        let fall = movement.apply(&mut self.game)?;
        self.record.moves.push(movement);
        if let Fall::Locked(resolution) = &fall
            && resolution.clear.points > 0
        {
            self.last_scoring = resolution.clear;
            self.record.best = self.record.best.max(self.game.score());
        }
        Some(fall)
    }

    pub fn reset(&mut self, new_seed: Option<u64>) {
        if let Some(seed) = new_seed {
            self.game = Game::new(seed);
            self.record.seed = seed;
        } else {
            self.game.restart();
        }
        self.record.moves.clear();
        self.record.attempt = AttemptId::default();
        self.last_scoring = Clear::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_record_replays_gravity_and_keeps_best_after_retry() {
        let mut run = Run::new(14);
        for movement in [
            Move::Fall,
            Move::Right,
            Move::TurnLeft,
            Move::Drop,
            Move::Drop,
        ] {
            assert!(run.play(movement).is_some());
        }
        assert_eq!(run.game.score(), 40);
        let encoded = serde_json::to_string(&run.record).unwrap();
        let mut restored = Run::restore(serde_json::from_str(&encoded).unwrap());
        assert_eq!(restored.game, run.game);
        assert_eq!(restored.last_scoring, run.last_scoring);
        assert_eq!(restored.best(), 40);
        restored.reset(None);
        assert_eq!(restored.game, Game::new(14));
        assert!(!restored.has_moves());
        assert_eq!(restored.best(), 40);
        restored.reset(Some(8));
        assert_eq!(restored.game, Game::new(8));
        assert_eq!(restored.best(), 40);
    }

    #[test]
    fn restore_keeps_the_last_valid_position_of_an_inconsistent_record() {
        let mut run = Run::new(14);
        assert!(run.play(Move::Left).is_some());
        assert!(run.play(Move::Left).is_some());
        run.record.moves.extend([Move::Left, Move::Drop]);
        let restored = Run::restore(run.record);
        assert_eq!(restored.game, run.game);
        assert_eq!(restored.record.moves, [Move::Left, Move::Left]);
    }
}
