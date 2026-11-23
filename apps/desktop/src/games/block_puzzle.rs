use corner_table_block_puzzle::{Game, Move, Piece, SIDE, Wood};
use gpui::{Context, Div, FontWeight, Window, div, prelude::*, px, rgb};
use serde::{Deserialize, Serialize};

use crate::storage::Session;
use crate::theme as t;

use super::{GameLaunch, board_space, control};

const BOARD: u32 = 0x49382e;
const WELL: u32 = 0x625043;
const BLOCKED: u32 = 0xc94e4e;

#[derive(Deserialize, Serialize)]
struct SavedRound {
    seed: u64,
    turns: Vec<(usize, usize, usize)>,
    can_undo: bool,
}

#[derive(Clone, Copy)]
struct Preview {
    turn: Move,
    piece: Piece,
    fits: bool,
}

impl Preview {
    fn covers(self, column: usize, row: usize) -> bool {
        self.piece
            .tiles
            .iter()
            .any(|&(x, y)| self.turn.column + x == column && self.turn.row + y == row)
    }
}

pub struct GameView {
    session: Session,
    game: Game,
    seed: u64,
    turns: Vec<(usize, usize, usize)>,
    undo: Option<Game>,
    selected: Option<usize>,
    hovered: Option<(usize, usize)>,
    feedback: String,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _cx: &mut Context<Self>) -> Self {
        let seed = launch.seed();
        let session = Session::new(launch);
        let mut view = Self {
            session,
            game: Game::new(seed),
            seed,
            turns: Vec::new(),
            undo: None,
            selected: None,
            hovered: None,
            feedback: String::new(),
        };
        if let Some(saved) = view.session.load::<SavedRound>() {
            view.restore(saved);
        }
        view
    }

    fn restore(&mut self, saved: SavedRound) {
        let mut game = Game::new(saved.seed);
        let mut undo = None;
        for (index, &(slot, column, row)) in saved.turns.iter().enumerate() {
            if saved.can_undo && index + 1 == saved.turns.len() {
                undo = Some(game.clone());
            }
            if game.place(Move { slot, column, row }).is_none() {
                return;
            }
        }
        self.game = game;
        self.seed = saved.seed;
        self.turns = saved.turns;
        self.undo = undo;
    }

    fn save(&self) {
        self.session.save(&SavedRound {
            seed: self.seed,
            turns: self.turns.clone(),
            can_undo: self.undo.is_some(),
        });
    }

    fn reset(&mut self) {
        self.game = Game::new(self.seed);
        self.turns.clear();
        self.undo = None;
        self.selected = None;
        self.hovered = None;
        self.feedback.clear();
        self.save();
    }

    fn preview(&self) -> Option<Preview> {
        let slot = self.selected?;
        let (column, row) = self.hovered?;
        let piece = self.game.tray()[slot]?;
        let turn = Move { slot, column, row };
        Some(Preview {
            turn,
            piece,
            fits: self.game.can_place(turn),
        })
    }

    fn place(&mut self, column: usize, row: usize) {
        let Some(slot) = self.selected else {
            return;
        };
        let turn = Move { slot, column, row };
        if !self.game.can_place(turn) {
            self.feedback = "That piece needs more space.".into();
            return;
        }
        let before = self.game.clone();
        if let Some(placement) = self.game.place(turn) {
            self.undo = Some(before);
            self.turns.push((slot, column, row));
            self.selected = None;
            self.feedback = match placement.lines {
                0 => format!("+{} points", placement.points),
                1 => format!("Cleared 1 line for +{} points", placement.points),
                n => format!("Cleared {n} lines for +{} points", placement.points),
            };
            self.save();
        }
    }

    fn board(&self, side: f32, cx: &Context<Self>) -> Div {
        let preview = self.preview();
        div()
            .flex()
            .flex_col()
            .flex_none()
            .p_2()
            .gap(px(3.))
            .rounded_lg()
            .bg(rgb(BOARD))
            .children(self.game.board().iter().enumerate().map(|(row, cells)| {
                div()
                    .flex()
                    .gap(px(3.))
                    .children(cells.iter().enumerate().map(|(column, &tile)| {
                        let ghost = preview.filter(|preview| preview.covers(column, row));
                        let fill = if let Some(wood) = tile {
                            wood_color(wood)
                        } else if let Some(preview) = ghost {
                            if preview.fits {
                                wood_color(preview.piece.wood)
                            } else {
                                BLOCKED
                            }
                        } else {
                            WELL
                        };
                        let outline = match ghost {
                            Some(preview) if preview.fits => t::PAPER,
                            Some(_) => BLOCKED,
                            None => fill,
                        };

                        div()
                            .id(("block-puzzle-square", row * SIDE + column))
                            .size(px(side))
                            .rounded_sm()
                            .border_2()
                            .border_color(rgb(outline))
                            .bg(rgb(fill))
                            .on_hover(cx.listener(move |view, hovered: &bool, _, cx| {
                                let here = Some((column, row));
                                if *hovered {
                                    view.hovered = here;
                                } else if view.hovered == here {
                                    view.hovered = None;
                                } else {
                                    return;
                                }
                                if view.selected.is_some() {
                                    cx.notify();
                                }
                            }))
                            .when(self.selected.is_some(), |cell| {
                                cell.tab_index(0)
                                    .cursor_pointer()
                                    .focus(|style| style.border_color(rgb(t::GOLD)))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.place(column, row);
                                        cx.notify();
                                    }))
                            })
                    }))
            }))
    }

    fn tray(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .gap_2()
            .children(self.game.tray().iter().enumerate().map(|(slot, piece)| {
                let selected = self.selected == Some(slot);
                div()
                    .id(("block-puzzle-piece", slot))
                    .size(px(76.))
                    .rounded_md()
                    .border_2()
                    .border_color(rgb(if selected { t::GOLD } else { t::BORDER }))
                    .bg(rgb(if selected { t::SAGE } else { t::PAPER }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(piece.map(piece_shape))
                    .when(piece.is_some(), |card| {
                        card.tab_index(0)
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(t::SAGE)))
                            .focus(|style| style.border_color(rgb(t::INK)))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.selected = (view.selected != Some(slot)).then_some(slot);
                                view.feedback.clear();
                                cx.notify();
                            }))
                    })
            }))
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let side = ((board_space(window) - 37.) / SIDE as f32).clamp(26., 48.);
        let status = if !self.game.has_moves() {
            "No pieces fit. Undo your last move or start a new board."
        } else if !self.feedback.is_empty() {
            self.feedback.as_str()
        } else if self.selected.is_some() {
            "Point to a square to preview the placement, then click."
        } else {
            "Choose a piece to place. Complete rows or columns to clear space."
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
                    .w(px(266.))
                    .max_w_full()
                    .child(t::display_type(
                        format!("{} points", self.game.score()),
                        24.,
                    ))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Pieces"))
                    .child(self.tray(cx))
                    .child(
                        div()
                            .min_h(px(48.))
                            .text_color(rgb(t::MUTED))
                            .child(status.to_owned()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                control("block-puzzle-undo", "Undo", self.undo.is_some()).on_click(
                                    cx.listener(|view, _, _, cx| {
                                        if let Some(game) = view.undo.take() {
                                            view.game = game;
                                            view.turns.pop();
                                            view.selected = None;
                                            view.feedback = "Placement undone.".into();
                                            view.save();
                                            cx.notify();
                                        }
                                    }),
                                ),
                            )
                            .child(control("block-puzzle-restart", "Restart", true).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.reset();
                                    cx.notify();
                                }),
                            ))
                            .child(control("block-puzzle-new", "New board", true).on_click(
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

fn wood_color(wood: Wood) -> u32 {
    match wood {
        Wood::Amber => 0xe1a552,
        Wood::Cherry => 0xcf7771,
        Wood::Juniper => 0x9caf78,
        Wood::Plum => 0xb092bd,
        Wood::Lagoon => 0x79bac2,
    }
}

fn piece_shape(piece: Piece) -> Div {
    let columns = piece.tiles.iter().map(|tile| tile.0).max().unwrap_or(0) + 1;
    let rows = piece.tiles.iter().map(|tile| tile.1).max().unwrap_or(0) + 1;
    let color = wood_color(piece.wood);
    div()
        .relative()
        .w(px(columns as f32 * 15. - 1.))
        .h(px(rows as f32 * 15. - 1.))
        .children(piece.tiles.iter().map(|&(column, row)| {
            div()
                .absolute()
                .left(px(column as f32 * 15.))
                .top(px(row as f32 * 15.))
                .size(px(14.))
                .rounded_sm()
                .bg(rgb(color))
        }))
}
