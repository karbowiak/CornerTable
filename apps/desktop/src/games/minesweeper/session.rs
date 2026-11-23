use std::{collections::BTreeMap, time::Duration};

use corner_table_minesweeper::{Board, Checkpoint, Difficulty, Status};
use serde::{Deserialize, Serialize};

use crate::storage::{AttemptId, Outcome};

const FORMAT: u8 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Mode {
    #[default]
    Classic,
    TimeTrial,
    LogicOnly,
}

impl Mode {
    pub(super) const ALL: [Self; 3] = [Self::Classic, Self::TimeTrial, Self::LogicOnly];

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::TimeTrial => "time-trial",
            Self::LogicOnly => "logic-only",
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::TimeTrial => "Time trial",
            Self::LogicOnly => "Logic only",
        }
    }

    pub(super) fn limit(self, difficulty: Difficulty) -> Option<Duration> {
        (self == Self::TimeTrial).then(|| {
            Duration::from_secs(match difficulty {
                Difficulty::Meadow => 3 * 60,
                Difficulty::Highlands => 6 * 60,
                Difficulty::Summit => 10 * 60,
            })
        })
    }
}

pub(super) struct Round {
    pub(super) board: Board,
    pub(super) mode: Mode,
    pub(super) flag_mode: bool,
    pub(super) selected: (usize, usize),
    pub(super) replay: bool,
    pub(super) attempt: AttemptId,
}

impl Round {
    pub(super) fn new(difficulty: Difficulty, mode: Mode, seed: u64) -> Self {
        Self {
            board: Board::new(difficulty, seed),
            mode,
            flag_mode: false,
            selected: (0, 0),
            replay: false,
            attempt: AttemptId::default(),
        }
    }

    pub(super) fn replay(&self) -> Option<Self> {
        Self::practice(self.board.replay()?, self.mode)
    }

    pub(super) fn practice(board: Board, mode: Mode) -> Option<Self> {
        Some(Self {
            selected: board.required_opening()?,
            board,
            mode,
            flag_mode: false,
            replay: true,
            attempt: AttemptId::default(),
        })
    }

    pub(super) fn limit(&self) -> Option<Duration> {
        self.mode.limit(self.board.difficulty())
    }

    pub(super) fn outcome(&self, elapsed: Duration) -> Option<Outcome> {
        match self.board.status() {
            Status::Ready | Status::Playing => None,
            Status::Won if !self.replay && self.limit().is_some_and(|limit| elapsed >= limit) => {
                Some(Outcome::Finished)
            }
            Status::Won => Some(Outcome::Won),
            Status::Lost => Some(Outcome::Lost),
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub(super) struct Bests(BTreeMap<String, BTreeMap<Mode, u64>>);

impl Bests {
    pub(super) fn time(&self, round: &Round) -> Option<Duration> {
        self.0
            .get(round.board.difficulty().id())?
            .get(&round.mode)
            .copied()
            .map(Duration::from_millis)
    }

    pub(super) fn record(&mut self, round: &Round, elapsed: Duration) {
        if round.replay
            || round.board.status() != Status::Won
            || round.limit().is_some_and(|limit| elapsed >= limit)
        {
            return;
        }
        let time = milliseconds(elapsed);
        self.0
            .entry(round.board.difficulty().id().into())
            .or_default()
            .entry(round.mode)
            .and_modify(|best| *best = (*best).min(time))
            .or_insert(time);
    }
}

#[derive(Deserialize, Serialize)]
pub(super) struct SavedGame {
    format: u8,
    difficulty: String,
    mode: Mode,
    seed: u64,
    opening: Option<usize>,
    mines: Vec<usize>,
    open: Vec<usize>,
    flags: Vec<usize>,
    detonated: Option<usize>,
    flag_mode: bool,
    selected: (usize, usize),
    replay: bool,
    elapsed_ms: u64,
    bests: Bests,
    #[serde(default)]
    attempt: AttemptId,
}

impl SavedGame {
    pub(super) fn capture(round: &Round, elapsed: Duration, bests: &Bests) -> Self {
        let Checkpoint {
            seed,
            opening,
            mines,
            open,
            flags,
            detonated,
        } = round.board.checkpoint();
        Self {
            format: FORMAT,
            difficulty: round.board.difficulty().id().into(),
            mode: round.mode,
            seed,
            opening,
            mines,
            open,
            flags,
            detonated,
            flag_mode: round.flag_mode,
            selected: round.selected,
            replay: round.replay,
            elapsed_ms: milliseconds(elapsed),
            bests: bests.clone(),
            attempt: round.attempt,
        }
    }

    pub(super) fn restore(self, challenge_seed: Option<u64>) -> Option<(Round, Duration, Bests)> {
        if self.format != FORMAT
            || (self.replay && self.opening.is_none())
            || challenge_seed.is_some_and(|seed| self.seed != seed || self.mode != Mode::Classic)
        {
            return None;
        }
        let difficulty = Difficulty::ALL
            .into_iter()
            .find(|difficulty| difficulty.id() == self.difficulty)?;
        let board = Board::restore(
            difficulty,
            Checkpoint {
                seed: self.seed,
                opening: self.opening,
                mines: self.mines,
                open: self.open,
                flags: self.flags,
                detonated: self.detonated,
            },
        )?;
        let elapsed = if board.status() == Status::Ready {
            if self.replay != board.required_opening().is_some() {
                return None;
            }
            Duration::ZERO
        } else {
            Duration::from_millis(self.elapsed_ms)
        };
        let selected = if board.tile(self.selected.0, self.selected.1).is_some() {
            self.selected
        } else {
            board.required_opening().unwrap_or((0, 0))
        };
        Some((
            Round {
                board,
                mode: self.mode,
                flag_mode: self.flag_mode,
                selected,
                replay: self.replay,
                attempt: self.attempt,
            },
            elapsed,
            self.bests,
        ))
    }
}

pub(super) fn milliseconds(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_round_trip_preserves_playable_and_terminal_states_with_timing_and_mode() {
        let mut round = Round::new(Difficulty::Highlands, Mode::TimeTrial, 42);
        round.flag_mode = true;
        round.selected = (7, 3);
        assert!(round.board.toggle_flag(0, 0));
        assert!(round.board.reveal(7, 3));
        let elapsed = Duration::from_millis(24_567);
        let bests = Bests::default();
        let (restored, restored_elapsed, _) = round_trip(&round, elapsed, &bests);
        assert_eq!(restored.board.status(), Status::Playing);
        assert_eq!(restored.board.checkpoint(), round.board.checkpoint());
        assert_eq!(restored.mode, Mode::TimeTrial);
        assert!(restored.attempt == round.attempt);
        assert!(restored.flag_mode);
        assert_eq!(restored.selected, (7, 3));
        assert_eq!(restored_elapsed, elapsed);

        let mine = round.board.checkpoint().mines[1];
        assert!(round.board.reveal(mine % 16, mine / 16));
        let elapsed = Duration::from_millis(47_912);
        let (terminal, restored_elapsed, _) = round_trip(&round, elapsed, &bests);
        assert_eq!(terminal.board.status(), Status::Lost);
        assert_eq!(terminal.outcome(elapsed), Some(Outcome::Lost));
        assert!(terminal.attempt == round.attempt);
        assert_eq!(terminal.board.checkpoint(), round.board.checkpoint());
        assert_eq!(restored_elapsed, elapsed);

        let next = terminal.replay().unwrap();
        let (replay, restored_elapsed, _) = round_trip(&next, Duration::ZERO, &bests);
        assert!(replay.attempt == next.attempt && replay.attempt != terminal.attempt);
        assert_eq!(replay.board.status(), Status::Ready);
        assert_eq!(replay.board.required_opening(), Some((7, 3)));
        assert_eq!(
            replay.board.checkpoint().mines,
            terminal.board.checkpoint().mines
        );
        assert_eq!(replay.selected, (7, 3));
        assert_eq!(restored_elapsed, Duration::ZERO);
        assert!(replay.replay);
    }

    #[test]
    fn personal_bests_only_count_eligible_wins_and_survive_round_trip() {
        let mut bests = Bests::default();
        let mut round = Round::new(Difficulty::Meadow, Mode::TimeTrial, 30);
        bests.record(&round, Duration::from_secs(8));
        assert!(bests.time(&round).is_none());

        clear_field(&mut round);
        let limit = round.limit().unwrap();
        bests.record(&round, limit);
        assert!(bests.time(&round).is_none());
        assert_eq!(round.outcome(limit), Some(Outcome::Finished));
        let best = Duration::from_millis(31_594);
        bests.record(&round, Duration::from_secs(40));
        bests.record(&round, best);
        bests.record(&round, Duration::from_secs(32));
        assert_eq!(bests.time(&round), Some(best));
        assert_eq!(round.outcome(best), Some(Outcome::Won));

        let mut replay = round.replay().unwrap();
        clear_field(&mut replay);
        bests.record(&replay, Duration::from_secs(2));
        assert_eq!(bests.time(&round), Some(best));
        let (restored, _, bests) = round_trip(&round, best, &bests);
        assert_eq!(restored.board.status(), Status::Won);
        assert_eq!(bests.time(&restored), Some(best));
        assert!(
            bests
                .time(&Round::new(Difficulty::Meadow, Mode::Classic, 30))
                .is_none()
        );
        assert!(
            bests
                .time(&Round::new(Difficulty::Summit, Mode::TimeTrial, 30))
                .is_none()
        );
    }

    fn clear_field(round: &mut Round) {
        let (x, y) = round.board.required_opening().unwrap_or((0, 0));
        assert!(round.board.reveal(x, y));
        let mines = round.board.checkpoint().mines;
        let (width, height) = round.board.difficulty().dimensions();
        for index in 0..width * height {
            if !mines.contains(&index) {
                round.board.reveal(index % width, index / width);
            }
        }
        assert_eq!(round.board.status(), Status::Won);
    }

    fn round_trip(round: &Round, elapsed: Duration, bests: &Bests) -> (Round, Duration, Bests) {
        let json = serde_json::to_string(&SavedGame::capture(round, elapsed, bests)).unwrap();
        serde_json::from_str::<SavedGame>(&json)
            .unwrap()
            .restore(None)
            .unwrap()
    }
}
