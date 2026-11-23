use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_minesweeper::{Board, Difficulty, Status, Tile};
use gpui::{
    Context, Div, FontWeight, MouseButton, Render, Stateful, Window, div, prelude::*, px, rgb,
};

use crate::{
    games::{GameEntry, GameLaunch},
    theme as t,
};

pub struct GameView {
    board: Board,
    challenge_seed: Option<u64>,
    flag_mode: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _cx: &mut Context<Self>) -> Self {
        let challenge_seed = match &launch.entry {
            GameEntry::Challenge(id) => Some(stable_seed(id)),
            _ => None,
        };
        Self {
            board: Board::new(
                Difficulty::Meadow,
                challenge_seed.unwrap_or_else(fresh_seed),
            ),
            challenge_seed,
            flag_mode: false,
        }
    }

    fn reset(&mut self, difficulty: Difficulty, cx: &mut Context<Self>) {
        self.board = Board::new(difficulty, self.challenge_seed.unwrap_or_else(fresh_seed));
        self.flag_mode = false;
        cx.notify();
    }

    fn cell(&self, x: usize, y: usize, size: f32, cx: &Context<Self>) -> Stateful<Div> {
        let tile = self.board.tile(x, y).expect("board cell within dimensions");
        let (symbol, background, border, ink) = match tile {
            Tile::Covered => ("", 0x315749, 0x436b56, t::PAPER),
            Tile::Flagged => ("⚑", 0x315749, 0x436b56, 0xffd17c),
            Tile::Open(0) => ("", 0xd6dfcb, 0xc3ceb7, t::INK),
            Tile::Open(count) => {
                let (symbol, ink) = match count {
                    1 => ("1", 0x21526d),
                    2 => ("2", 0x275b36),
                    3 => ("3", 0x9b3d31),
                    4 => ("4", 0x604a83),
                    5 => ("5", 0x803348),
                    6 => ("6", 0x176466),
                    7 => ("7", 0x344347),
                    _ => ("8", 0x594e43),
                };
                (symbol, 0xf1ecdd, 0xd6d0be, ink)
            }
            Tile::Mine => ("●", 0xeccebc, 0xca9f87, 0x733d30),
            Tile::Detonated => ("●", 0xa34335, 0xc35646, 0xfff2df),
            Tile::Misflagged => ("×", 0xf4dcd0, 0xca9f87, 0x8e372d),
        };
        let mut cell = div()
            .id((
                "minesweeper-cell",
                y * self.board.difficulty().dimensions().0 + x,
            ))
            .size(px(size))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .bg(rgb(background))
            .border_color(rgb(border))
            .text_color(rgb(ink))
            .text_size(px(size * 0.54))
            .font_weight(FontWeight::BOLD)
            .child(symbol);

        if matches!(self.board.status(), Status::Ready | Status::Playing)
            && matches!(tile, Tile::Covered | Tile::Flagged)
        {
            cell = cell
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x49745b)))
                .focus(|style| style.border_color(rgb(t::GOLD)))
                .on_click(cx.listener(move |view, _, _, cx| {
                    let changed = if view.flag_mode {
                        view.board.toggle_flag(x, y)
                    } else {
                        view.board.reveal(x, y)
                    };
                    if changed {
                        cx.notify();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |view, _, _, cx| {
                        if view.board.toggle_flag(x, y) {
                            cx.notify();
                        }
                    }),
                );
        }
        cell
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let difficulty = self.board.difficulty();
        let (width, height) = difficulty.dimensions();
        let viewport = f32::from(window.viewport_size().width);
        let size = match difficulty {
            Difficulty::Meadow => 38.,
            Difficulty::Highlands => (viewport / 31.).clamp(20., 30.),
        };
        let (status, status_color) = match self.board.status() {
            Status::Ready => ("Choose a tile to begin.", t::MUTED),
            Status::Playing => ("Clear every safe tile to win.", t::MUTED),
            Status::Won => ("Field cleared!", 0x286248),
            Status::Lost => ("Mine hit. Start a new field to try again.", 0x963f33),
        };
        let mut toolbar = div().flex().flex_wrap().items_center().gap_2();
        if self.challenge_seed.is_none() {
            for (id, option) in [
                ("minesweeper-meadow", Difficulty::Meadow),
                ("minesweeper-highlands", Difficulty::Highlands),
            ] {
                toolbar =
                    toolbar.child(control(id, option.label(), difficulty == option).on_click(
                        cx.listener(move |view, _, _, cx| {
                            if view.board.difficulty() != option {
                                view.reset(option, cx);
                            }
                        }),
                    ));
            }
        }
        toolbar = toolbar
            .child(
                control(
                    "minesweeper-flag-mode",
                    if self.flag_mode {
                        "Flag mode on"
                    } else {
                        "Flag mode"
                    },
                    self.flag_mode,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.flag_mode = !view.flag_mode;
                    cx.notify();
                })),
            )
            .child(
                control(
                    "minesweeper-reset",
                    if self.challenge_seed.is_some() {
                        "Restart"
                    } else {
                        "New field"
                    },
                    false,
                )
                .on_click(cx.listener(|view, _, _, cx| view.reset(view.board.difficulty(), cx))),
            );

        let board = div()
            .flex()
            .flex_col()
            .gap(px(3.))
            .p(px(9.))
            .rounded_lg()
            .bg(rgb(t::RAIL))
            .children((0..height).map(|y| {
                div()
                    .flex()
                    .gap(px(3.))
                    .children((0..width).map(|x| self.cell(x, y, size, cx)))
            }));

        t::surface()
            .flex()
            .flex_col()
            .items_start()
            .gap_3()
            .p_4()
            .text_color(rgb(t::INK))
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(stat("Mines", difficulty.mines()))
                    .child(stat("Flags", self.board.flags()))
                    .child(stat("Safe tiles left", self.board.safe_remaining())),
            )
            .child(board)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(status_color))
                    .child(status),
            )
            .child(
                div().text_size(px(13.)).text_color(rgb(t::MUTED)).child(
                    "Click to reveal. Right-click or use Flag mode to mark a suspected mine.",
                ),
            )
    }
}

fn control(id: &'static str, label: &'static str, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .tab_index(0)
        .cursor_pointer()
        .flex()
        .items_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if selected { t::GOLD } else { t::BORDER }))
        .bg(rgb(if selected { t::GOLD } else { t::SURFACE }))
        .hover(|style| style.bg(rgb(if selected { t::GOLD_HOVER } else { t::SAGE })))
        .focus(|style| style.border_color(rgb(t::MUTED)))
        .px_3()
        .h(px(36.))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}

fn stat(label: &str, count: usize) -> Div {
    div()
        .rounded_md()
        .bg(rgb(t::SAGE))
        .px_2()
        .py_1()
        .text_size(px(13.))
        .child(format!("{label}  {count}"))
}

fn stable_seed(id: &str) -> u64 {
    id.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64)
}
