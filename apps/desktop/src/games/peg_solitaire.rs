mod board;

use std::time::{Duration, Instant};

use corner_table_peg_solitaire::{Hole, Jump, Position, Round, Status};
use gpui::{
    AnyElement, Context, Div, FocusHandle, KeyDownEvent, Render, Role, Window, div, prelude::*, px,
    rgb,
};
use serde::{Deserialize, Serialize};

use crate::{
    app::ShellLayout,
    audio::{self, Effect},
    motion,
    storage::{AttemptId, Outcome, ResultMetric, Session},
    theme,
};

use super::{GameLaunch, round_result, themed_control};

const SAVE_VERSION: u8 = 1;
const JUMP_DURATION: Duration = Duration::from_millis(185);

#[derive(Deserialize, Serialize)]
struct Saved {
    version: u8,
    attempt: AttemptId,
    jumps: Vec<[u8; 2]>,
}

#[derive(Clone, Copy)]
enum Notice {
    Blocked(Hole),
    ChoosePeg,
    IllegalLanding(Hole),
    Undo,
}

pub struct GameView {
    session: Session,
    attempt: AttemptId,
    round: Round,
    focus: FocusHandle,
    initial_focus: bool,
    cursor: Hole,
    selected: Option<Hole>,
    hint: Option<Jump>,
    notice: Option<Notice>,
    confirm_replay: bool,
    flight: Option<(Jump, Instant)>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let session = Session::new(launch);
        let (round, attempt) = session
            .load::<Saved>()
            .filter(|save| save.version == SAVE_VERSION)
            .and_then(|save| Some((Round::restore(&save.jumps)?, save.attempt)))
            .unwrap_or_default();
        let cursor = start_cursor(&round);
        let view = Self {
            session,
            attempt,
            round,
            focus: cx.focus_handle().tab_stop(true),
            initial_focus: true,
            cursor,
            selected: None,
            hint: None,
            notice: None,
            confirm_replay: false,
            flight: None,
        };
        view.record_result();
        view
    }

    fn position(&self) -> Position {
        self.round.position()
    }

    fn save(&self) {
        self.session.save(&Saved {
            version: SAVE_VERSION,
            attempt: self.attempt,
            jumps: self
                .round
                .history()
                .iter()
                .map(|jump| [jump.from.index(), jump.to.index()])
                .collect(),
        });
    }

    fn record_result(&self) {
        let outcome = match self.position().status() {
            Status::Playing => return,
            Status::Won { .. } => Outcome::Won,
            Status::Stuck => Outcome::Lost,
        };
        self.session.record_result(
            self.attempt,
            "Classic",
            outcome,
            ResultMetric::Moves(self.round.history().len() as u64),
        );
    }

    fn activate(&mut self, hole: Hole, cx: &mut Context<Self>) {
        let position = self.position();
        if position.status() != Status::Playing {
            return;
        }
        self.cursor = hole;
        self.notice = None;
        self.confirm_replay = false;

        if self.selected == Some(hole) {
            self.clear_selection();
        } else if position.occupied(hole) {
            if position.jumps_from(hole).next().is_some() {
                self.selected = Some(hole);
                self.hint = self.hint.filter(|jump| jump.from == hole);
            } else {
                self.notice = Some(Notice::Blocked(hole));
            }
        } else if let Some(from) = self.selected {
            let jump = Jump { from, to: hole };
            if self.round.play(jump).is_some() {
                self.flight = motion::enabled(cx).then(|| (jump, cx.background_executor().now()));
                self.clear_selection();
                self.save();
                self.record_result();
                let effect = match self.position().status() {
                    Status::Won { .. } => Effect::Complete,
                    _ => Effect::CeramicTap,
                };
                audio::play(cx, effect);
            } else {
                self.notice = Some(Notice::IllegalLanding(hole));
            }
        } else {
            self.notice = Some(Notice::ChoosePeg);
        }

        if self.notice.is_some() {
            audio::play(cx, Effect::InvalidMove);
        }
        cx.notify();
    }

    fn clear_selection(&mut self) {
        self.selected = None;
        self.hint = None;
    }

    fn show_hint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_replay = false;
        self.notice = None;
        let from = self.selected;
        let jumps = || {
            self.position()
                .jumps()
                .filter(move |jump| from.is_none_or(|hole| jump.from == hole))
        };
        self.hint = self
            .hint
            .and_then(|previous| jumps().skip_while(|jump| *jump != previous).nth(1))
            .or_else(|| jumps().next());
        if let Some(jump) = self.hint {
            self.cursor = jump.from;
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let was_over = self.position().status() != Status::Playing;
        let Some(jump) = self.round.undo() else {
            return;
        };
        if was_over {
            // The prior terminal result is immutable. Continuing can produce a new result.
            self.attempt = AttemptId::default();
        }
        self.cursor = jump.from;
        self.flight = None;
        self.clear_selection();
        self.confirm_replay = false;
        self.notice = Some(Notice::Undo);
        self.save();
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn replay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.position().status() == Status::Playing && !self.confirm_replay {
            if !self.round.history().is_empty() {
                self.confirm_replay = true;
                cx.notify();
            }
            return;
        }
        self.round = Round::default();
        self.attempt = AttemptId::default();
        self.cursor = start_cursor(&self.round);
        self.flight = None;
        self.clear_selection();
        self.confirm_replay = false;
        self.notice = None;
        self.save();
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keys = &event.keystroke;
        if keys.modifiers.platform || keys.modifiers.control || keys.modifiers.alt {
            return;
        }
        match keys.key.as_str() {
            "h" if !event.is_held && self.position().status() == Status::Playing => {
                self.show_hint(window, cx);
            }
            "u" if !event.is_held => self.undo(window, cx),
            "escape" if self.confirm_replay || self.selected.is_some() || self.hint.is_some() => {
                self.confirm_replay = false;
                self.clear_selection();
                self.notice = None;
            }
            key if self.focus.is_focused(window) => {
                let direction = match key {
                    "up" => (-1, 0),
                    "down" => (1, 0),
                    "left" => (0, -1),
                    "right" => (0, 1),
                    "enter" | "space" => {
                        if !event.is_held {
                            self.activate(self.cursor, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    _ => return,
                };
                if let Some(hole) = self.cursor.offset(direction.0, direction.1) {
                    self.cursor = hole;
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn instruction(&self) -> String {
        if self.confirm_replay {
            return "Replay from the opening? Your current progress will be cleared.".into();
        }
        if let Some(notice) = self.notice {
            return match notice {
                Notice::Blocked(hole) => format!("The peg at {hole} has no jump."),
                Notice::ChoosePeg => "Choose a peg before selecting a landing hole.".into(),
                Notice::IllegalLanding(hole) => {
                    format!("{hole} is out of reach. Choose a marked hole.")
                }
                Notice::Undo => "Last jump undone.".into(),
            };
        }
        if let Some(jump) = self.hint {
            return format!("Jump from {} to {}.", jump.from, jump.to);
        }
        if self.selected.is_some() {
            return "Choose a marked hole to land your jump.".into();
        }
        "Jump over a peg into an empty hole, moving horizontally or vertically. Remove the jumped peg. Aim to leave one peg in the brass center.".into()
    }

    fn controls(&self, cx: &Context<Self>) -> Div {
        let moved = !self.round.history().is_empty();
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                themed_control(cx, "peg-solitaire-undo", "Undo", moved)
                    .aria_label("Undo (U)")
                    .on_click(cx.listener(|view, _, window, cx| view.undo(window, cx))),
            )
            .child(
                themed_control(
                    cx,
                    "peg-solitaire-replay",
                    if self.confirm_replay {
                        "Confirm replay"
                    } else {
                        "Replay"
                    },
                    moved,
                )
                .on_click(cx.listener(|view, _, window, cx| view.replay(window, cx))),
            )
    }

    fn rail(&self, cx: &Context<Self>) -> AnyElement {
        let position = self.position();
        let count = position.peg_count();
        let jumps = self.round.history().len();
        if let Some((outcome, headline, detail)) = result_copy(position) {
            return round_result(
                cx,
                "peg-solitaire-result",
                outcome,
                headline,
                detail,
                self.controls(cx),
            );
        }

        let colors = theme::colors(cx);
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .pb_3()
                    .border_b_1()
                    .border_color(rgb(colors.border))
                    .child(theme::display_type(format!("{count} pegs left"), 27.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(colors.muted))
                            .child(if jumps == 1 {
                                "1 jump".into()
                            } else {
                                format!("{jumps} jumps")
                            }),
                    ),
            )
            .child(
                div()
                    .id("peg-solitaire-instruction")
                    .role(Role::Status)
                    .aria_label(self.instruction())
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .min_h(px(100.))
                    .child(self.instruction()),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        themed_control(cx, "peg-solitaire-hint", "Hint", true)
                            .aria_label("Hint (H)")
                            .debug_selector(|| "peg-solitaire-hint".into())
                            .on_click(
                                cx.listener(|view, _, window, cx| view.show_hint(window, cx)),
                            ),
                    )
                    .child(self.controls(cx)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(rgb(colors.muted))
                    .child("Arrows move · Enter selects or jumps · H hints · U undoes"),
            )
            .into_any_element()
    }
}

fn start_cursor(round: &Round) -> Hole {
    round
        .history()
        .last()
        .map(|jump| jump.to)
        .or_else(|| round.position().jumps().next().map(|jump| jump.from))
        .unwrap_or(Hole::CENTER)
}

fn result_copy(position: Position) -> Option<(Outcome, &'static str, String)> {
    Some(match position.status() {
        Status::Playing => return None,
        Status::Won { center: true } => (
            Outcome::Won,
            "Center finish",
            "One peg remains, right in the brass center.".into(),
        ),
        Status::Won { center: false } => (
            Outcome::Won,
            "One peg left",
            "You cleared 31 pegs. Try leaving the last peg in the brass center.".into(),
        ),
        Status::Stuck => (
            Outcome::Lost,
            "No jumps left",
            format!(
                "{} pegs remain. Undo a jump or replay the board.",
                position.peg_count()
            ),
        ),
    })
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.initial_focus) {
            let previous = window.focused(cx);
            cx.on_next_frame(window, move |view, window, cx| {
                if window.focused(cx) == previous
                    && (previous.is_none() || view.focus.within_focused(window, cx))
                {
                    view.focus.focus(window, cx);
                }
            });
        }
        let width = ShellLayout::new(window.viewport_size().width).content_width();
        let height = f32::from(window.viewport_size().height);
        let rail = 288.;
        let gap = if width < 710. { 18. } else { 28. };
        let board_width = width - rail - gap;
        let cell = ((board_width.min(height - 142.) - board::FRAME * 2.) / 7.)
            .floor()
            .clamp(32., 69.);
        let flight = self.flight.and_then(|(jump, started)| {
            let elapsed = cx
                .background_executor()
                .now()
                .saturating_duration_since(started);
            (elapsed < JUMP_DURATION && motion::enabled(cx))
                .then(|| (jump, elapsed.as_secs_f32() / JUMP_DURATION.as_secs_f32()))
        });
        if flight.is_some() {
            window.request_animation_frame();
        } else {
            self.flight = None;
        }

        div()
            .id("peg-solitaire")
            .flex()
            .items_start()
            .gap(px(gap))
            .on_key_down(cx.listener(Self::key_down))
            .child(self.board(cell, flight, window, cx))
            .child(
                div()
                    .id("peg-solitaire-rail")
                    .debug_selector(|| "peg-solitaire-rail".into())
                    .w(px(rail))
                    .flex_none()
                    .pt(px(12.))
                    .child(self.rail(cx)),
            )
    }
}

#[cfg(test)]
mod tests;
