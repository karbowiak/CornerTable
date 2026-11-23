mod chain;
mod pieces;
mod state;
mod table;

use std::time::Duration;

use corner_table_dominoes::{
    Action, End, Event, Finish, Phase, Player, Round, Tile, computer_action,
};
use gpui::{Context, FocusHandle, KeyDownEvent, Render, Role, Task, Window, div, prelude::*, px};

use crate::{
    audio::{self, Effect},
    storage::{Outcome, ResultMetric, Session},
};

use super::{GameEntry, GameLaunch, board_space};
use state::{Mode, Saved, Table};

pub struct GameView {
    tables: [Table; 2],
    mode: Mode,
    session: Session,
    computer: Option<Task<()>>,
    revision: u64,
    focus: FocusHandle,
    needs_focus: bool,
    cursor: usize,
    selected: Option<Tile>,
    revealed: bool,
    confirm_restart: bool,
    mismatch: Option<End>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let session = Session::new(launch);
        let seed = launch.seed();
        let fresh = |index| Table::new(seed.wrapping_add(index as u64));
        let (mode, tables) = session
            .load::<Saved>()
            .and_then(|saved| saved.restore(fresh))
            .unwrap_or_else(|| {
                let mode = match &launch.entry {
                    GameEntry::Mode(id)
                        if matches!(id.as_str(), "hot-seat" | "local" | "two-players") =>
                    {
                        Mode::HotSeat
                    }
                    _ => Mode::Computer,
                };
                (mode, std::array::from_fn(fresh))
            });
        let mut view = Self {
            tables,
            mode,
            session,
            computer: None,
            revision: 0,
            focus: cx.focus_handle().tab_stop(true),
            needs_focus: true,
            cursor: 0,
            selected: None,
            revealed: false,
            confirm_restart: false,
            mismatch: None,
        };
        view.reset_cursor();
        view.save();
        for mode in Mode::ALL {
            view.record_result(mode);
        }
        view.queue_computer(cx);
        view
    }

    fn table(&self) -> &Table {
        &self.tables[self.mode.index()]
    }

    fn round(&self) -> &Round {
        &self.table().round
    }

    fn save(&self) {
        self.session.save(&Saved::capture(self.mode, &self.tables));
    }

    fn record_result(&self, mode: Mode) {
        let table = &self.tables[mode.index()];
        let Some(outcome) = table.round.outcome() else {
            return;
        };
        let (result, earned) = match (mode, outcome.winner) {
            (Mode::HotSeat, _) => (Outcome::Finished, outcome.points),
            (Mode::Computer, Some(Player::One)) => (Outcome::Won, outcome.points),
            (Mode::Computer, Some(Player::Two)) => (Outcome::Lost, 0),
            (Mode::Computer, None) => (Outcome::Finished, 0),
        };
        self.session.record_result(
            table.attempt(),
            mode.label(),
            result,
            ResultMetric::Points(u64::from(earned)),
        );
    }

    fn computer_turn(&self) -> bool {
        self.mode == Mode::Computer
            && self.round().player() == Player::Two
            && self.round().outcome().is_none()
    }

    fn handoff(&self) -> bool {
        self.mode == Mode::HotSeat && !self.revealed && self.round().outcome().is_none()
    }

    fn human_turn(&self) -> bool {
        self.round().outcome().is_none() && !self.computer_turn() && !self.handoff()
    }

    fn visible_hand(&self) -> &[Tile] {
        if self.handoff() || self.round().outcome().is_some() {
            return &[];
        }
        let player = match self.mode {
            Mode::Computer => Player::One,
            Mode::HotSeat => self.round().player(),
        };
        self.round().hand(player)
    }

    fn queue_computer(&mut self, cx: &mut Context<Self>) {
        if self.computer.is_some() || !self.computer_turn() {
            return;
        }
        let revision = self.revision;
        let millis = match self.round().last_event() {
            Event::Opened { .. } => 720,
            Event::Drew { .. } | Event::Passed { .. } => 550,
            Event::Played { .. } => 450,
        };
        let pause = cx
            .background_executor()
            .timer(Duration::from_millis(millis));
        self.computer = Some(cx.spawn(async move |view, cx| {
            pause.await;
            let _ = view.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                view.computer = None;
                if view.computer_turn()
                    && let Some(action) = computer_action(view.round())
                {
                    view.apply(action, cx);
                }
            });
        }));
    }

    fn apply(&mut self, action: Action, cx: &mut Context<Self>) {
        if self.tables[self.mode.index()].act(action).is_none() {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        if !matches!(action, Action::Draw) {
            self.revealed = false;
        }
        self.selected = None;
        self.mismatch = None;
        self.reset_cursor();
        self.save();
        let finished = self.round().outcome().is_some();
        if finished {
            self.confirm_restart = false;
            self.record_result(self.mode);
        }
        audio::play(
            cx,
            if finished {
                Effect::Complete
            } else {
                Effect::Move
            },
        );
        self.queue_computer(cx);
        cx.notify();
    }

    fn reset_cursor(&mut self) {
        self.cursor = if self.human_turn() {
            self.visible_hand()
                .iter()
                .position(|&tile| self.round().fits(tile).contains(&true))
                .unwrap_or(0)
        } else {
            0
        };
    }

    fn reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.handoff() {
            self.revealed = true;
            self.reset_cursor();
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    fn hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.revealed = false;
        self.selected = None;
        self.mismatch = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn activate_tile(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.human_turn()
            && let Some(&tile) = self.visible_hand().get(index)
        {
            self.cursor = index;
            self.selected = if self.selected == Some(tile) {
                None
            } else {
                Some(tile)
            };
            self.mismatch = None;
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    fn place(&mut self, end: End, window: &mut Window, cx: &mut Context<Self>) {
        if !self.human_turn() {
            return;
        }
        let Some(tile) = self.selected else {
            return;
        };
        self.focus.focus(window, cx);
        if self.round().fits(tile)[end.index()] {
            self.apply(Action::Play(tile, end), cx);
        } else {
            self.mismatch = Some(end);
            audio::play(cx, Effect::InvalidMove);
            cx.notify();
        }
    }

    fn turn_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.human_turn() {
            self.focus.focus(window, cx);
            self.apply(action, cx);
        }
    }

    fn valid_end(&self, end: End) -> bool {
        self.human_turn()
            && self
                .selected
                .is_some_and(|tile| self.round().fits(tile)[end.index()])
    }

    fn switch_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            self.refresh(window, cx);
        }
    }

    fn restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirm_restart || self.round().outcome().is_some() || !self.table().has_moves() {
            self.tables[self.mode.index()].redeal(new_seed());
            self.refresh(window, cx);
        } else {
            self.confirm_restart = true;
            cx.notify();
        }
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.computer = None;
        self.revision = self.revision.wrapping_add(1);
        self.revealed = false;
        self.selected = None;
        self.mismatch = None;
        self.confirm_restart = false;
        self.reset_cursor();
        self.focus.focus(window, cx);
        self.save();
        self.queue_computer(cx);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keys = &event.keystroke;
        if keys.modifiers.platform || keys.modifiers.control || keys.modifiers.alt {
            return;
        }
        if keys.key == "escape" && (self.confirm_restart || self.selected.is_some()) {
            self.confirm_restart = false;
            self.selected = None;
            self.mismatch = None;
        } else if !self.focus.is_focused(window) {
            return;
        } else if self.round().outcome().is_some() {
            if keys.key != "enter" || event.is_held {
                return;
            }
            self.restart(window, cx);
        } else if self.handoff() {
            if !matches!(keys.key.as_str(), "enter" | "space") || event.is_held {
                return;
            }
            self.reveal(window, cx);
        } else if self.human_turn() {
            match keys.key.as_str() {
                "left" => self.cursor = self.cursor.saturating_sub(1),
                "right" => self.cursor = (self.cursor + 1).min(self.visible_hand().len() - 1),
                "enter" | "space" if !event.is_held => self.activate_tile(self.cursor, window, cx),
                "l" if !event.is_held => self.place(End::Left, window, cx),
                "r" if !event.is_held => self.place(End::Right, window, cx),
                "d" if !event.is_held && self.round().phase() == Phase::Draw => {
                    self.turn_action(Action::Draw, window, cx);
                }
                "p" if !event.is_held && self.round().phase() == Phase::Pass => {
                    self.turn_action(Action::Pass, window, cx);
                }
                _ => return,
            }
        } else {
            return;
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn player_name(&self, player: Player) -> &'static str {
        self.mode.player_name(player)
    }

    fn headline(&self) -> String {
        if let Some(outcome) = self.round().outcome() {
            return match outcome.winner {
                Some(player) => format!("{} won the round", self.player_name(player)),
                None => "Tied round".to_owned(),
            };
        }
        let player = self.round().player();
        if self.handoff() {
            format!("Pass the table to {}", self.player_name(player))
        } else if self.computer_turn() {
            "Computer’s turn".to_owned()
        } else if self.mode == Mode::HotSeat {
            format!("{}’s turn", self.player_name(player))
        } else {
            "Your turn".to_owned()
        }
    }

    fn instruction(&self) -> Option<String> {
        if let Some(outcome) = self.round().outcome() {
            let reason = match outcome.finish {
                Finish::WentOut => "Last tile played.",
                Finish::Blocked => "Both players passed.",
            };
            let [first, second] = outcome.pips;
            let points = outcome.points;
            let point_label = if points == 1 { "point" } else { "points" };
            return Some(format!(
                "{reason} {} {first} pips, {} {second} pips. {points} {point_label} awarded.",
                self.player_name(Player::One),
                self.player_name(Player::Two),
            ));
        }
        if self.handoff() {
            return Some("Both hands are face down.".to_owned());
        }
        if self.computer_turn() {
            return None;
        }
        let ends = self.round().ends();
        if let (Some(tile), Some(end)) = (self.selected, self.mismatch) {
            return Some(format!(
                "{tile} doesn’t match the {} end ({}).",
                end_name(end),
                ends[end.index()]
            ));
        }
        Some(match self.round().phase() {
            Phase::Draw => "No tile fits. Draw until you find a match.".to_owned(),
            Phase::Pass => "The boneyard is empty and no tile fits.".to_owned(),
            Phase::Play => match self.selected {
                Some(tile) => match self.round().fits(tile) {
                    [true, true] => format!("{tile} fits both ends. Choose where to place it."),
                    [true, false] => format!("Place {tile} at the left end."),
                    [false, true] => format!("Place {tile} at the right end."),
                    [false, false] => {
                        format!("{tile} doesn’t fit. Select a tile with a matching face.")
                    }
                },
                None => "Select a tile, then choose a matching end.".to_owned(),
            },
            Phase::Finished => unreachable!(),
        })
    }

    fn last_action(&self) -> String {
        let round = self.round();
        match round.last_event() {
            Event::Opened { player, tile } => {
                format!("{} opened with {tile}.", self.player_name(player))
            }
            Event::Played { player, tile, end } => {
                let side = end_name(end);
                let name = self.player_name(player);
                match self.table().preceding_event {
                    Some(Event::Drew {
                        player: drew,
                        count,
                    }) if drew == player => {
                        format!("{name} drew {count} and played {tile} at the {side} end.")
                    }
                    _ => format!("{name} played {tile} at the {side} end."),
                }
            }
            Event::Drew { player, count } => {
                let label = if count == 1 { "tile" } else { "tiles" };
                format!("{} drew {count} {label}.", self.player_name(player))
            }
            Event::Passed { player } => match self.table().preceding_event {
                Some(Event::Drew {
                    player: drew,
                    count,
                }) if drew == player => {
                    format!("{} drew {count} and passed.", self.player_name(player))
                }
                _ => format!("{} passed.", self.player_name(player)),
            },
        }
    }
}

fn end_name(end: End) -> &'static str {
    match end {
        End::Left => "left",
        End::Right => "right",
    }
}

fn new_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.needs_focus) {
            cx.on_next_frame(window, |view, window, cx| view.focus.focus(window, cx));
        }
        // Leave a little margin for the shell's page gutters at both breakpoints.
        let width = board_space(window).clamp(330., 1150.);
        let narrow = width < 660.;
        let short = window.viewport_size().height < px(640.);
        let compact = narrow || short;
        div()
            .id("dominoes")
            .w_full()
            .max_w(px(1150.))
            .flex()
            .flex_col()
            .gap(px(if short { 8. } else if narrow { 9. } else { 13. }))
            .track_focus(&self.focus)
            .tab_index(0)
            .role(Role::Group)
            .aria_label("Draw Dominoes table")
            .aria_description("Arrow keys browse your hand. Enter selects. L and R play at either end. D draws. P passes.")
            .on_key_down(cx.listener(Self::key_down))
            .child(self.toolbar(cx))
            .child(self.table_surface(width, narrow, short, cx))
            .child(self.rack(compact, window, cx))
    }
}
