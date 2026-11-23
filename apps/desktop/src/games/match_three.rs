use corner_table_match_three::{Cell, GOAL, Game, Gem, SIDE, Status, SwapError};
use gpui::{Context, Div, FontWeight, Window, div, prelude::*, px, rgb};

use crate::theme as t;

use super::{GameLaunch, board_space, control};

const BOARD: u32 = 0x183a34;
const WELL: u32 = 0x244c42;
const WELL_HOVER: u32 = 0x315f52;

pub struct GameView {
    game: Game,
    seed: u64,
    selected: Option<Cell>,
    hint: Option<(Cell, Cell)>,
    feedback: String,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _cx: &mut Context<Self>) -> Self {
        let seed = launch.seed();
        Self {
            game: Game::new(seed),
            seed,
            selected: None,
            hint: None,
            feedback: String::new(),
        }
    }

    fn reset(&mut self) {
        self.game = Game::new(self.seed);
        self.selected = None;
        self.hint = None;
        self.feedback.clear();
    }

    fn choose(&mut self, cell: Cell) {
        if self.game.status() != Status::Playing {
            return;
        }
        self.hint = None;
        self.feedback.clear();
        match self.selected.take() {
            Some(first) if first == cell => {}
            Some(first) if first.touches(cell) => match self.game.swap(first, cell) {
                Ok(turn) => {
                    let cascades = turn.cascades.len();
                    self.feedback = if turn.reshuffled {
                        format!("+{} points. The board was reshuffled.", turn.points)
                    } else if cascades > 1 {
                        format!("+{} points from a {cascades}-match chain.", turn.points)
                    } else {
                        format!("+{} points for {} gems.", turn.points, turn.gems)
                    };
                }
                Err(SwapError::NoMatch) => {
                    self.feedback = "That swap needs to make a line of three or more.".into();
                }
                Err(_) => {}
            },
            _ => self.selected = Some(cell),
        }
    }

    fn board(&self, side: f32, cx: &Context<Self>) -> Div {
        let playing = self.game.status() == Status::Playing;
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(4.))
            .p_2()
            .rounded_lg()
            .bg(rgb(BOARD))
            .children((0..SIDE).map(|row| {
                div().flex().gap(px(4.)).children((0..SIDE).map(|col| {
                    let cell = Cell { row, col };
                    let gem = self.game.gem(cell).expect("board cell");
                    let highlighted = self.selected == Some(cell)
                        || self.hint.is_some_and(|(a, b)| cell == a || cell == b);
                    div()
                        .id(("match-three-cell", row * SIDE + col))
                        .size(px(side))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .bg(rgb(WELL))
                        .border_2()
                        .border_color(rgb(if highlighted { t::GOLD } else { WELL }))
                        .child(gem_face(gem, side))
                        .when(playing, |tile| {
                            tile.tab_index(0)
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(WELL_HOVER)))
                                .focus(|style| style.border_color(rgb(t::PAPER)))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.choose(cell);
                                    cx.notify();
                                }))
                        })
                }))
            }))
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let space = board_space(window);
        let side = ((space - 44.) / SIDE as f32).clamp(28., 52.);
        let playing = self.game.status() == Status::Playing;
        let message = match self.game.status() {
            Status::Won => "You reached the score goal!",
            Status::Lost => "Out of moves. Restart this board or try a new one.",
            Status::Playing if !self.feedback.is_empty() => self.feedback.as_str(),
            Status::Playing if self.hint.is_some() => "Swap the two outlined gems.",
            Status::Playing if self.selected.is_some() => "Choose an adjacent gem to swap.",
            Status::Playing => "Swap neighboring gems to make a row or column of three or more.",
        };

        t::surface()
            .p_4()
            .w_full()
            .flex()
            .flex_wrap()
            .gap_5()
            .items_start()
            .child(self.board(side, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(270.))
                    .max_w_full()
                    .child(t::display_type(
                        format!("{} points", self.game.score()),
                        24.,
                    ))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                        "Goal: {GOAL}   ·   Moves left: {}",
                        self.game.moves_left()
                    )))
                    .child(
                        div()
                            .min_h(px(48.))
                            .text_color(rgb(t::MUTED))
                            .child(message.to_owned()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(control("match-three-hint", "Hint", playing).on_click(
                                cx.listener(move |view, _, _, cx| {
                                    if playing {
                                        view.selected = None;
                                        view.hint = view.game.hint();
                                        view.feedback.clear();
                                        cx.notify();
                                    }
                                }),
                            ))
                            .child(control("match-three-restart", "Restart", true).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.reset();
                                    cx.notify();
                                }),
                            ))
                            .child(control("match-three-new", "New board", true).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.seed = view.seed.wrapping_add(1);
                                    view.reset();
                                    cx.notify();
                                }),
                            )),
                    ),
            )
    }
}

fn gem_face(gem: Gem, side: f32) -> Div {
    let (mark, face, rim, ink) = match gem {
        Gem::Ruby => ("R", 0xdb7983, 0xf3a6a6, 0x502031),
        Gem::Amber => ("A", 0xf0b65c, 0xffd58e, 0x5d3b15),
        Gem::Peridot => ("P", 0xb4cb70, 0xd5e69e, 0x344219),
        Gem::Aquamarine => ("Q", 0x73c4ca, 0x9ce7e5, 0x19494b),
        Gem::Amethyst => ("V", 0xb5a0db, 0xe3caff, 0x442960),
        Gem::Moonstone => ("M", 0xeee1c7, 0xfff8e7, 0x50483c),
    };
    div()
        .size(px(side - 10.))
        .rounded_md()
        .border_1()
        .border_color(rgb(rim))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(face))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(ink))
        .text_size(px((side * 0.36).max(12.)))
        .child(mark)
}
