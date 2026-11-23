//! Desktop entry points for game views. Each game is hosted in its own GPUI entity.

mod block_puzzle;
mod bubble_shooter;
mod checkers;
pub(crate) mod chess;
mod dominoes;
mod gem_cascade;
mod hearts;
mod host;
mod jigsaw;
mod kakuro;
mod mahjong;
mod mahjong_cubes;
mod match_three;
mod memory_match;
mod mine_trails;
mod minesweeper;
mod picture_logic;
mod reversi;
mod solitaire;
mod sudoku;
mod word_games;

use corner_table_api::GameId;
use gpui::{
    AnyView, App, AppContext as _, Context, Div, FontWeight, Role, SharedString, Stateful, Window,
    div, prelude::*, px, rgb,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{app::Desktop, theme};

pub use host::GameHost;

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
        "checkers" => cx.new(|cx| checkers::GameView::new(launch, cx)).into(),
        "chess" => cx.new(|cx| chess::GameView::new(launch, cx)).into(),
        "dominoes" => cx.new(|cx| dominoes::GameView::new(launch, cx)).into(),
        "gem-cascade" => cx.new(|cx| gem_cascade::GameView::new(launch, cx)).into(),
        "hearts" => cx.new(|cx| hearts::GameView::new(launch, cx)).into(),
        "jigsaw" => cx.new(|cx| jigsaw::GameView::new(launch, cx)).into(),
        "kakuro" => cx.new(|cx| kakuro::GameView::new(launch, cx)).into(),
        "mahjong" => cx.new(|cx| mahjong::GameView::new(launch, cx)).into(),
        "mahjong-cubes" => cx.new(|cx| mahjong_cubes::GameView::new(launch, cx)).into(),
        "match-three" => cx.new(|cx| match_three::GameView::new(launch, cx)).into(),
        "memory-match" => cx.new(|cx| memory_match::GameView::new(launch, cx)).into(),
        "mine-trails" => cx.new(|cx| mine_trails::GameView::new(launch, cx)).into(),
        "minesweeper" => cx.new(|cx| minesweeper::GameView::new(launch, cx)).into(),
        "picture-logic" => cx.new(|cx| picture_logic::GameView::new(launch, cx)).into(),
        "reversi" => cx.new(|cx| reversi::GameView::new(launch, cx)).into(),
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
    styled_control(theme::LIGHT, id, label, enabled)
}

fn themed_control(
    cx: &App,
    id: &'static str,
    label: impl Into<SharedString>,
    enabled: bool,
) -> Stateful<Div> {
    styled_control(theme::colors(cx), id, label, enabled)
}

fn styled_control(
    palette: theme::Palette,
    id: &'static str,
    label: impl Into<SharedString>,
    enabled: bool,
) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .flex()
        .items_center()
        .min_h(px(36.))
        .px_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(palette.border))
        .bg(rgb(if enabled {
            palette.surface
        } else {
            palette.paper
        }))
        .text_color(rgb(if enabled { palette.ink } else { palette.muted }))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |button| {
            button
                .role(Role::Button)
                .aria_label(label.clone())
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(palette.hover)))
                .focus_visible(|style| style.border_color(rgb(palette.ink)))
        })
        .child(label)
}
