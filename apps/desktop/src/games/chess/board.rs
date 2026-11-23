use std::time::Duration;

use corner_table_chess::{Color, File, Move, Piece, Rank, Square};
use gpui::{
    Animation, AnimationExt, AppContext as _, BoxShadow, Context, Div, EntityId, FontWeight, Img,
    ObjectFit, Render, Role, Stateful, Window, div, ease_out_quint, img, prelude::*, px, rgb, rgba,
};

use crate::{motion, theme};

use super::{GameView, PROMOTIONS, Promotion, piece_name, side_name};

pub(super) const FRAME: f32 = 23.;
const FRAME_INK: u32 = 0x192e39;
const CREAM: u32 = 0xf3e8ce;
const BRASS: u32 = 0xf8d785;

impl GameView {
    pub(super) fn board(&self, tile: f32, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let flipped = self.table.flipped;
        let mut files = File::ALL;
        let mut ranks = Rank::ALL;
        if flipped {
            files.reverse();
        } else {
            ranks.reverse();
        }
        let focused = self.board_focus.is_focused(window);
        let keyboard_cursor = focused && window.last_input_was_keyboard();
        let side = 8. * tile + 2. * FRAME;
        let input = self.moves_for_input();
        let arrival = self.arrival.filter(|_| motion::enabled(cx));
        div()
            .id("chess-board")
            .role(Role::Grid)
            .aria_label(if flipped { "Chess board, Black side" } else { "Chess board, White side" })
            .aria_row_count(8)
            .aria_column_count(8)
            .aria_description(if self.game().ending().is_some() {
                "Arrow keys review squares. U undoes. F flips the board."
            } else {
                "Arrow keys move the cursor. Enter or Space selects a piece or destination. U undoes. F flips the board."
            })
            .track_focus(&self.board_focus)
            .when(self.promotion.is_none(), |board| board.tab_index(0))
            .on_key_down(cx.listener(Self::key_down))
            .size(px(side))
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .rounded(px(10.))
            .overflow_hidden()
            .bg(rgb(FRAME_INK))
            .shadow(vec![BoxShadow::new(px(0.), px(7.), rgba(0x15232a45).into()).blur_radius(px(18.))])
            .child(
                div().absolute().inset(px(4.)).rounded(px(7.)).border_1().border_color(rgb(0x59727b)),
            )
            .child(file_labels(files, tile))
            .children(ranks.into_iter().enumerate().map(|(row, rank)| {
                let mark = (rank as u8 + 1).to_string();
                div()
                    .id(("chess-rank", row))
                    .role(Role::Row)
                    .aria_row_index(row)
                    .flex()
                    .flex_none()
                    .child(axis(FRAME, tile, mark.clone()))
                    .children(files.into_iter().map(|file| {
                        self.cell(Square::new(file, rank), tile, (focused, keyboard_cursor), input, arrival, cx)
                    }))
                    .child(axis(FRAME, tile, mark))
            }))
            .child(file_labels(files, tile))
            .children(self.promotion.map(|promotion| self.promotion_picker(promotion, window, cx)))
    }

    fn cell(
        &self,
        square: Square,
        tile: f32,
        (focused, keyboard_cursor): (bool, bool),
        input: &[Move],
        arrival: Option<Move>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let file = usize::from(square.file() as u8);
        let column = if self.table.flipped { 7 - file } else { file };
        let occupant = self.game().piece(square);
        let selected = self.selected == Some(square);
        let keyed = focused && self.cursor == square;
        let legal_source = input.iter().any(|movement| movement.from == square);
        let target = input
            .iter()
            .find(|movement| self.selected == Some(movement.from) && square == movement.to);
        let checked = self.game().checked_king() == Some(square);
        let last =
            self.game().moves().next_back().is_some_and(|played| {
                played.movement.from == square || played.movement.to == square
            });
        let dark = (square.rank() as u8 + square.file() as u8).is_multiple_of(2);
        let surface = match (checked, last, dark) {
            (true, _, true) => 0xa7524c,
            (true, _, false) => 0xdd9a87,
            (false, true, true) => 0x81804d,
            (false, true, false) => 0xeed4a0,
            (false, false, true) => 0x5d8190,
            (false, false, false) => 0xe6dbbe,
        };
        let label = occupant.map_or_else(
            || format!("{square}, empty"),
            |(color, piece)| format!("{square}, {} {}", side_name(color), piece_name(piece)),
        );
        let capture = target.is_some_and(|m| occupant.is_some() || is_en_passant(self, *m));
        let castle = target.is_some_and(|m| {
            self.game()
                .piece(m.from)
                .is_some_and(|(_, piece)| piece == Piece::King)
                && (m.from.file() as u8).abs_diff(m.to.file() as u8) == 2
        });
        let description = if checked {
            "King in check"
        } else if castle {
            "Castle here"
        } else if capture {
            "Capture here"
        } else if target.is_some() {
            "Legal destination"
        } else if legal_source && self.claim_mode {
            "Has a draw-claim move"
        } else if legal_source {
            "Has legal moves"
        } else if last {
            "Last move"
        } else {
            ""
        };

        let active = self.human_turn() && self.confirmation.is_none() && self.promotion.is_none();
        let origin = cx.entity_id();
        let revision = self.revision;
        let mut cell = div()
            .id(("chess-square", square as usize))
            .role(Role::GridCell)
            .aria_column_index(column)
            .aria_label(label)
            .aria_selected(selected)
            .when(!description.is_empty(), |cell| {
                cell.aria_description(description)
            })
            .when(keyed, |cell| cell.aria_active_descendant())
            .when(active, |cell| {
                cell.on_click(cx.listener(move |view, _, window, cx| {
                    view.board_focus.focus(window, cx);
                    view.activate(square, window, cx);
                }))
            })
            .when(legal_source || target.is_some(), |cell| {
                cell.cursor_pointer()
                    .hover(move |style| style.bg(rgb(if dark { 0x71919e } else { 0xf1e7cf })))
            })
            .relative()
            .size(px(tile))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(surface))
            .when_some(occupant, |cell, (color, piece)| {
                let image = piece_image(color, piece, tile * 0.91);
                if let Some(movement) = arrival.filter(|movement| movement.to == square) {
                    let direction = if self.table.flipped { -1. } else { 1. };
                    let file = movement.from.file() as i8 - square.file() as i8;
                    let rank = square.rank() as i8 - movement.from.rank() as i8;
                    let offset = (tile * 0.15).min(9.) * direction;
                    let dx = f32::from(file.signum()) * offset;
                    let dy = f32::from(rank.signum()) * offset;
                    cell.child(image.with_animation(
                        ("chess-arrival", revision),
                        Animation::new(Duration::from_millis(185)).with_easing(ease_out_quint()),
                        move |image, progress| {
                            image
                                .relative()
                                .left(px(dx * (1. - progress)))
                                .top(px(dy * (1. - progress)))
                        },
                    ))
                } else {
                    cell.child(image)
                }
            })
            .when(checked, |cell| cell.child(outline(2., 2., 0x8c332b)))
            .when(last && !checked, |cell| {
                cell.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .h(px(3.))
                        .w(px(tile * 0.34))
                        .bg(rgb(BRASS)),
                )
            })
            .when(target.is_some(), |cell| {
                cell.child(target_marker(tile, capture))
            })
            .when(
                self.claim_mode && legal_source && self.selected.is_none(),
                |cell| {
                    cell.child(
                        div()
                            .absolute()
                            .top(px(5.))
                            .right(px(5.))
                            .size(px(8.))
                            .rounded_full()
                            .bg(rgb(BRASS))
                            .border_1()
                            .border_color(rgb(FRAME_INK)),
                    )
                },
            )
            .when(selected, |cell| {
                cell.child(outline(1., 3., 0x563f29))
                    .child(outline(3., 2., BRASS))
            })
            .when(keyed && keyboard_cursor, |cell| {
                cell.child(outline(5., 3., FRAME_INK))
                    .child(outline(6., 1.5, 0xffffff))
            });

        if active {
            let sources: Vec<_> = input
                .iter()
                .filter(|movement| movement.to == square)
                .map(|movement| movement.from)
                .collect();
            cell = cell
                .drag_over(move |style, drag: &DraggedPiece, _, _| {
                    if drag.origin == origin
                        && drag.revision == revision
                        && sources.contains(&drag.from)
                    {
                        style.bg(rgb(0xbdab72))
                    } else {
                        style
                    }
                })
                .on_drop(cx.listener(move |view, drag: &DraggedPiece, window, cx| {
                    if drag.origin == cx.entity_id() && drag.revision == view.revision {
                        view.board_focus.focus(window, cx);
                        view.try_destination(drag.from, square, window, cx);
                    }
                }));
        }
        if let Some((color, piece)) = occupant.filter(|_| legal_source) {
            let view = cx.weak_entity();
            cell = cell.cursor_grab().on_drag(
                DraggedPiece {
                    from: square,
                    origin,
                    revision,
                    color,
                    piece,
                    tile,
                },
                move |drag, _, window, cx| {
                    if let Some(view) = view.upgrade() {
                        view.update(cx, |view, cx| {
                            if view.revision == revision {
                                view.cursor = square;
                                view.selected = Some(square);
                                view.notice = None;
                                view.board_focus.focus(window, cx);
                                cx.notify();
                            }
                        });
                    }
                    cx.new(|_| *drag)
                },
            );
        }
        cell
    }

    fn promotion_picker(
        &self,
        promotion: Promotion,
        window: &Window,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let palette = theme::colors(cx);
        let color = self.game().side_to_move();
        let row_focused = self.promotion_focus.is_focused(window);
        let from = promotion.movement.from;
        let to = promotion.movement.to;
        div()
            .id("chess-promotion")
            .role(Role::Group)
            .aria_label(format!("Choose a promotion piece for {to}"))
            .aria_description("Use arrows and Enter, or press Q, R, B or N. Escape cancels.")
            .track_focus(&self.promotion_focus)
            .tab_index(0)
            .absolute()
            .inset_0()
            .bg(rgba(0x10232bb3))
            .flex()
            .items_center()
            .justify_center()
            .child(
                theme::panel(cx)
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(theme::display_type("Promote pawn", 19.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(palette.muted))
                            .child(format!("{from} to {to}. Choose a piece.")),
                    )
                    .child(
                        div().flex().gap_1().children(
                            PROMOTIONS
                                .into_iter()
                                .zip([
                                    ("Queen", "Q"),
                                    ("Rook", "R"),
                                    ("Bishop", "B"),
                                    ("Knight", "N"),
                                ])
                                .enumerate()
                                .map(|(index, (piece, (label, key)))| {
                                    let keyed = promotion.choice == index && row_focused;
                                    div()
                                        .id(("chess-promote-piece", index))
                                        .role(Role::Button)
                                        .aria_label(format!("Promote to {label}. Shortcut {key}."))
                                        .when(keyed, |button| button.aria_active_descendant())
                                        .tab_index(0)
                                        .cursor_pointer()
                                        .w(px(61.))
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .rounded_md()
                                        .border_2()
                                        .border_color(rgb(if keyed {
                                            palette.gold_text
                                        } else {
                                            palette.border
                                        }))
                                        .bg(rgb(palette.paper))
                                        .hover(|style| style.bg(rgb(palette.hover)))
                                        .focus_visible(|style| style.border_color(rgb(palette.ink)))
                                        .child(piece_image(color, piece, 42.))
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .line_height(px(18.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(label),
                                        )
                                        .on_click(cx.listener(move |view, _, window, cx| {
                                            view.promote(piece, window, cx)
                                        }))
                                }),
                        ),
                    )
                    .child(
                        super::super::themed_control(cx, "chess-cancel-promotion", "Cancel", true)
                            .self_start()
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.cancel(window, cx);
                            })),
                    ),
            )
    }
}

fn is_en_passant(view: &GameView, movement: Move) -> bool {
    movement.from.file() != movement.to.file()
        && view
            .game()
            .piece(movement.from)
            .is_some_and(|(_, piece)| piece == Piece::Pawn)
        && view.game().piece(movement.to).is_none()
}

fn outline(inset: f32, width: f32, stroke: u32) -> Div {
    div()
        .absolute()
        .inset(px(inset))
        .border(px(width))
        .border_color(rgb(stroke))
        .rounded(px(3.))
}

fn target_marker(tile: f32, capture: bool) -> Div {
    if capture {
        div()
            .absolute()
            .inset(px(4.))
            .border(px(3.))
            .border_color(rgb(BRASS))
            .rounded(px(5.))
            .child(outline(1., 1., FRAME_INK))
    } else {
        div()
            .absolute()
            .size(px((tile * 0.19).clamp(7., 13.)))
            .rounded_full()
            .bg(rgb(FRAME_INK))
            .border(px(1.5))
            .border_color(rgb(CREAM))
    }
}

fn axis(width: f32, height: f32, mark: String) -> Div {
    div()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(CREAM))
        .text_size(px(12.))
        .line_height(px(14.))
        .font_weight(FontWeight::BOLD)
        .child(mark)
}

fn file_labels(files: [File; 8], tile: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .px(px(FRAME))
        .children(files.map(|file| axis(tile, FRAME, char::from(b'a' + file as u8).to_string())))
}

pub(super) fn piece_image(color: Color, piece: Piece, diameter: f32) -> Img {
    let side = if color == Color::White {
        "white"
    } else {
        "black"
    };
    img(format!("games/chess/{side}-{}.svg", piece_name(piece)))
        .size(px(diameter))
        .object_fit(ObjectFit::Contain)
        .flex_none()
}

#[derive(Clone, Copy)]
struct DraggedPiece {
    from: Square,
    origin: EntityId,
    revision: usize,
    color: Color,
    piece: Piece,
    tile: f32,
}

impl Render for DraggedPiece {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(px(self.tile))
            .flex()
            .items_center()
            .justify_center()
            .child(piece_image(self.color, self.piece, self.tile * 0.96))
    }
}
