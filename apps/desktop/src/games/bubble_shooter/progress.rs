use corner_table_bubble_shooter::{Angle, Round, SHOT_LIMIT, STAGE_COUNT, Shot, Status};
use serde::{Deserialize, Serialize};

use crate::{
    games::{GameEntry, GameLaunch},
    storage::Session,
};

const SAVE_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Mode {
    #[default]
    Stages,
    Classic,
    Challenge,
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
    turns: Vec<Turn>,
    best: Option<Record>,
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
        }
    }

    fn attempt(&self) -> &Attempt {
        match self.mode {
            Mode::Stages => &self.stages[self.stage],
            Mode::Classic => &self.classic,
            Mode::Challenge => &self.challenge,
        }
    }

    fn attempt_mut(&mut self) -> &mut Attempt {
        match self.mode {
            Mode::Stages => &mut self.stages[self.stage],
            Mode::Classic => &mut self.classic,
            Mode::Challenge => &mut self.challenge,
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

    pub fn restart(&mut self) {
        self.saved.attempt_mut().turns.clear();
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
        let attempt = self.saved.attempt_mut();
        attempt.turns.push(Turn::Shoot(angle.ticks()));
        if self.round.status() == Status::Won {
            let previous = attempt.best;
            self.new_best = previous.is_none_or(|best| score > best.score);
            attempt.best = Some(Record {
                score: previous.map_or(score, |best| best.score.max(score)),
                stars: previous.map_or(stars, |best| best.stars.max(stars)),
            });
        }
        self.last_shot = Some(shot);
        self.save();
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
                self.saved.attempt_mut().turns.clear();
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
