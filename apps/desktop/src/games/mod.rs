//! Desktop entry points for game views.
//!
//! Register game-specific views here as their rule crates land under `games/`.

use corner_table_api::GameId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameEntry {
    Classic,
    Mode(String),
    Challenge(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameLaunch {
    pub game: GameId,
    pub entry: GameEntry,
}
