//! Desktop entry points for game views. Each game is hosted in its own GPUI entity.

mod jigsaw;
mod mahjong;
mod minesweeper;
mod solitaire;
mod sudoku;

use corner_table_api::GameId;
use gpui::{AnyView, AppContext as _, Context};

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
    let view = match launch.game.0.as_str() {
        "jigsaw" => cx.new(|cx| jigsaw::GameView::new(launch, cx)).into(),
        "mahjong" => cx.new(|cx| mahjong::GameView::new(launch, cx)).into(),
        "minesweeper" => cx.new(|cx| minesweeper::GameView::new(launch, cx)).into(),
        "solitaire" => cx.new(|cx| solitaire::GameView::new(launch, cx)).into(),
        "sudoku" => cx.new(|cx| sudoku::GameView::new(launch, cx)).into(),
        _ => return None,
    };
    Some(view)
}
