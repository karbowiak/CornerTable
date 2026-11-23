mod session;
mod table;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use corner_table_hearts::{Card, Phase, PlayedCard, Seat, TRICKS_PER_HAND};
use gpui::{Context, FocusHandle, IntoElement, KeyDownEvent, Render, Task, Window};

use crate::{
    audio::{self, Effect},
    games::GameLaunch,
};

use self::session::{TableSession, Turn};

const OPPONENT_DELAY: Duration = Duration::from_millis(365);
const TRICK_REVIEW: Duration = Duration::from_millis(1_650);

pub struct GameView {
    session: TableSession,
    focus: FocusHandle,
    cancel_focus: FocusHandle,
    focus_on_open: bool,
    cursor: Option<Card>,
    timer: Option<Task<()>>,
    revision: usize,
    arrival: Option<PlayedCard>,
    confirm_new: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        Self {
            session: TableSession::open(launch),
            focus: cx.focus_handle().tab_stop(true),
            cancel_focus: cx.focus_handle().tab_stop(true),
            focus_on_open: true,
            cursor: None,
            timer: None,
            revision: 0,
            arrival: None,
            confirm_new: false,
        }
    }

    fn choose_card(&mut self, card: Card, cx: &mut Context<Self>) {
        if self.confirm_new || self.session.ready_to_open() {
            return;
        }
        match self.session.game().phase() {
            Phase::Passing => {
                self.session.toggle(card);
                self.cursor = Some(card);
                cx.notify();
            }
            Phase::Playing(Seat::You) => self.perform(Turn::Play { card: card.id() }, cx),
            _ => {}
        }
    }

    fn perform(&mut self, turn: Turn, cx: &mut Context<Self>) {
        if self.confirm_new || !self.session.apply(turn) {
            return;
        }
        self.timer = None;
        self.cursor = None;
        self.revision = self.revision.wrapping_add(1);
        self.arrival = match turn {
            Turn::Play { .. } => self.session.game().trick().last().copied(),
            _ => None,
        };
        match turn {
            Turn::Pass { .. } => audio::play(cx, Effect::CardSlide),
            Turn::Play { .. } => audio::play(cx, Effect::CardPlace),
            Turn::Collect => {
                let effect = match self.session.game().phase() {
                    Phase::HandComplete(_) => Effect::Complete,
                    _ => Effect::CardCollect,
                };
                audio::play(cx, effect);
            }
            _ => {}
        }
        cx.notify();
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos() as u64);
        self.session.restart(seed);
        self.timer = None;
        self.cursor = None;
        self.arrival = None;
        self.confirm_new = false;
        self.revision = self.revision.wrapping_add(1);
        cx.notify();
    }

    fn schedule_turn(&mut self, cx: &mut Context<Self>) {
        if self.timer.is_some() || self.confirm_new || self.session.ready_to_open() {
            return;
        }
        let game = self.session.game();
        let (turn, delay) = match game.phase() {
            Phase::Playing(seat) if seat != Seat::You => {
                let Some(card) = game.suggested_play() else {
                    return;
                };
                (Turn::Play { card: card.id() }, OPPONENT_DELAY)
            }
            Phase::TrickComplete(_)
                if !self.session.pause_tricks() && game.trick_number() < TRICKS_PER_HAND =>
            {
                (Turn::Collect, TRICK_REVIEW)
            }
            _ => return,
        };
        let timer = cx.background_executor().timer(delay);
        let revision = self.revision;
        self.timer = Some(cx.spawn(async move |view, cx| {
            timer.await;
            let _ = view.update(cx, |view, cx| {
                view.timer = None;
                if revision == view.revision {
                    view.perform(turn, cx);
                }
            });
        }));
    }

    fn keyboard_cards(&self) -> Vec<Card> {
        if self.confirm_new || self.session.ready_to_open() {
            return Vec::new();
        }
        let game = self.session.game();
        match game.phase() {
            Phase::Passing => game.hand(Seat::You).to_vec(),
            Phase::Playing(Seat::You) => game.legal_plays(),
            _ => Vec::new(),
        }
    }

    fn cursor_card(&self) -> Option<Card> {
        let cards = self.keyboard_cards();
        self.cursor
            .filter(|card| cards.contains(card))
            .or_else(|| cards.first().copied())
    }

    fn step_card(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let cards = self.keyboard_cards();
        if let Some(index) = self
            .cursor_card()
            .and_then(|cursor| cards.iter().position(|&card| card == cursor))
        {
            let step = if backwards { cards.len() - 1 } else { 1 };
            self.cursor = Some(cards[(index + step) % cards.len()]);
            cx.notify();
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.modifiers.alt || key.modifiers.control || key.modifiers.platform {
            return;
        }
        if self.confirm_new && key.key == "escape" && !event.is_held {
            self.cancel_restart(window, cx);
            cx.stop_propagation();
            return;
        }
        if !self.focus.is_focused(window) || self.confirm_new {
            return;
        }
        match key.key.as_str() {
            "left" => self.step_card(true, cx),
            "right" => self.step_card(false, cx),
            "space" if !event.is_held => {
                if let Some(card) = self.cursor_card() {
                    self.choose_card(card, cx);
                } else {
                    self.advance(cx);
                }
            }
            "enter" if !event.is_held => {
                if self.session.game().phase() == Phase::Passing {
                    if let Some(turn) = self.session.pass_turn() {
                        self.perform(turn, cx);
                    }
                } else if let Some(card) = self.cursor_card() {
                    self.choose_card(card, cx);
                } else {
                    self.advance(cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn cancel_restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_new = false;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn advance(&mut self, cx: &mut Context<Self>) {
        if self.session.ready_to_open() {
            self.perform(Turn::Begin, cx);
            return;
        }
        match self.session.game().phase() {
            Phase::TrickComplete(_) => self.perform(Turn::Collect, cx),
            Phase::HandComplete(result) if result.winner.is_none() => self.perform(Turn::Deal, cx),
            Phase::HandComplete(_) => self.restart(cx),
            _ => {}
        }
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_on_open) {
            cx.on_next_frame(window, |view, window, cx| view.focus.focus(window, cx));
        }
        self.schedule_turn(cx);
        self.render_table(window, cx)
    }
}
