use corner_table_dominoes::{Action, End, Event, Player, Round, Tile};
use serde::{Deserialize, Serialize};

use crate::storage::AttemptId;

const VERSION: u8 = 1;
const MAX_ROUND_ACTIONS: usize = 70;

#[derive(Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Mode {
    #[default]
    Computer,
    HotSeat,
}

impl Mode {
    pub(super) const ALL: [Self; 2] = [Self::Computer, Self::HotSeat];

    pub(super) const fn index(self) -> usize {
        match self {
            Self::Computer => 0,
            Self::HotSeat => 1,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Computer => "Computer",
            Self::HotSeat => "Two players",
        }
    }

    pub(super) const fn player_name(self, player: Player) -> &'static str {
        match (self, player) {
            (Self::Computer, Player::One) => "You",
            (Self::Computer, Player::Two) => "Computer",
            (Self::HotSeat, Player::One) => "Player 1",
            (Self::HotSeat, Player::Two) => "Player 2",
        }
    }
}

pub(super) struct Table {
    pub(super) round: Round,
    saved: SavedTable,
    pub(super) preceding_event: Option<Event>,
}

impl Table {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            round: Round::new(seed),
            saved: SavedTable {
                seed,
                moves: Vec::new(),
                banked_scores: [0; 2],
                attempt: AttemptId::default(),
            },
            preceding_event: None,
        }
    }

    pub(super) fn act(&mut self, action: Action) -> Option<Event> {
        let preceding = self.round.last_event();
        let event = self.round.act(action)?;
        self.saved.moves.push(RecordedAction::from(action));
        self.preceding_event = Some(preceding);
        Some(event)
    }

    pub(super) fn redeal(&mut self, seed: u64) {
        let banked_scores = self.scores();
        *self = Self::new(seed);
        self.saved.banked_scores = banked_scores;
    }

    pub(super) fn scores(&self) -> [u64; 2] {
        let mut scores = self.saved.banked_scores;
        if let Some(outcome) = self.round.outcome()
            && let Some(winner) = outcome.winner
        {
            scores[winner.index()] =
                scores[winner.index()].saturating_add(u64::from(outcome.points));
        }
        scores
    }

    pub(super) fn attempt(&self) -> AttemptId {
        self.saved.attempt
    }

    pub(super) fn has_moves(&self) -> bool {
        !self.saved.moves.is_empty()
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct SavedTable {
    seed: u64,
    moves: Vec<RecordedAction>,
    banked_scores: [u64; 2],
    attempt: AttemptId,
}

impl SavedTable {
    fn restore(self) -> Option<Table> {
        if self.moves.len() > MAX_ROUND_ACTIONS {
            return None;
        }
        let mut round = Round::new(self.seed);
        let mut preceding_event = None;
        for recorded in &self.moves {
            preceding_event = Some(round.last_event());
            round.act(recorded.action()?)?;
        }
        Some(Table {
            round,
            saved: self,
            preceding_event,
        })
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RecordedAction {
    Left([u8; 2]),
    Right([u8; 2]),
    Draw,
    Pass,
}

impl RecordedAction {
    fn action(self) -> Option<Action> {
        Some(match self {
            Self::Left([a, b]) => Action::Play(Tile::new(a, b)?, End::Left),
            Self::Right([a, b]) => Action::Play(Tile::new(a, b)?, End::Right),
            Self::Draw => Action::Draw,
            Self::Pass => Action::Pass,
        })
    }
}

impl From<Action> for RecordedAction {
    fn from(action: Action) -> Self {
        match action {
            Action::Play(tile, End::Left) => Self::Left(tile.pips()),
            Action::Play(tile, End::Right) => Self::Right(tile.pips()),
            Action::Draw => Self::Draw,
            Action::Pass => Self::Pass,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub(super) struct Saved {
    version: u8,
    mode: Mode,
    tables: [SavedTable; 2],
}

impl Saved {
    pub(super) fn capture(mode: Mode, tables: &[Table; 2]) -> Self {
        Self {
            version: VERSION,
            mode,
            tables: tables.each_ref().map(|table| table.saved.clone()),
        }
    }

    pub(super) fn restore(self, fresh: impl Fn(usize) -> Table) -> Option<(Mode, [Table; 2])> {
        if self.version != VERSION {
            return None;
        }
        let [computer, hotseat] = self.tables;
        Some((
            self.mode,
            [
                computer.restore().unwrap_or_else(|| fresh(0)),
                hotseat.restore().unwrap_or_else(|| fresh(1)),
            ],
        ))
    }
}

#[cfg(test)]
mod tests {
    use corner_table_dominoes::computer_action;

    use super::*;

    fn play_out(table: &mut Table) {
        while let Some(action) = computer_action(&table.round) {
            table.act(action).unwrap();
        }
    }

    #[test]
    fn scores_survive_resume_and_bank_once_on_the_next_deal() {
        let mut tables = [Table::new(35), Table::new(9)];
        play_out(&mut tables[0]);
        let action = computer_action(&tables[1].round).unwrap();
        tables[1].act(action).unwrap();
        let json = serde_json::to_vec(&Saved::capture(Mode::HotSeat, &tables)).unwrap();
        let saved = serde_json::from_slice::<Saved>(&json).unwrap();
        let (mode, mut restored) = saved.restore(|_| unreachable!()).unwrap();
        assert!(mode == Mode::HotSeat);
        for (actual, expected) in restored.iter().zip(tables.iter()) {
            assert_eq!(actual.round, expected.round);
            assert_eq!(actual.scores(), expected.scores());
            assert_eq!(actual.preceding_event, expected.preceding_event);
            assert!(actual.attempt() == expected.attempt());
        }
        let table = &mut restored[0];
        let scores = table.scores();
        assert!(scores.into_iter().any(|score| score > 0));
        table.redeal(2);
        assert!(table.attempt() != tables[0].attempt());
        assert_eq!(table.scores(), scores);
        table.redeal(3);
        assert_eq!(table.scores(), scores);
    }

    #[test]
    fn a_bad_replay_only_replaces_its_own_table() {
        let tables = [Table::new(17), Table::new(19)];
        let mut saved = Saved::capture(Mode::HotSeat, &tables);
        saved.tables[0].moves.push(RecordedAction::Left([7, 9]));
        let (_, restored) = saved.restore(|_| Table::new(13)).unwrap();
        assert_eq!(restored[0].round, Round::new(13));
        assert_eq!(restored[1].round, tables[1].round);
    }
}
