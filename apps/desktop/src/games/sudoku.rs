use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_sudoku::{CELLS, Difficulty, Game, SIZE};
use gpui::{
    Context, Div, FocusHandle, FontWeight, KeyDownEvent, SharedString, Stateful, Window, div,
    prelude::*, px, relative, rgb,
};

use crate::games::{GameEntry, GameLaunch};
use crate::theme as t;

const GRID_LINE: u32 = 0xd8dacb;
const PLAYER_INK: u32 = 0x23695e;
const ERROR: u32 = 0x9d3f37;

pub struct GameView {
    focus: FocusHandle,
    game: Game,
    seed: u64,
    challenge: bool,
    difficulty: Difficulty,
    selected: usize,
    note_mode: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let (seed, challenge, difficulty) = match &launch.entry {
            GameEntry::Challenge(id) => {
                let seed = id.bytes().fold(0u64, |hash, byte| {
                    hash.wrapping_mul(131).wrapping_add(u64::from(byte))
                });
                (seed, true, Difficulty::Medium)
            }
            GameEntry::Classic | GameEntry::Mode(_) => (
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |time| time.as_nanos() as u64),
                false,
                Difficulty::Easy,
            ),
        };

        let game = Game::new(seed, difficulty);
        Self {
            focus: cx.focus_handle().tab_stop(true),
            selected: first_empty(&game),
            game,
            seed,
            challenge,
            difficulty,
            note_mode: false,
        }
    }

    fn reset(&mut self, difficulty: Difficulty, advance_seed: bool) {
        if advance_seed {
            self.seed = self.seed.wrapping_add(1);
        }
        self.difficulty = difficulty;
        self.game = Game::new(self.seed, self.difficulty);
        self.selected = first_empty(&self.game);
        self.note_mode = false;
    }

    fn input(&mut self, digit: u8) {
        self.game.enter(self.selected, digit, self.note_mode);
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        let key = stroke.key.as_str();
        if stroke.modifiers.control || stroke.modifiers.platform {
            if key != "z" || stroke.modifiers.shift {
                return;
            }
            self.game.undo();
        } else if stroke.modifiers.alt {
            return;
        } else {
            let row = self.selected / SIZE;
            let column = self.selected % SIZE;
            match key {
                "up" => self.selected = row.saturating_sub(1) * SIZE + column,
                "down" => self.selected = (row + 1).min(SIZE - 1) * SIZE + column,
                "left" => self.selected = row * SIZE + column.saturating_sub(1),
                "right" => self.selected = row * SIZE + (column + 1).min(SIZE - 1),
                "backspace" | "delete" | "0" => self.input(0),
                "n" => self.note_mode = !self.note_mode,
                _ => {
                    let Ok(digit @ 1..=9) = stroke.key_char.as_deref().unwrap_or(key).parse()
                    else {
                        return;
                    };
                    self.input(digit);
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn cell(&self, index: usize, size: f32, cx: &Context<Self>) -> Stateful<Div> {
        let selected = self.selected == index;
        let row = index / SIZE;
        let column = index % SIZE;
        let given = self.game.is_given(index);
        let value = self.game.value(index);
        let same_digit = value != 0 && value == self.game.value(self.selected);
        let conflict = self.game.has_conflict(index);

        let fill = if selected {
            0xd1e4ba
        } else if conflict {
            0xf8e4dd
        } else if same_digit {
            0xe1ecce
        } else if Game::related(self.selected, index) {
            0xf0f2e2
        } else if given {
            0xf7f4eb
        } else {
            t::SURFACE
        };
        let foreground = if conflict {
            ERROR
        } else if given {
            t::INK
        } else {
            PLAYER_INK
        };

        let mut cell = div()
            .id(("sudoku-cell", index))
            .size(px(size))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(fill))
            .hover(|style| style.bg(rgb(0xe4eccf)))
            .cursor_pointer()
            .text_color(rgb(foreground))
            .text_size(px(size * 0.50))
            .font_weight(if given {
                FontWeight::BOLD
            } else {
                FontWeight::SEMIBOLD
            })
            .on_click(cx.listener(move |view, _, window, cx| {
                view.selected = index;
                view.focus.focus(window);
                cx.notify();
            }));

        if column % 3 < 2 {
            cell = cell.border_r_1();
        }
        if row % 3 < 2 {
            cell = cell.border_b_1();
        }

        if value != 0 {
            cell.border_color(rgb(GRID_LINE)).child(value.to_string())
        } else if self.game.notes(index) != 0 {
            let notes = self.game.notes(index);
            let content = div()
                .flex()
                .flex_col()
                .text_size(px((size * 0.22).max(8.)))
                .line_height(px(size * 0.27))
                .text_color(rgb(t::MUTED))
                .children((0..3).map(|row| {
                    div().flex().children((1..=3).map(|column| {
                        let digit = row * 3 + column;
                        div()
                            .w(px(size * 0.27))
                            .text_center()
                            .children(((notes & (1 << digit)) != 0).then(|| digit.to_string()))
                    }))
                }));
            cell.border_color(rgb(GRID_LINE)).child(content)
        } else {
            cell.border_color(rgb(GRID_LINE))
        }
    }

    fn board(&self, size: f32, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(2.))
            .p(px(2.))
            .bg(rgb(t::INK))
            .rounded_md()
            .overflow_hidden()
            .children((0..3).map(|band| {
                div().flex().gap(px(2.)).children((0..3).map(|stack| {
                    div().flex().flex_col().children((0..3).map(|row| {
                        div().flex().children((0..3).map(|column| {
                            self.cell((band * 3 + row) * SIZE + stack * 3 + column, size, cx)
                        }))
                    }))
                }))
            }))
    }

    fn controls(&self, cx: &Context<Self>) -> Div {
        let count = self.game.filled();
        let conflicting = (0..CELLS).any(|index| self.game.has_conflict(index));
        let (status, color) = if self.game.is_solved() {
            ("Puzzle complete!", PLAYER_INK)
        } else if conflicting {
            ("Check the highlighted duplicates.", ERROR)
        } else {
            ("Each row, column, and box needs 1–9.", t::MUTED)
        };
        let mut panel = t::surface()
            .w(px(258.))
            .flex_shrink_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Grid progress")
                            .child(format!("{count} / {CELLS}")),
                    )
                    .child(
                        div().h(px(6.)).rounded_full().bg(rgb(t::SAGE)).child(
                            div()
                                .h_full()
                                .w(relative(count as f32 / CELLS as f32))
                                .rounded_full()
                                .bg(rgb(PLAYER_INK)),
                        ),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(color))
                            .child(status),
                    ),
            );

        if !self.challenge {
            panel = panel.child(
                div()
                    .flex()
                    .gap_1()
                    .children(Difficulty::ALL.map(|difficulty| {
                        control(
                            format!("sudoku-{}", difficulty.label()),
                            difficulty.label(),
                            self.difficulty == difficulty,
                        )
                        .flex_1()
                        .on_click(cx.listener(
                            move |view, _, window, cx| {
                                if view.difficulty != difficulty {
                                    view.reset(difficulty, false);
                                }
                                view.focus.focus(window);
                                cx.notify();
                            },
                        ))
                    })),
            );
        }

        panel
            .child(div().flex().flex_col().gap_1().children((0..3).map(|row| {
                div().flex().gap_1().children((1..=3).map(|column| {
                    let digit = row * 3 + column;
                    control(format!("sudoku-digit-{digit}"), digit.to_string(), false)
                        .h(px(42.))
                        .flex_1()
                        .text_size(px(21.))
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.input(digit);
                            view.focus.focus(window);
                            cx.notify();
                        }))
                }))
            })))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        control("sudoku-notes", "Notes", self.note_mode)
                            .flex_1()
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.note_mode = !view.note_mode;
                                view.focus.focus(window);
                                cx.notify();
                            })),
                    )
                    .child(
                        control("sudoku-erase", "Erase", false)
                            .flex_1()
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.input(0);
                                view.focus.focus(window);
                                cx.notify();
                            })),
                    )
                    .child(
                        control("sudoku-undo", "Undo", false)
                            .flex_1()
                            .when(!self.game.can_undo(), |style| {
                                style.text_color(rgb(t::MUTED))
                            })
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.game.undo();
                                view.focus.focus(window);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                control(
                    "sudoku-new",
                    if self.challenge {
                        "Restart puzzle"
                    } else {
                        "New puzzle"
                    },
                    false,
                )
                .bg(rgb(t::GOLD))
                .border_color(rgb(t::GOLD))
                .hover(|style| style.bg(rgb(t::GOLD_HOVER)))
                .on_click(cx.listener(|view, _, window, cx| {
                    view.reset(view.difficulty, !view.challenge);
                    view.focus.focus(window);
                    cx.notify();
                })),
            )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = f32::from(window.viewport_size().width);
        let beside = width >= 840.;
        let board_space = if beside { width - 512. } else { width - 226. };
        let size = (board_space / 9.).clamp(22., 49.).floor();
        div()
            .id("sudoku-game")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(t::display_type("Sudoku", 38.))
                    .child(div().text_color(rgb(t::MUTED)).child(if self.challenge {
                        "Daily puzzle · Medium"
                    } else {
                        "Fill the grid one digit at a time."
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_4()
                    .when(!beside, |style| style.flex_col())
                    .child(self.board(size, cx))
                    .child(self.controls(cx)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(t::MUTED))
                    .child("Arrow keys move · 1–9 enter digits · N toggles notes · Delete erases"),
            )
    }
}

fn first_empty(game: &Game) -> usize {
    (0..CELLS).find(|&index| !game.is_given(index)).unwrap_or(0)
}

fn control(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    active: bool,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .tab_index(0)
        .min_h(px(34.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if active { PLAYER_INK } else { t::BORDER }))
        .bg(rgb(if active { t::SAGE } else { t::SURFACE }))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(12.))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(t::SAGE)))
        .focus(|style| style.border_color(rgb(PLAYER_INK)))
        .child(title.into())
}
