use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_sudoku::{Difficulty, Format, Game, Hint, HintReason, Unit};
use gpui::{
    Context, Div, FocusHandle, FontWeight, KeyDownEvent, SharedString, Stateful, Window, div,
    prelude::*, px, relative, rgb,
};

use super::{GameEntry, GameLaunch, board_space};
use crate::theme as t;

const GRID_LINE: u32 = 0xd0d6c7;
const PLAYER_INK: u32 = 0x23695e;
const ERROR: u32 = 0x9d3f37;
const ERROR_SURFACE: u32 = 0xf9e3db;
const HINT_INK: u32 = 0x80551c;
const HINT_SURFACE: u32 = 0xfcf0d3;
const GROUP_GAP: f32 = 2.;
const BOARD_EDGE: f32 = 2.;
const SECTION_GAP: f32 = 18.;
const PANEL_WIDTH: f32 = 284.;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Puzzle {
    Practice(Format, Difficulty),
    Daily(u64),
    Challenge(u64),
}

impl Puzzle {
    fn is_daily(self) -> bool {
        matches!(self, Self::Daily(_) | Self::Challenge(_))
    }

    fn start(self, seed: u64) -> Game {
        match self {
            Self::Practice(format, difficulty) => Game::with_format(seed, difficulty, format),
            Self::Daily(day) => Game::daily(day),
            Self::Challenge(seed) => Game::new(seed, Difficulty::Medium),
        }
    }
}

struct Round {
    game: Game,
    selected: usize,
    note_mode: bool,
    hint: Option<Hint>,
    hint_open: bool,
    hints_used: usize,
    best: Option<usize>,
}

impl Round {
    fn new(puzzle: Puzzle, seed: u64) -> Self {
        let game = puzzle.start(seed);
        let selected = (0..game.format().cell_count())
            .find(|&index| !game.is_given(index))
            .unwrap_or(0);
        Self {
            game,
            selected,
            note_mode: false,
            hint: None,
            hint_open: false,
            hints_used: 0,
            best: None,
        }
    }

    fn edit(&mut self, digit: u8, note: bool) {
        if self.game.enter(self.selected, digit, note) {
            self.clear_hint();
            if self.game.is_solved() && self.best.is_none_or(|best| self.hints_used < best) {
                self.best = Some(self.hints_used);
            }
        }
    }

    fn undo(&mut self) {
        if self.game.undo() {
            self.clear_hint();
        }
    }

    fn move_selection(&mut self, rows: isize, columns: isize) {
        let size = self.game.format().size();
        let row = (self.selected / size)
            .saturating_add_signed(rows)
            .min(size - 1);
        let column = (self.selected % size)
            .saturating_add_signed(columns)
            .min(size - 1);
        self.selected = row * size + column;
    }

    fn select_blank(&mut self, backwards: bool) {
        let cells = self.game.format().cell_count();
        if let Some(index) = (1..cells)
            .map(|offset| if backwards { cells - offset } else { offset })
            .map(|offset| (self.selected + offset) % cells)
            .find(|&index| self.game.value(index) == 0)
        {
            self.selected = index;
        }
    }

    fn toggle_notes(&mut self) {
        if !self.game.is_solved() {
            self.note_mode = !self.note_mode;
        }
    }

    fn show_hint(&mut self) {
        if self.hint.is_none() {
            self.hint = self.game.hint(self.selected);
            if self.hint.is_some() {
                self.hints_used += 1;
            }
        }
        if let Some(hint) = self.hint {
            self.selected = hint.index;
            self.hint_open = true;
        }
    }

    fn visible_hint(&self) -> Option<Hint> {
        self.hint.filter(|_| self.hint_open)
    }

    fn apply_hint(&mut self) {
        if let Some(hint) = self.visible_hint() {
            self.selected = hint.index;
            self.edit(hint.entry(), false);
        }
    }

    fn clear_hint(&mut self) {
        self.hint = None;
        self.hint_open = false;
    }
}

pub struct GameView {
    focus: FocusHandle,
    focus_on_mount: bool,
    rounds: HashMap<Puzzle, Round>,
    puzzle: Puzzle,
    practice_seed: u64,
    standard_difficulty: Difficulty,
    mini_difficulty: Difficulty,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let puzzle = match &launch.entry {
            GameEntry::Challenge(id) => {
                let seed = id.bytes().fold(0u64, |hash, byte| {
                    hash.wrapping_mul(131).wrapping_add(u64::from(byte))
                });
                Puzzle::Challenge(seed)
            }
            GameEntry::Classic | GameEntry::Mode(_) => {
                Puzzle::Practice(Format::Standard, Difficulty::Easy)
            }
        };
        let practice_seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos() as u64);
        Self {
            focus: cx.focus_handle().tab_stop(true),
            focus_on_mount: true,
            rounds: HashMap::from([(puzzle, Round::new(puzzle, practice_seed))]),
            puzzle,
            practice_seed,
            standard_difficulty: Difficulty::Easy,
            mini_difficulty: Difficulty::Easy,
        }
    }

    fn round(&self) -> &Round {
        &self.rounds[&self.puzzle]
    }

    fn round_mut(&mut self) -> &mut Round {
        self.rounds
            .get_mut(&self.puzzle)
            .expect("active Sudoku round exists")
    }

    fn open(&mut self, puzzle: Puzzle) {
        if let Puzzle::Daily(_) = puzzle {
            self.rounds
                .retain(|&key, _| !matches!(key, Puzzle::Daily(_)) || key == puzzle);
        }
        if let Puzzle::Practice(format, difficulty) = puzzle {
            match format {
                Format::Standard => self.standard_difficulty = difficulty,
                Format::Mini => self.mini_difficulty = difficulty,
            }
        }
        self.rounds.entry(puzzle).or_insert_with(|| {
            if !puzzle.is_daily() {
                self.practice_seed = self.practice_seed.wrapping_add(1);
            }
            Round::new(puzzle, self.practice_seed)
        });
        self.puzzle = puzzle;
    }

    fn start_over(&mut self) {
        let puzzle = self.puzzle;
        let best = if puzzle.is_daily() {
            self.round().best
        } else {
            self.practice_seed = self.practice_seed.wrapping_add(1);
            None
        };
        let mut next = Round::new(puzzle, self.practice_seed);
        next.best = best;
        self.rounds.insert(puzzle, next);
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        let key = stroke.key.as_str();
        let modifiers = stroke.modifiers;
        if !self.focus.is_focused(window) || modifiers.alt || modifiers.function {
            return;
        }
        let round = self.round_mut();
        if modifiers.control || modifiers.platform {
            if key != "z" || modifiers.shift {
                return;
            }
            if !event.is_held {
                round.undo();
            }
        } else {
            match key {
                "up" => round.move_selection(-1, 0),
                "down" => round.move_selection(1, 0),
                "left" => round.move_selection(0, -1),
                "right" => round.move_selection(0, 1),
                "space" => round.select_blank(modifiers.shift),
                "backspace" | "delete" => round.edit(0, false),
                "n" if !event.is_held => round.toggle_notes(),
                "h" if !event.is_held => round.show_hint(),
                "enter" if round.hint_open && !event.is_held => round.apply_hint(),
                "escape" if round.hint_open => round.hint_open = false,
                _ => {
                    let Some(digit) = stroke
                        .key_char
                        .as_deref()
                        .and_then(|character| character.parse::<u8>().ok())
                        .or_else(|| key.parse().ok())
                        .filter(|&digit| usize::from(digit) <= round.game.format().size())
                    else {
                        return;
                    };
                    if !event.is_held {
                        round.edit(digit, round.note_mode);
                    }
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn modes(&self, cx: &Context<Self>) -> Div {
        let format = self.round().game.format();
        let today = utc_day();
        let practice = matches!(self.puzzle, Puzzle::Practice(..));
        div()
            .flex()
            .gap_2()
            .when(!matches!(self.puzzle, Puzzle::Challenge(_)), |row| {
                row.child(control(
                    "sudoku-classic",
                    "Classic 9×9",
                    ButtonStyle::selected(practice && format == Format::Standard),
                    cx,
                    |view| view.open(Puzzle::Practice(Format::Standard, view.standard_difficulty)),
                ))
                .child(control(
                    "sudoku-mini",
                    "Mini 6×6",
                    ButtonStyle::selected(practice && format == Format::Mini),
                    cx,
                    |view| view.open(Puzzle::Practice(Format::Mini, view.mini_difficulty)),
                ))
                .child(control(
                    "sudoku-daily",
                    "Daily puzzle",
                    ButtonStyle::selected(self.puzzle == Puzzle::Daily(today)),
                    cx,
                    |view| view.open(Puzzle::Daily(utc_day())),
                ))
            })
    }

    fn cell(&self, index: usize, size: f32, solved: bool, cx: &Context<Self>) -> Stateful<Div> {
        let round = self.round();
        let game = &round.game;
        let format = game.format();
        let row = index / format.size();
        let column = index % format.size();
        let selected = !solved && round.selected == index;
        let given = game.is_given(index);
        let value = game.value(index);
        let same_digit = !solved && value != 0 && value == game.value(round.selected);
        let conflict = game.has_conflict(index);
        let hint = round.visible_hint();
        let hint_target = hint.is_some_and(|hint| hint.index == index);
        let hint_evidence = hint.is_some_and(|hint| match hint.reason {
            HintReason::OnlyCandidate { .. } => format.related(hint.index, index),
            HintReason::OnlyPlace { unit, .. } => unit.contains(format, index),
            HintReason::Duplicate { peer, .. } => peer == index,
            HintReason::IncorrectEntry { .. } | HintReason::Forced { .. } => false,
        });

        let fill = if conflict {
            ERROR_SURFACE
        } else if hint_target {
            0xf9dfa3
        } else if selected {
            0xc3dfbd
        } else if hint_evidence {
            HINT_SURFACE
        } else if same_digit {
            0xd6e9d7
        } else if !solved && format.related(round.selected, index) {
            0xeef2e4
        } else if given {
            0xf7f3e9
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

        div()
            .id(("sudoku-cell", index))
            .relative()
            .size(px(size))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(fill))
            .text_color(rgb(foreground))
            .text_size(px(size * 0.51))
            .line_height(px(size * 0.9))
            .font_weight(if given {
                FontWeight::BOLD
            } else {
                FontWeight::SEMIBOLD
            })
            .border_color(rgb(GRID_LINE))
            .when(
                column % format.box_columns() + 1 < format.box_columns(),
                |cell| cell.border_r_1(),
            )
            .when(row % format.box_rows() + 1 < format.box_rows(), |cell| {
                cell.border_b_1()
            })
            .when(!solved, |cell| {
                cell.cursor_pointer()
                    .when(
                        !conflict && !selected && !hint_target && !hint_evidence,
                        |cell| cell.hover(|style| style.bg(rgb(0xe4ead8))),
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.round_mut().selected = index;
                        view.focus.focus(window, cx);
                        cx.notify();
                    }))
            })
            .when_some((value != 0).then(|| value.to_string()), |cell, digit| {
                cell.child(digit)
            })
            .when(game.notes(index) != 0, |cell| {
                cell.child(notes_grid(format, size, game.notes(index)))
            })
            .when(conflict, |cell| {
                cell.child(
                    div()
                        .absolute()
                        .bottom(px(size * 0.13))
                        .left(px(size * 0.36))
                        .w(px(size * 0.26))
                        .h(px(2.))
                        .rounded_full()
                        .bg(rgb(ERROR)),
                )
            })
            .when(selected || hint_target, |cell| {
                cell.child(
                    div()
                        .absolute()
                        .inset_0()
                        .border_2()
                        .border_color(rgb(if hint_target { HINT_INK } else { PLAYER_INK })),
                )
            })
    }

    fn board(&self, size: f32, cx: &Context<Self>) -> Div {
        let game = &self.round().game;
        let format = game.format();
        let boxes_wide = format.size() / format.box_columns();
        let boxes_high = format.size() / format.box_rows();
        let solved = game.is_solved();
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(GROUP_GAP))
            .p(px(BOARD_EDGE))
            .bg(rgb(if solved { PLAYER_INK } else { t::INK }))
            .rounded_md()
            .overflow_hidden()
            .children((0..boxes_high).map(|band| {
                div()
                    .flex()
                    .gap(px(GROUP_GAP))
                    .children((0..boxes_wide).map(|stack| {
                        div()
                            .flex()
                            .flex_col()
                            .children((0..format.box_rows()).map(|row| {
                                div()
                                    .flex()
                                    .children((0..format.box_columns()).map(|column| {
                                        let row = band * format.box_rows() + row;
                                        let column = stack * format.box_columns() + column;
                                        self.cell(row * format.size() + column, size, solved, cx)
                                    }))
                            }))
                    }))
            }))
    }

    fn difficulty(&self, format: Format, selected: Difficulty, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .gap_1()
            .children(Difficulty::ALL.map(|difficulty| {
                control(
                    format!("sudoku-{}", difficulty.label()),
                    difficulty.label(),
                    ButtonStyle::selected(selected == difficulty),
                    cx,
                    move |view| view.open(Puzzle::Practice(format, difficulty)),
                )
                .flex_1()
            }))
    }

    fn keypad(&self, cx: &Context<Self>) -> Div {
        let round = self.round();
        let game = &round.game;
        let value = game.value(round.selected);
        let editable = !game.is_given(round.selected);
        let can_enter = editable && (!round.note_mode || value == 0);
        let can_erase = editable && (value != 0 || game.notes(round.selected) != 0);
        let notes = game.notes(round.selected);
        let size = game.format().size();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .children((0..size / 3).map(|row| {
                div().flex().gap_2().children((1..=3).map(|column| {
                    let digit = (row * 3 + column) as u8;
                    let active = if round.note_mode {
                        notes & (1 << digit) != 0
                    } else {
                        value == digit
                    };
                    let style = if can_enter {
                        ButtonStyle::selected(active)
                    } else {
                        ButtonStyle::Disabled
                    };
                    control(
                        format!("sudoku-digit-{digit}"),
                        digit.to_string(),
                        style,
                        cx,
                        move |view| {
                            let round = view.round_mut();
                            round.edit(digit, round.note_mode);
                        },
                    )
                    .h(px(43.))
                    .flex_1()
                    .text_size(px(21.))
                }))
            }))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        control(
                            "sudoku-notes",
                            if round.note_mode { "Notes on" } else { "Notes" },
                            ButtonStyle::selected(round.note_mode),
                            cx,
                            |view| view.round_mut().toggle_notes(),
                        )
                        .flex_1(),
                    )
                    .child(
                        control(
                            "sudoku-erase",
                            "Erase",
                            ButtonStyle::enabled(can_erase),
                            cx,
                            |view| {
                                view.round_mut().edit(0, false);
                            },
                        )
                        .flex_1(),
                    ),
            )
    }

    fn hint_panel(&self, hint: Hint, cx: &Context<Self>) -> Div {
        let (title, detail) = hint_message(hint, self.round().game.format());
        let action = if hint.entry() == 0 {
            "Clear entry".to_owned()
        } else {
            format!("Place {}", hint.entry())
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .bg(rgb(HINT_SURFACE))
            .p_3()
            .child(div().font_weight(FontWeight::BOLD).child(title))
            .child(div().text_size(px(13.)).line_height(px(18.)).child(detail))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        control(
                            "sudoku-apply-hint",
                            action,
                            ButtonStyle::Primary,
                            cx,
                            |view| {
                                view.round_mut().apply_hint();
                            },
                        )
                        .flex_1(),
                    )
                    .child(
                        control(
                            "sudoku-dismiss-hint",
                            "Dismiss",
                            ButtonStyle::Normal,
                            cx,
                            |view| {
                                view.round_mut().hint_open = false;
                            },
                        )
                        .flex_1(),
                    ),
            )
    }

    fn controls(&self, width: f32, cx: &Context<Self>) -> Div {
        let round = self.round();
        let game = &round.game;
        let solved = game.is_solved();
        let daily = self.puzzle.is_daily();
        let mut panel = t::surface()
            .w(px(width))
            .flex_shrink_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_3();

        if let Puzzle::Practice(format, selected) = self.puzzle {
            panel = panel.child(self.difficulty(format, selected, cx));
        }

        if solved {
            panel = panel.child(completed(round, daily));
        } else {
            panel = panel
                .child(progress(game))
                .when(daily, |panel| panel.child(daily_goal(round)))
                .child(self.keypad(cx));
        }

        panel
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        control(
                            "sudoku-undo",
                            "Undo",
                            ButtonStyle::enabled(game.can_undo()),
                            cx,
                            |view| {
                                view.round_mut().undo();
                            },
                        )
                        .flex_1(),
                    )
                    .when(!solved, |row| {
                        row.child(
                            control(
                                "sudoku-hint",
                                "Hint",
                                ButtonStyle::selected(round.hint_open),
                                cx,
                                |view| {
                                    view.round_mut().show_hint();
                                },
                            )
                            .flex_1(),
                        )
                    }),
            )
            .when_some(round.visible_hint(), |panel, hint| {
                panel.child(self.hint_panel(hint, cx))
            })
            .child(control(
                "sudoku-next",
                if daily { "Retry puzzle" } else { "New puzzle" },
                if solved {
                    ButtonStyle::Primary
                } else {
                    ButtonStyle::Normal
                },
                cx,
                Self::start_over,
            ))
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_on_mount {
            self.focus_on_mount = false;
            cx.defer_in(window, |view, window, cx| view.focus.focus(window, cx));
        }

        let format = self.round().game.format();
        let size = format.size();
        let content = board_space(window);
        let beside = content >= 682.;
        let available = if beside {
            content - PANEL_WIDTH - SECTION_GAP
        } else {
            content
        };
        let gutters = GROUP_GAP * (size / format.box_columns() - 1) as f32 + BOARD_EDGE * 2.;
        let max_cell = if format == Format::Mini { 69. } else { 54. };
        let cell = ((available - gutters) / size as f32)
            .clamp(22., max_cell)
            .floor();
        let board_width = cell * size as f32 + gutters;
        let panel_width = if beside { PANEL_WIDTH } else { board_width };
        let instruction = match self.puzzle {
            Puzzle::Practice(Format::Standard, _) => "Fill every row, column, and box with 1–9.",
            Puzzle::Practice(Format::Mini, _) => "Fill every row, column, and 2×3 box with 1–6.",
            Puzzle::Daily(day) if day != utc_day() => {
                "Previous daily · Select Daily puzzle for today’s grid."
            }
            Puzzle::Daily(_) | Puzzle::Challenge(_) => "Classic 9×9 · Medium difficulty",
        };

        div()
            .id("sudoku-game")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .flex()
            .flex_col()
            .gap_3()
            .when(!matches!(self.puzzle, Puzzle::Challenge(_)), |root| {
                root.child(self.modes(cx))
            })
            .child(
                div()
                    .text_color(rgb(t::MUTED))
                    .text_size(px(14.))
                    .child(instruction),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(SECTION_GAP))
                    .when(!beside, |row| row.flex_col())
                    .child(
                        div()
                            .w(px(board_width))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(self.board(cell, cx))
                            .child(shortcuts(size)),
                    )
                    .child(self.controls(panel_width, cx)),
            )
    }
}

fn utc_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() / 86_400)
}

fn notes_grid(format: Format, size: f32, notes: u16) -> Div {
    let unit = px(size * 0.29);
    div()
        .flex()
        .flex_col()
        .text_size(px((size * 0.22).max(8.)))
        .line_height(unit)
        .text_color(rgb(t::MUTED))
        .children((0..format.size() / 3).map(|row| {
            div().flex().h(unit).children((1..=3).map(|column| {
                let digit = row * 3 + column;
                div()
                    .w(unit)
                    .text_center()
                    .children(((notes & (1 << digit)) != 0).then(|| digit.to_string()))
            }))
        }))
}

fn progress(game: &Game) -> Div {
    let cells = game.format().cell_count();
    let left = cells - game.filled();
    let to_fill = (0..cells).filter(|&index| !game.is_given(index)).count();
    let progress = (to_fill - left) as f32 / to_fill as f32;
    let conflict = (0..cells).any(|index| game.has_conflict(index));
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
                .child("Progress")
                .child(if left == 1 {
                    "1 square left".to_owned()
                } else {
                    format!("{left} squares left")
                }),
        )
        .child(
            div().h(px(5.)).rounded_full().bg(rgb(t::SAGE)).child(
                div()
                    .h_full()
                    .w(relative(progress))
                    .rounded_full()
                    .bg(rgb(PLAYER_INK)),
            ),
        )
        .when(conflict, |status| {
            status.child(
                div()
                    .text_color(rgb(ERROR))
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .child("Check the underlined digits for duplicates."),
            )
        })
}

fn daily_goal(round: &Round) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .bg(rgb(t::SAGE))
        .rounded_md()
        .p_3()
        .text_size(px(13.))
        .line_height(px(18.))
        .child(div().font_weight(FontWeight::BOLD).child("Daily goal"))
        .child(if round.hints_used == 0 {
            "Solve without a hint.".to_owned()
        } else {
            format!(
                "{} used this attempt. Retry for a clean solve.",
                hint_count(round.hints_used)
            )
        })
        .when_some(round.best, |goal, best| {
            goal.child(
                div()
                    .text_color(rgb(t::MUTED))
                    .child(format!("Best · {}", result(best))),
            )
        })
}

fn completed(round: &Round, daily: bool) -> Div {
    let description = if daily && round.hints_used == 0 {
        "Daily goal met. Solved without hints.".to_owned()
    } else if round.hints_used == 0 {
        "Solved without hints.".to_owned()
    } else {
        format!(
            "Solved with {}.",
            hint_count(round.hints_used).to_lowercase()
        )
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .rounded_md()
        .bg(rgb(t::SAGE))
        .p_3()
        .child(t::display_type("Puzzle complete", 20.))
        .child(
            div()
                .text_size(px(13.))
                .line_height(px(18.))
                .child(description),
        )
        .when_some(
            round.best.filter(|&best| daily && best < round.hints_used),
            |panel, best| {
                panel.child(
                    div()
                        .text_size(px(13.))
                        .text_color(rgb(t::MUTED))
                        .child(format!("Best · {}", result(best))),
                )
            },
        )
}

fn result(hints: usize) -> String {
    if hints == 0 {
        "Clean solve".to_owned()
    } else {
        format!("Solved with {}", hint_count(hints).to_lowercase())
    }
}

fn hint_count(hints: usize) -> String {
    if hints == 1 {
        "1 hint".to_owned()
    } else {
        format!("{hints} hints")
    }
}

fn hint_message(hint: Hint, format: Format) -> (String, String) {
    match hint.reason {
        HintReason::OnlyCandidate { digit } => (
            "One digit fits".to_owned(),
            format!(
                "The shaded row, column, and box already contain the other digits. Only {digit} fits here."
            ),
        ),
        HintReason::OnlyPlace { digit, unit } => (
            format!("One place for {digit}"),
            format!(
                "Look at {}. In every other empty square there, {digit} is blocked by a row, column, or box.",
                unit_name(unit)
            ),
        ),
        HintReason::Duplicate { digit, unit, .. } => (
            format!("A repeated {digit}"),
            format!(
                "The outlined entry repeats {digit} in {}. Clear this entry to remove the duplicate.",
                match unit {
                    Unit::Box(_) => "the same box".to_owned(),
                    _ => unit_name(unit),
                }
            ),
        ),
        HintReason::IncorrectEntry { digit } => (
            "Revisit this entry".to_owned(),
            format!(
                "With {digit} here, there is no way to finish the puzzle. Clear this entry and look for another deduction."
            ),
        ),
        HintReason::Forced { digit, candidates } => {
            let alternatives = (1..=format.size() as u8)
                .filter(|&other| other != digit && candidates & (1 << other) != 0)
                .map(|other| other.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            (
                "Reasoning ahead".to_owned(),
                format!(
                    "Try checking the other candidates ({alternatives}). Each leads to a contradiction farther on, so this square must be {digit}."
                ),
            )
        }
    }
}

fn unit_name(unit: Unit) -> String {
    match unit {
        Unit::Row(row) => format!("row {}", row + 1),
        Unit::Column(column) => format!("column {}", column + 1),
        Unit::Box(_) => "the highlighted box".to_owned(),
    }
}

fn shortcuts(size: usize) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap_x(px(12.))
        .text_size(px(12.))
        .line_height(px(19.))
        .text_color(rgb(t::MUTED))
        .children([
            "Arrows move".to_owned(),
            format!("1–{size} enter digits"),
            "Space finds a blank".to_owned(),
            "N toggles notes".to_owned(),
            "H shows a hint".to_owned(),
            "Delete erases".to_owned(),
        ])
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonStyle {
    Normal,
    Active,
    Primary,
    Disabled,
}

impl ButtonStyle {
    fn selected(selected: bool) -> Self {
        if selected { Self::Active } else { Self::Normal }
    }

    fn enabled(enabled: bool) -> Self {
        if enabled {
            Self::Normal
        } else {
            Self::Disabled
        }
    }
}

fn control(
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    style: ButtonStyle,
    cx: &Context<GameView>,
    action: impl Fn(&mut GameView) + 'static,
) -> Stateful<Div> {
    let (fill, border, ink) = match style {
        ButtonStyle::Normal => (t::SURFACE, t::BORDER, t::INK),
        ButtonStyle::Active => (t::SAGE, PLAYER_INK, t::INK),
        ButtonStyle::Primary => (t::GOLD, t::GOLD, t::INK),
        ButtonStyle::Disabled => (t::PAPER, t::BORDER, t::MUTED),
    };
    let hover = if style == ButtonStyle::Primary {
        t::GOLD_HOVER
    } else {
        t::SAGE
    };
    div()
        .id(id.into())
        .min_h(px(36.))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(border))
        .bg(rgb(fill))
        .text_color(rgb(ink))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(13.))
        .line_height(px(20.))
        .when(style != ButtonStyle::Disabled, |button| {
            button
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(hover)))
                .focus(|style| style.border_color(rgb(t::INK)).bg(rgb(hover)))
                .on_click(cx.listener(move |view, _, window, cx| {
                    action(view);
                    view.focus.focus(window, cx);
                    cx.notify();
                }))
        })
        .child(title.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_does_not_edit_and_hint_use_survives_dismissal_and_undo() {
        let mut round = Round::new(Puzzle::Daily(20_000), 0);
        let clues = round.game.filled();
        round.show_hint();
        let hint = round.visible_hint().unwrap();
        assert_eq!(round.selected, hint.index);
        assert_eq!(round.game.filled(), clues);

        round.hint_open = false;
        round.select_blank(false);
        round.show_hint();
        assert_eq!(round.visible_hint(), Some(hint));
        assert_eq!(round.hints_used, 1);

        round.note_mode = true;
        round.apply_hint();
        assert_eq!(round.game.value(hint.index), hint.entry());
        assert!(round.hint.is_none());
        round.undo();
        assert_eq!(round.game.filled(), clues);
        assert_eq!(round.hints_used, 1);

        while let Some(move_hint) = round.game.hint(round.selected) {
            round.selected = move_hint.index;
            round.edit(move_hint.entry(), false);
        }
        assert!(round.game.is_solved());
        assert_eq!(round.best, Some(1));
    }

    #[test]
    fn mini_keyboard_movement_stays_in_bounds_and_skips_to_a_blank() {
        let mut round = Round::new(Puzzle::Practice(Format::Mini, Difficulty::Easy), 0);
        round.selected = 0;
        round.move_selection(-1, -1);
        assert_eq!(round.selected, 0);
        round.move_selection(1, 1);
        assert_eq!(round.selected, 7);
        round.select_blank(true);
        assert_ne!(round.selected, 7);
        assert_eq!(round.game.value(round.selected), 0);
        round.selected = 35;
        round.move_selection(1, 1);
        assert_eq!(round.selected, 35);
    }
}
