mod field_art;
mod session;
#[cfg(all(test, debug_assertions))]
mod tests;

use std::{
    sync::{Arc, LazyLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use corner_table_minesweeper::{Board, Clue, Difficulty, Hint, HintReason, Status, Tile};
use gpui::{
    App, Context, Div, FocusHandle, FontFeatures, FontWeight, KeyDownEvent, MouseButton, Render,
    Role, SharedString, Stateful, Task, Window, div, prelude::*, px, rgb,
};

use crate::{
    audio::{self, Effect},
    daily,
    games::{GameEntry, GameLaunch, themed_control},
    storage::{Outcome, ResultMetric, Session},
    theme as t,
};

use self::{
    field_art::{FieldArt, OPENING_EDGE, fastener},
    session::{Bests, Mode, Round, SavedGame, milliseconds},
};

const CELL_GAP: f32 = 2.;
const FRAME_PADDING: f32 = 7.;
const WELL_PADDING: f32 = 3.;
const FIELD_INSET: f32 = FRAME_PADDING + WELL_PADDING + 2.; // Includes both borders.
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(5);
const IMPACT_DURATION: Duration = Duration::from_millis(200);

#[derive(Clone, Copy)]
enum Move {
    Reveal,
    Flag,
}

#[derive(Clone, Copy, Default)]
enum HintState {
    #[default]
    Hidden,
    NoDeduction,
    Shown(Hint),
}

impl HintState {
    fn target(self, column: usize, row: usize) -> Option<bool> {
        match self {
            Self::Shown(Hint::Square {
                column: x,
                row: y,
                mine,
                ..
            }) if (x, y) == (column, row) => Some(mine),
            _ => None,
        }
    }

    fn feedback(self, palette: t::Palette) -> Option<(String, String, u32)> {
        let label = |clue| match clue {
            Clue::Square { column, row } => {
                format!("the clue at row {}, column {}", row + 1, column + 1)
            }
            Clue::MinesRemaining => "the remaining mine count".to_owned(),
        };
        match self {
            Self::Hidden => None,
            Self::NoDeduction => Some((
                "No deduction found".into(),
                "Review your flags. Hints use the numbers visible on the board.".into(),
                palette.ink,
            )),
            Self::Shown(Hint::FlagConflict(clue)) => Some((
                "Check your flags".into(),
                format!("Your current flags conflict with {}.", label(clue)),
                palette.danger,
            )),
            Self::Shown(Hint::Square {
                column,
                row,
                mine,
                reason,
            }) => {
                let action = if mine { "Flag" } else { "Reveal" };
                let title = format!("{action} row {}, column {}", row + 1, column + 1);
                let detail = match reason {
                    HintReason::Direct(clue) => format!(
                        "This move follows from {} if your flags are correct.",
                        label(clue)
                    ),
                    HintReason::Difference { larger, smaller } => {
                        format!(
                            "Compare {} with {}. This move assumes your flags are correct.",
                            label(larger),
                            label(smaller)
                        )
                    }
                };
                Some((title, detail, palette.gold_text))
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Clock {
    Ready,
    Running { started: Instant, carried: Duration },
    Finished(Duration),
}

impl Clock {
    fn elapsed(&self) -> Duration {
        match *self {
            Self::Ready => Duration::ZERO,
            Self::Running { started, carried } => carried.saturating_add(started.elapsed()),
            Self::Finished(elapsed) => elapsed,
        }
    }
}

pub struct GameView {
    storage: Session,
    round: Round,
    bests: Bests,
    challenge_seed: Option<u64>,
    challenge_label: Option<String>,
    clock: Clock,
    clock_task: Option<Task<()>>,
    generation_task: Option<Task<()>>,
    generation_failed: bool,
    hint: HintState,
    impact: Option<Instant>,
    focus: FocusHandle,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let challenge_seed = match &launch.entry {
            GameEntry::Challenge(id) => Some(stable_seed(id)),
            _ => None,
        };
        let challenge_label = challenge_seed.map(|_| {
            daily::from_launch(launch).map_or_else(
                || "Challenge".to_owned(),
                |puzzle| format!("Daily {}", puzzle.date),
            )
        });
        let storage = Session::new(launch);
        let (round, elapsed, bests) = storage
            .load::<SavedGame>()
            .and_then(|saved| saved.restore(challenge_seed))
            .unwrap_or_else(|| {
                (
                    Round::new(
                        Difficulty::Meadow,
                        Mode::Classic,
                        challenge_seed.unwrap_or_else(fresh_seed),
                    ),
                    Duration::ZERO,
                    Bests::default(),
                )
            });
        let mut view = Self {
            storage,
            round,
            bests,
            challenge_seed,
            challenge_label,
            clock: Clock::Ready,
            clock_task: None,
            generation_task: None,
            generation_failed: false,
            hint: HintState::default(),
            impact: None,
            focus: cx.focus_handle().tab_stop(true),
        };
        match view.round.board.status() {
            Status::Ready => {}
            Status::Playing => view.start_clock(Instant::now(), elapsed, cx),
            Status::Won | Status::Lost => view.clock = Clock::Finished(elapsed),
        }
        // A native window close on macOS can release its views before app quit.
        cx.on_release(|view, _| view.save()).detach();
        cx.on_app_quit(|view, _| {
            view.save();
            async {}
        })
        .detach();
        view
    }

    fn save(&self) {
        self.storage.save(&SavedGame::capture(
            &self.round,
            self.clock.elapsed(),
            &self.bests,
        ));
    }

    fn reset(&mut self, difficulty: Difficulty, mode: Mode, cx: &mut Context<Self>) {
        self.replace_round(
            Round::new(
                difficulty,
                mode,
                self.challenge_seed.unwrap_or_else(fresh_seed),
            ),
            cx,
        );
    }

    fn replace_round(&mut self, round: Round, cx: &mut Context<Self>) {
        self.round = round;
        self.clock = Clock::Ready;
        self.clock_task = None;
        self.generation_task = None;
        self.generation_failed = false;
        self.hint = HintState::Hidden;
        self.impact = None;
        self.save();
        cx.notify();
    }

    fn start_clock(&mut self, started: Instant, carried: Duration, cx: &Context<Self>) {
        let clock = Clock::Running { started, carried };
        self.clock = clock;
        let executor = cx.background_executor().clone();
        self.clock_task = Some(cx.spawn(async move |view, cx| {
            let mut last_saved = carried;
            loop {
                let until_next_second =
                    Duration::from_nanos(1_000_000_000 - u64::from(clock.elapsed().subsec_nanos()));
                executor.timer(until_next_second).await;
                if view
                    .update(cx, |view, cx| {
                        let elapsed = clock.elapsed();
                        if elapsed.saturating_sub(last_saved) >= CHECKPOINT_INTERVAL {
                            view.save();
                            last_saved = elapsed;
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    fn apply_move(&mut self, position: (usize, usize), action: Move, cx: &mut Context<Self>) {
        if self.generation_task.is_some() {
            return;
        }
        if self.round.mode == Mode::LogicOnly
            && self.round.board.status() == Status::Ready
            && !self.round.replay
            && matches!(action, Move::Reveal)
            && self.round.board.tile(position.0, position.1) == Some(Tile::Covered)
        {
            self.prepare_logical_field(position, cx);
            return;
        }
        self.play(position, action, cx);
    }

    fn prepare_logical_field(&mut self, (x, y): (usize, usize), cx: &mut Context<Self>) {
        let difficulty = self.round.board.difficulty();
        let seed = self.round.board.seed();
        let search = cx
            .background_executor()
            .spawn(async move { Board::logical(difficulty, seed, x, y) });
        self.generation_failed = false;
        self.generation_task = Some(cx.spawn(async move |view, cx| {
            let generated = search.await;
            let _ = view.update(cx, |view, cx| {
                view.generation_task = None;
                if let Some(mut board) = generated {
                    let (width, _) = difficulty.dimensions();
                    for index in view.round.board.checkpoint().flags {
                        board.toggle_flag(index % width, index / width);
                    }
                    view.round.board = board;
                    view.play((x, y), Move::Reveal, cx);
                } else {
                    view.generation_failed = true;
                    audio::play(cx, Effect::InvalidMove);
                }
                cx.notify();
            });
        }));
    }

    fn play(&mut self, (x, y): (usize, usize), action: Move, cx: &mut Context<Self>) {
        let started = Instant::now();
        let board = &mut self.round.board;
        let previous = board.status();
        let tile = board.tile(x, y);
        let changed = match action {
            Move::Flag => board.toggle_flag(x, y),
            Move::Reveal if matches!(tile, Some(Tile::Open(_))) => board.chord(x, y),
            Move::Reveal => board.reveal(x, y),
        };
        if !changed {
            if matches!(action, Move::Reveal)
                && !matches!(previous, Status::Won | Status::Lost)
                && (tile == Some(Tile::Flagged)
                    || (tile == Some(Tile::Covered) && board.required_opening().is_some()))
            {
                audio::play(cx, Effect::InvalidMove);
            }
            return;
        }
        self.hint = HintState::Hidden;
        let mut result = None;
        match board.status() {
            Status::Playing if previous == Status::Ready => {
                self.start_clock(started, Duration::ZERO, cx)
            }
            status @ (Status::Won | Status::Lost) => {
                let elapsed = if previous == Status::Ready {
                    started.elapsed()
                } else {
                    self.clock.elapsed()
                };
                self.clock = Clock::Finished(elapsed);
                self.clock_task = None;
                if status == Status::Lost && impact_motion_enabled(cx) {
                    self.impact = Some(Instant::now());
                }
                if self.challenge_seed.is_none() {
                    self.bests.record(&self.round, elapsed);
                }
                result = self
                    .round
                    .outcome(elapsed)
                    .map(|outcome| (outcome, elapsed));
            }
            _ => {}
        }
        self.save();
        let effect = if let Some((outcome, elapsed)) = result {
            self.storage.record_result(
                self.round.attempt,
                self.result_mode(),
                outcome,
                ResultMetric::TimeMillis(milliseconds(elapsed)),
            );
            if outcome == Outcome::Won {
                Effect::Complete
            } else {
                Effect::InvalidMove
            }
        } else {
            match action {
                Move::Flag if tile == Some(Tile::Flagged) => Effect::Unflag,
                Move::Flag => Effect::Flag,
                Move::Reveal => Effect::Reveal,
            }
        };
        audio::play(cx, effect);
    }

    fn result_mode(&self) -> String {
        let mode = self
            .challenge_label
            .as_deref()
            .unwrap_or(self.round.mode.label());
        let difficulty = self.round.board.difficulty().label();
        let replay = if self.round.replay { " · Replay" } else { "" };
        format!("{difficulty} · {mode}{replay}")
    }

    fn select_and_play(
        &mut self,
        position: (usize, usize),
        action: Move,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.round.selected = position;
        self.focus.focus(window, cx);
        self.apply_move(position, action, cx);
        cx.notify();
    }

    fn show_hint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.round.board.status() != Status::Playing {
            return;
        }
        let hint = self.round.board.hint();
        self.hint = hint.map_or(HintState::NoDeduction, HintState::Shown);
        if let Some(Hint::Square { column, row, .. }) = hint {
            self.round.selected = (column, row);
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.modifiers.control || key.modifiers.platform || key.modifiers.alt {
            return;
        }
        let (width, height) = self.round.board.difficulty().dimensions();
        let (x, y) = self.round.selected;
        match key.key.as_str() {
            "up" => self.round.selected = (x, y.saturating_sub(1)),
            "down" => self.round.selected = (x, (y + 1).min(height - 1)),
            "left" => self.round.selected = (x.saturating_sub(1), y),
            "right" => self.round.selected = ((x + 1).min(width - 1), y),
            "enter" | "space" if !event.is_held => self.apply_move((x, y), Move::Reveal, cx),
            "f" if !event.is_held => self.apply_move((x, y), Move::Flag, cx),
            "h" if !event.is_held => self.show_hint(window, cx),
            "escape" if !event.is_held => self.hint = HintState::Hidden,
            "enter" | "space" | "f" | "h" => {}
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn cell(
        &self,
        (x, y): (usize, usize),
        cell_size: f32,
        art: &'static FieldArt,
        impact: Option<f32>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let board = &self.round.board;
        let tile = board.tile(x, y).expect("cell within field");
        let covered = matches!(tile, Tile::Covered | Tile::Flagged);
        let required_opening = board.required_opening();
        let opening = required_opening == Some((x, y));
        let chordable = board.can_chord(x, y);
        let active = self.can_play();
        let primary = match tile {
            Tile::Covered => opening || self.round.flag_mode || required_opening.is_none(),
            Tile::Flagged => self.round.flag_mode,
            _ => chordable,
        };
        let variant = (x + y * 3) % 5 < 2;
        let face = art.face(tile, chordable, variant);
        let hint = self.hint.target(x, y);
        let focus_ink = if hint.is_some() {
            t::GOLD
        } else {
            art.focus_ink(tile)
        };
        let mut cell = div()
            .id(("minesweeper-cell", x))
            .role(Role::GridCell)
            .aria_column_index(x)
            .aria_label(if opening {
                "Replay opening".into()
            } else {
                tile_label(tile)
            })
            .when(chordable, |cell| {
                cell.aria_description("Matching flags. Activate to open unmarked neighbors.")
            })
            .when_some(hint, |cell, mine| {
                cell.aria_description(if mine {
                    "Hint suggests a flag"
                } else {
                    "Hint suggests a reveal"
                })
            })
            .relative()
            .w_full()
            .aspect_square()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px((cell_size * 0.15).clamp(3., 5.)))
            .border_1()
            .border_color(rgb(if opening { OPENING_EDGE } else { face.edge }))
            .when(hint.is_some(), |cell| {
                cell.border_2().border_color(rgb(t::GOLD))
            })
            .bg(face.finish())
            .when(self.round.selected == (x, y), |cell| {
                cell.track_focus(&self.focus)
                    .focus(|style| style.border_2().border_color(rgb(focus_ink)))
            });

        if active {
            cell = cell
                .when(primary, |cell| {
                    cell.cursor_pointer()
                        .hover(|style| {
                            style.bg(rgb(if covered {
                                art.cover_hover
                            } else {
                                art.chord_hover
                            }))
                        })
                        .active(|style| {
                            style.bg(rgb(if covered {
                                art.cover_pressed
                            } else {
                                art.chord_pressed
                            }))
                        })
                })
                .on_click(cx.listener(move |view, _, window, cx| {
                    let action = if covered && view.round.flag_mode && !opening {
                        Move::Flag
                    } else {
                        Move::Reveal
                    };
                    view.select_and_play((x, y), action, window, cx);
                }));
            if covered && !opening {
                cell = cell.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |view, _, window, cx| {
                        view.select_and_play((x, y), Move::Flag, window, cx);
                    }),
                );
            }
        }
        if chordable || !matches!(tile, Tile::Open(_)) {
            cell = cell.child(
                div()
                    .absolute()
                    .inset_0()
                    .child(art.detail(tile, opening, chordable, impact)),
            );
        }
        cell.when_some(
            match tile {
                Tile::Open(count) if count > 0 => Some(count),
                _ => None,
            },
            |cell, count| {
                let clue_size = (cell_size * 0.62).round();
                cell.text_color(rgb(art.clue_ink(count)))
                    .text_size(px(clue_size))
                    .line_height(px(clue_size))
                    .font_weight(FontWeight::BOLD)
                    .child(count.to_string())
            },
        )
    }

    fn can_play(&self) -> bool {
        self.generation_task.is_none()
            && matches!(self.round.board.status(), Status::Ready | Status::Playing)
    }

    fn toolbar(&self, cx: &Context<Self>) -> Div {
        let palette = t::colors(cx);
        let difficulty = self.round.board.difficulty();
        let mode = self.round.mode;
        let reset_label = if self.challenge_seed.is_some() {
            "Restart"
        } else {
            "New field"
        };
        let actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(if self.can_play() {
                toggle(
                    palette,
                    "minesweeper-flag-mode",
                    "Flag mode",
                    self.round.flag_mode,
                )
                .aria_description("Click covered squares to place or remove a flag.")
                .on_click(cx.listener(|view, _, _, cx| {
                    view.round.flag_mode = !view.round.flag_mode;
                    view.save();
                    cx.notify();
                }))
            } else {
                themed_control(cx, "minesweeper-flag-mode", "Flag mode", false)
            })
            .when(
                matches!(self.round.board.status(), Status::Won | Status::Lost),
                |actions| {
                    actions.child(
                        themed_control(cx, "minesweeper-replay", "Replay field", true)
                            .aria_description("Play the same mines and opening again as practice.")
                            .on_click(cx.listener(|view, _, window, cx| {
                                if let Some(round) = view.round.replay() {
                                    view.replace_round(round, cx);
                                    view.focus.focus(window, cx);
                                }
                            })),
                    )
                },
            )
            .when(self.round.board.status() == Status::Playing, |actions| {
                actions.child(
                    themed_control(cx, "minesweeper-hint", "Hint", true)
                        .aria_keyshortcuts("H")
                        .on_click(cx.listener(|view, _, window, cx| view.show_hint(window, cx))),
                )
            })
            .child(
                themed_control(cx, "minesweeper-reset", reset_label, true).on_click(cx.listener(
                    |view, _, window, cx| {
                        view.reset(view.round.board.difficulty(), view.round.mode, cx);
                        view.focus.focus(window, cx);
                    },
                )),
            );

        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x_4()
            .gap_y_2()
            .when(self.challenge_seed.is_none(), |toolbar| {
                toolbar
                    .child(
                        div()
                            .id("minesweeper-sizes")
                            .role(Role::Group)
                            .aria_label("Field size")
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(Difficulty::ALL.map(|option| {
                                toggle(palette, option.id(), option.label(), difficulty == option)
                                    .aria_description("Changing the size starts a new field.")
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.round.board.difficulty() != option {
                                            view.reset(option, view.round.mode, cx);
                                        }
                                    }))
                            })),
                    )
                    .child(
                        div()
                            .id("minesweeper-modes")
                            .role(Role::Group)
                            .aria_label("Field mode")
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(Mode::ALL.map(|option| {
                                toggle(palette, option.id(), option.label(), mode == option)
                                    .aria_description("Changing the mode starts a new field.")
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.round.mode != option {
                                            view.reset(view.round.board.difficulty(), option, cx);
                                        }
                                    }))
                            })),
                    )
            })
            .child(actions)
    }

    fn status(
        &self,
        elapsed: Duration,
        limit: Option<Duration>,
        palette: t::Palette,
    ) -> Stateful<Div> {
        let difficulty = self.round.board.difficulty();
        let (width, height) = difficulty.dimensions();
        let field = format!("{} · {width} × {height}", difficulty.label());
        let elapsed_label = format_result(elapsed);
        let flag_hint = "Flag mode is on. Click a covered square to mark it.";
        let (title, detail, ink) = match (self.round.board.status(), limit) {
            (Status::Ready, _) if self.generation_task.is_some() => (
                "Finding a logic field".into(),
                "Checking for a path you can solve by deduction.".into(),
                palette.ink,
            ),
            (Status::Ready, _) if self.generation_failed => (
                "Try a different opening".into(),
                "No deduction path was found here. Choose another square or start a new field."
                    .into(),
                palette.gold_text,
            ),
            (Status::Ready, _) if self.round.replay => (
                "Replay field".into(),
                "Open the marked square to practice the same field.".into(),
                palette.ink,
            ),
            (Status::Ready, _) if self.round.flag_mode => (field, flag_hint.into(), palette.ink),
            (Status::Ready, Some(limit)) => (
                field,
                format!(
                    "Clear the field in {}. The clock starts on your first reveal.",
                    format_time(limit.as_secs())
                ),
                palette.ink,
            ),
            (Status::Ready, None) if self.round.mode == Mode::LogicOnly => (
                field,
                "Choose any square to generate a field you can solve by deduction.".into(),
                palette.ink,
            ),
            (Status::Ready, None) => (
                field,
                "Choose any square to open a safe area.".into(),
                palette.ink,
            ),
            (Status::Playing, Some(limit)) if elapsed >= limit => (
                "Time is up".into(),
                "Keep playing to clear the field.".into(),
                palette.gold_text,
            ),
            (Status::Playing, _) => (
                field,
                if self.round.flag_mode {
                    flag_hint
                } else if self.round.replay {
                    "Practice with the same field."
                } else {
                    "Uncover every safe square."
                }
                .into(),
                palette.ink,
            ),
            (Status::Won, _) if self.round.replay => (
                "Replay cleared".into(),
                format!("You cleared the field in {elapsed_label}."),
                palette.success,
            ),
            (Status::Won, Some(limit)) if elapsed < limit => (
                "Time trial cleared".into(),
                format!("You cleared the field in {elapsed_label}."),
                palette.success,
            ),
            (Status::Won, Some(limit)) => (
                "Field cleared".into(),
                format!(
                    "You finished in {elapsed_label}. The time trial target was {}.",
                    format_time(limit.as_secs())
                ),
                palette.success,
            ),
            (Status::Won, None) => (
                "Field cleared".into(),
                format!("You cleared every safe square in {elapsed_label}."),
                palette.success,
            ),
            (Status::Lost, _) => (
                "Mine hit".into(),
                "Mines and misplaced flags are marked. Replay this field or start a new one."
                    .into(),
                palette.danger,
            ),
        };
        let (title, detail, ink) = self.hint.feedback(palette).unwrap_or((title, detail, ink));

        div()
            .id("minesweeper-status")
            .role(Role::Status)
            .aria_label(format!("{title}. {detail}"))
            .flex()
            .flex_col()
            .gap_px()
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(ink))
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(rgb(palette.muted))
                    .child(detail),
            )
    }

    fn field(
        &self,
        window: &Window,
        elapsed: Duration,
        limit: Option<Duration>,
        art: &'static FieldArt,
        impact: Option<f32>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let board = &self.round.board;
        let difficulty = board.difficulty();
        let (width, height) = difficulty.dimensions();
        let comfortable_tile = match difficulty {
            Difficulty::Meadow => 46.,
            Difficulty::Highlands => 39.,
            Difficulty::Summit => 34.,
        };
        let viewport_height = f32::from(window.viewport_size().height);
        let available_width = super::board_space(window);
        let terminal = matches!(board.status(), Status::Lost | Status::Won);
        // The control groups take a second row when the rail leaves less room.
        let controls_width = if terminal { 820. } else { 720. };
        let header_room = if self.challenge_seed.is_none() && available_width < controls_width {
            284.
        } else {
            240.
        };
        let seams = (width - 1) as f32 * CELL_GAP;
        let tile_by_width = (available_width - 2. * FIELD_INSET - seams) / width as f32;
        let tile = ((viewport_height - header_room) / height as f32 - CELL_GAP)
            .clamp(22., comfortable_tile)
            .min(tile_by_width)
            .max(1.)
            .floor();
        let field_width = width as f32 * tile + seams + 2. * FIELD_INSET;
        let counter_count = if self.challenge_seed.is_some() {
            3.
        } else {
            4.
        };
        let counter_width = (field_width - 2. * (FRAME_PADDING + 1.)) / counter_count;
        let (mine_label, mines) = if board.status() == Status::Lost {
            ("Mines", difficulty.mines() as isize)
        } else {
            (
                "Mines left",
                difficulty.mines() as isize - board.flags() as isize,
            )
        };
        let remaining = limit.filter(|&limit| elapsed < limit && !terminal);
        let (clock_label, clock_value, low_time) = if let Some(limit) = remaining {
            let left = limit - elapsed;
            let seconds = left.as_secs() + u64::from(left.subsec_nanos() > 0);
            (
                "Time left",
                format_time(seconds),
                left <= Duration::from_secs(30),
            )
        } else if terminal {
            ("Elapsed", format_result(elapsed), false)
        } else {
            ("Elapsed", format_time(elapsed.as_secs()), false)
        };

        let counters = div()
            .flex()
            .items_center()
            .child(meter(
                mine_label,
                mines.to_string(),
                mines < 0,
                counter_width,
                art,
            ))
            .child(
                meter(
                    "Safe left",
                    board.safe_remaining().to_string(),
                    false,
                    counter_width,
                    art,
                )
                .border_l_1()
                .border_color(rgb(art.rule)),
            )
            .child(
                meter(clock_label, clock_value, low_time, counter_width, art)
                    .border_l_1()
                    .border_color(rgb(art.rule)),
            )
            .when(self.challenge_seed.is_none(), |meters| {
                let best = self.bests.time(&self.round);
                meters.child(
                    meter(
                        "Best",
                        best.map_or_else(|| "—".into(), format_result),
                        false,
                        counter_width,
                        art,
                    )
                    .aria_label(best.map_or_else(
                        || "No personal best for this size and mode".into(),
                        |time| format!("Personal best: {}", format_result(time)),
                    ))
                    .border_l_1()
                    .border_color(rgb(art.rule)),
                )
            });
        let grid = div()
            .id("minesweeper-field")
            .debug_selector(|| "minesweeper-field".into())
            .role(Role::Grid)
            .aria_label("Minefield")
            .aria_row_count(height)
            .aria_column_count(width)
            .aria_description(if terminal {
                "Use the arrow keys to inspect the final field."
            } else {
                "Arrow keys move, Enter reveals or chords, and F flags a square."
            })
            // Handle Enter and Space before a focused cell can synthesize a click.
            .capture_key_down(cx.listener(Self::key_down))
            .flex()
            .flex_col()
            .gap(px(CELL_GAP))
            .children((0..height).map(|y| {
                div()
                    .id(("minesweeper-row", y))
                    .role(Role::Row)
                    .aria_row_index(y)
                    .w_full()
                    .grid()
                    .grid_cols(width as u16)
                    .items_start()
                    .gap(px(CELL_GAP))
                    .children((0..width).map(|x| self.cell((x, y), tile, art, impact, cx)))
            }));

        div()
            .relative()
            .w_full()
            .max_w(px(field_width))
            .rounded(px(11.))
            .border_1()
            .border_color(rgb(art.frame.edge))
            .bg(art.frame.finish())
            .p(px(FRAME_PADDING))
            .flex()
            .flex_col()
            .gap_1()
            .children(
                [(true, true), (true, false), (false, true), (false, false)]
                    .map(|(top, left)| fastener(top, left, art)),
            )
            .child(counters)
            .child(
                div()
                    .rounded(px(7.))
                    .border_1()
                    .border_color(rgb(art.well.edge))
                    .bg(art.well.finish())
                    .p(px(WELL_PADDING))
                    .child(grid),
            )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let elapsed = self.clock.elapsed();
        let limit = self.round.limit();
        let palette = t::colors(cx);
        let art = FieldArt::for_palette(palette);
        let impact = self
            .impact
            .map(|started| started.elapsed())
            .filter(|&elapsed| elapsed < IMPACT_DURATION && impact_motion_enabled(cx))
            .map(|elapsed| elapsed.as_secs_f32() / IMPACT_DURATION.as_secs_f32());
        if impact.is_some() {
            window.request_animation_frame();
        } else {
            self.impact = None;
        }
        div()
            .w_full()
            .max_w(px(940.))
            .flex()
            .flex_col()
            .items_start()
            .gap_3()
            .child(self.toolbar(cx))
            .child(self.status(elapsed, limit, palette))
            .child(self.field(window, elapsed, limit, art, impact, cx))
            .when(matches!(self.round.board.status(), Status::Ready | Status::Playing), |game| {
                game.child(
                    div()
                        .flex()
                        .flex_col()
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(rgb(palette.muted))
                        .child("Right-click to flag · Highlighted clues open unmarked neighbors")
                        .child("Tab to the field · Arrow keys move · Enter or Space opens · F flags · H hints"),
                )
            })
    }
}

fn toggle(
    palette: t::Palette,
    id: &'static str,
    label: &'static str,
    selected: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .aria_toggled(selected.into())
        .tab_index(0)
        .flex()
        .items_center()
        .min_h(px(36.))
        .px_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(if selected { 0xcd9836 } else { palette.border }))
        .bg(rgb(if selected { t::GOLD } else { palette.surface }))
        .text_color(rgb(if selected { t::INK } else { palette.ink }))
        .hover(|style| {
            style.bg(rgb(if selected {
                t::GOLD_HOVER
            } else {
                palette.hover
            }))
        })
        .focus_visible(|style| style.border_color(rgb(palette.ink)))
        .cursor_pointer()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}

fn meter(
    label: &'static str,
    value: String,
    warning: bool,
    width: f32,
    art: &FieldArt,
) -> Stateful<Div> {
    static TABULAR: LazyLock<FontFeatures> =
        LazyLock::new(|| FontFeatures(Arc::new(vec![("tnum".into(), 1)])));
    let compact = width < 88.;
    let padding = if compact { 6. } else { 10. };
    // A tabular digit in the bundled typeface takes under 0.6 em.
    let text_width = width - 2. * padding - 1.;
    let value_size = (text_width / (0.6 * value.chars().count() as f32))
        .floor()
        .clamp(11., if compact { 16. } else { 19. });
    div()
        .id(label)
        .role(Role::Label)
        .aria_label(format!("{label}: {value}"))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .px(px(padding))
        .whitespace_nowrap()
        .child(
            div()
                .text_size(px(11.))
                .line_height(px(14.))
                .text_color(rgb(art.label))
                .font_weight(FontWeight::SEMIBOLD)
                .child(label),
        )
        .child(
            div()
                .text_size(px(value_size))
                .line_height(px(23.))
                .text_color(rgb(if warning { art.warning } else { art.value }))
                .font_features(TABULAR.clone())
                .font_weight(FontWeight::BOLD)
                .child(value),
        )
}

fn tile_label(tile: Tile) -> SharedString {
    match tile {
        Tile::Covered => "Covered".into(),
        Tile::Flagged => "Flagged".into(),
        Tile::Open(0) => "Clear".into(),
        Tile::Open(1) => "1 neighboring mine".into(),
        Tile::Open(count) => format!("{count} neighboring mines").into(),
        Tile::Mine => "Mine".into(),
        Tile::Detonated => "Detonated mine".into(),
        Tile::Misflagged => "Misplaced flag".into(),
    }
}

#[cfg(target_os = "macos")]
fn impact_motion_enabled(cx: &App) -> bool {
    use objc2::{
        MainThreadMarker, class, msg_send,
        runtime::{AnyObject, Bool},
    };

    if cx.reduce_motion() || MainThreadMarker::new().is_none() {
        return false;
    }
    // GPUI's app preference is independent of the macOS accessibility setting.
    let workspace: &AnyObject = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
    let reduced: Bool = unsafe { msg_send![workspace, accessibilityDisplayShouldReduceMotion] };
    !reduced.as_bool()
}

#[cfg(not(target_os = "macos"))]
fn impact_motion_enabled(_: &App) -> bool {
    false
}

fn format_time(seconds: u64) -> String {
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            (seconds / 60) % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

fn format_result(elapsed: Duration) -> String {
    format!(
        "{}.{}",
        format_time(elapsed.as_secs()),
        elapsed.subsec_millis() / 100
    )
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
