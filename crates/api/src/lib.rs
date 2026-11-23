//! Wire types shared by the desktop and a future HTTP API.

mod fixtures;

pub use fixtures::FixtureApi;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct GameId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Library {
    pub games: Vec<Game>,
    pub solitaire_modes: Vec<Mode>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Game {
    pub id: GameId,
    pub title: String,
    pub description: String,
    pub category: GameCategory,
    #[serde(default)]
    pub search_terms: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameCategory {
    Cards,
    Puzzles,
    Board,
    Arcade,
}

impl GameCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cards => "Cards",
            Self::Puzzles => "Puzzles",
            Self::Board => "Board games",
            Self::Arcade => "Arcade",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Mode {
    pub id: String,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Home {
    pub guest: Guest,
    pub challenge: Challenge,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Guest {
    pub id: String,
    pub name: String,
    pub initials: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Challenge {
    pub id: String,
    pub game_id: GameId,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Period {
    Today,
    ThisWeek,
    AllTime,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Board {
    pub game_id: GameId,
    pub period: Period,
    pub leaders: Vec<Standing>,
    pub guest_standing: Option<Standing>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Standing {
    pub player_id: String,
    pub name: String,
    pub rank: u32,
    pub score: u64,
    pub elapsed_seconds: u32,
}
