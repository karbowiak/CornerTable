use std::{cell::Cell, rc::Rc, time::Duration};

use corner_table_word_games::SearchLine;
use gpui::{
    Animation, AnimationExt, Bounds, Context, Div, FontWeight, MouseButton, Pixels, Point, Role,
    Stateful, Window, canvas, div, point, prelude::*, px, relative, rgb,
};

use super::{
    Action, Button, GameView,
    board::{self, Fibre},
    button, caption, divider,
    search_play::SearchPlay,
    state::Play,
    status,
};
use crate::{motion, theme as t};

const KEYBOARD: &str = "Arrows move. Space or Enter picks an end. Esc cancels.";

#[derive(Clone, Copy)]
struct BoardColors {
    paper: u32,
    fibre: Fibre,
    letter: u32,
    frame: u32,
    cursor: u32,
    hint: u32,
    active: u32,
    marks: [u32; 4],
}

impl BoardColors {
    fn for_theme(palette: t::Palette) -> Self {
        if palette.paper == t::PAPER {
            Self {
                paper: board::PAPER,
                fibre: Fibre::Paper,
                letter: 0x18332d,
                frame: 0x6c543a,
                cursor: 0x305d69,
                hint: 0x8d5610,
                active: 0x95d3c0,
                marks: [0xc2d9b5, 0xf0d395, 0xbad7e1, 0xecc5be],
            }
        } else {
            Self {
                paper: 0x293b30,
                fibre: Fibre::DarkPaper,
                letter: 0xfff2d7,
                frame: 0x584937,
                cursor: 0xb6e0ee,
                hint: 0xffce74,
                active: 0x326d61,
                marks: [0x3e6048, 0x6b502a, 0x365a6e, 0x704a52],
            }
        }
    }

    fn mark(self, word_index: usize) -> u32 {
        self.marks[word_index % self.marks.len()]
    }
}

impl GameView {
    pub(super) fn search_key_down(
        &mut self,
        key: &str,
        held: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_finished = self.play().is_finished();
        let Play::Search(play) = self.play_mut() else {
            return;
        };
        let mut effect = None;
        match key {
            "left" => play.move_cursor(-1, 0),
            "right" => play.move_cursor(1, 0),
            "up" => play.move_cursor(0, -1),
            "down" => play.move_cursor(0, 1),
            "escape" => play.cancel_selection(),
            "enter" | "space" => {
                if !held {
                    play.pointer = None;
                    effect = play.click(play.cursor);
                }
            }
            "h" => {
                if !held {
                    effect = play.hint();
                }
            }
            _ => return,
        }
        if effect.is_some() {
            self.save_action(was_finished, effect, window, cx);
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn search(
        &self,
        play: &SearchPlay,
        content: f32,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = play.board.theme();
        let colors = BoardColors::for_theme(t::colors(cx));
        let word_bank_width = (content - 490.).clamp(164., 320.);
        let stacked = content < 530.;
        let board_width = if stacked {
            content
        } else {
            content - word_bank_width - 12.
        };
        let compact = word_bank_width < 290. && !stacked;
        let inset = if compact { 42. } else { 56. };
        let height_limit = (f32::from(window.viewport_size().height) - 245.).max(325.);
        let step = ((board_width - inset).min(height_limit) / theme.side as f32)
            .floor()
            .clamp(24., 46.);
        let board = t::panel(cx)
            .p(px(if compact { 12. } else { 16. }))
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .gap_1()
                    .child(t::display_type(theme.title, 28.))
                    .child(caption(format!("{} × {}", theme.side, theme.side), cx)),
            )
            .child(
                div()
                    .mx_auto()
                    .rounded_sm()
                    .p(px(if compact { 6. } else { 8. }))
                    .bg(rgb(colors.frame))
                    .child(self.search_grid(play, step, colors, window, cx)),
            )
            .when(!play.board.complete(), |board| {
                board.child(status(
                    &play.notice,
                    if play.anchor.is_some() {
                        "Choose the other end of the word."
                    } else {
                        "Drag across a word or pick its two ends. Either direction works."
                    },
                    cx,
                ))
            });
        let list = self.search_word_bank(play, colors, compact, cx);
        let layout = div().flex().gap_3().items_start();
        if stacked {
            layout.flex_col().child(board.w_full()).child(list.w_full())
        } else {
            layout
                .child(board.flex_1().min_w_0())
                .child(list.w(px(word_bank_width)).flex_shrink_0())
        }
    }

    fn search_grid(
        &self,
        play: &SearchPlay,
        step: f32,
        colors: BoardColors,
        window: &Window,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = play.board.theme();
        let side = theme.side;
        let complete = play.board.complete();
        let focused = self.focus.is_focused(window);
        let (start, end) = play
            .pointer
            .map_or((play.anchor, Some(play.cursor)), |pointer| {
                (Some(pointer.start), pointer.end)
            });
        let preview = start
            .zip(end)
            .and_then(|(start, end)| SearchLine::between(side, start, end));
        let hinted = play.hint.map(|word| play.board.line(word).start);
        let latest = play.latest_mark.filter(|_| motion::enabled(cx));
        let mut marks = (0..theme.words.len())
            .filter(|&index| play.board.found(index))
            .map(|index| {
                let traced = latest.filter(|&(word, _)| word == index);
                let line = traced.map_or_else(|| play.board.line(index), |(_, line)| line);
                (line, colors.mark(index), traced.is_some())
            })
            .collect::<Vec<_>>();
        if let Some(line) = preview {
            marks.push((line, colors.active, false));
        }

        let bounds = Rc::new(Cell::new(None));
        let down_bounds = Rc::clone(&bounds);
        let move_bounds = Rc::clone(&bounds);
        let up_bounds = Rc::clone(&bounds);
        let current_letter = char::from(play.board.letters()[play.cursor]);
        let mark_progress = Rc::new(Cell::new(1.));
        let paint_progress = Rc::clone(&mark_progress);
        let highlights = canvas(
            move |area, _, _| bounds.set(Some(area)),
            move |area, (), window, _| {
                for &(line, color, tracing) in &marks {
                    let progress = if tracing { paint_progress.get() } else { 1. };
                    paint_mark(line, color, area, step, side, progress, window);
                }
            },
        )
        .absolute()
        .size_full();
        let highlights = if let Some((word, _)) = latest {
            highlights
                .with_animation(
                    ("word-search-found-mark", word),
                    Animation::new(Duration::from_millis(230))
                        .with_easing(|progress| 1. - (1. - progress).powi(3)),
                    move |highlights, progress| {
                        mark_progress.set(progress);
                        highlights
                    },
                )
                .into_any_element()
        } else {
            highlights.into_any_element()
        };
        div()
            .id("word-search-grid")
            .debug_selector(|| "word-search-grid".into())
            .role(Role::Grid)
            .aria_label(format!("{} word search", theme.title))
            .aria_description(format!(
                "{}{} {current_letter}, row {}, column {}.",
                if complete {
                    "All words found. Arrows move."
                } else {
                    KEYBOARD
                },
                if complete { "" } else { " H shows a hint." },
                play.cursor / side + 1,
                play.cursor % side + 1,
            ))
            .aria_row_count(side)
            .aria_column_count(side)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .relative()
            .size(px(step * side as f32))
            .flex()
            .flex_col()
            .bg(rgb(colors.paper))
            .text_color(rgb(colors.letter))
            .font_family(t::DISPLAY)
            .font_weight(FontWeight::BOLD)
            .text_size(px((step * 0.51).clamp(14., 23.)))
            .line_height(px(step))
            .when(!complete, |grid| {
                grid.cursor_crosshair()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &gpui::MouseDownEvent, window, cx| {
                            if let Some(cell) = hit(down_bounds.get(), event.position, step, side)
                                && let Play::Search(play) = view.play_mut()
                                && !play.board.complete()
                            {
                                play.pointer_down(cell);
                                view.focus.focus(window, cx);
                                cx.notify();
                            }
                        }),
                    )
                    .on_mouse_move(
                        cx.listener(move |view, event: &gpui::MouseMoveEvent, _, cx| {
                            let Play::Search(play) = view.play_mut() else {
                                return;
                            };
                            let cell = hit(move_bounds.get(), event.position, step, side);
                            if event.dragging() {
                                if play.pointer_move(cell) {
                                    cx.notify();
                                }
                            } else if play.pointer.take().is_some() {
                                cx.notify();
                            } else if let Some(cell) = cell
                                && play.anchor.is_some()
                                && cell != play.cursor
                            {
                                play.cursor = cell;
                                cx.notify();
                            }
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, event: &gpui::MouseUpEvent, window, cx| {
                            view.search_release(
                                hit(up_bounds.get(), event.position, step, side),
                                window,
                                cx,
                            );
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|view, _, window, cx| view.search_release(None, window, cx)),
                    )
            })
            .child(board::grain(colors.fibre, 29))
            .child(highlights)
            .children(
                play.board
                    .letters()
                    .chunks(side)
                    .enumerate()
                    .map(|(row, letters)| {
                        div()
                            .id(("search-row", row))
                            .role(Role::Row)
                            .aria_row_index(row)
                            .flex()
                            .flex_shrink_0()
                            .children(letters.iter().enumerate().map(|(column, &letter)| {
                                let index = row * side + column;
                                let letter = char::from(letter);
                                let is_hint = hinted == Some(index);
                                let is_cursor = focused && play.cursor == index;
                                let is_start = start == Some(index);
                                div()
                                    .id(("search-square", index))
                                    .role(Role::GridCell)
                                    .aria_row_index(row)
                                    .aria_column_index(column)
                                    .aria_label(if is_hint {
                                        format!("{letter}, hinted first letter")
                                    } else {
                                        letter.to_string()
                                    })
                                    .aria_selected(is_start)
                                    .when(is_cursor, |cell| cell.aria_active_descendant())
                                    .size(px(step))
                                    .flex_shrink_0()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .child(
                                        div()
                                            .size(px(step - 5.))
                                            .flex()
                                            .relative()
                                            .justify_center()
                                            .items_center()
                                            .rounded_md()
                                            .border_2()
                                            .border_color(if is_cursor || is_start {
                                                rgb(colors.cursor).into()
                                            } else if is_hint {
                                                rgb(colors.hint).into()
                                            } else {
                                                gpui::transparent_black()
                                            })
                                            .child(letter.to_string())
                                            .when(is_hint, |cell| {
                                                cell.child(
                                                    div()
                                                        .absolute()
                                                        .right(px(2.))
                                                        .top(px(2.))
                                                        .size(px(5.))
                                                        .rounded_full()
                                                        .bg(rgb(colors.hint)),
                                                )
                                            }),
                                    )
                            }))
                    }),
            )
    }

    fn search_release(&mut self, cell: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Play::Search(play) = self.play_mut() else {
            return;
        };
        if play.pointer.is_none() {
            return;
        }
        let was_finished = play.board.complete();
        let effect = play.pointer_up(cell);
        if effect.is_some() {
            self.save_action(was_finished, effect, window, cx);
        }
        cx.notify();
    }

    fn search_word_bank(
        &self,
        play: &SearchPlay,
        colors: BoardColors,
        compact: bool,
        cx: &Context<Self>,
    ) -> Div {
        let palette = t::colors(cx);
        let theme = play.board.theme();
        let count = play.board.found_count();
        let total = theme.words.len();
        let list = div()
            .id("word-search-targets")
            .role(Role::List)
            .aria_label("Words to find")
            .flex()
            .flex_wrap()
            .when(!compact, |list| list.gap_y_1())
            .children(theme.words.iter().enumerate().map(|(index, &word)| {
                let found = play.board.found(index);
                let hinted = play.hint == Some(index);
                div()
                    .id(("search-target", index))
                    .role(Role::ListItem)
                    .aria_label(if found {
                        format!("{word}, found")
                    } else if hinted {
                        format!("{word}, hint shown")
                    } else {
                        word.to_owned()
                    })
                    .w(relative(if compact { 1. } else { 0.5 }))
                    .min_h(px(if compact { 23. } else { 26. }))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(
                        div()
                            .size(px(9.))
                            .flex_shrink_0()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(if found {
                                colors.mark(index)
                            } else if hinted {
                                palette.gold_text
                            } else {
                                palette.muted
                            }))
                            .when(found, |swatch| swatch.bg(rgb(colors.mark(index))))
                            .when(hinted, |swatch| swatch.bg(rgb(palette.gold_text))),
                    )
                    .child(
                        div()
                            .when(found, |word| {
                                word.text_color(rgb(palette.muted)).line_through()
                            })
                            .when(hinted, |word| word.text_color(rgb(palette.gold_text)))
                            .child(word),
                    )
            }));
        t::panel(cx)
            .p(px(if compact { 12. } else { 16. }))
            .flex()
            .flex_col()
            .gap(px(if compact { 8. } else { 12. }))
            .child(
                div()
                    .id("word-search-progress")
                    .role(Role::ProgressIndicator)
                    .aria_label(format!("{count} of {total} words found"))
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(total as f64)
                    .aria_numeric_value(count as f64)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .justify_between()
                            .gap_2()
                            .child(if compact {
                                t::display_type("Word list", 17.)
                            } else {
                                t::display_type("Words to find", 20.)
                            })
                            .child(caption(format!("{count} / {total}"), cx)),
                    )
                    .child(
                        div().h(px(5.)).rounded_full().bg(rgb(palette.hover)).child(
                            div()
                                .w(relative(count as f32 / total as f32))
                                .h_full()
                                .rounded_full()
                                .bg(rgb(palette.success)),
                        ),
                    ),
            )
            .child(list)
            .when(!play.board.complete(), |panel| {
                let hint = play.hint.map(|index| {
                    let start = play.board.line(index).start;
                    super::callout(
                        "word-search-hint-message",
                        format!(
                            "{} starts at row {}, column {}.",
                            theme.words[index],
                            start / theme.side + 1,
                            start % theme.side + 1,
                        ),
                        cx,
                    )
                });
                panel
                    .child(divider(cx))
                    .child(div().flex().child(button(
                        "word-search-hint",
                        "First-letter hint",
                        Button::enabled(play.hint.is_none()),
                        Action::SearchHint,
                        cx,
                    )))
                    .children(hint)
                    .child(caption(KEYBOARD, cx))
                    .child(caption(
                        "Selections and hints count as moves. Press H for a hint.",
                        cx,
                    ))
            })
    }
}

fn hit(
    bounds: Option<Bounds<Pixels>>,
    position: Point<Pixels>,
    step: f32,
    side: usize,
) -> Option<usize> {
    let origin = bounds?.origin;
    let x = f32::from(position.x - origin.x);
    let y = f32::from(position.y - origin.y);
    let width = step * side as f32;
    if !(0. ..width).contains(&x) || !(0. ..width).contains(&y) {
        return None;
    }
    Some((y / step) as usize * side + (x / step) as usize)
}

fn paint_mark(
    line: SearchLine,
    color: u32,
    area: Bounds<Pixels>,
    step: f32,
    side: usize,
    progress: f32,
    window: &mut Window,
) {
    let center = |index| {
        area.origin
            + point(
                px((index % side) as f32 * step + step / 2.),
                px((index / side) as f32 * step + step / 2.),
            )
    };
    let start = center(line.start);
    let end = start + (center(line.end) - start) * progress;
    let strike = [start, end];
    let ink = rgb(color);
    let edge = ink.blend(rgb(0x38281c).opacity(0.25));
    board::ribbon(&strike, step * 0.76, edge.into(), window);
    board::ribbon(&strike, step * 0.66, ink.into(), window);
}
