use corner_table_bubble_shooter::{
    Angle, PUZZLE_COUNT, Round, SHOT_LIMIT, STAGE_COUNT, STAGES, Shot, Status,
};
use serde::{Deserialize, Serialize};

use crate::{
    games::{GameEntry, GameLaunch},
    storage::{AttemptId, Outcome, ResultMetric, Session},
};

const SAVE_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Mode {
    #[default]
    Stages,
    Classic,
    Challenge,
    Puzzles,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Turn {
    Shoot(i16),
    Swap,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(super) struct Record {
    pub score: usize,
    pub stars: usize,
}

#[derive(Default, Deserialize, Serialize)]
struct Attempt {
    #[serde(default)]
    id: AttemptId,
    turns: Vec<Turn>,
    best: Option<Record>,
}

#[derive(Default, Deserialize, Serialize)]
struct Practice {
    selected: usize,
    attempts: [Attempt; PUZZLE_COUNT],
}

#[derive(Deserialize, Serialize)]
struct SavedGame {
    version: u8,
    mode: Mode,
    classic_seed: u64,
    classic: Attempt,
    stage: usize,
    stages: [Attempt; STAGE_COUNT],
    challenge: Attempt,
    #[serde(default)]
    practice: Practice,
}

impl SavedGame {
    fn new(seed: u64, mode: Mode) -> Self {
        Self {
            version: SAVE_VERSION,
            mode,
            classic_seed: seed,
            classic: Attempt::default(),
            stage: 0,
            stages: Default::default(),
            challenge: Attempt::default(),
            practice: Practice::default(),
        }
    }

    fn attempt(&self) -> &Attempt {
        match self.mode {
            Mode::Stages => &self.stages[self.stage],
            Mode::Classic => &self.classic,
            Mode::Challenge => &self.challenge,
            Mode::Puzzles => &self.practice.attempts[self.practice.selected],
        }
    }

    fn attempt_mut(&mut self) -> &mut Attempt {
        match self.mode {
            Mode::Stages => &mut self.stages[self.stage],
            Mode::Classic => &mut self.classic,
            Mode::Challenge => &mut self.challenge,
            Mode::Puzzles => &mut self.practice.attempts[self.practice.selected],
        }
    }

    fn stage_unlocked(&self, stage: usize) -> bool {
        stage < STAGE_COUNT && (stage == 0 || self.stages[stage - 1].best.is_some())
    }

    fn make_round(&self, challenge_seed: Option<u64>) -> Round {
        match self.mode {
            Mode::Stages => Round::stage(self.stage).expect("selected stage exists"),
            Mode::Classic => Round::new(self.classic_seed),
            Mode::Challenge => {
                Round::challenge(challenge_seed.expect("challenge launch has a seed"))
            }
            Mode::Puzzles => Round::puzzle(self.practice.selected).expect("selected puzzle exists"),
        }
    }
}

pub(super) struct Progress {
    session: Session,
    saved: SavedGame,
    challenge_seed: Option<u64>,
    pub round: Round,
    pub last_shot: Option<Shot>,
    pub new_best: bool,
}

impl Progress {
    pub fn new(launch: &GameLaunch) -> Self {
        let (default_mode, challenge_seed) = match &launch.entry {
            GameEntry::Challenge(id) => (Mode::Challenge, Some(stable_seed(id))),
            GameEntry::Mode(id) if id == "classic" => (Mode::Classic, None),
            GameEntry::Mode(id) if id == "puzzles" => (Mode::Puzzles, None),
            _ => (Mode::Stages, None),
        };
        let session = Session::new(launch);
        let mut saved = session
            .load::<SavedGame>()
            .filter(|saved| saved.version == SAVE_VERSION)
            .unwrap_or_else(|| SavedGame::new(launch.seed(), default_mode));
        if default_mode == Mode::Challenge || saved.mode == Mode::Challenge {
            saved.mode = default_mode;
        }
        if !saved.stage_unlocked(saved.stage) {
            saved.stage = 0;
        }
        if saved.practice.selected >= PUZZLE_COUNT {
            saved.practice.selected = 0;
        }
        let round = saved.make_round(challenge_seed);
        let mut progress = Self {
            session,
            saved,
            challenge_seed,
            round,
            last_shot: None,
            new_best: false,
        };
        progress.resume();
        progress
    }

    pub fn mode(&self) -> Mode {
        self.saved.mode
    }

    pub fn stage(&self) -> usize {
        self.saved.stage
    }

    pub fn puzzle(&self) -> usize {
        self.saved.practice.selected
    }

    pub fn puzzle_solved(&self, index: usize) -> bool {
        self.saved.practice.attempts[index].best.is_some()
    }

    pub fn stage_unlocked(&self, stage: usize) -> bool {
        self.saved.stage_unlocked(stage)
    }

    pub fn stage_record(&self, stage: usize) -> Option<Record> {
        self.saved.stages[stage].best
    }

    pub fn best(&self) -> Option<Record> {
        self.saved.attempt().best
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if self.challenge_seed.is_none() && mode != Mode::Challenge && self.mode() != mode {
            self.saved.mode = mode;
            self.resume();
            self.save();
        }
    }

    pub fn select_stage(&mut self, stage: usize) {
        if self.mode() == Mode::Stages && self.stage() != stage && self.stage_unlocked(stage) {
            self.saved.stage = stage;
            self.resume();
            self.save();
        }
    }

    pub fn select_puzzle(&mut self, index: usize) {
        if self.mode() == Mode::Puzzles && self.puzzle() != index && index < PUZZLE_COUNT {
            self.saved.practice.selected = index;
            self.resume();
            self.save();
        }
    }

    pub fn restart(&mut self) {
        let attempt = self.saved.attempt_mut();
        attempt.id = AttemptId::default();
        attempt.turns.clear();
        self.resume();
        self.save();
    }

    pub fn new_board(&mut self) {
        if self.mode() == Mode::Classic {
            self.saved.classic_seed = self.saved.classic_seed.wrapping_add(1);
            self.restart();
        }
    }

    pub fn fire(&mut self, angle: Angle) -> Option<&Shot> {
        let shot = self.round.fire_at(angle)?;
        let score = self.round.score();
        let stars = self.round.stars();
        let status = self.round.status();
        let attempt = self.saved.attempt_mut();
        attempt.turns.push(Turn::Shoot(angle.ticks()));
        if status == Status::Won {
            let previous = attempt.best;
            self.new_best = previous.is_none_or(|best| score > best.score);
            attempt.best = Some(Record {
                score: previous.map_or(score, |best| best.score.max(score)),
                stars: previous.map_or(stars, |best| best.stars.max(stars)),
            });
        }
        let id = attempt.id;
        self.last_shot = Some(shot);
        self.save();
        let outcome = match status {
            Status::Playing => None,
            Status::Won => Some(Outcome::Won),
            Status::HitDangerLine | Status::OutOfShots | Status::BankGoalMissed => {
                Some(Outcome::Lost)
            }
        };
        if let Some(outcome) = outcome {
            let mode = match self.mode() {
                Mode::Stages => {
                    let stage = self.stage();
                    Some(format!("Stage {} · {}", stage + 1, STAGES[stage].name))
                }
                Mode::Classic => Some("Classic".into()),
                Mode::Challenge => Some("Challenge".into()),
                Mode::Puzzles => None,
            };
            if let Some(mode) = mode {
                self.session
                    .record_result(id, mode, outcome, ResultMetric::Points(score as u64));
            }
        }
        self.last_shot.as_ref()
    }

    pub fn swap(&mut self) -> bool {
        if !self.round.swap() {
            return false;
        }
        self.saved.attempt_mut().turns.push(Turn::Swap);
        self.save();
        true
    }

    fn resume(&mut self) {
        let fresh = || self.saved.make_round(self.challenge_seed);
        match replay(fresh(), &self.saved.attempt().turns) {
            Some((round, last_shot)) => {
                self.round = round;
                self.last_shot = last_shot;
            }
            None => {
                self.round = fresh();
                self.last_shot = None;
                let attempt = self.saved.attempt_mut();
                attempt.id = AttemptId::default();
                attempt.turns.clear();
            }
        }
        self.new_best = false;
    }

    fn save(&self) {
        self.session.save(&self.saved);
    }
}

fn replay(mut round: Round, turns: &[Turn]) -> Option<(Round, Option<Shot>)> {
    if turns.len() > 2 * SHOT_LIMIT {
        return None;
    }
    let mut last_shot = None;
    for &turn in turns {
        match turn {
            Turn::Shoot(ticks) => last_shot = Some(round.fire_at(Angle::from_ticks(ticks)?)?),
            Turn::Swap if round.swap() => {}
            Turn::Swap => return None,
        }
    }
    Some((round, last_shot))
}

fn stable_seed(id: &str) -> u64 {
    id.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_value, json};

    #[test]
    fn legacy_saves_replay_with_empty_practice_progress() {
        let stage = json!({"turns": [{"shoot": 0}], "best": {"score": 340, "stars": 3}});
        let fresh = json!({"turns": [], "best": null});
        let save = json!({
            "version": 1,
            "mode": "stages",
            "classic_seed": 10,
            "classic": fresh,
            "stage": 0,
            "stages": [stage, fresh, fresh, fresh, fresh, fresh],
            "challenge": fresh,
        });
        let mut saved: SavedGame = from_value(save).unwrap();
        assert!(saved.stage_unlocked(1));
        assert_eq!(saved.attempt().best.unwrap().stars, 3);
        let (stage, _) = replay(saved.make_round(None), &saved.attempt().turns).unwrap();
        assert_eq!(stage.shots_used(), 1);

        saved.mode = Mode::Puzzles;
        assert!(saved.attempt().turns.is_empty());
        assert!(saved.attempt().best.is_none());
        assert_eq!(saved.make_round(None).shots_left(), 1);

        saved.mode = Mode::Classic;
        assert_eq!(saved.make_round(None).shots_left(), SHOT_LIMIT);
        saved.mode = Mode::Challenge;
        assert!(saved.make_round(Some(10)).puzzle_info().is_none());
    }
}
