mod progress;
mod table;
#[cfg(all(test, debug_assertions))]
mod tests;

use std::cell::Cell;
use std::rc::Rc;

use corner_table_bubble_shooter::{
    Aim, Angle, Cell as BubbleCell, Color, PUZZLE_COUNT, PUZZLES, STAGE_COUNT, STAGES, Status,
};
use gpui::{
    Context, Div, FocusHandle, FontWeight, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, Render, Role, SharedString, Stateful, Toggled, Window, canvas, div, prelude::*,
    px, relative, rgb, rgba,
};

use crate::{
    audio::{self, Effect},
    theme as t,
};

use self::progress::{Mode, Progress};
use self::table::{Flight, Impact, Playfield, TABLE_HEIGHT, TABLE_WIDTH};
use super::{GameLaunch, board_space};

pub struct GameView {
    progress: Progress,
    focus: FocusHandle,
    focus_on_open: bool,
    angle: Angle,
    aim: Option<Aim>,
    flight: Option<Flight>,
    impact: Option<Impact>,
}

#[derive(Clone, Copy)]
enum Action {
    Mode(Mode),
    Stage(usize),
    Puzzle(usize),
    Restart,
    NewBoard,
    Swap,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let progress = Progress::new(launch);
        let angle = Angle::default();
        let aim = progress.round.aim_at(angle);
        Self {
            progress,
            focus: cx.focus_handle().tab_stop(true),
            focus_on_open: true,
            angle,
            aim,
            flight: None,
            impact: None,
        }
    }

    fn next_level(&self) -> Option<(Action, &'static str)> {
        let progress = &self.progress;
        if progress.round.status() != Status::Won {
            return None;
        }
        match progress.mode() {
            Mode::Stages => {
                let next = progress.stage() + 1;
                progress
                    .stage_unlocked(next)
                    .then_some((Action::Stage(next), "Next stage"))
            }
            Mode::Puzzles => {
                let next = progress.puzzle() + 1;
                (next < PUZZLE_COUNT).then_some((Action::Puzzle(next), "Next puzzle"))
            }
            _ => None,
        }
    }

    fn has_next_ammo(&self) -> bool {
        let round = &self.progress.round;
        round.puzzle_info().is_none() || round.shots_left() > 1
    }

    fn perform(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::Mode(mode) if self.progress.mode() != mode => self.progress.set_mode(mode),
            Action::Stage(index) if self.progress.stage() != index => {
                self.progress.select_stage(index);
            }
            Action::Puzzle(index) if self.progress.puzzle() != index => {
                self.progress.select_puzzle(index);
            }
            Action::Restart => self.progress.restart(),
            Action::NewBoard => self.progress.new_board(),
            Action::Swap => {
                if self.flight.is_some() || self.progress.round.status() != Status::Playing {
                    return;
                }
                if self.progress.swap() {
                    self.refresh_aim();
                    self.focus.focus(window, cx);
                    cx.notify();
                    audio::play(cx, Effect::Move);
                } else {
                    audio::play(cx, Effect::InvalidMove);
                }
                return;
            }
            Action::Mode(_) | Action::Stage(_) | Action::Puzzle(_) => return,
        }
        self.flight = None;
        self.impact = None;
        self.angle = Angle::default();
        self.refresh_aim();
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn set_angle(&mut self, angle: Angle, cx: &mut Context<Self>) {
        if self.angle != angle && self.flight.is_none() {
            self.angle = angle;
            self.refresh_aim();
            cx.notify();
        }
    }

    fn refresh_aim(&mut self) {
        self.aim = self.progress.round.aim_at(self.angle);
    }

    fn fire(&mut self, cx: &mut Context<Self>) {
        if self.flight.is_some() || self.progress.round.status() != Status::Playing {
            return;
        }
        let Some(aim) = self.aim.take() else {
            audio::play(cx, Effect::InvalidMove);
            return;
        };
        self.impact = None;
        if cx.reduce_motion() {
            self.resolve_shot(self.angle, None, cx);
        } else {
            self.flight = Some(Flight::new(aim, self.angle));
        }
        cx.notify();
    }

    fn resolve_shot(&mut self, angle: Angle, landing: Option<BubbleCell>, cx: &Context<Self>) {
        if let Some(shot) = self.progress.fire(angle) {
            let popped = !shot.popped.is_empty();
            self.impact = landing.map(|landing| Impact::new(landing, shot));
            let effect = if self.progress.round.status() == Status::Won {
                Effect::Complete
            } else if popped {
                Effect::Clear
            } else {
                Effect::Move
            };
            audio::play(cx, effect);
        }
        self.refresh_aim();
    }

    fn animate(&mut self, cx: &mut Context<Self>) {
        if self
            .flight
            .as_ref()
            .is_some_and(|flight| flight.position(0.).is_none())
            && let Some(flight) = self.flight.take()
        {
            self.resolve_shot(
                flight.angle,
                (!cx.reduce_motion()).then_some(flight.aim.landing),
                cx,
            );
        }
        if self.impact.as_ref().is_some_and(Impact::finished) {
            self.impact = None;
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if !self.focus.is_focused(window)
            || key.modifiers.alt
            || key.modifiers.control
            || key.modifiers.platform
        {
            return;
        }
        let step = if key.modifiers.shift { 2 } else { 16 };
        match key.key.as_str() {
            "left" => self.set_angle(self.angle.adjusted(-step), cx),
            "right" => self.set_angle(self.angle.adjusted(step), cx),
            "up" | "home" => self.set_angle(Angle::default(), cx),
            "space" if !event.is_held => self.fire(cx),
            "x" if !event.is_held => self.perform(Action::Swap, window, cx),
            "r" if !event.is_held => self.perform(Action::Restart, window, cx),
            "enter" if !event.is_held && self.progress.round.status() != Status::Playing => {
                let action = self
                    .next_level()
                    .map_or(Action::Restart, |(action, _)| action);
                self.perform(action, window, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn board(&self, width: f32, height: f32, cx: &Context<Self>) -> Stateful<Div> {
        let field = Rc::new(Cell::new(None::<Playfield>));
        let hover_field = Rc::clone(&field);
        let click_field = Rc::clone(&field);
        let view = cx.entity();
        let status = self.progress.round.status();
        div()
            .id("bubble-shooter-board")
            .debug_selector(|| "bubble-shooter-board".into())
            .accessibility_id("bubble-shooter-board")
            .role(Role::Group)
            .aria_label("Playfield")
            .aria_description(match (status, self.has_next_ammo()) {
                (Status::Playing, true) => "Left and Right aim. Shift adjusts more finely. Up centers. Space shoots, X swaps, and R restarts.",
                (Status::Playing, false) => "Left and Right aim. Shift adjusts more finely. Up centers. Space shoots, and R restarts.",
                _ => "The round has ended. Enter continues, and R restarts.",
            })
            .aria_value(self.aim_description())
            .track_focus(&self.focus)
            .tab_index(0)
            .on_key_down(cx.listener(Self::key_down))
            .focus_visible(|style| style.border_color(rgb(t::GOLD)))
            .border_2()
            .border_color(rgba(0x00000000))
            .rounded_xl()
            .relative()
            .w(px(width))
            .h(px(height))
            .flex_none()
            .when(status == Status::Playing, |board| {
                board
                    .when(self.flight.is_none(), |board| board.cursor_crosshair())
                    .on_mouse_move(cx.listener(move |view, event: &MouseMoveEvent, _, cx| {
                        if let Some(angle) = hover_field.get().and_then(|field| field.angle_at(event.position)) {
                            view.set_angle(angle, cx);
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                            view.focus.focus(window, cx);
                            if let Some(angle) = click_field.get().and_then(|field| field.angle_at(event.position)) {
                                view.set_angle(angle, cx);
                                view.fire(cx);
                            }
                        }),
                    )
            })
            .child(
                canvas(
                    move |bounds, _, _| {
                        let fitted = Playfield::fit(bounds);
                        field.set(Some(fitted));
                        fitted
                    },
                    move |_, field, window, cx| {
                        let view = view.read(cx);
                        let progress = &view.progress;
                        field.paint(
                            &progress.round,
                            view.angle,
                            view.aim.as_ref(),
                            view.flight.as_ref(),
                            view.impact.as_ref().zip(progress.last_shot.as_ref()),
                            window,
                        );
                    },
                )
                .size_full(),
            )
            .when(status == Status::Won && self.impact.is_none(), |board| {
                board.child(self.completion(cx))
            })
    }

    fn aim_description(&self) -> String {
        if self.flight.is_some() {
            return "Shot in flight".into();
        }
        let round = &self.progress.round;
        if round.status() != Status::Playing {
            return format!("{} points", round.score());
        }
        let color = table::color_name(round.current());
        match self.aim.as_ref() {
            Some(aim) => {
                let mut description = format!(
                    "{color}. Row {}, column {}.",
                    aim.landing.row() + 1,
                    aim.landing.column() + 1
                );
                if aim.matching >= 3 {
                    description.push_str(&format!(" Matches {}.", aim.matching));
                }
                if aim.bounces() > 0 {
                    description.push_str(&format!(" {} wall bounces.", aim.bounces()));
                }
                description
            }
            None => format!("{color}. Try a different angle."),
        }
    }

    fn completion(&self, cx: &Context<Self>) -> Div {
        let round = &self.progress.round;
        let mode = self.progress.mode();
        let next = self.next_level();
        let title = match mode {
            Mode::Stages if next.is_none() => "All stages cleared",
            Mode::Stages => "Stage cleared",
            Mode::Classic => "Board cleared",
            Mode::Challenge => "Challenge cleared",
            Mode::Puzzles => "Puzzle solved",
        };
        let replay = if mode == Mode::Puzzles {
            "Replay puzzle"
        } else {
            "Play again"
        };
        let (action, button_label) = next.unwrap_or((Action::Restart, replay));
        let shots = round.shots_used();
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .px_4()
            .text_color(rgb(0xfff2d6))
            .child(t::display_type(title, 25.).text_center())
            .when(round.stars() > 0, |summary| {
                summary.child(star_row(round.stars(), 24., 0xecc064, 0x59716a))
            })
            .child(div().text_size(px(14.)).text_center().child(format!(
                "{} points · {shots} {}",
                round.score(),
                if shots == 1 { "shot" } else { "shots" },
            )))
            .when(self.progress.new_best, |summary| {
                summary.child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xffdc87))
                        .child("New personal best"),
                )
            })
            .child(
                self.button(
                    "bubble-next-or-replay",
                    button_label,
                    action,
                    ButtonStyle::Gold,
                    cx,
                )
                .aria_keyshortcuts("Enter"),
            )
    }

    fn rail(&self, width: f32, cx: &Context<Self>) -> Div {
        let p = t::colors(cx);
        let state = &self.progress;
        let round = &state.round;
        let stage = round.stage_info();
        let puzzle = round.puzzle_info();
        let status = round.status();
        let lost = matches!(
            status,
            Status::HitDangerLine | Status::OutOfShots | Status::BankGoalMissed
        );
        let title = puzzle
            .map(|puzzle| puzzle.name)
            .or_else(|| stage.map(|stage| stage.name))
            .unwrap_or("Classic");
        let compact = width < 220.;
        div()
            .flex_none()
            .w(px(width))
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(if compact { 12. } else { 16. }))
            .pt_1()
            .when(state.mode() != Mode::Challenge, |rail| {
                rail.child(self.mode_buttons(cx))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(t::display_type(title, if compact { 23. } else { 28. }))
                    .child(muted(p).child(match state.mode() {
                        Mode::Stages if compact => format!(
                            "{} shots for 3 stars",
                            stage.expect("stage mode has stage info").par,
                        ),
                        Mode::Stages => format!(
                            "Stage {} of {STAGE_COUNT} · {} shots for 3 stars",
                            state.stage() + 1,
                            stage.expect("stage mode has stage info").par,
                        ),
                        Mode::Challenge => format!(
                            "Challenge · {} shots for 3 stars",
                            stage.expect("challenge mode has stage info").par,
                        ),
                        Mode::Classic => "Clear the table".into(),
                        Mode::Puzzles => {
                            let shots = puzzle.expect("puzzle mode has puzzle info").shots();
                            format!(
                                "Clear in {shots} {}",
                                if shots == 1 { "shot" } else { "shots" }
                            )
                        }
                    })),
            )
            .when(state.mode() == Mode::Stages, |rail| {
                rail.child(self.stage_picker(cx))
            })
            .when(state.mode() == Mode::Puzzles, |rail| {
                rail.child(self.puzzle_picker(cx))
            })
            .child(self.scoreboard(cx))
            .when(status == Status::Playing, |rail| {
                rail.child(self.ammunition(cx))
            })
            .when(lost, |rail| rail.child(self.feedback(cx)))
            .child(self.round_actions(cx))
            .when(!lost, |rail| rail.child(self.feedback(cx)))
            .when(status == Status::Playing && !compact, |rail| {
                rail.child(
                    keyboard_help(p, self.has_next_ammo())
                        .border_t_1()
                        .border_color(rgb(p.border))
                        .pt_3(),
                )
            })
    }

    fn mode_buttons(&self, cx: &Context<Self>) -> Div {
        div().flex().gap(px(4.)).children(
            [
                (Mode::Stages, "bubble-mode-stages", "Stages"),
                (Mode::Classic, "bubble-mode-classic", "Classic"),
                (Mode::Puzzles, "bubble-mode-puzzles", "Puzzles"),
            ]
            .map(|(mode, id, label)| {
                let active = self.progress.mode() == mode;
                self.button(
                    id,
                    label,
                    Action::Mode(mode),
                    if active {
                        ButtonStyle::Selected
                    } else {
                        ButtonStyle::Plain
                    },
                    cx,
                )
                .aria_toggled(if active {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .px_1()
                .min_w_0()
                .flex_1()
            }),
        )
    }

    fn stage_picker(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .gap(px(4.))
            .children(STAGES.iter().enumerate().map(|(index, stage)| {
                let active = self.progress.stage() == index;
                let enabled = self.progress.stage_unlocked(index);
                let record = self.progress.stage_record(index);
                let label = match record {
                    Some(record) => format!(
                        "Stage {}, {}, {} stars",
                        index + 1,
                        stage.name,
                        record.stars.min(3)
                    ),
                    None if !enabled => format!("Stage {}, {}, locked", index + 1, stage.name),
                    None => format!("Stage {}, {}", index + 1, stage.name),
                };
                self.level_chip(
                    ("bubble-stage", index),
                    label,
                    enabled.then_some(Action::Stage(index)),
                    active,
                    record.is_some(),
                    cx,
                )
            }))
    }

    fn puzzle_picker(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .gap(px(4.))
            .children(PUZZLES.iter().enumerate().map(|(index, puzzle)| {
                let solved = self.progress.puzzle_solved(index);
                self.level_chip(
                    ("bubble-puzzle", index),
                    format!(
                        "Puzzle {}, {}{}",
                        index + 1,
                        puzzle.name,
                        if solved { ", solved" } else { "" },
                    ),
                    Some(Action::Puzzle(index)),
                    self.progress.puzzle() == index,
                    solved,
                    cx,
                )
            }))
    }

    fn level_chip(
        &self,
        id: (&'static str, usize),
        label: String,
        action: Option<Action>,
        active: bool,
        cleared: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let p = t::colors(cx);
        div()
            .id(id)
            .debug_selector(move || format!("{}-{}", id.0, id.1))
            .role(if action.is_some() {
                Role::Button
            } else {
                Role::Label
            })
            .aria_label(label)
            .aria_selected(active)
            .h(px(36.))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(2.))
            .rounded_md()
            .border_1()
            .border_color(rgb(if active { p.gold_text } else { p.border }))
            .bg(rgb(if active { p.hover } else { p.paper }))
            .text_size(px(13.))
            .line_height(px(15.))
            .text_color(rgb(if active { p.ink } else { p.muted }))
            .font_weight(if active {
                FontWeight::BOLD
            } else {
                FontWeight::SEMIBOLD
            })
            .when(action.is_none(), |chip| chip.opacity(0.65))
            .when_some(action, |chip, action| {
                chip.tab_index(0)
                    .cursor_pointer()
                    .hover(move |style| style.bg(rgb(p.hover)))
                    .focus_visible(move |style| style.border_color(rgb(p.ink)))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.perform(action, window, cx);
                    }))
            })
            .child((id.1 + 1).to_string())
            .child(
                div()
                    .h(px(3.))
                    .w(px(11.))
                    .rounded_full()
                    .when(cleared, |dot| dot.bg(rgb(p.gold_text))),
            )
    }

    fn scoreboard(&self, cx: &Context<Self>) -> Div {
        let p = t::colors(cx);
        let round = &self.progress.round;
        let left = round.shots_left();
        let initial = left + round.shots_used();
        let best = self.progress.best();
        let puzzle = round.puzzle_info();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .border_y_1()
            .border_color(rgb(p.border))
            .py_3()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(metric("Shots left", left.to_string(), p))
                    .child(if let Some(puzzle) = puzzle {
                        metric(
                            "Bank matches",
                            format!(
                                "{} / {}",
                                round.banked_matches().min(puzzle.bank_target),
                                puzzle.bank_target,
                            ),
                            p,
                        )
                    } else {
                        metric("Score", round.score().to_string(), p)
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(if initial > 20 { 2. } else { 3. }))
                    .children((0..initial).map(|index| {
                        div()
                            .flex_1()
                            .h(px(4.))
                            .rounded_full()
                            .bg(rgb(if index < left {
                                if puzzle.is_none() && left <= 3 {
                                    0xd78a64
                                } else {
                                    p.gold_text
                                }
                            } else {
                                p.border
                            }))
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .gap_x_3()
                    .child(muted(p).child(format!("{} bubbles left", round.bubbles().count())))
                    .when(puzzle.is_some(), |row| {
                        row.child(muted(p).child(format!("{} points", round.score())))
                    })
                    .children(
                        best.filter(|_| puzzle.is_none())
                            .map(|record| muted(p).child(format!("Best {}", record.score))),
                    ),
            )
    }

    fn ammunition(&self, cx: &Context<Self>) -> Div {
        let p = t::colors(cx);
        let round = &self.progress.round;
        div()
            .flex()
            .gap_2()
            .child(ammo("Loaded", round.current(), p))
            .when(self.has_next_ammo(), |rack| {
                rack.child(ammo("Next", round.next(), p))
            })
    }

    fn round_actions(&self, cx: &Context<Self>) -> Div {
        let round = &self.progress.round;
        let status = round.status();
        let can_swap = self.flight.is_none() && round.can_swap();
        let puzzle = self.progress.mode() == Mode::Puzzles;
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .when(
                status == Status::Playing && self.has_next_ammo(),
                |actions| {
                    actions.child(
                    self.button(
                        "bubble-swap",
                        "Swap",
                        Action::Swap,
                        if can_swap {
                            ButtonStyle::Plain
                        } else {
                            ButtonStyle::Disabled
                        },
                        cx,
                    )
                    .aria_label(if can_swap {
                        "Swap bubbles"
                    } else {
                        "Swap unavailable"
                    })
                    .aria_keyshortcuts("X")
                    .aria_description(
                        "Trade the loaded bubble for the next one once per shot when they differ.",
                    ),
                )
                },
            )
            .when(
                status != Status::Won || self.next_level().is_some(),
                |actions| {
                    let (label, style) = match (status, puzzle) {
                        (Status::Playing, _) => ("Restart", ButtonStyle::Plain),
                        (Status::Won, true) => ("Replay puzzle", ButtonStyle::Plain),
                        (Status::Won, false) => ("Replay stage", ButtonStyle::Plain),
                        (_, true) => ("Retry puzzle", ButtonStyle::Gold),
                        _ => ("Try again", ButtonStyle::Gold),
                    };
                    actions.child(
                        self.button("bubble-restart", label, Action::Restart, style, cx)
                            .when(puzzle, |button| {
                                button
                                    .aria_label(label)
                                    .aria_keyshortcuts("R")
                                    .gap_2()
                                    .child(div().text_size(px(11.)).opacity(0.8).child("R"))
                            }),
                    )
                },
            )
            .when(self.progress.mode() == Mode::Classic, |actions| {
                actions.child(self.button(
                    "bubble-new-board",
                    "New board",
                    Action::NewBoard,
                    ButtonStyle::Plain,
                    cx,
                ))
            })
    }

    fn feedback(&self, cx: &Context<Self>) -> Stateful<Div> {
        let p = t::colors(cx);
        let round = &self.progress.round;
        let shot = self.progress.last_shot.as_ref();
        let (title, detail) = match (round.status(), shot) {
            (Status::Won, Some(shot)) if shot.clear_bonus > 0 => (
                "Unused-shot bonus".into(),
                format!("+{} points", shot.clear_bonus),
            ),
            (Status::Won, _) => (
                "Last-shot clear".into(),
                "You used every shot in the rack.".into(),
            ),
            (Status::OutOfShots, _) => (
                "Out of shots".into(),
                "Clear the rack within the shot budget.".into(),
            ),
            (Status::BankGoalMissed, _) => (
                "Bank goal missed".into(),
                "A bank match bounces off a wall and pops a group.".into(),
            ),
            (Status::HitDangerLine, _) => (
                "Danger line reached".into(),
                "A bubble crossed the dashed line. Clear the low rows first.".into(),
            ),
            (Status::Playing, Some(shot)) if !shot.popped.is_empty() => {
                let mut title = format!("{} popped", shot.popped.len());
                if !shot.dropped.is_empty() {
                    title.push_str(&format!(" · {} dropped", shot.dropped.len()));
                }
                let mut detail = format!("+{} points", shot.points);
                if shot.bounces > 0 {
                    detail.push_str(" · Bank shot");
                }
                if shot.streak > 1 {
                    detail.push_str(&format!(" · {} in a row", shot.streak));
                }
                (title, detail)
            }
            (Status::Playing, Some(_)) => (
                "Bubble placed".into(),
                "Join three matching marks to pop the cluster.".into(),
            ),
            (Status::Playing, None) => (
                "Line up a shot".into(),
                if self.progress.mode() == Mode::Challenge {
                    "Score with pops, drops, bank shots, and unused shots.".into()
                } else if let Some(puzzle) = round.puzzle_info() {
                    puzzle.hint.into()
                } else {
                    round
                        .stage_info()
                        .map_or(
                            "Join three matching marks. Bank shots earn bonus points.",
                            |stage| stage.hint,
                        )
                        .into()
                },
            ),
        };
        div()
            .id("bubble-feedback")
            .role(Role::Status)
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(15.))
                    .line_height(px(19.))
                    .child(title),
            )
            .child(muted(p).child(detail))
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        action: Action,
        style: ButtonStyle,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let p = t::colors(cx);
        let (bg, border, ink, hover) = match style {
            ButtonStyle::Gold => (t::GOLD, t::GOLD, t::INK, t::GOLD_HOVER),
            ButtonStyle::Selected => (p.hover, p.gold_text, p.ink, p.hover),
            ButtonStyle::Plain => (p.surface, p.border, p.ink, p.hover),
            ButtonStyle::Disabled => (p.paper, p.border, p.muted, p.paper),
        };
        let enabled = style != ButtonStyle::Disabled;
        div()
            .id(id)
            .debug_selector(move || id.into())
            .role(if enabled { Role::Button } else { Role::Label })
            .flex()
            .items_center()
            .justify_center()
            .min_h(px(37.))
            .px_3()
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(bg))
            .text_color(rgb(ink))
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .when(enabled, |button| {
                button
                    .tab_index(0)
                    .cursor_pointer()
                    .hover(move |button| button.bg(rgb(hover)))
                    .focus_visible(move |button| {
                        button.border_color(rgb(if style == ButtonStyle::Gold {
                            t::INK
                        } else {
                            p.ink
                        }))
                    })
                    .on_click(
                        cx.listener(move |view, _, window, cx| view.perform(action, window, cx)),
                    )
            })
            .child(label)
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_on_open) {
            let focus = self.focus.clone();
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        if self.flight.is_some() || self.impact.is_some() {
            cx.on_next_frame(window, |view, _, cx| view.animate(cx));
        }
        let available = board_space(window);
        let compact = available < 580.;
        let rail_width = if compact {
            186.
        } else {
            (available * 0.30).clamp(224., 296.)
        };
        let gap = if compact { 12. } else { 22. };
        let aspect = (TABLE_WIDTH / TABLE_HEIGHT) as f32;
        let width_limited = (available - rail_width - gap) / aspect;
        let height = (f32::from(window.viewport_size().height) - 116.)
            .min(width_limited)
            .clamp(306., 840.);
        let width = height * aspect;
        t::panel(cx)
            .w(px(width + rail_width + gap + 34.))
            .max_w(relative(1.))
            .p_4()
            .flex()
            .items_start()
            .gap(px(gap))
            .child(
                div()
                    .flex_none()
                    .w(px(width))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.board(width, height, cx))
                    .when(
                        compact && self.progress.round.status() == Status::Playing,
                        |column| {
                            column.child(keyboard_help(t::colors(cx), self.has_next_ammo()).px_1())
                        },
                    ),
            )
            .child(self.rail(rail_width, cx))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonStyle {
    Gold,
    Selected,
    Plain,
    Disabled,
}

fn muted(p: t::Palette) -> Div {
    div()
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(rgb(p.muted))
}

fn keyboard_help(p: t::Palette, has_next_ammo: bool) -> Div {
    muted(p)
        .child("Click the table to shoot.")
        .child(div().child("← or → aims · Hold Shift for fine aim"))
        .child(div().child(if has_next_ammo {
            "Space shoots · X swaps · R restarts"
        } else {
            "Space shoots · R restarts"
        }))
}

fn metric(label: &'static str, value: impl Into<SharedString>, p: t::Palette) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(muted(p).child(label))
        .child(t::display_type(value, 27.))
}

fn ammo(label: &'static str, color: Color, p: t::Palette) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(6.))
        .p(px(6.))
        .rounded_md()
        .bg(rgb(p.paper))
        .child(
            div().flex_none().rounded_full().bg(rgb(0x244543)).child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        table::paint_bubble(window, bounds.center(), px(12.3), color, 1.);
                    },
                )
                .size(px(29.)),
            ),
        )
        .child(
            muted(p)
                .line_height(px(15.))
                .text_size(px(11.))
                .child(label)
                .child(
                    div()
                        .text_color(rgb(p.ink))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(table::color_name(color)),
                ),
        )
}

fn star_row(stars: usize, side: f32, earned: u32, blank: u32) -> Stateful<Div> {
    div()
        .id("bubble-result-stars")
        .role(Role::Image)
        .aria_label(format!("{stars} of 3 stars"))
        .flex()
        .gap_1()
        .children((0..3).map(|star| {
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    table::paint_star(
                        window,
                        bounds.center(),
                        px(side * 0.46),
                        if star < stars { earned } else { blank },
                    );
                },
            )
            .size(px(side))
        }))
}
