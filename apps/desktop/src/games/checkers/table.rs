use corner_table_checkers::{Difficulty, Game, Square, Step};
use serde::{Deserialize, Serialize};

use super::super::{GameEntry, GameLaunch};
use crate::storage::Session;

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
}

#[derive(Deserialize, Serialize)]
struct SavedTable {
    version: u8,
    mode: Mode,
    difficulty: u8,
    computer: Vec<[u8; 2]>,
    local: Vec<[u8; 2]>,
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
            table.computer = replay(&saved.computer).unwrap_or_default();
            table.local = replay(&saved.local).unwrap_or_default();
        }
        table
    }

    pub fn game(&self) -> &Game {
        match self.mode {
            Mode::Computer => &self.computer,
            Mode::Local => &self.local,
        }
    }

    pub fn game_mut(&mut self) -> &mut Game {
        match self.mode {
            Mode::Computer => &mut self.computer,
            Mode::Local => &mut self.local,
        }
    }

    pub fn save(&self) {
        self.session.save(&SavedTable {
            version: REPLAY_VERSION,
            mode: self.mode,
            difficulty: DIFFICULTIES
                .iter()
                .position(|&difficulty| difficulty == self.difficulty)
                .unwrap_or(1) as u8,
            computer: encode(&self.computer),
            local: encode(&self.local),
        });
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
        let saved = encode(&game);
        assert_eq!(saved, journal);
        let mut restored = replay(&saved).unwrap();
        assert_eq!(restored.continuation(), game.continuation());
        assert_eq!(restored.legal_steps(), game.legal_steps());
        assert!(restored.undo_turn());
        assert_eq!(encode(&restored), &journal[..11]);
        assert!(replay(&[[32, 16]]).is_none());
        assert!(replay(&[[0, 5]]).is_none());
    }
}
