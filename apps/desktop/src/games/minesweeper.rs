use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use corner_table_minesweeper::{Board, Difficulty, Status, Tile};
use gpui::{
    Context, Div, FocusHandle, FontWeight, KeyDownEvent, MouseButton, PathBuilder, Render, Role,
    SharedString, Stateful, Task, Window, canvas, div, point, prelude::*, px, rgb,
};

use crate::{
    games::{GameEntry, GameLaunch, control},
    theme as t,
};

const CELL_GAP: f32 = 2.;
const FIELD_INSET: f32 = 10.;
const FIELD_RULE: u32 = 0x426257;
const FIELD_CAPTION: u32 = 0xbdd2bf;
const SUCCESS: u32 = 0x286248;
const WARNING: u32 = 0x825207;
const ERROR: u32 = 0x963f33;

#[derive(Clone, Copy)]
enum Move {
    Reveal,
    Flag,
}

enum Clock {
    Ready,
    Running(Instant),
    Finished(Duration),
}

impl Clock {
    fn elapsed(&self) -> Duration {
        match *self {
            Self::Ready => Duration::ZERO,
            Self::Running(started) => started.elapsed(),
            Self::Finished(elapsed) => elapsed,
        }
    }
}

pub struct GameView {
    board: Board,
    challenge_seed: Option<u64>,
    flag_mode: bool,
    time_trial: bool,
    clock: Clock,
    clock_task: Option<Task<()>>,
    focus: FocusHandle,
    selected: (usize, usize),
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let challenge_seed = match &launch.entry {
            GameEntry::Challenge(id) => Some(stable_seed(id)),
            _ => None,
        };
        Self {
            board: Board::new(
                Difficulty::Meadow,
                challenge_seed.unwrap_or_else(fresh_seed),
            ),
            challenge_seed,
            flag_mode: false,
            time_trial: false,
            clock: Clock::Ready,
            clock_task: None,
            focus: cx.focus_handle().tab_stop(true),
            selected: (0, 0),
        }
    }

    fn reset(&mut self, difficulty: Difficulty, cx: &mut Context<Self>) {
        self.board = Board::new(difficulty, self.challenge_seed.unwrap_or_else(fresh_seed));
        self.flag_mode = false;
        self.clock = Clock::Ready;
        self.clock_task = None;
        self.selected = (0, 0);
        cx.notify();
    }

    fn trial_limit(&self) -> Option<Duration> {
        self.time_trial.then(|| {
            Duration::from_secs(match self.board.difficulty() {
                Difficulty::Meadow => 3 * 60,
                Difficulty::Highlands => 6 * 60,
                Difficulty::Summit => 10 * 60,
            })
        })
    }

    fn start_clock(&mut self, cx: &Context<Self>) {
        let started = Instant::now();
        self.clock = Clock::Running(started);
        let executor = cx.background_executor().clone();
        let limit = self.trial_limit();
        self.clock_task = Some(cx.spawn(async move |view, cx| {
            loop {
                let until_next_second =
                    Duration::new(0, 1_000_000_000 - started.elapsed().subsec_nanos());
                executor.timer(until_next_second).await;
                if view.update(cx, |_, cx| cx.notify()).is_err()
                    || limit.is_some_and(|limit| started.elapsed() >= limit)
                {
                    break;
                }
            }
        }));
    }

    fn apply_move(&mut self, (x, y): (usize, usize), action: Move, cx: &mut Context<Self>) {
        let previous = self.board.status();
        let changed = match action {
            Move::Flag => self.board.toggle_flag(x, y),
            Move::Reveal if matches!(self.board.tile(x, y), Some(Tile::Open(_))) => {
                self.board.chord(x, y)
            }
            Move::Reveal => self.board.reveal(x, y),
        };
        if !changed {
            return;
        }
        match self.board.status() {
            Status::Playing if previous == Status::Ready => self.start_clock(cx),
            Status::Won | Status::Lost => {
                self.clock = Clock::Finished(self.clock.elapsed());
                self.clock_task = None;
            }
            _ => {}
        }
    }

    fn select_and_play(
        &mut self,
        position: (usize, usize),
        action: Move,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected = position;
        self.focus.focus(window, cx);
        self.apply_move(position, action, cx);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.modifiers.control || key.modifiers.platform || key.modifiers.alt {
            return;
        }
        let (width, height) = self.board.difficulty().dimensions();
        let (x, y) = self.selected;
        match key.key.as_str() {
            "up" => self.selected = (x, y.saturating_sub(1)),
            "down" => self.selected = (x, (y + 1).min(height - 1)),
            "left" => self.selected = (x.saturating_sub(1), y),
            "right" => self.selected = ((x + 1).min(width - 1), y),
            "enter" | "space" if !event.is_held => self.apply_move(self.selected, Move::Reveal, cx),
            "f" if !event.is_held => self.apply_move(self.selected, Move::Flag, cx),
            "enter" | "space" | "f" => {}
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn cell(&self, x: usize, y: usize, clue_size: f32, cx: &Context<Self>) -> Stateful<Div> {
        let tile = self.board.tile(x, y).expect("cell within field");
        let covered = matches!(tile, Tile::Covered | Tile::Flagged);
        let (background, border, ink) = match tile {
            Tile::Covered | Tile::Flagged => (
                if (x + y).is_multiple_of(2) {
                    0x3b6651
                } else {
                    0x335d4b
                },
                0x4d745f,
                t::GOLD,
            ),
            Tile::Open(0) => (0xcbd8c1, 0xcbd8c1, t::INK),
            Tile::Open(count) => (0xf1ecdd, 0xf1ecdd, clue_color(count)),
            Tile::Mine => (0xe6c1ae, 0xe6c1ae, 0x713a2d),
            Tile::Detonated => (0xa34335, 0xc56a55, 0xfff2df),
            Tile::Misflagged => (0xf3d9cc, 0xf3d9cc, 0x922e26),
        };
        let active = matches!(self.board.status(), Status::Ready | Status::Playing);
        let mut cell = div()
            .id(("minesweeper-cell", x))
            .role(Role::GridCell)
            .aria_column_index(x)
            .aria_label(tile_label(tile))
            .w_full()
            .aspect_square()
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_2()
            .border_color(rgb(border))
            .bg(rgb(background))
            .text_color(rgb(ink))
            .text_size(px(clue_size))
            .line_height(px(clue_size))
            .font_weight(FontWeight::BOLD)
            .when(self.selected == (x, y), |cell| {
                cell.track_focus(&self.focus)
                    .focus(|style| style.border_color(rgb(if covered { t::GOLD } else { t::RAIL })))
            });

        if active && !matches!(tile, Tile::Open(0)) {
            cell = cell
                .cursor_pointer()
                .hover(|style| style.bg(rgb(if covered { 0x537d64 } else { 0xfff7dc })))
                .active(|style| style.bg(rgb(if covered { 0x426c55 } else { 0xe2e5cf })))
                .on_click(cx.listener(move |view, _, window, cx| {
                    let action = if covered && view.flag_mode {
                        Move::Flag
                    } else {
                        Move::Reveal
                    };
                    view.select_and_play((x, y), action, window, cx);
                }));
            if covered {
                cell = cell.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |view, _, window, cx| {
                        view.select_and_play((x, y), Move::Flag, window, cx);
                    }),
                );
            }
        }
        match tile {
            Tile::Open(count) if count > 0 => cell.child(count.to_string()),
            Tile::Flagged | Tile::Mine | Tile::Detonated | Tile::Misflagged => {
                cell.child(mark(tile, ink))
            }
            _ => cell,
        }
    }

    fn toolbar(&self, cx: &Context<Self>) -> Div {
        let difficulty = self.board.difficulty();
        let active = matches!(self.board.status(), Status::Ready | Status::Playing);
        let reset_label = if self.challenge_seed.is_some() {
            "Restart"
        } else {
            "New field"
        };
        let mut actions = div().flex().flex_wrap().items_center().gap_1();
        if self.challenge_seed.is_none() {
            actions = actions.child(
                toggle("minesweeper-time-trial", "Time trial", self.time_trial)
                    .aria_description("Start a new field with or without a countdown.")
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.time_trial = !view.time_trial;
                        view.reset(view.board.difficulty(), cx);
                    })),
            );
        }
        actions = actions
            .child(if active {
                toggle("minesweeper-flag-mode", "Flag mode", self.flag_mode).on_click(cx.listener(
                    |view, _, _, cx| {
                        view.flag_mode = !view.flag_mode;
                        cx.notify();
                    },
                ))
            } else {
                control("minesweeper-flag-mode", "Flag mode", false)
            })
            .child(
                control("minesweeper-reset", reset_label, true)
                    .role(Role::Button)
                    .aria_label(reset_label)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.reset(view.board.difficulty(), cx);
                        view.focus.focus(window, cx);
                    })),
            );

        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x_4()
            .gap_y_2()
            .when(self.challenge_seed.is_none(), |toolbar| {
                toolbar.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .children(Difficulty::ALL.map(|option| {
                            toggle(option.label(), option.label(), difficulty == option).on_click(
                                cx.listener(move |view, _, _, cx| {
                                    if view.board.difficulty() != option {
                                        view.reset(option, cx);
                                    }
                                }),
                            )
                        })),
                )
            })
            .child(actions)
    }

    fn status(&self, elapsed: Duration, limit: Option<Duration>) -> Stateful<Div> {
        let difficulty = self.board.difficulty();
        let (width, height) = difficulty.dimensions();
        let field = format!(
            "{} · {width} × {height} · {} mines",
            difficulty.label(),
            difficulty.mines()
        );
        let elapsed_label = format_time(elapsed.as_secs());
        let flag_hint = "Flag mode is on. Click a covered square to mark it.";
        let (title, detail, ink) = match (self.board.status(), limit) {
            (Status::Ready, _) if self.flag_mode => (field, flag_hint.into(), t::INK),
            (Status::Ready, Some(limit)) => (
                field,
                format!(
                    "Clear the field in {}. The clock starts on your first reveal.",
                    format_time(limit.as_secs())
                ),
                t::INK,
            ),
            (Status::Ready, None) => (
                field,
                "Choose any square to open a safe area.".into(),
                t::INK,
            ),
            (Status::Playing, Some(limit)) if elapsed >= limit => (
                "Time is up".into(),
                "Keep playing to clear the field.".into(),
                WARNING,
            ),
            (Status::Playing, _) => (
                field,
                if self.flag_mode {
                    flag_hint
                } else {
                    "Uncover every safe square."
                }
                .into(),
                t::INK,
            ),
            (Status::Won, Some(limit)) if elapsed < limit => (
                "Time trial cleared".into(),
                format!("You cleared the field in {elapsed_label}."),
                SUCCESS,
            ),
            (Status::Won, Some(limit)) => (
                "Field cleared".into(),
                format!(
                    "You finished in {elapsed_label}. The time trial target was {}.",
                    format_time(limit.as_secs())
                ),
                SUCCESS,
            ),
            (Status::Won, None) => (
                "Field cleared".into(),
                format!("You cleared every safe square in {elapsed_label}."),
                SUCCESS,
            ),
            (Status::Lost, _) => (
                "Mine hit".into(),
                "Mines and misplaced flags are marked. Start a new field to try again.".into(),
                ERROR,
            ),
        };

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
                    .text_color(rgb(t::MUTED))
                    .child(detail),
            )
    }

    fn field(
        &self,
        window: &Window,
        elapsed: Duration,
        limit: Option<Duration>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let difficulty = self.board.difficulty();
        let (width, height) = difficulty.dimensions();
        let comfortable_tile = match difficulty {
            Difficulty::Meadow => 46.,
            Difficulty::Highlands => 39.,
            Difficulty::Summit => 34.,
        };
        let viewport_height = f32::from(window.viewport_size().height);
        // Leave space for the game header, controls, meters, and keyboard help.
        let tile = ((viewport_height - 390.) / height as f32 - CELL_GAP)
            .clamp(22., comfortable_tile)
            .floor();
        let field_width = width as f32 * (tile + CELL_GAP) - CELL_GAP + 2. * FIELD_INSET;
        let viewport_width = f32::from(window.viewport_size().width);
        let clue_size = ((super::board_space(window).min(field_width) - 2. * FIELD_INSET)
            / width as f32
            * 0.56)
            .clamp(13., 22.);
        let compact = viewport_width < 900.;
        let mines_left = difficulty.mines() as isize - self.board.flags() as isize;
        let seconds = limit.map_or(elapsed.as_secs(), |limit| {
            limit.as_secs().saturating_sub(elapsed.as_secs())
        });
        let low_time = limit.is_some() && seconds < 30 && self.board.status() == Status::Playing;
        let clock_label = if limit.is_some() {
            "TIME LEFT"
        } else {
            "ELAPSED"
        };

        div()
            .w_full()
            .max_w(px(field_width))
            .rounded_lg()
            .bg(rgb(t::RAIL))
            .p(px(FIELD_INSET))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .border_b_1()
                    .border_color(rgb(FIELD_RULE))
                    .pb_2()
                    .child(meter(
                        "MINES LEFT",
                        mines_left.to_string(),
                        mines_left < 0,
                        compact,
                    ))
                    .child(
                        meter(
                            "SAFE LEFT",
                            self.board.safe_remaining().to_string(),
                            false,
                            compact,
                        )
                        .border_l_1()
                        .border_color(rgb(FIELD_RULE)),
                    )
                    .child(
                        meter(clock_label, format_time(seconds), low_time, compact)
                            .border_l_1()
                            .border_color(rgb(FIELD_RULE)),
                    ),
            )
            .child(
                div()
                    .id("minesweeper-field")
                    .role(Role::Grid)
                    .aria_label("Minefield")
                    .aria_row_count(height)
                    .aria_column_count(width)
                    .aria_description(
                        "Arrow keys move, Enter reveals or chords, and F flags a square.",
                    )
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
                            .children((0..width).map(|x| self.cell(x, y, clue_size, cx)))
                    })),
            )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let elapsed = self.clock.elapsed();
        let limit = self.trial_limit();
        div()
            .w_full()
            .max_w(px(940.))
            .flex()
            .flex_col()
            .items_start()
            .gap_3()
            .child(self.toolbar(cx))
            .child(self.status(elapsed, limit))
            .child(self.field(window, elapsed, limit, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(rgb(t::MUTED))
                    .child("Click to reveal · Right-click to flag · Click a clue when its neighboring flags match the number")
                    .child("Arrow keys to move · Enter or Space to reveal or chord · F to flag"),
            )
    }
}

fn toggle(id: &'static str, label: &'static str, selected: bool) -> Stateful<Div> {
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
        .border_color(rgb(if selected { 0xcd9836 } else { t::BORDER }))
        .bg(rgb(if selected { t::GOLD } else { t::SURFACE }))
        .hover(|style| style.bg(rgb(if selected { t::GOLD_HOVER } else { t::SAGE })))
        .focus(|style| style.border_color(rgb(t::INK)))
        .cursor_pointer()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}

fn meter(label: &'static str, value: String, warning: bool, compact: bool) -> Stateful<Div> {
    div()
        .id(label)
        .role(Role::Label)
        .aria_label(format!("{label}: {value}"))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .px(px(if compact { 6. } else { 10. }))
        .child(
            div()
                .text_size(px(10.))
                .line_height(px(15.))
                .text_color(rgb(FIELD_CAPTION))
                .font_weight(FontWeight::BOLD)
                .child(label),
        )
        .child(
            div()
                .text_size(px(22.))
                .line_height(px(27.))
                .text_color(rgb(if warning { t::GOLD } else { t::PAPER }))
                .font_weight(FontWeight::BOLD)
                .child(value),
        )
}

fn clue_color(count: u8) -> u32 {
    match count {
        1 => 0x21526d,
        2 => 0x275b36,
        3 => 0x9b3d31,
        4 => 0x604a83,
        5 => 0x803348,
        6 => 0x176466,
        7 => 0x344347,
        _ => 0x594e43,
    }
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

fn mark(tile: Tile, ink: u32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let unit = bounds.size.width.min(bounds.size.height) * 0.6;
            let origin = bounds.center() - point(unit / 2., unit / 2.);
            let at = |x: f32, y: f32| origin + point(unit * x, unit * y);
            let mut stroke = PathBuilder::stroke(px(1.35));
            match tile {
                Tile::Flagged => {
                    stroke.move_to(at(0.24, 0.06));
                    stroke.line_to(at(0.24, 0.94));
                    stroke.move_to(at(0.12, 0.94));
                    stroke.line_to(at(0.56, 0.94));
                    let mut pennant = PathBuilder::fill();
                    pennant.move_to(at(0.24, 0.1));
                    pennant.line_to(at(0.91, 0.34));
                    pennant.line_to(at(0.24, 0.58));
                    pennant.close();
                    if let Ok(path) = pennant.build() {
                        window.paint_path(path, rgb(ink));
                    }
                }
                Tile::Mine | Tile::Detonated => {
                    for ((x1, y1), (x2, y2)) in [
                        ((0., 0.5), (1., 0.5)),
                        ((0.5, 0.), (0.5, 1.)),
                        ((0.14, 0.14), (0.86, 0.86)),
                        ((0.14, 0.86), (0.86, 0.14)),
                    ] {
                        stroke.move_to(at(x1, y1));
                        stroke.line_to(at(x2, y2));
                    }
                    window.paint_quad(
                        gpui::fill(
                            gpui::Bounds::new(at(0.17, 0.17), gpui::size(unit * 0.66, unit * 0.66)),
                            rgb(ink),
                        )
                        .corner_radii(unit * 0.33),
                    );
                }
                Tile::Misflagged => {
                    stroke.move_to(at(0.15, 0.15));
                    stroke.line_to(at(0.85, 0.85));
                    stroke.move_to(at(0.15, 0.85));
                    stroke.line_to(at(0.85, 0.15));
                }
                _ => return,
            }
            if let Ok(path) = stroke.build() {
                window.paint_path(path, rgb(ink));
            }
        },
    )
    .size_full()
}

fn format_time(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
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
