mod board;
mod controls;
mod state;

use std::time::Duration;

use corner_table_reversi::{Disc, Position, Square, Turn, choose_move};
use gpui::{Context, FocusHandle, KeyDownEvent, Render, Task, Window, div, prelude::*, rgb};

use crate::{
    audio::{self, Effect},
    storage::Session,
    theme,
};

use super::{
    GameEntry, GameLaunch,
    stage::{self, Layout, Stage},
};
use state::{DisplayOptions, Opponent, Round, Saved};

#[derive(Clone, Copy)]
enum Notice {
    Illegal(Square),
    Undo,
    Restart,
}

pub struct GameView {
    rounds: [Round; 3],
    opponent: Opponent,
    display: DisplayOptions,
    session: Session,
    computer: Option<Task<()>>,
    revision: usize,
    animate: bool,
    board_focus: FocusHandle,
    cursor: Square,
    notice: Option<Notice>,
    confirm_restart: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let session = Session::new(launch);
        let (opponent, display, rounds) = session
            .load::<Saved>()
            .and_then(Saved::restore)
            .unwrap_or_else(|| {
                let opponent = match &launch.entry {
                    GameEntry::Mode(id) => Opponent::from_id(id),
                    _ => Opponent::Local,
                };
                (opponent, DisplayOptions::default(), Default::default())
            });
        for opponent in Opponent::ALL {
            rounds[opponent.slot()].record_result(&session, opponent);
        }
        let cursor = starting_cursor(&rounds[opponent.slot()]);
        let mut view = Self {
            rounds,
            opponent,
            display,
            session,
            cursor,
            computer: None,
            revision: 0,
            animate: false,
            board_focus: cx.focus_handle().tab_stop(true),
            notice: None,
            confirm_restart: false,
        };
        view.queue_computer(cx);
        view
    }

    fn round(&self) -> &Round {
        &self.rounds[self.opponent.slot()]
    }

    fn position(&self) -> Position {
        self.round().position
    }

    fn human_turn(&self) -> bool {
        match self.position().turn() {
            Turn::Play(Disc::Black) => true,
            Turn::Play(Disc::White) => self.opponent == Opponent::Local,
            Turn::Finished(_) => false,
        }
    }

    fn save(&self) {
        self.session
            .save(&Saved::capture(self.opponent, self.display, &self.rounds));
    }

    fn queue_computer(&mut self, cx: &mut Context<Self>) {
        let position = self.position();
        if self.computer.is_some() || position.turn() != Turn::Play(Disc::White) {
            return;
        }
        let Some(difficulty) = self.opponent.difficulty() else {
            return;
        };
        let revision = self.revision;
        let executor = cx.background_executor();
        let after_pass = self
            .round()
            .last()
            .is_some_and(|last| last.passed.is_some());
        let pause = executor.timer(Duration::from_millis(if after_pass { 700 } else { 380 }));
        let selection = executor.spawn(async move { choose_move(position, difficulty) });
        self.computer = Some(cx.spawn(async move |view, cx| {
            let choice = selection.await;
            pause.await;
            let _ = view.update(cx, |view, cx| {
                if view.revision == revision {
                    view.computer = None;
                    if let Some(square) = choice {
                        view.play(square, cx);
                    }
                }
            });
        }));
    }

    fn play(&mut self, square: Square, cx: &mut Context<Self>) {
        let round = &mut self.rounds[self.opponent.slot()];
        if round.play(square).is_none() {
            return;
        }
        round.remember_attempt();
        self.revision = self.revision.wrapping_add(1);
        self.animate = true;
        self.notice = None;
        let finished = matches!(self.position().turn(), Turn::Finished(_));
        if finished {
            self.confirm_restart = false;
        }
        self.save();
        self.round().record_result(&self.session, self.opponent);
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

    fn activate(&mut self, square: Square, cx: &mut Context<Self>) {
        if !self.human_turn() {
            return;
        }
        if self.position().legal_moves().contains(square) {
            self.confirm_restart = false;
            self.play(square, cx);
        } else {
            self.notice = Some(Notice::Illegal(square));
            audio::play(cx, Effect::InvalidMove);
            cx.notify();
        }
    }

    fn change_opponent(&mut self, opponent: Opponent, cx: &mut Context<Self>) {
        if self.opponent != opponent {
            self.opponent = opponent;
            self.refresh_round(None, cx);
        }
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.rounds[self.opponent.slot()].undo(self.opponent) {
            self.refresh_round(Some(Notice::Undo), cx);
            self.board_focus.focus(window, cx);
        }
    }

    fn restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirm_restart || matches!(self.position().turn(), Turn::Finished(_)) {
            self.rounds[self.opponent.slot()] = Round::default();
            self.refresh_round(Some(Notice::Restart), cx);
            self.board_focus.focus(window, cx);
        } else if self.round().last().is_some() {
            self.confirm_restart = true;
            cx.notify();
        }
    }

    fn refresh_round(&mut self, notice: Option<Notice>, cx: &mut Context<Self>) {
        self.computer = None;
        self.revision = self.revision.wrapping_add(1);
        self.animate = false;
        self.cursor = starting_cursor(self.round());
        self.confirm_restart = false;
        self.notice = notice;
        self.save();
        self.queue_computer(cx);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keys = &event.keystroke;
        if keys.modifiers.platform || keys.modifiers.control || keys.modifiers.alt {
            return;
        }
        match keys.key.as_str() {
            "up" => self.move_cursor(-1, 0),
            "down" => self.move_cursor(1, 0),
            "left" => self.move_cursor(0, -1),
            "right" => self.move_cursor(0, 1),
            "enter" | "space" => self.activate(self.cursor, cx),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn move_cursor(&mut self, dr: i8, dc: i8) {
        let row = self.cursor.row().saturating_add_signed(dr).min(7);
        let column = self.cursor.column().saturating_add_signed(dc).min(7);
        self.cursor = Square::new(row, column).unwrap();
        if matches!(self.notice, Some(Notice::Illegal(_))) {
            self.notice = None;
        }
    }

    fn headline(&self, disc: Disc) -> String {
        match (self.opponent, disc) {
            (Opponent::Local, _) => format!("{} to move", disc.label()),
            (_, Disc::Black) => "Your turn".into(),
            (_, Disc::White) => "Computer thinking…".into(),
        }
    }

    fn detail(&self) -> String {
        match self.notice {
            Some(Notice::Illegal(square)) if self.position().disc(square).is_some() => {
                return format!("{square} already has a disc.");
            }
            Some(Notice::Illegal(square)) => {
                return format!("{square} would flip no discs.");
            }
            Some(Notice::Undo) => return "Last turn undone.".into(),
            Some(Notice::Restart) => return "The board is reset.".into(),
            None => {}
        }
        let Some(last) = self.round().last() else {
            return "Place a disc to trap an unbroken line of opposing discs.".into();
        };
        if let Some(passed) = last.passed {
            return format!(
                "{} passes. {} goes again.",
                passed.label(),
                last.disc.label()
            );
        }
        let count = last.flipped.count();
        let discs = if count == 1 { "disc" } else { "discs" };
        let pass = if self.opponent != Opponent::Local
            && self.round().preceding_pass() == Some(Disc::Black)
        {
            "Black passed. "
        } else {
            ""
        };
        format!(
            "{pass}{} played {} and flipped {count} {discs}.",
            last.disc.label(),
            last.square
        )
    }
}

fn starting_cursor(round: &Round) -> Square {
    round
        .position
        .legal_moves()
        .next()
        .or_else(|| round.last().map(|last| last.square))
        .unwrap_or_else(|| Square::new(3, 3).unwrap())
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stage = Stage::measure(window, Layout::Rail, false);
        let cell = ((stage.board_width.min(stage.board_height).max(300.) - 2. * board::FRAME) / 8.)
            .floor()
            .min(96.);
        let rail = stage::rail(cx)
            .child(
                div()
                    .border_b_1()
                    .border_color(rgb(theme::colors(cx).border))
                    .child(self.scoreboard(cx)),
            )
            .child(stage::rail_section(cx).child(self.controls(cx)))
            .child(stage::controls_help(
                cx,
                &[
                    "Click a marked square to place a disc",
                    "Tab to the board · Arrows select · Enter plays",
                ],
            ));
        div()
            .id("reversi")
            .w_full()
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && view.confirm_restart {
                    view.confirm_restart = false;
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(stage.render(
                cx,
                None,
                self.board(cell, window, cx),
                self.result_card(cx),
                rail,
            ))
    }
}
