//! Desktop entry points for game views.
//!
//! Register game-specific views here as their rule crates land under `games/`.

use corner_table_api::GameId;
use gpui::{AnyView, Context};

use crate::app::Desktop;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GameEntry {
    Classic,
    Mode(String),
    Challenge(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GameLaunch {
    pub game: GameId,
    pub entry: GameEntry,
}

pub fn open(launch: &GameLaunch, cx: &mut Context<Desktop>) -> Option<AnyView> {
    let _ = (launch, cx);
    None
}
