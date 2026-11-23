use corner_table_checkers::{Difficulty, Game, Outcome, Side, Square, Step, StepEffect};
use serde::{Deserialize, Serialize};

use super::super::{GameEntry, GameLaunch};
use crate::storage::{AttemptId, Outcome as RecordOutcome, ResultMetric, Session};

use super::difficulty_label;

// Bump on incompatible changes to the opening, square indexing, or difficulty indices.
const REPLAY_VERSION: u8 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Mode {
    Computer,
    Local,
}

pub(super) const DIFFICULTIES: [Difficulty; 3] =
    [Difficulty::Easy, Difficulty::Standard, Difficulty::Expert];

pub(super) struct Table {
    session: Session,
    pub mode: Mode,
    pub difficulty: Difficulty,
    computer: Game,
    local: Game,
    computer_attempt: Option<AttemptId>,
    computer_difficulties: Vec<Option<u8>>,
}

#[derive(Deserialize, Serialize)]
struct SavedTable {
    version: u8,
    mode: Mode,
    difficulty: u8,
    computer: Vec<[u8; 2]>,
    local: Vec<[u8; 2]>,
    #[serde(default)]
    computer_attempt: Option<AttemptId>,
    #[serde(default)]
    computer_difficulties: Vec<Option<u8>>,
}

impl Table {
    pub fn open(launch: &GameLaunch) -> Self {
        let mut table = Self {
            session: Session::new(launch),
            mode: match &launch.entry {
                GameEntry::Mode(id) if id == "local" => Mode::Local,
                _ => Mode::Computer,
            },
            difficulty: Difficulty::default(),
            computer: Game::default(),
            local: Game::default(),
            computer_attempt: None,
            computer_difficulties: Vec::new(),
        };
        if let Some(saved) = table
            .session
            .load::<SavedTable>()
            .filter(|saved| saved.version == REPLAY_VERSION)
        {
            table.mode = saved.mode;
            table.difficulty = DIFFICULTIES
                .get(usize::from(saved.difficulty))
                .copied()
                .unwrap_or_default();
            if let Some(game) = replay(&saved.computer) {
                table.computer = game;
                table.computer_attempt = saved.computer_attempt;
                table.computer_difficulties = saved.computer_difficulties;
                // Older saves carry the current preference but no difficulty history.
                table
                    .computer_difficulties
                    .resize(table.computer.turn_count() / 2, None);
            }
            table.local = replay(&saved.local).unwrap_or_default();
        }
        table.record_computer_result();
        table
    }

    pub fn game(&self) -> &Game {
        match self.mode {
            Mode::Computer => &self.computer,
            Mode::Local => &self.local,
        }
    }

    fn game_mut(&mut self) -> &mut Game {
        match self.mode {
            Mode::Computer => &mut self.computer,
            Mode::Local => &mut self.local,
        }
    }

    pub fn play(&mut self, step: Step) -> Option<StepEffect> {
        let effect = self.game_mut().play(step)?;
        if self.mode == Mode::Computer {
            self.computer_attempt.get_or_insert_with(AttemptId::default);
        }
        self.save();
        if self.mode == Mode::Computer {
            self.record_computer_result();
        }
        Some(effect)
    }

    pub fn play_computer(&mut self, turn: Vec<Step>, difficulty: Difficulty) {
        if turn.is_empty() {
            return;
        }
        for step in turn {
            self.computer
                .play(step)
                .expect("computer search returns a legal turn");
        }
        self.computer_attempt.get_or_insert_with(AttemptId::default);
        self.computer_difficulties
            .push(Some(difficulty_index(difficulty)));
        self.save();
        self.record_computer_result();
    }

    pub fn undo(&mut self) -> bool {
        let vs_computer = self.mode == Mode::Computer;
        let game = self.game_mut();
        if !game.undo_turn() {
            return false;
        }
        // Return to the last human decision, including when a reply is still pending.
        if vs_computer {
            if game.side() == Side::Light {
                game.undo_turn();
            }
            let computer_turns = game.turn_count() / 2;
            self.computer_difficulties.truncate(computer_turns);
        }
        self.save();
        true
    }

    pub fn restart(&mut self) {
        *self.game_mut() = Game::default();
        if self.mode == Mode::Computer {
            self.computer_attempt = None;
            self.computer_difficulties.clear();
        }
        self.save();
    }

    pub fn save(&self) {
        self.session.save(&SavedTable {
            version: REPLAY_VERSION,
            mode: self.mode,
            difficulty: difficulty_index(self.difficulty),
            computer: encode(&self.computer),
            local: encode(&self.local),
            computer_attempt: self.computer_attempt,
            computer_difficulties: self.computer_difficulties.clone(),
        });
    }

    fn record_computer_result(&self) {
        let Some(attempt) = self.computer_attempt else {
            return;
        };
        let outcome = match self.computer.outcome() {
            Outcome::Playing => return,
            Outcome::Won(Side::Red) => RecordOutcome::Won,
            Outcome::Won(Side::Light) => RecordOutcome::Lost,
            Outcome::Draw(_) => RecordOutcome::Draw,
        };
        self.session.record_result(
            attempt,
            computer_mode(&self.computer_difficulties),
            outcome,
            ResultMetric::Moves(self.computer.turn_count() as u64),
        );
    }
}

fn difficulty_index(difficulty: Difficulty) -> u8 {
    DIFFICULTIES
        .iter()
        .position(|&choice| choice == difficulty)
        .expect("each difficulty has a save index") as u8
}

fn computer_mode(difficulties: &[Option<u8>]) -> &'static str {
    let known_difficulty = |index: Option<u8>| DIFFICULTIES.get(usize::from(index?)).copied();
    let mut played = difficulties.iter().copied().map(known_difficulty);
    let Some(Some(first)) = played.next() else {
        return "Computer";
    };
    let mut mixed = false;
    for difficulty in played {
        let Some(difficulty) = difficulty else {
            return "Computer";
        };
        mixed |= difficulty != first;
    }
    if mixed {
        "Mixed difficulty"
    } else {
        difficulty_label(first).1
    }
}

fn encode(game: &Game) -> Vec<[u8; 2]> {
    game.steps()
        .iter()
        .map(|step| [step.from.index(), step.to.index()])
        .collect()
}

fn replay(steps: &[[u8; 2]]) -> Option<Game> {
    let mut game = Game::default();
    for &[from, to] in steps {
        game.play(Step {
            from: Square::from_index(from)?,
            to: Square::from_index(to)?,
        })?;
    }
    Some(game)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_restores_a_partial_jump_chain_and_turn_undo() {
        // Reach the first half of Maple's c5 × a3 × c1 jump from the normal opening.
        let journal = [
            [23, 18],
            [8, 13],
            [18, 14],
            [9, 18],
            [22, 15],
            [11, 18],
            [25, 22],
            [18, 25],
            [29, 22],
            [5, 9],
            [20, 16],
            [13, 20],
        ];
        let game = replay(&journal).unwrap();
        assert_eq!(game.continuation(), Square::from_index(20));
        assert_eq!(game.turn_count(), 11);
        let saved = encode(&game);
        assert_eq!(saved, journal);
        let mut restored = replay(&saved).unwrap();
        assert_eq!(restored.continuation(), game.continuation());
        assert_eq!(restored.legal_steps(), game.legal_steps());
        assert!(restored.undo_turn());
        assert_eq!(encode(&restored), &journal[..11]);
        assert_eq!(restored.turn_count(), 11);
        assert!(replay(&[[32, 16]]).is_none());
        assert!(replay(&[[0, 5]]).is_none());
    }

    #[test]
    fn difficulty_label_uses_played_turns_and_accepts_older_saves() {
        let saved: SavedTable = serde_json::from_value(serde_json::json!({
            "version": 1,
            "mode": "computer",
            "difficulty": 2,
            "computer": [[23, 18], [8, 13]],
            "local": []
        }))
        .unwrap();
        let game = replay(&saved.computer).unwrap();
        assert_eq!(game.turn_count(), 2);
        assert!(saved.computer_attempt.is_none());
        assert!(saved.computer_difficulties.is_empty());
        assert_eq!(computer_mode(&[None, Some(2)]), "Computer");
        assert_eq!(computer_mode(&[Some(0), Some(0)]), "Easy");
        assert_eq!(computer_mode(&[Some(1), Some(1)]), "Standard");
        assert_eq!(computer_mode(&[Some(2), Some(2)]), "Expert");
        assert_eq!(computer_mode(&[Some(0), Some(2)]), "Mixed difficulty");
    }
}
