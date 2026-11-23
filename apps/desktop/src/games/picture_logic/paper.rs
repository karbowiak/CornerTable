use corner_table_picture_logic::{Cell, PUZZLES, Series};
use gpui::{
    Animation, AnimationExt as _, BoxShadow, Context, Div, ElementId, FontWeight, Role,
    SharedString, Stateful, Window, div, prelude::*, px, relative, rgb,
};

use crate::{games::round_result, motion, storage::Outcome, theme};

use super::{
    Action, GameView, Notice, REVEAL_DURATION, Tool,
    board::{cross, impression, proof},
    plate_name,
};

pub(super) const PRINT_STOCK: u32 = 0xf7edda;
const TOOLS_WIDTH: f32 = 248.;
const COLUMN_GAP: f32 = 18.;

#[derive(Clone, Copy)]
pub(super) struct Colors {
    pub table: u32,
    pub stock: u32,
    pub clue: u32,
    pub track: u32,
    pub active: u32,
    pub rule: u32,
    pub major: u32,
    pub ink: u32,
    pub quiet: u32,
    pub fill: u32,
    pub mark: u32,
    pub select: u32,
    pub hint: u32,
    pub alert: u32,
    button_ink: u32,
}

impl Colors {
    pub(super) fn new(cx: &gpui::App) -> Self {
        if theme::colors(cx) == theme::LIGHT {
            Self {
                table: 0xe5e9dc,
                stock: 0xfffbef,
                clue: 0xf0f0e2,
                track: 0xe1ecd9,
                active: 0xcfe2cb,
                rule: 0xd3d8c9,
                major: 0x8b9b8a,
                ink: 0x223e35,
                quiet: 0x536b5a,
                fill: 0x2c584e,
                mark: 0x6c6256,
                select: 0xac4e34,
                hint: 0x885411,
                alert: 0xa63d37,
                button_ink: 0xfffbef,
            }
        } else {
            Self {
                table: 0x172923,
                stock: 0x293c35,
                clue: 0x31463d,
                track: 0x3a5348,
                active: 0x4c6859,
                rule: 0x4c6056,
                major: 0x728b7d,
                ink: 0xf4efe3,
                quiet: 0xb8c9b7,
                fill: 0xc2d396,
                mark: 0xd4dccc,
                select: 0xffbd94,
                hint: 0xffd789,
                alert: 0xff9f97,
                button_ink: 0x1a2a23,
            }
        }
    }
}

impl GameView {
    pub(super) fn render_plate(
        &self,
        width: f32,
        paint: Colors,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let play = self.book.play();
        let puzzle = play.board.puzzle();
        let wide = width >= 856.;
        let show_guide = self.guide.unwrap_or(wide && self.book.active == 0);
        let inner = width - if wide { TOOLS_WIDTH + COLUMN_GAP } else { 0. };
        let shape = format!(
            "{} × {} · {}",
            puzzle.width(),
            puzzle.height(),
            puzzle.series.label()
        );
        let head = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_baseline()
                    .gap_2()
                    .child(theme::display_type(plate_name(self.book.active), 23.))
                    .child(small(shape, cx)),
            )
            .child(div().flex().flex_wrap().gap_2().when(!play.solved, |line| {
                line.when(!wide, |line| line.child(guide_toggle(show_guide, cx)))
                    .child(control(
                        "picture-gallery",
                        "Puzzle gallery",
                        Button::Normal,
                        Action::Gallery,
                        cx,
                    ))
            }));

        let page = div().w_full().flex().flex_col().gap_3().child(head);
        if play.solved {
            return page.child(self.finished(width, paint, cx));
        }
        let padding = if inner >= 600. { 24. } else { 12. };
        let margin = if inner >= 600. { 13. } else { 9. };
        let active = self.hover.unwrap_or(play.cursor);
        let cell = play.board.cells()[active];
        let (grid, scrolls) = self.grid(
            inner - (padding + margin) * 2. - 2.,
            !wide,
            paint,
            window,
            cx,
        );
        let plate = div()
            .id("picture-logic-grid")
            .role(Role::Grid)
            .aria_label(self.grid_label(window))
            .aria_description("Arrows move. F selects Fill, X selects Mark, and E selects Erase. Space applies the tool. H gives a hint.")
            .track_focus(&self.board_focus)
            .on_key_down(cx.listener(Self::key_down))
            .min_w_0()
            .w_full()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .bg(rgb(paint.table))
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(paint.rule))
            .pt(px(padding))
            .pb_2()
            .px(px(padding))
            .child(
                div()
                    .p(px(margin))
                    .bg(rgb(paint.stock))
                    .shadow(vec![
                        BoxShadow::new(px(0.), px(3.), rgb(0x0d2219).opacity(0.24).into())
                            .blur_radius(px(9.)),
                    ])
                    .child(grid),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(20.))
                    .text_color(rgb(paint.quiet))
                    .child(format!(
                        "Row {}  ·  Column {}  ·  {}{}",
                        active / puzzle.width() + 1,
                        active % puzzle.width() + 1,
                        cell.label(),
                        if scrolls { "  ·  Scroll for more rows" } else { "" },
                    )),
            );
        let tools = self.tool_strip(paint, cx);
        let actions = self.action_strip(cx);
        let progress = self.progress(paint, !wide, cx);
        let status = self.status(paint);
        if wide {
            page.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(COLUMN_GAP))
                    .child(plate)
                    .child(
                        div()
                            .w(px(TOOLS_WIDTH))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(tools)
                            .child(progress)
                            .child(actions)
                            .children(status)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_start()
                                    .gap_2()
                                    .child(small("Drag to paint. Right-click to mark.", cx))
                                    .child(guide_toggle(show_guide, cx))
                                    .when(show_guide, |help| help.child(guide(paint, cx))),
                            ),
                    ),
            )
        } else {
            page.when(show_guide, |page| {
                page.child(
                    div()
                        .p_3()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(rgb(paint.rule))
                        .bg(rgb(paint.table))
                        .child(guide(paint, cx)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(tools.min_w(px(195.)).flex_1())
                    .child(actions.flex_shrink_0()),
            )
            .child(progress)
            .child(
                div()
                    .h(px(20.))
                    .children(status)
                    .when(self.notice.is_none(), |line| {
                        line.child(small("Drag to paint. Right-click to mark.", cx))
                    }),
            )
            .child(plate)
        }
    }

    fn tool_strip(&self, paint: Colors, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .gap_1()
            .children(Tool::ALL.into_iter().map(|tool| {
                let title = div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(swatch(tool, paint))
                    .child(tool.label());
                let button = if self.tool == tool {
                    Button::Selected
                } else {
                    Button::Normal
                };
                press(
                    ("picture-tool", tool as usize),
                    tool.label(),
                    button,
                    Action::Tool(tool),
                    cx,
                )
                .aria_description(format!(
                    "{} selects this tool. Space applies it.",
                    tool.key()
                ))
                .flex_1()
                .px_1p5()
                .child(title)
            }))
    }

    fn progress(&self, paint: Colors, compact: bool, cx: &Context<Self>) -> Div {
        let play = self.book.play();
        let board = &play.board;
        let (placed, need) = board.ink();
        let (rows, columns) = board.matched();
        let rows_left = rows.iter().filter(|met| !**met).count();
        let columns_left = columns.iter().filter(|met| !**met).count();
        let fills = match need.cmp(&placed) {
            std::cmp::Ordering::Greater => match need - placed {
                1 => "1 square to fill".to_owned(),
                remaining => format!("{remaining} squares to fill"),
            },
            std::cmp::Ordering::Equal => "All ink placed".to_owned(),
            std::cmp::Ordering::Less => match placed - need {
                1 => "1 extra filled square".to_owned(),
                extra => format!("{extra} extra filled squares"),
            },
        };
        let total_lines = rows.len() + columns.len();
        let matched_lines = total_lines - rows_left - columns_left;
        let palette = theme::colors(cx);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .when(!compact, |meter| {
                meter.border_b_1().border_color(rgb(palette.border)).pb_3()
            })
            .child(
                div()
                    .flex()
                    .gap_1()
                    .when(compact, |labels| {
                        labels.flex_wrap().items_baseline().justify_between()
                    })
                    .when(!compact, |labels| labels.flex_col())
                    .child(
                        div()
                            .text_size(px(if compact { 14. } else { 16. }))
                            .font_weight(FontWeight::BOLD)
                            .child(fills),
                    )
                    .child(small(
                        format!("Rows left {rows_left}  ·  Columns left {columns_left}"),
                        cx,
                    )),
            )
            .child(
                div()
                    .id("picture-matched-clues")
                    .role(Role::ProgressIndicator)
                    .aria_label("Clues matched")
                    .aria_numeric_value(matched_lines as f64)
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(total_lines as f64)
                    .h(px(3.))
                    .w_full()
                    .rounded(px(2.))
                    .bg(rgb(paint.rule))
                    .child(
                        div()
                            .h_full()
                            .w(relative(matched_lines as f32 / total_lines as f32))
                            .rounded(px(2.))
                            .bg(rgb(paint.fill)),
                    ),
            )
    }

    fn status(&self, paint: Colors) -> Option<Stateful<Div>> {
        self.notice.map(|notice| {
            let (text, color) = match notice {
                Notice::Checked(0) => {
                    ("Every mark agrees with the solution.".to_owned(), paint.ink)
                }
                Notice::Checked(1) => (
                    "One incorrect square is highlighted.".to_owned(),
                    paint.alert,
                ),
                Notice::Checked(count) => (
                    format!("{count} incorrect squares are highlighted."),
                    paint.alert,
                ),
                Notice::Hint(hint) => (
                    format!(
                        "{} {} forces the highlighted square to be {}.",
                        hint.axis.label(),
                        hint.line + 1,
                        if hint.cell == Cell::Filled {
                            "filled"
                        } else {
                            "empty"
                        },
                    ),
                    paint.ink,
                ),
                Notice::NoDeduction => (
                    "Compare two crossing clues for the next move.".to_owned(),
                    paint.ink,
                ),
            };
            div()
                .id("picture-status")
                .role(Role::Status)
                .aria_label(text.clone())
                .w_full()
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(rgb(color))
                .child(text)
        })
    }

    fn action_strip(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(control(
                "picture-check",
                "Check",
                Button::Normal,
                Action::Check,
                cx,
            ))
            .child(control(
                "picture-hint",
                "Hint",
                Button::Normal,
                Action::Hint,
                cx,
            ))
            .child(control(
                "picture-undo",
                "Undo",
                if self.book.play().can_undo() {
                    Button::Normal
                } else {
                    Button::Disabled
                },
                Action::Undo,
                cx,
            ))
    }

    fn finished(&self, width: f32, paint: Colors, cx: &Context<Self>) -> Stateful<Div> {
        let play = self.book.play();
        let puzzle = play.board.puzzle();
        let next = self.book.next_unfinished();
        let wide = width >= 620.;
        let art_width = if wide { width * 0.42 } else { width - 64. }.clamp(185., 326.);
        let step = (art_width / puzzle.width() as f32).floor();
        let animate = self
            .reveal_at
            .is_some_and(|at| at.elapsed() < REVEAL_DURATION)
            && motion::enabled(cx);
        let artwork = div()
            .debug_selector(|| "picture-finished-print".into())
            .relative()
            .flex_none()
            .p_3()
            .bg(rgb(PRINT_STOCK))
            .border_1()
            .border_color(rgb(0xc8ab7d))
            .child(impression(puzzle, None, step, paint))
            .when(animate, |art| {
                art.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .bg(rgb(PRINT_STOCK))
                        .with_animation(
                            ("picture-ink-reveal", self.book.active),
                            Animation::new(REVEAL_DURATION)
                                .with_easing(|progress| 1. - (1. - progress).powi(3)),
                            |paper, progress| paper.h(relative(1. - progress)),
                        ),
                )
            });
        let moves = play.moves;
        let detail = if moves == 0 {
            "Plate solved".to_owned()
        } else {
            format!(
                "{moves} {} · {}",
                if moves == 1 { "turn" } else { "turns" },
                if play.used_hint {
                    "Solved with hints"
                } else {
                    "Clean solve"
                },
            )
        };
        let actions = div()
            .flex()
            .flex_col()
            .items_start()
            .gap_2()
            .child(div().child(puzzle.caption))
            .when(next.is_none(), |actions| {
                actions.child(small(format!("All {} plates complete.", PUZZLES.len()), cx))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .when(next.is_some(), |row| {
                        row.child(
                            control(
                                "picture-next",
                                "Next unfinished puzzle",
                                Button::Primary,
                                Action::Next,
                                cx,
                            )
                            .debug_selector(|| "picture-next".into()),
                        )
                    })
                    .child(control(
                        "picture-result-gallery",
                        "Puzzle gallery",
                        if next.is_some() {
                            Button::Normal
                        } else {
                            Button::Primary
                        },
                        Action::Gallery,
                        cx,
                    )),
            );
        let details = round_result(
            cx,
            "picture-logic-result",
            Outcome::Won,
            puzzle.title,
            detail,
            actions,
        );
        div()
            .id("picture-logic-complete")
            .role(Role::Group)
            .aria_label(format!(
                "{} solved. {}",
                plate_name(self.book.active),
                puzzle.title
            ))
            .aria_description("Enter opens the next unfinished puzzle. G opens the gallery.")
            .track_focus(&self.board_focus)
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .when(!wide, |layout| layout.flex_col())
            .gap(px(if wide { 42. } else { 18. }))
            .p(px(if wide { 32. } else { 16. }))
            .rounded(px(8.))
            .bg(rgb(paint.table))
            .border_1()
            .border_color(rgb(paint.rule))
            .child(artwork)
            .child(
                div()
                    .min_w_0()
                    .w_full()
                    .when(wide, |panel| panel.flex_1())
                    .child(details),
            )
    }

    pub(super) fn render_gallery(
        &self,
        series: Series,
        width: f32,
        paint: Colors,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.book.active;
        let plates = self
            .book
            .plays
            .iter()
            .enumerate()
            .filter(|(_, play)| play.board.puzzle().series == series);
        let columns = if width >= 900. {
            5.
        } else if width >= 590. {
            3.
        } else {
            2.
        };
        let tile_width = ((width - 12. * (columns - 1.)) / columns).floor();
        let solved = self.book.plays.iter().filter(|play| play.solved).count();
        let palette = theme::colors(cx);
        div()
            .id("picture-logic-gallery")
            .role(Role::Group)
            .aria_label("Puzzle gallery")
            .track_focus(&self.gallery_focus)
            .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    view.act(Action::BackToPlate, window, cx);
                    cx.stop_propagation();
                }
            }))
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_baseline()
                            .gap_2()
                            .child(theme::display_type("Puzzle gallery", 23.))
                            .child(small(format!("{solved} of {} solved", PUZZLES.len()), cx)),
                    )
                    .child(control(
                        "picture-return",
                        format!("Back to {}", plate_name(selected)),
                        Button::Normal,
                        Action::BackToPlate,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .items_center()
                    .children(Series::ALL.into_iter().enumerate().map(|(i, category)| {
                        control(
                            ("picture-series", i),
                            category.label(),
                            if series == category {
                                Button::Selected
                            } else {
                                Button::Normal
                            },
                            Action::Series(category),
                            cx,
                        )
                    }))
                    .child(small(series.sizes(), cx).px_2()),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(12.))
                    .children(plates.map(|(index, play)| {
                        let puzzle = play.board.puzzle();
                        let number = plate_name(index);
                        let status = if play.solved {
                            "Solved"
                        } else if play.started() {
                            "In progress"
                        } else {
                            "New"
                        };
                        let dimensions = format!("{} × {}", puzzle.width(), puzzle.height());
                        let title = if play.solved {
                            puzzle.title.to_owned()
                        } else {
                            number.clone()
                        };
                        div()
                            .id(("picture-gallery-plate", index))
                            .role(Role::Button)
                            .aria_label(format!("{} · {} · {}", title, dimensions, status))
                            .aria_selected(index == selected)
                            .tab_index(0)
                            .cursor_pointer()
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.act(Action::Open(index), window, cx);
                            }))
                            .w(px(tile_width))
                            .p_2()
                            .rounded(px(7.))
                            .border_1()
                            .border_color(rgb(if index == selected {
                                paint.fill
                            } else {
                                palette.border
                            }))
                            .bg(rgb(palette.surface))
                            .hover(move |style| style.bg(rgb(palette.hover)))
                            .focus_visible(move |style| style.border_color(rgb(paint.select)))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .h(px(105.))
                                    .bg(rgb(if play.solved {
                                        PRINT_STOCK
                                    } else {
                                        paint.table
                                    }))
                                    .child(proof(&play.board, play.solved, paint)),
                            )
                            .child(
                                div()
                                    .mt_2()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(14.))
                                    .child(title),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_baseline()
                                    .justify_between()
                                    .gap_1()
                                    .child(small(dimensions, cx))
                                    .child(small(status, cx).when(play.solved, |text| {
                                        text.text_color(rgb(palette.success))
                                    })),
                            )
                    })),
            )
    }
}

fn guide_toggle(expanded: bool, cx: &Context<GameView>) -> Stateful<Div> {
    control(
        "picture-how-to-play",
        if expanded {
            "Hide instructions"
        } else {
            "How to play"
        },
        Button::Normal,
        Action::Guide(!expanded),
        cx,
    )
    .aria_expanded(expanded)
}

fn guide(paint: Colors, cx: &Context<GameView>) -> Div {
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap_2()
        .max_w(px(430.))
        .child(small("Fill squares to match the clues across and down. Each number counts one group. Leave at least one empty square between groups. A 0 means an empty line.", cx))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(small("2  1", cx).font_weight(FontWeight::BOLD))
                .child(
                    div()
                        .flex()
                        .gap(px(1.))
                        .p(px(1.))
                        .bg(rgb(paint.major))
                        .children([Tool::Fill, Tool::Fill, Tool::Mark, Tool::Fill, Tool::Erase]
                            .into_iter().map(|tool| swatch(tool, paint))),
                ),
        )
        .child(small("Arrows move and Space paints. F selects Fill, X selects Mark, and E selects Erase. Use C to check, H for a hint, and Z to undo.", cx))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Button {
    Normal,
    Primary,
    Selected,
    Disabled,
}

fn control(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: Button,
    action: Action,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    let label = label.into();
    press(id, label.clone(), variant, action, cx).child(label)
}

fn press(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: Button,
    action: Action,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    let palette = theme::colors(cx);
    let paint = Colors::new(cx);
    let (background, border, ink) = match variant {
        Button::Normal => (palette.surface, palette.border, palette.ink),
        Button::Primary => (paint.fill, paint.fill, paint.button_ink),
        Button::Selected => (paint.track, paint.fill, paint.ink),
        Button::Disabled => (palette.paper, palette.border, palette.muted),
    };
    let label = label.into();
    div()
        .id(id)
        .when(variant == Button::Selected, |button| {
            button.aria_selected(true)
        })
        .flex()
        .items_center()
        .justify_center()
        .min_h(px(35.))
        .rounded(px(5.))
        .border_1()
        .border_color(rgb(border))
        .px_2p5()
        .py_1()
        .bg(rgb(background))
        .text_color(rgb(ink))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(variant != Button::Disabled, |button| {
            button
                .role(Role::Button)
                .aria_label(label)
                .tab_index(0)
                .cursor_pointer()
                .hover(move |style| {
                    if variant == Button::Primary {
                        style.opacity(0.88)
                    } else {
                        style.bg(rgb(palette.hover))
                    }
                })
                .focus_visible(move |style| style.border_color(rgb(paint.select)))
                .on_click(cx.listener(move |view, _, window, cx| view.act(action, window, cx)))
        })
}

fn small(text: impl Into<SharedString>, cx: &Context<GameView>) -> Div {
    div()
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(rgb(theme::colors(cx).muted))
        .child(text.into())
}

fn swatch(tool: Tool, paint: Colors) -> Div {
    div()
        .relative()
        .size(px(16.))
        .flex_shrink_0()
        .border_1()
        .border_color(rgb(paint.major))
        .bg(rgb(if tool == Tool::Fill {
            paint.fill
        } else {
            paint.stock
        }))
        .when(tool == Tool::Mark, |icon| {
            icon.child(cross(paint.mark, 0.66))
        })
}
