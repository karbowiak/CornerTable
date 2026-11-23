mod killer;
mod persistence;

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::NaiveDate;
use corner_table_sudoku::{Difficulty, Format, Game, Hint, HintReason, KillerPuzzle, Unit};
use gpui::{
    Context, Div, FocusHandle, FontWeight, KeyDownEvent, SharedString, Stateful, Window, div,
    prelude::*, px, relative, rgb,
};

use super::{GameEntry, GameLaunch, board_space};
use crate::{
    audio::{self, Effect},
    daily,
    storage::{AttemptId, Outcome, ResultMetric, Session},
    theme as t,
};
use persistence::PracticeModes;

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
    Killer(KillerPuzzle),
    Daily(u64),
    Challenge(u64),
}

impl Puzzle {
    fn is_daily(self) -> bool {
        matches!(self, Self::Daily(_) | Self::Challenge(_))
    }

    fn fixed_seed(self) -> Option<u64> {
        match self {
            Self::Daily(day) | Self::Challenge(day) => Some(day),
            Self::Killer(puzzle) => Some(puzzle.id()),
            Self::Practice(..) => None,
        }
    }

    fn start(self, seed: u64) -> Game {
        match self {
            Self::Practice(format, difficulty) => Game::with_format(seed, difficulty, format),
            Self::Killer(puzzle) => Game::killer(puzzle),
            Self::Daily(day) => Game::daily(day),
            Self::Challenge(seed) => Game::new(seed, Difficulty::Medium),
        }
    }

    fn result_mode(self, launch: &GameLaunch, hints_used: u32) -> String {
        match self {
            Self::Practice(format, difficulty) => {
                let size = format.size();
                format!("{size}×{size} · {}", difficulty.label())
            }
            Self::Killer(puzzle) => {
                let result = if hints_used == 0 {
                    "Clean solve"
                } else {
                    "With hints"
                };
                format!("Killer Sudoku · {} · {result}", puzzle.label())
            }
            Self::Daily(day) => i32::try_from(day)
                .ok()
                .and_then(NaiveDate::from_epoch_days)
                .map_or_else(|| "Daily".to_owned(), |date| format!("Daily {date}")),
            Self::Challenge(_) => daily::from_launch(launch).map_or_else(
                || "Challenge".to_owned(),
                |puzzle| format!("Daily challenge · {}", puzzle.date),
            ),
        }
    }
}

struct Round {
    seed: u64,
    game: Game,
    selected: usize,
    note_mode: bool,
    hint: Option<Hint>,
    hint_open: bool,
    hints_used: u32,
    best: Option<u32>,
    attempt: AttemptId,
    moves: u64,
}

impl Round {
    fn new(puzzle: Puzzle, practice_seed: u64) -> Self {
        let seed = puzzle.fixed_seed().unwrap_or(practice_seed);
        let game = puzzle.start(seed);
        let selected = (0..game.format().cell_count())
            .find(|&index| !game.is_given(index))
            .unwrap_or(0);
        Self {
            seed,
            game,
            selected,
            note_mode: false,
            hint: None,
            hint_open: false,
            hints_used: 0,
            best: None,
            attempt: AttemptId::default(),
            moves: 0,
        }
    }

    fn edit(&mut self, digit: u8, note: bool) -> Option<Effect> {
        let value = self.game.value(self.selected);
        if !self.game.enter(self.selected, digit, note) {
            return None;
        }
        self.clear_hint();
        if value == self.game.value(self.selected) {
            return None;
        }
        self.moves = self.moves.saturating_add(1);
        self.update_best();
        Some(if self.game.is_solved() {
            Effect::Complete
        } else {
            Effect::Move
        })
    }

    fn update_best(&mut self) {
        if self.game.is_solved() && self.best.is_none_or(|best| self.hints_used < best) {
            self.best = Some(self.hints_used);
        }
    }

    fn undo(&mut self) -> Option<Effect> {
        let before = self.game.checkpoint(1).values;
        let values_changed = before != self.game.checkpoint(0).values;
        if !self.game.undo() {
            return None;
        }
        self.clear_hint();
        values_changed.then_some(Effect::Move)
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
                self.hints_used = self.hints_used.saturating_add(1);
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

    fn apply_hint(&mut self) -> Option<Effect> {
        let hint = self.visible_hint()?;
        self.selected = hint.index;
        self.edit(hint.entry(), false)
    }

    fn clear_hint(&mut self) {
        self.hint = None;
        self.hint_open = false;
    }
}

pub struct GameView {
    focus: FocusHandle,
    focus_on_mount: bool,
    launch: GameLaunch,
    session: Session,
    rounds: HashMap<Puzzle, Round>,
    puzzle: Puzzle,
    modes: PracticeModes,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let session = Session::new(launch);
        let modes = match launch.entry {
            GameEntry::Challenge(_) => PracticeModes::default(),
            _ => session.load::<PracticeModes>().unwrap_or_default(),
        };
        let puzzle = match &launch.entry {
            GameEntry::Challenge(id) => {
                let seed = id.bytes().fold(0u64, |hash, byte| {
                    hash.wrapping_mul(131).wrapping_add(u64::from(byte))
                });
                Puzzle::Challenge(seed)
            }
            GameEntry::Classic | GameEntry::Mode(_) => modes.selected_puzzle(),
        };
        let round = Round::load(puzzle, &puzzle.session(launch));
        Self {
            focus: cx.focus_handle().tab_stop(true),
            focus_on_mount: true,
            launch: launch.clone(),
            session,
            rounds: HashMap::from([(puzzle, round)]),
            puzzle,
            modes,
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
        if self.puzzle == puzzle {
            return;
        }
        self.rounds
            .entry(puzzle)
            .or_insert_with(|| Round::load(puzzle, &puzzle.session(&self.launch)));
        self.puzzle = puzzle;
        self.modes.select(puzzle);
        self.session.save(&self.modes);
    }

    fn start_over(&mut self) {
        let previous = self.round();
        let mut next = Round::new(self.puzzle, previous.seed.wrapping_add(1));
        next.best = previous.best;
        self.rounds.insert(self.puzzle, next);
    }

    fn save_round(&self, effect: Option<Effect>, cx: &Context<Self>) {
        let round = self.round();
        let session = self.puzzle.session(&self.launch);
        round.save(&session);
        if let Some(effect) = effect {
            if matches!(effect, Effect::Complete) {
                session.record_result(
                    round.attempt,
                    self.puzzle.result_mode(&self.launch, round.hints_used),
                    Outcome::Won,
                    ResultMetric::Moves(round.moves),
                );
            }
            audio::play(cx, effect);
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        let key = stroke.key.as_str();
        let modifiers = stroke.modifiers;
        if !self.focus.is_focused(window) || modifiers.alt || modifiers.function {
            return;
        }
        let round = self.round_mut();
        let mut effect = None;
        if modifiers.control || modifiers.platform {
            if key != "z" || modifiers.shift {
                return;
            }
            if !event.is_held {
                effect = round.undo();
            }
        } else {
            match key {
                "up" => round.move_selection(-1, 0),
                "down" => round.move_selection(1, 0),
                "left" => round.move_selection(0, -1),
                "right" => round.move_selection(0, 1),
                "space" => round.select_blank(modifiers.shift),
                "backspace" | "delete" => effect = round.edit(0, false),
                "n" if !event.is_held => round.toggle_notes(),
                "h" if !event.is_held => round.show_hint(),
                "enter" if round.hint_open && !event.is_held => effect = round.apply_hint(),
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
                        effect = round.edit(digit, round.note_mode);
                    }
                }
            }
        }
        cx.stop_propagation();
        self.save_round(effect, cx);
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
                    |view| {
                        view.open(view.modes.practice(Format::Standard));
                        None
                    },
                ))
                .child(control(
                    "sudoku-mini",
                    "Mini 6×6",
                    ButtonStyle::selected(practice && format == Format::Mini),
                    cx,
                    |view| {
                        view.open(view.modes.practice(Format::Mini));
                        None
                    },
                ))
                .child(control(
                    "sudoku-killer",
                    "Killer Sudoku",
                    ButtonStyle::selected(matches!(self.puzzle, Puzzle::Killer(_))),
                    cx,
                    |view| {
                        view.open(view.modes.killer());
                        None
                    },
                ))
                .child(control(
                    "sudoku-daily",
                    "Daily puzzle",
                    ButtonStyle::selected(self.puzzle == Puzzle::Daily(today)),
                    cx,
                    |view| {
                        view.open(Puzzle::Daily(utc_day()));
                        None
                    },
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
        let cage = killer::Cell::new(game, index, round.selected, solved);
        let hint = round.visible_hint();
        let hint_target = hint.is_some_and(|hint| hint.index == index);
        let hint_source = hint.is_some_and(|hint| {
            matches!(hint.reason, HintReason::LockedCandidate { source, .. } if source.contains(format, index))
        });
        let hint_evidence = hint.is_some_and(|hint| match hint.reason {
            HintReason::OnlyCandidate { .. } => format.related(hint.index, index),
            HintReason::OnlyPlace { unit, .. } => unit.contains(format, index),
            HintReason::LockedCandidate { shared, .. } => shared.contains(format, index),
            HintReason::Duplicate { peer, .. } => peer == index,
            HintReason::CageCandidate { .. } | HintReason::CageConflict { .. } => {
                cage.is_some_and(|cell| cell.contains(hint.index))
            }
            HintReason::IncorrectEntry { .. } | HintReason::Forced { .. } => false,
        });

        let fill = if conflict {
            ERROR_SURFACE
        } else if cage.is_some_and(|cell| cell.has_issue()) {
            killer::CAGE_ERROR_SURFACE
        } else if hint_target {
            0xf9dfa3
        } else if selected {
            0xc3dfbd
        } else if hint_source {
            0xf7e6bd
        } else if hint_evidence {
            HINT_SURFACE
        } else if same_digit {
            0xd6e9d7
        } else if cage.is_some_and(|cell| cell.is_active()) {
            killer::CAGE_ACTIVE_SURFACE
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
            .debug_selector(move || format!("sudoku-cell-{index}"))
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
                        !conflict
                            && !selected
                            && !hint_target
                            && !hint_source
                            && !hint_evidence
                            && !cage.is_some_and(|cell| cell.has_issue()),
                        |cell| cell.hover(|style| style.bg(rgb(0xe4ead8))),
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.round_mut().selected = index;
                        view.save_round(None, cx);
                        view.focus.focus(window, cx);
                        cx.notify();
                    }))
            })
            .when_some(cage, |cell, cage| {
                cage.render(cell, size, game.notes(index), value, selected, conflict)
            })
            .when(cage.is_none(), |cell| {
                cell.children((value != 0).then(|| value.to_string()))
                    .children(
                        (game.notes(index) != 0)
                            .then(|| notes_grid(format, size, game.notes(index))),
                    )
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

    fn board(&self, size: f32, cx: &Context<Self>) -> Stateful<Div> {
        let game = &self.round().game;
        let format = game.format();
        let boxes_wide = format.size() / format.box_columns();
        let boxes_high = format.size() / format.box_rows();
        let solved = game.is_solved();
        div()
            .id("sudoku-board")
            .debug_selector(|| "sudoku-board".into())
            .when(!game.cages().is_empty(), |board| {
                board
                    .role(gpui::Role::Grid)
                    .aria_label("Killer Sudoku grid")
            })
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
                    move |view| {
                        view.open(Puzzle::Practice(format, difficulty));
                        None
                    },
                )
                .flex_1()
            }))
    }

    fn keypad(&self, compact: bool, cx: &Context<Self>) -> Div {
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
                            round.edit(digit, round.note_mode)
                        },
                    )
                    .h(px(if compact { 36. } else { 43. }))
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
                            |view| {
                                view.round_mut().toggle_notes();
                                None
                            },
                        )
                        .flex_1(),
                    )
                    .child(
                        control(
                            "sudoku-erase",
                            "Erase",
                            ButtonStyle::enabled(can_erase),
                            cx,
                            |view| view.round_mut().edit(0, false),
                        )
                        .flex_1(),
                    ),
            )
    }

    fn hint_panel(&self, hint: Hint, stack_buttons: bool, cx: &Context<Self>) -> Div {
        let game = &self.round().game;
        let cage = game.cage(hint.index);
        let (title, detail) = cage.map_or_else(
            || hint_message(hint, game.format()),
            |cage| killer::hint_message(hint, cage, game),
        );
        let action = if hint.entry() == 0 {
            "Clear".to_owned()
        } else {
            format!("Place {}", hint.entry())
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .bg(rgb(HINT_SURFACE))
            .text_color(rgb(t::INK))
            .p_3()
            .when(cage.is_some(), |panel| {
                panel.p_2().text_size(px(13.)).line_height(px(17.))
            })
            .child(div().font_weight(FontWeight::BOLD).child(title))
            .child(div().text_size(px(13.)).line_height(px(18.)).child(detail))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .when(stack_buttons, |actions| actions.flex_col().gap_1())
                    .child(
                        control(
                            "sudoku-apply-hint",
                            action,
                            ButtonStyle::Primary,
                            cx,
                            |view| view.round_mut().apply_hint(),
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
                                None
                            },
                        )
                        .flex_1(),
                    ),
            )
    }

    fn controls(&self, width: f32, compact: bool, cx: &Context<Self>) -> Div {
        let palette = t::colors(cx);
        let round = self.round();
        let game = &round.game;
        let solved = game.is_solved();
        let daily = self.puzzle.is_daily();
        let mut panel = t::panel(cx)
            .w(px(width))
            .flex_shrink_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .when(compact, |panel| panel.p_3().gap_2());

        if let Puzzle::Practice(format, selected) = self.puzzle {
            panel = panel.child(self.difficulty(format, selected, cx));
        }

        if solved {
            panel = panel.child(completed(round, daily, palette));
        } else {
            panel = panel
                .child(progress(game, palette))
                .when(daily, |panel| panel.child(daily_goal(round, palette)))
                .when_some(round.best.filter(|_| !daily), |panel, best| {
                    panel.child(best_result(best, palette))
                })
                .when(!compact, |panel| panel.child(self.keypad(compact, cx)));
        }

        panel
            .child(self.edit_actions(cx))
            .when_some(round.visible_hint(), |panel, hint| {
                panel.child(self.hint_panel(hint, false, cx))
            })
            .when(compact && !solved, |panel| {
                panel.child(self.keypad(compact, cx))
            })
            .child(self.reset_button(daily, cx))
    }

    fn edit_actions(&self, cx: &Context<Self>) -> Div {
        let round = self.round();
        div()
            .flex()
            .gap_2()
            .child(
                control(
                    "sudoku-undo",
                    "Undo",
                    ButtonStyle::enabled(round.game.can_undo()),
                    cx,
                    |view| view.round_mut().undo(),
                )
                .flex_1(),
            )
            .when(!round.game.is_solved(), |row| {
                row.child(
                    control(
                        "sudoku-hint",
                        "Hint",
                        ButtonStyle::selected(round.hint_open),
                        cx,
                        |view| {
                            view.round_mut().show_hint();
                            None
                        },
                    )
                    .flex_1(),
                )
            })
    }

    fn reset_button(&self, retry_same: bool, cx: &Context<Self>) -> Stateful<Div> {
        control(
            "sudoku-next",
            if retry_same {
                "Retry puzzle"
            } else {
                "New puzzle"
            },
            if self.round().game.is_solved() {
                ButtonStyle::Primary
            } else {
                ButtonStyle::Normal
            },
            cx,
            |view| {
                view.start_over();
                None
            },
        )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = t::colors(cx);
        if self.focus_on_mount {
            self.focus_on_mount = false;
            cx.defer_in(window, |view, window, cx| view.focus.focus(window, cx));
        }

        let root = div()
            .id("sudoku-game")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .flex()
            .flex_col()
            .gap_3()
            .when(!matches!(self.puzzle, Puzzle::Challenge(_)), |root| {
                root.child(self.modes(cx))
            });
        if let Puzzle::Killer(puzzle) = self.puzzle {
            return root
                .role(gpui::Role::Group)
                .aria_label("Sudoku practice")
                .gap_2()
                .child(self.killer_body(puzzle, window, cx));
        }

        let format = self.round().game.format();
        let size = format.size();
        let content = board_space(window);
        let beside = content >= 500.;
        let panel_width = if beside {
            (content * 0.38).clamp(196., PANEL_WIDTH)
        } else {
            0.
        };
        let available = if beside {
            content - panel_width - SECTION_GAP
        } else {
            content
        };
        let compact = window.viewport_size().height < px(700.) || (beside && panel_width < 250.);
        let gutters = GROUP_GAP * (size / format.box_columns() - 1) as f32 + BOARD_EDGE * 2.;
        let max_cell = if format == Format::Mini { 69. } else { 54. };
        let cell = ((available - gutters) / size as f32)
            .clamp(22., max_cell)
            .floor();
        let board_width = cell * size as f32 + gutters;
        let panel_width = if beside { panel_width } else { board_width };
        let instruction = match self.puzzle {
            Puzzle::Practice(Format::Standard, _) => "Fill every row, column, and box with 1–9.",
            Puzzle::Practice(Format::Mini, _) => "Fill every row, column, and 2×3 box with 1–6.",
            Puzzle::Daily(day) if day != utc_day() => {
                "Previous daily · Select Daily puzzle for today’s grid."
            }
            Puzzle::Daily(_) | Puzzle::Challenge(_) => "Classic 9×9 · Medium difficulty",
            Puzzle::Killer(_) => unreachable!(),
        };

        root.child(
            div()
                .text_color(rgb(palette.muted))
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
                        .child(shortcuts(size, palette)),
                )
                .child(self.controls(panel_width, compact, cx)),
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

fn progress(game: &Game, palette: t::Palette) -> Div {
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
            div().h(px(5.)).rounded_full().bg(rgb(palette.hover)).child(
                div()
                    .h_full()
                    .w(relative(progress))
                    .rounded_full()
                    .bg(rgb(palette.success)),
            ),
        )
        .when(conflict, |status| {
            status.child(
                div()
                    .text_color(rgb(palette.danger))
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .child("Check the underlined digits for duplicates."),
            )
        })
}

fn daily_goal(round: &Round, palette: t::Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .bg(rgb(palette.hover))
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
            goal.child(best_result(best, palette))
        })
}

fn completed(round: &Round, daily: bool, palette: t::Palette) -> Div {
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
        .bg(rgb(palette.hover))
        .p_3()
        .child(t::display_type("Puzzle complete", 20.))
        .child(
            div()
                .text_size(px(13.))
                .line_height(px(18.))
                .child(description),
        )
        .when_some(
            round.best.filter(|&best| best < round.hints_used),
            |panel, best| panel.child(best_result(best, palette)),
        )
}

fn best_result(hints: u32, palette: t::Palette) -> Div {
    div()
        .text_size(px(13.))
        .line_height(px(18.))
        .text_color(rgb(palette.muted))
        .child(if hints == 0 {
            "Best · Clean solve".to_owned()
        } else {
            format!("Best · {}", hint_count(hints).to_lowercase())
        })
}

fn hint_count(hints: u32) -> String {
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
        HintReason::LockedCandidate {
            digit,
            excluded,
            source,
            shared,
        } => {
            let detail = if matches!(source, Unit::Box(_)) {
                format!(
                    "In the shaded box, all spots for {excluded} lie in {}. That line has to use {excluded} inside the box, leaving only {digit} here.",
                    unit_name(shared)
                )
            } else {
                format!(
                    "In {}, all spots for {excluded} lie in the shaded box. That box has to use {excluded} on that line, leaving only {digit} here.",
                    unit_name(source)
                )
            };
            (format!("{excluded} is locked"), detail)
        }
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
        HintReason::CageConflict { .. } | HintReason::CageCandidate { .. } => unreachable!(),
    }
}

fn unit_name(unit: Unit) -> String {
    match unit {
        Unit::Row(row) => format!("row {}", row + 1),
        Unit::Column(column) => format!("column {}", column + 1),
        Unit::Box(_) => "the highlighted box".to_owned(),
    }
}

fn shortcuts(size: usize, palette: t::Palette) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap_x(px(12.))
        .text_size(px(12.))
        .line_height(px(19.))
        .text_color(rgb(palette.muted))
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
    action: impl Fn(&mut GameView) -> Option<Effect> + 'static,
) -> Stateful<Div> {
    let palette = t::colors(cx);
    let id = id.into();
    let title = title.into();
    let (fill, border, ink) = match style {
        ButtonStyle::Normal => (palette.surface, palette.border, palette.ink),
        ButtonStyle::Active => (palette.hover, palette.success, palette.ink),
        ButtonStyle::Primary => (t::GOLD, t::GOLD, t::INK),
        ButtonStyle::Disabled => (palette.paper, palette.border, palette.muted),
    };
    let hover = if style == ButtonStyle::Primary {
        t::GOLD_HOVER
    } else {
        palette.hover
    };
    div()
        .id(id.clone())
        .debug_selector(move || id.to_string())
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
                .role(gpui::Role::Button)
                .aria_label(title.clone())
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(hover)))
                .focus_visible(|style| style.border_color(rgb(palette.ink)).bg(rgb(hover)))
                .on_click(cx.listener(move |view, _, window, cx| {
                    let effect = action(view);
                    view.save_round(effect, cx);
                    view.focus.focus(window, cx);
                    cx.notify();
                }))
        })
        .child(title)
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
