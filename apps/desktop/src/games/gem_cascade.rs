mod art;
mod run;
mod well;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use corner_table_gem_cascade::{
    COLUMNS, Clear, Fall, GROUP_SIZE, Gem, PAIRS_PER_LEVEL, PlacedGem, ROWS, Wave,
};
use gpui::{
    Bounds, ClickEvent, Context, Div, FocusHandle, FontWeight, KeyDownEvent, MouseButton, Render,
    Role, SharedString, Stateful, Subscription, Task, Toggled, Window, canvas, div, point,
    prelude::*, px, rgb, rgba, size,
};

use crate::{
    audio::{self, Effect},
    games::{GameEntry, GameLaunch},
    storage::{Outcome, ResultMetric, Session},
    theme::{self as t, Palette},
};

use self::{
    art::{Artwork, name},
    run::{Mode, Record, Run, SAVE_VERSION, SavedSession},
    well::{ClearFrame, Field, GAP, INSET as WELL_INSET, Settling},
};
use run::Move;

const WAVE_TIME: Duration = Duration::from_millis(260);
const FALL_TIME: Duration = Duration::from_millis(85);
const IMPACT_TIME: Duration = Duration::from_millis(125);
const MAX_PANEL_WIDTH: f32 = 780.;
const MIN_CONSOLE_WIDTH: f32 = 238.;

pub struct GameView {
    session: Session,
    runs: [Run; 2],
    mode: Mode,
    phase: Phase,
    challenge: bool,
    timer: Option<Task<()>>,
    motion: Option<Motion>,
    cascade: Option<Cascade>,
    scope: FocusHandle,
    focus: FocusHandle,
    window_observers: Vec<Subscription>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Playing,
    Paused,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Play(Move),
    Toggle,
    Pause,
    Retry,
    NewDeal,
    Select(Mode),
}

enum MoveSource {
    Input,
    KeyRepeat,
    Timer,
}

#[derive(Clone, Copy)]
enum Motion {
    Falling(Instant),
    Impact(Instant, [PlacedGem; 2]),
}

struct Cascade {
    started: Instant,
    waves: Vec<PaintedWave>,
    previous_best: u32,
}

struct PaintedWave {
    wave: Wave,
    settling: Settling,
}

#[derive(Clone, Copy)]
struct WaveFrame<'a> {
    current: &'a PaintedWave,
    number: usize,
    progress: f32,
    points_pending: u32,
    previous_best: u32,
}

impl Cascade {
    fn frame(&self, now: Instant) -> WaveFrame<'_> {
        let elapsed = now.saturating_duration_since(self.started).as_secs_f32();
        let stage = elapsed / WAVE_TIME.as_secs_f32();
        let index = (stage as usize).min(self.waves.len() - 1);
        WaveFrame {
            current: &self.waves[index],
            number: index + 1,
            progress: (stage - index as f32).min(1.),
            points_pending: self.waves[index + 1..]
                .iter()
                .map(|wave| wave.wave.points)
                .sum(),
            previous_best: self.previous_best,
        }
    }
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let session = Session::new(launch);
        let saved = session
            .load::<SavedSession<Record>>()
            .filter(|save| save.version == SAVE_VERSION);
        let (mode, runs) = if let Some(saved) = saved {
            (saved.mode, saved.runs.map(Run::restore))
        } else {
            let seed = match &launch.entry {
                GameEntry::Challenge(id) => id.bytes().fold(0xcbf29ce484222325, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
                }),
                _ => fresh_seed(),
            };
            (Mode::default(), [Run::new(seed), Run::new(seed)])
        };
        let phase = open_phase(mode, &runs[mode.index()]);
        Self {
            session,
            runs,
            mode,
            phase,
            challenge: matches!(launch.entry, GameEntry::Challenge(_)),
            timer: None,
            motion: None,
            cascade: None,
            scope: cx.focus_handle(),
            focus: cx.focus_handle().tab_stop(true),
            window_observers: Vec::new(),
        }
    }

    fn run(&self) -> &Run {
        &self.runs[self.mode.index()]
    }

    fn save(&self) {
        self.session.save(&SavedSession {
            version: SAVE_VERSION,
            mode: self.mode,
            runs: self.runs.each_ref().map(|run| &run.record),
        });
    }

    fn playable(&self) -> bool {
        self.phase == Phase::Playing && self.cascade.is_none() && !self.run().game.is_over()
    }

    fn reset_clock(&mut self, window: &Window, cx: &Context<Self>) {
        self.timer = None;
        if self.phase != Phase::Playing {
            return;
        }
        let delay = if let Some(cascade) = &self.cascade {
            (WAVE_TIME * cascade.waves.len() as u32).saturating_sub(cascade.started.elapsed())
        } else if self.mode == Mode::Timed && !self.run().game.is_over() {
            self.run().game.fall_interval()
        } else {
            return;
        };

        let executor = cx.background_executor().clone();
        self.timer = Some(cx.spawn_in(window, async move |view, cx| {
            executor.timer(delay).await;
            let _ = view.update_in(cx, |view, window, cx| view.clock_tick(window, cx));
        }));
    }

    fn clock_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.timer = None;
        if self.cascade.take().is_some() {
            self.reset_clock(window, cx);
            cx.notify();
        } else if self.mode == Mode::Timed && self.playable() {
            self.play(Move::Fall, MoveSource::Timer, window, cx);
        }
    }

    fn pause(&mut self, cx: &mut Context<Self>) {
        if self.phase != Phase::Playing {
            return;
        }
        self.phase = Phase::Paused;
        self.timer = None;
        self.motion = None;
        self.cascade = None;
        self.save();
        cx.notify();
    }

    fn leave_play_area(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::Timed {
            self.pause(cx);
        } else if self.phase == Phase::Playing {
            self.save();
        }
    }

    fn enabled(&self, action: Action) -> bool {
        match action {
            Action::Play(Move::Fall) if self.mode == Mode::Practice => false,
            Action::Play(_) => self.playable(),
            Action::Toggle => !self.run().game.is_over(),
            Action::Pause => {
                self.phase == Phase::Playing
                    && (self.cascade.is_some() || !self.run().game.is_over())
            }
            Action::Retry => self.run().has_moves(),
            Action::NewDeal => !self.challenge,
            Action::Select(_) => true,
        }
    }

    fn perform(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled(action) {
            return;
        }
        match action {
            Action::Play(movement) => self.play(movement, MoveSource::Input, window, cx),
            Action::Toggle if self.phase == Phase::Playing => self.pause(cx),
            Action::Toggle => {
                self.phase = Phase::Playing;
                self.save();
                self.reset_clock(window, cx);
                cx.notify();
            }
            Action::Pause => self.pause(cx),
            Action::Retry | Action::NewDeal => {
                let seed = (action == Action::NewDeal).then(fresh_seed);
                let run = &mut self.runs[self.mode.index()];
                run.reset(seed);
                self.phase = open_phase(self.mode, run);
                self.timer = None;
                self.cascade = None;
                self.motion = None;
                self.save();
                cx.notify();
            }
            Action::Select(mode) if self.mode != mode => {
                self.mode = mode;
                self.phase = open_phase(mode, self.run());
                self.timer = None;
                self.cascade = None;
                self.motion = None;
                self.save();
                cx.notify();
            }
            Action::Select(_) => {}
        }
    }

    fn play(
        &mut self,
        movement: Move,
        source: MoveSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let run = &mut self.runs[self.mode.index()];
        let landing = run.game.landing();
        let previous_best = run.best();
        let Some(result) = run.play(movement) else {
            if matches!(source, MoveSource::Input) {
                audio::play(cx, Effect::InvalidMove);
            }
            return;
        };
        let effect = match result {
            Fall::Moved => {
                if movement == Move::Fall {
                    if !cx.reduce_motion() {
                        self.motion = Some(Motion::Falling(Instant::now()));
                    }
                    self.reset_clock(window, cx);
                } else {
                    self.motion = None;
                }
                // Holding a key can cross the well in a burst of small steps.
                matches!(source, MoveSource::Input).then_some(Effect::Move)
            }
            Fall::Locked(result) => {
                let waves = result.waves.len();
                if run.game.is_over() {
                    self.session.record_result(
                        run.record.attempt,
                        self.mode.label(),
                        Outcome::Finished,
                        ResultMetric::Points(run.game.score().into()),
                    );
                }
                if !cx.reduce_motion() {
                    let started = Instant::now();
                    if waves > 0 {
                        self.cascade = Some(Cascade {
                            started,
                            waves: result
                                .waves
                                .into_iter()
                                .map(|wave| PaintedWave {
                                    settling: Settling::new(&wave),
                                    wave,
                                })
                                .collect(),
                            previous_best,
                        });
                        self.motion = None;
                    } else {
                        self.motion = landing.map(|cells| Motion::Impact(started, cells));
                    }
                }
                self.save();
                self.reset_clock(window, cx);
                if waves > 0 {
                    Some(Effect::gems(waves))
                } else {
                    (!matches!(source, MoveSource::Timer)).then_some(Effect::Move)
                }
            }
        };
        if let Some(effect) = effect {
            audio::play(cx, effect);
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        if stroke.modifiers.control || stroke.modifiers.platform || stroke.modifiers.alt {
            return;
        }
        let game_key = self.focus.is_focused(window);
        let action = match stroke.key.as_str() {
            "left" => Action::Play(Move::Left),
            "right" => Action::Play(Move::Right),
            "up" => Action::Play(Move::TurnRight),
            "z" => Action::Play(Move::TurnLeft),
            "down" => Action::Play(Move::Fall),
            "p" => Action::Toggle,
            "escape" if self.playable() || self.cascade.is_some() => Action::Pause,
            "r" => Action::Retry,
            "space" | "enter" if game_key && self.cascade.is_none() => {
                if self.run().game.is_over() {
                    Action::Retry
                } else if self.phase != Phase::Playing {
                    Action::Toggle
                } else {
                    Action::Play(Move::Drop)
                }
            }
            _ => return,
        };
        cx.stop_propagation();
        if !event.is_held {
            self.perform(action, window, cx);
        } else if let Action::Play(movement @ (Move::Left | Move::Right | Move::Fall)) = action
            && self.enabled(action)
        {
            self.play(movement, MoveSource::KeyRepeat, window, cx);
        }
    }

    fn observe_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.window_observers.is_empty() {
            return;
        }
        self.window_observers = vec![
            cx.on_focus_out(&self.scope, window, |view, _, _, cx| {
                view.leave_play_area(cx);
            }),
            cx.observe_window_activation(window, |view, window, cx| {
                if !window.is_window_active() {
                    view.leave_play_area(cx);
                }
            }),
        ];
        let focus = self.focus.clone();
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
    }

    fn well(
        &self,
        size: f32,
        wave: Option<WaveFrame<'_>>,
        now: Instant,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let game = &self.run().game;
        let board = wave.map_or(game.board(), |frame| &frame.current.wave.board);
        let active = wave.is_none().then(|| game.active()).flatten();
        let landing = (wave.is_none() && self.phase == Phase::Playing)
            .then(|| game.landing())
            .flatten();
        let impact = match self.motion {
            Some(Motion::Impact(started, cells)) => {
                let progress = (now - started).as_secs_f32() / IMPACT_TIME.as_secs_f32();
                Some((cells, (1. - progress.clamp(0., 1.)).powi(3)))
            }
            _ => None,
        };
        let fall_offset = match self.motion {
            Some(Motion::Falling(started)) => {
                let progress = (now - started).as_secs_f32() / FALL_TIME.as_secs_f32();
                -(size + GAP) * (1. - progress.min(1.)).powi(3)
            }
            _ => 0.,
        };
        let art = Artwork::shared(cx);
        let field = Field {
            board: *board,
            active,
            landing,
            clear: wave.map(|frame| ClearFrame {
                settling: frame.current.settling,
                progress: frame.progress,
            }),
            fall_offset,
            impact,
        };
        let grid = div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .children(board.iter().enumerate().map(|(row, cells)| {
                div()
                    .id(("gem-row", row))
                    .role(Role::Row)
                    .aria_row_index(row)
                    .flex()
                    .gap(px(GAP))
                    .children(cells.iter().enumerate().map(|(column, &settled)| {
                        let label: SharedString = if let Some(gem) = gem_at(active, row, column) {
                            format!("{} falling", name(gem)).into()
                        } else if let Some(gem) = settled {
                            name(gem).into()
                        } else if let Some(gem) = gem_at(landing, row, column) {
                            format!("{} landing outline", name(gem)).into()
                        } else {
                            "Empty".into()
                        };
                        div()
                            .id(("gem-cell", column))
                            .role(Role::GridCell)
                            .aria_column_index(column)
                            .aria_label(label)
                            .size(px(size))
                            .flex_none()
                    }))
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        field.paint(art, bounds, size, window);
                    },
                )
                .absolute()
                .inset_0(),
            );

        div()
            .id("gem-cascade-board")
            .role(Role::Grid)
            .aria_label("Gem Cascade board")
            .aria_row_count(ROWS)
            .aria_column_count(COLUMNS)
            .aria_description("Left and Right move. Up and Z rotate. Down lowers the pair in Timed. Space drops. P pauses or resumes.")
            .track_focus(&self.focus)
            .on_mouse_down(MouseButton::Left, cx.listener(|view, _, window, cx| view.focus.focus(window, cx)))
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .overflow_hidden()
            .p(px(WELL_INSET))
            .rounded(px(10.))
            .border_2()
            .border_color(rgb(well::RIM))
            .focus(|style| style.border_color(rgb(0xd9ad74)))
            .bg(rgb(well::DARK_METAL))
            .child(grid)
            .children(self.overlay(cx))
    }

    fn overlay(&self, cx: &Context<Self>) -> Option<Div> {
        let palette = t::colors(cx);
        let (title, detail, label, action) = if self.cascade.is_some() {
            return None;
        } else if self.run().game.is_over() {
            (
                "Game over",
                Some("The entrance is blocked."),
                "Retry deal",
                Action::Retry,
            )
        } else {
            match self.phase {
                Phase::Ready => (
                    "Ready?",
                    Some("Join 4 gems of the same kind along their edges."),
                    "Start timed run",
                    Action::Toggle,
                ),
                Phase::Paused => ("Paused", None, "Resume", Action::Toggle),
                Phase::Playing => return None,
            }
        };
        let offer_deal = self.run().game.is_over() && !self.challenge;
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(0x14141cb8))
                .p_3()
                .child(
                    t::panel(cx)
                        .w_full()
                        .max_w(px(232.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .text_center()
                        .px_3()
                        .py_4()
                        .child(t::display_type(title, 23.))
                        .when_some(detail, |overlay, detail| {
                            overlay.child(
                                div()
                                    .line_height(px(17.))
                                    .text_size(px(13.))
                                    .text_color(rgb(palette.muted))
                                    .child(detail),
                            )
                        })
                        .child(
                            self.button("gem-overlay-primary", label, action, true, cx)
                                .mt_1()
                                .w_full(),
                        )
                        .when(offer_deal, |overlay| {
                            overlay.child(
                                self.button("gem-new-deal", "New deal", Action::NewDeal, false, cx)
                                    .w_full(),
                            )
                        }),
                ),
        )
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        action: Action,
        primary: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let palette = t::colors(cx);
        let enabled = self.enabled(action);
        let accent = primary && enabled;
        let selected = matches!(action, Action::Select(mode) if mode == self.mode);
        let background = if accent {
            t::GOLD
        } else if selected {
            palette.hover
        } else {
            palette.surface
        };
        let stroke = if accent {
            t::GOLD
        } else if selected {
            palette.gold_text
        } else {
            palette.border
        };
        let shortcut = shortcut(action);
        let button = div()
            .id(id)
            .role(Role::Button)
            .accessibility_id(id)
            .aria_label(label)
            .tab_index(0)
            .tab_stop(enabled)
            .when(matches!(action, Action::Select(_)), |button| {
                button.aria_toggled(if selected {
                    Toggled::True
                } else {
                    Toggled::False
                })
            })
            .flex()
            .flex_1()
            .items_center()
            .justify_between()
            .gap_1()
            .min_h(px(34.))
            .px_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(stroke))
            .bg(rgb(background))
            .text_color(rgb(if accent {
                t::INK
            } else if enabled {
                palette.ink
            } else {
                palette.muted
            }))
            .text_size(px(13.))
            .line_height(px(17.))
            .font_weight(FontWeight::SEMIBOLD)
            .focus(move |style| style.border_color(rgb(palette.ink)).border_2())
            .child(label)
            .when_some(shortcut, |button, (visible, spoken)| {
                button.aria_keyshortcuts(spoken).child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(if accent { t::INK } else { palette.muted }))
                        .child(visible),
                )
            });
        if !enabled {
            return button.aria_description("Unavailable");
        }
        button
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(if accent { t::GOLD_HOVER } else { palette.hover })))
            .active(move |style| style.bg(rgb(if accent { 0xcd9634 } else { palette.paper })))
            .on_click(cx.listener(move |view, event: &ClickEvent, window, cx| {
                if matches!(event, ClickEvent::Mouse(_)) || !matches!(action, Action::Play(_)) {
                    view.focus.focus(window, cx);
                }
                view.perform(action, window, cx);
            }))
    }

    fn console(&self, wave: Option<WaveFrame<'_>>, compact: bool, cx: &Context<Self>) -> Div {
        let p = t::colors(cx);
        let run = self.run();
        let clear = if let Some(frame) = wave {
            Clear {
                gems: frame.current.wave.cleared.len(),
                chain: frame.number as u32,
                points: frame.current.wave.points,
            }
        } else {
            run.last_scoring
        };
        let row = |left, right| div().flex().w_full().gap_2().child(left).child(right);
        let move_button = |id, label, movement| {
            self.button(
                id,
                label,
                Action::Play(movement),
                movement == Move::Drop,
                cx,
            )
        };
        let group = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(row(
                move_button("gem-left", "Move left", Move::Left),
                move_button("gem-right", "Move right", Move::Right),
            ))
            .child(row(
                move_button("gem-turn-left", "Rotate left", Move::TurnLeft),
                move_button("gem-turn-right", "Rotate right", Move::TurnRight),
            ))
            .child(if self.mode == Mode::Timed {
                row(
                    move_button("gem-lower", "Lower", Move::Fall),
                    move_button("gem-drop", "Drop", Move::Drop),
                )
            } else {
                div()
                    .flex()
                    .child(move_button("gem-drop", "Drop", Move::Drop))
            })
            .child(row(
                self.button("gem-pause", "Pause", Action::Pause, false, cx),
                self.button("gem-retry", "Retry", Action::Retry, false, cx),
            ));

        div()
            .flex()
            .flex_1()
            .min_w_0()
            .flex_col()
            .gap(px(if compact { 10. } else { 12. }))
            .child(row(
                self.button("gem-timed", Mode::Timed.label(), Action::Select(Mode::Timed), false, cx),
                self.button("gem-practice", Mode::Practice.label(), Action::Select(Mode::Practice), false, cx),
            ))
            .child(self.scoreboard(wave, p, cx))
            .child(self.pace(p))
            .child(clear_status(clear, wave.is_some(), p))
            .child(group)
            .child(
                div().text_size(px(12.)).line_height(px(17.)).text_color(rgb(p.muted))
                    .child(format!("Match {GROUP_SIZE} edge to edge. Each link in a chain multiplies the points it earns.")),
            )
    }

    fn scoreboard(&self, wave: Option<WaveFrame<'_>>, p: Palette, cx: &Context<Self>) -> Div {
        let run = self.run();
        let game = &run.game;
        let [pivot, satellite] = if wave.is_some() {
            game.active()
                .map_or_else(|| game.next_pair(), |pair| pair.map(|cell| cell.gem))
        } else {
            game.next_pair()
        };
        let score = game
            .score()
            .saturating_sub(wave.map_or(0, |frame| frame.points_pending));
        let best = wave.map_or(run.best(), |frame| frame.previous_best.max(score));
        let score = div()
            .id("gem-score")
            .role(Role::Group)
            .aria_label(format!("Score {score}. Best {best}."))
            .flex()
            .flex_col()
            .items_start()
            .child(caption("Score", p))
            .child(t::display_type(score.to_string(), 36.))
            .child(
                div()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(rgb(p.muted))
                    .child(format!("Best {best}")),
            );
        let preview_label = if game.is_over() {
            "No next pair".to_owned()
        } else {
            format!("Next pair. {} above {}", name(satellite), name(pivot))
        };
        let preview = div()
            .id("gem-preview")
            .role(Role::Group)
            .aria_label(preview_label)
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .child(caption("Next pair", p))
            .child(
                div()
                    .relative()
                    .w(px(44.))
                    .h(px(62.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(rgb(well::RIM))
                    .bg(rgb(well::DARK_METAL))
                    .opacity(if game.is_over() { 0.45 } else { 1. })
                    .child(pair_preview(satellite, pivot, Artwork::shared(cx))),
            );
        div()
            .flex()
            .justify_between()
            .items_center()
            .gap_2()
            .child(score)
            .child(preview)
    }

    fn pace(&self, p: Palette) -> Div {
        let game = &self.run().game;
        let pairs = game.pairs() % PAIRS_PER_LEVEL;
        let pace = div()
            .flex()
            .flex_col()
            .gap_1()
            .border_t_1()
            .border_color(rgb(p.border))
            .pt_2();
        match self.mode {
            Mode::Timed => pace
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(caption(format!("Level {}", game.level()), p))
                        .child(caption(
                            format!("{:.2} s / row", game.fall_interval().as_secs_f32()),
                            p,
                        )),
                )
                .child(
                    div()
                        .id("gem-level-progress")
                        .role(Role::ProgressIndicator)
                        .aria_label("Pairs placed toward the next level")
                        .aria_numeric_value(f64::from(pairs))
                        .aria_min_numeric_value(0.)
                        .aria_max_numeric_value(f64::from(PAIRS_PER_LEVEL))
                        .flex()
                        .gap(px(2.))
                        .children((0..PAIRS_PER_LEVEL).map(|step| {
                            div()
                                .flex_1()
                                .h(px(3.))
                                .rounded_sm()
                                .bg(rgb(if step < pairs { p.gold_text } else { p.border }))
                        })),
                ),
            Mode::Practice => pace.child(
                div()
                    .flex()
                    .justify_between()
                    .child(caption("Drop at your pace", p))
                    .child(caption(format!("{} pairs placed", game.pairs()), p)),
            ),
        }
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observe_window(window, cx);
        let now = Instant::now();
        let motion_time = match self.motion {
            Some(Motion::Falling(started)) => Some((started, FALL_TIME)),
            Some(Motion::Impact(started, _)) => Some((started, IMPACT_TIME)),
            None => None,
        };
        let reduce_motion = cx.reduce_motion();
        if reduce_motion || motion_time.is_some_and(|(started, duration)| now - started >= duration)
        {
            self.motion = None;
        }
        let wave = self
            .cascade
            .as_ref()
            .filter(|_| !reduce_motion)
            .map(|cascade| cascade.frame(now));
        if wave.is_some() || self.motion.is_some() {
            window.request_animation_frame();
        }

        let viewport = window.viewport_size();
        let compact = viewport.width < px(930.);
        let inset = if compact { 12. } else { 20. };
        let panel_gap = if compact { 16. } else { 24. };
        let padding = 2. * (inset + WELL_INSET + 3.);
        // The parent play page supplies the title, scroll padding and navigation rail.
        let board_height = f32::from(viewport.height) - 90. - padding;
        let panel_width = (super::board_space(window) + 36.).min(MAX_PANEL_WIDTH);
        let board_width = panel_width - padding - panel_gap - MIN_CONSOLE_WIDTH;
        let fit = |space, count: usize| (space - GAP * (count - 1) as f32) / count as f32;
        let size = fit(board_height, ROWS)
            .min(fit(board_width, COLUMNS))
            .floor()
            .clamp(26., 54.);

        t::panel(cx)
            .id("gem-cascade")
            .track_focus(&self.scope)
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .max_w(px(MAX_PANEL_WIDTH))
            .flex()
            .items_start()
            .gap(px(panel_gap))
            .p(px(inset))
            .child(self.well(size, wave, now, cx))
            .child(self.console(wave, compact, cx))
    }
}

fn open_phase(mode: Mode, run: &Run) -> Phase {
    match mode {
        Mode::Practice => Phase::Playing,
        Mode::Timed if run.has_moves() => Phase::Paused,
        Mode::Timed => Phase::Ready,
    }
}

fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos() as u64)
}

fn gem_at(cells: Option<[PlacedGem; 2]>, row: usize, column: usize) -> Option<Gem> {
    cells?
        .into_iter()
        .find(|cell| cell.row == row && cell.column == column)
        .map(|cell| cell.gem)
}

fn pair_preview(top: Gem, bottom: Gem, art: &'static Artwork) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let stone_size = size(px(25.), px(25.));
            let center = bounds.center();
            let top_at = Bounds::new(center - point(px(12.5), px(25.5)), stone_size);
            let bottom_at = Bounds::new(center + point(px(-12.5), px(0.5)), stone_size);
            well::paint_pair(art, [(top, top_at), (bottom, bottom_at)], window);
        },
    )
    .absolute()
    .inset_0()
}

fn caption(text: impl Into<gpui::SharedString>, p: Palette) -> Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(12.))
        .line_height(px(17.))
        .text_color(rgb(p.muted))
        .child(text.into())
}

fn clear_status(clear: Clear, live: bool, p: Palette) -> Stateful<Div> {
    let message = match clear.chain {
        0 => "Outlines show the landing".to_owned(),
        1 => format!("{} gems cleared", clear.gems),
        chain => format!("Chain ×{chain} · {} gems", clear.gems),
    };
    let spoken = if clear.points > 0 {
        format!("{message}. {} points.", clear.points)
    } else {
        message.clone()
    };
    div()
        .id("gem-clear-status")
        .role(Role::Status)
        .aria_label(spoken)
        .flex()
        .justify_between()
        .items_center()
        .min_h(px(30.))
        .gap_2()
        .px_2()
        .rounded(px(5.))
        .bg(rgb(if live { p.hover } else { p.paper }))
        .text_size(px(13.))
        .line_height(px(17.))
        .text_color(rgb(if clear.chain > 0 { p.ink } else { p.muted }))
        .child(
            div()
                .when(clear.chain > 1, |label| label.font_weight(FontWeight::BOLD))
                .child(message),
        )
        .when(clear.points > 0, |status| {
            status.child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(p.gold_text))
                    .child(format!("+{}", clear.points)),
            )
        })
}

fn shortcut(action: Action) -> Option<(&'static str, &'static str)> {
    match action {
        Action::Play(Move::Left) => Some(("←", "ArrowLeft")),
        Action::Play(Move::Right) => Some(("→", "ArrowRight")),
        Action::Play(Move::TurnLeft) => Some(("Z", "Z")),
        Action::Play(Move::TurnRight) => Some(("↑", "ArrowUp")),
        Action::Play(Move::Fall) => Some(("↓", "ArrowDown")),
        Action::Play(Move::Drop) => Some(("Space", "Space")),
        Action::Toggle | Action::Pause => Some(("P", "P")),
        Action::Retry => Some(("R", "R")),
        Action::Select(_) | Action::NewDeal => None,
    }
}
