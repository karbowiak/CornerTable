use corner_table_block_puzzle::{PuzzleGoal, PuzzleStatus};
use gpui::{Context, Div, FontWeight, div, prelude::*, px, rgb};

use super::{GameView, PREVIEW_FITS, WALNUT, t, themed_control};

impl GameView {
    pub(super) fn puzzle_brief(&self, cx: &Context<Self>) -> Div {
        let palette = t::colors(cx);
        let book = &self.puzzles;
        let current = book.current();
        let game = &current.round.game;
        let goal = match current.definition.goal {
            PuzzleGoal::Crossing => "Clear a row and column together.",
            PuzzleGoal::FitTray => "Place all tray pieces.",
            PuzzleGoal::CoverMarks => "Cover all marked squares at once.",
        };
        let remaining = game.placements_left().unwrap_or_default();
        let moves_left = format!("{} left", move_count(remaining));
        let progress = if current.definition.goal == PuzzleGoal::CoverMarks {
            let (covered, total) = current.definition.mark_progress(game);
            format!("{covered}/{total} covered · {moves_left}")
        } else {
            moves_left
        };
        let best = current
            .best_moves
            .filter(|&best| !game.puzzle_solved() || best < current.round.turns.len());
        let solved_count = format!("{}/{} solved", book.solved_count(), book.boards.len());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_baseline()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Boards"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(palette.muted))
                            .child(solved_count),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(3.))
                    .children(book.boards.iter().enumerate().map(|(index, board)| {
                        let number = index + 1;
                        let active = index == book.active;
                        let solved = board.best_moves.is_some();
                        let status = if solved {
                            ", solved"
                        } else if !board.round.turns.is_empty() {
                            ", in progress"
                        } else {
                            ""
                        };
                        themed_control(cx, "block-puzzle-study", number.to_string(), true)
                            .id(("block-puzzle-study", index))
                            .debug_selector(move || format!("block-puzzle-study-{number}"))
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .min_h(px(30.))
                            .px_0()
                            .justify_center()
                            .text_size(px(12.))
                            .aria_label(format!(
                                "Board {number}, {}{status}",
                                board.definition.title
                            ))
                            .aria_toggled(active.into())
                            .when(active, |button| {
                                button
                                    .bg(rgb(palette.hover))
                                    .border_color(rgb(palette.gold_text))
                            })
                            .when(solved, |button| {
                                button.child(
                                    div()
                                        .absolute()
                                        .bottom(px(3.))
                                        .size(px(3.))
                                        .rounded_full()
                                        .bg(rgb(palette.success)),
                                )
                            })
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.choose_puzzle(index, window, cx);
                            }))
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(t::display_type(current.definition.title, 22.))
                    .child(div().text_size(px(13.)).line_height(px(17.)).child(goal))
                    .when(!game.puzzle_solved() || best.is_some(), |brief| {
                        brief.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_x_2()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(rgb(palette.muted))
                                .when(!game.puzzle_solved(), |details| details.child(progress))
                                .children(best.map(|best| format!("Best {}", move_count(best)))),
                        )
                    }),
            )
    }

    pub(super) fn puzzle_status(&self) -> String {
        let round = &self.puzzles.current().round;
        match round.game.puzzle_status() {
            Some(PuzzleStatus::Solved) => {
                format!("Solved in {}.", move_count(round.turns.len()))
            }
            Some(PuzzleStatus::OutOfMoves) => "Goal missed. Undo or restart.".into(),
            Some(PuzzleStatus::Blocked) => "No pieces fit. Undo or restart.".into(),
            _ => String::new(),
        }
    }
}

fn move_count(count: usize) -> String {
    match count {
        1 => "1 move".into(),
        _ => format!("{count} moves"),
    }
}

pub(super) fn marked_square(cell: f32, filled: bool) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(if filled {
                    (cell * 0.18).clamp(6., 10.)
                } else {
                    (cell * 0.28).clamp(7., 15.)
                }))
                .rounded_sm()
                .when(filled, |mark| {
                    mark.bg(rgb(PREVIEW_FITS))
                        .border_1()
                        .border_color(rgb(WALNUT))
                })
                .when(!filled, |mark| {
                    mark.border_2().border_color(rgb(PREVIEW_FITS))
                }),
        )
}
