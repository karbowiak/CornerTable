//! Desktop entry points for game views. Each game is hosted in its own GPUI entity.

mod block_puzzle;
mod bubble_shooter;
mod gem_cascade;
mod jigsaw;
mod mahjong;
mod mahjong_cubes;
mod match_three;
mod mine_trails;
mod minesweeper;
mod solitaire;
mod sudoku;
mod word_games;

use corner_table_api::GameId;
use gpui::{
    AnyView, AppContext as _, Context, Div, FontWeight, SharedString, Stateful, Window, div,
    prelude::*, px, rgb,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{app::Desktop, theme};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "id")]
pub enum GameEntry {
    Classic,
    Mode(String),
    Challenge(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct GameLaunch {
    pub game: GameId,
    pub entry: GameEntry,
}

impl GameLaunch {
    fn seed(&self) -> u64 {
        match &self.entry {
            GameEntry::Challenge(id) => id.bytes().fold(0_u64, |seed, byte| {
                seed.wrapping_mul(31).wrapping_add(u64::from(byte))
            }),
            _ => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos() as u64),
        }
    }
}

pub fn open(launch: &GameLaunch, cx: &mut Context<Desktop>) -> Option<AnyView> {
    let view = match launch.game.0.as_str() {
        "block-puzzle" => cx.new(|cx| block_puzzle::GameView::new(launch, cx)).into(),
        "bubble-shooter" => cx
            .new(|cx| bubble_shooter::GameView::new(launch, cx))
            .into(),
        "gem-cascade" => cx.new(|cx| gem_cascade::GameView::new(launch, cx)).into(),
        "jigsaw" => cx.new(|cx| jigsaw::GameView::new(launch, cx)).into(),
        "mahjong" => cx.new(|cx| mahjong::GameView::new(launch, cx)).into(),
        "mahjong-cubes" => cx.new(|cx| mahjong_cubes::GameView::new(launch, cx)).into(),
        "match-three" => cx.new(|cx| match_three::GameView::new(launch, cx)).into(),
        "mine-trails" => cx.new(|cx| mine_trails::GameView::new(launch, cx)).into(),
        "minesweeper" => cx.new(|cx| minesweeper::GameView::new(launch, cx)).into(),
        "solitaire" => cx.new(|cx| solitaire::GameView::new(launch, cx)).into(),
        "sudoku" => cx.new(|cx| sudoku::GameView::new(launch, cx)).into(),
        "word-games" => cx.new(|cx| word_games::GameView::new(launch, cx)).into(),
        _ => return None,
    };
    Some(view)
}

fn board_space(window: &Window) -> f32 {
    let width = f32::from(window.viewport_size().width);
    let content = if width < 1080. {
        width - 218.
    } else {
        width - 288.
    };
    content.min(1372.) - 36.
}

fn control(id: &'static str, label: impl Into<SharedString>, enabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .min_h(px(36.))
        .px_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(theme::BORDER))
        .bg(rgb(if enabled {
            theme::SURFACE
        } else {
            theme::PAPER
        }))
        .text_color(rgb(if enabled { theme::INK } else { theme::MUTED }))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |button| {
            button
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme::SAGE)))
                .focus(|style| style.border_color(rgb(theme::INK)))
        })
        .child(label.into())
}
