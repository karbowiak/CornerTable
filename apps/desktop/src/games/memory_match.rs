mod art;
mod persistence;
mod table;

use std::time::Duration;

use corner_table_memory_match::{Board, Card, Face, Flip, Layout, Turn};
use gpui::{Context, FocusHandle, KeyDownEvent, Task, Window};

use crate::{
    audio::{self, Effect},
    storage::Session,
};

use super::{GameEntry, GameLaunch};
use art::CardArt;
use persistence::{Round, Saved};

const REVEAL_TIME: Duration = Duration::from_millis(1_600);

#[derive(Clone, Copy)]
enum Notice {
    Pair(Face),
    Concealed,
}

pub struct GameView {
    launch: GameLaunch,
    session: Session,
    rounds: [Round; 3],
    layout: Layout,
    hold_mismatches: bool,
    conceal_timer: Option<Task<()>>,
    art: CardArt,
    focus: [FocusHandle; Layout::MAX_CARDS],
    cursor: usize,
    revision: usize,
    last_flip: Option<usize>,
    confirm_new_deal: bool,
    notice: Option<Notice>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let session = Session::new(launch);
        let seed = launch.seed();
        let (layout, hold_mismatches, rounds) = match session.load::<Saved>() {
            Some(saved) => {
                let mut saved_rounds = saved.rounds.into_iter();
                let rounds = Layout::ALL.map(|layout| {
                    let saved_round = saved_rounds.next().unwrap();
                    saved_round.restore(layout, layout_seed(layout, seed))
                });
                (saved.layout, saved.hold_mismatches, rounds)
            }
            None => {
                let layout = match &launch.entry {
                    GameEntry::Mode(id) => Layout::from_id(id).unwrap_or_default(),
                    _ => Layout::default(),
                };
                let rounds =
                    Layout::ALL.map(|size| Round::deal(size, layout_seed(size, seed), None));
                (layout, false, rounds)
            }
        };

        // A completed checkpoint can survive an interrupted result write.
        // Session deduplicates the retry by the persisted attempt ID.
        for round in &rounds {
            round.record_result(&session);
        }

        let mut view = Self {
            launch: launch.clone(),
            session,
            cursor: resume_cursor(&rounds[layout.slot()].board),
            rounds,
            layout,
            hold_mismatches,
            conceal_timer: None,
            art: CardArt::load(cx),
            focus: std::array::from_fn(|_| cx.focus_handle()),
            revision: 0,
            last_flip: None,
            confirm_new_deal: false,
            notice: None,
        };
        view.schedule_conceal(cx);
        view
    }

    fn round(&self) -> &Round {
        &self.rounds[self.layout.slot()]
    }

    fn board(&self) -> &Board {
        &self.round().board
    }

    fn save(&self) {
        self.session.save(&Saved {
            layout: self.layout,
            hold_mismatches: self.hold_mismatches,
            rounds: std::array::from_fn(|slot| self.rounds[slot].capture()),
        });
    }

    fn select_layout(&mut self, layout: Layout, cx: &mut Context<Self>) {
        if self.layout != layout {
            self.layout = layout;
            self.changed_deal(cx);
        }
    }

    fn request_new_deal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pristine = self.board().attempts() == 0 && self.board().turn() == Turn::Ready;
        if !self.confirm_new_deal && !pristine && !self.board().is_complete() {
            self.confirm_new_deal = true;
            cx.notify();
            return;
        }

        self.rounds[self.layout.slot()] = Round::deal(
            self.layout,
            layout_seed(self.layout, self.launch.seed()),
            self.round().best,
        );
        self.changed_deal(cx);
        self.focus[self.cursor].focus(window, cx);
    }

    fn changed_deal(&mut self, cx: &mut Context<Self>) {
        self.conceal_timer = None;
        self.cursor = resume_cursor(self.board());
        self.last_flip = None;
        self.notice = None;
        self.confirm_new_deal = false;
        self.save();
        self.schedule_conceal(cx);
        cx.notify();
    }

    fn flip(&mut self, index: usize, cx: &mut Context<Self>) {
        let round = &mut self.rounds[self.layout.slot()];
        let Some(result) = round.board.flip(index) else {
            return;
        };

        let complete = matches!(result, Flip::Match { complete: true, .. });
        if complete {
            round.complete();
        }
        self.notice = match result {
            Flip::Match { face, .. } => Some(Notice::Pair(face)),
            _ => None,
        };
        self.cursor = index;
        self.last_flip = Some(index);
        self.revision = self.revision.wrapping_add(1);
        self.confirm_new_deal = false;
        self.schedule_conceal(cx);
        self.save();
        if complete {
            self.round().record_result(&self.session);
        }
        audio::play(
            cx,
            match result {
                Flip::Match { complete: true, .. } => Effect::Complete,
                Flip::Match { .. } => Effect::Clear,
                _ => Effect::Move,
            },
        );
        cx.notify();
    }

    fn schedule_conceal(&mut self, cx: &mut Context<Self>) {
        if self.hold_mismatches
            || self.conceal_timer.is_some()
            || !matches!(self.board().turn(), Turn::Mismatch { .. })
        {
            return;
        }

        let timer = cx.background_executor().timer(REVEAL_TIME);
        let layout = self.layout;
        let revision = self.revision;
        self.conceal_timer = Some(cx.spawn(async move |view, cx| {
            timer.await;
            let _ = view.update(cx, |view, cx| {
                if view.layout == layout && view.revision == revision && !view.hold_mismatches {
                    view.conceal(cx);
                }
            });
        }));
    }

    fn conceal(&mut self, cx: &mut Context<Self>) {
        self.conceal_timer = None;
        if self.rounds[self.layout.slot()].board.conceal_mismatch() {
            self.notice = Some(Notice::Concealed);
            self.last_flip = None;
            self.save();
            cx.notify();
        }
    }

    fn toggle_hold(&mut self, cx: &mut Context<Self>) {
        self.hold_mismatches = !self.hold_mismatches;
        self.conceal_timer = None;
        self.schedule_conceal(cx);
        self.save();
        cx.notify();
    }

    fn focused_card(&self, window: &Window) -> Option<usize> {
        self.focus[..self.layout.card_count()]
            .iter()
            .position(|handle| handle.is_focused(window))
    }

    fn board_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keys = &event.keystroke;
        if keys.modifiers.platform || keys.modifiers.control || keys.modifiers.alt {
            return;
        }

        let current = self.focused_card(window).unwrap_or(self.cursor);
        let (columns, rows) = self.layout.dimensions();
        let (row, column) = (current / columns, current % columns);
        let next = match keys.key.as_str() {
            "up" => Some((row.saturating_sub(1), column)),
            "down" => Some(((row + 1).min(rows - 1), column)),
            "left" => Some((row, column.saturating_sub(1))),
            "right" => Some((row, (column + 1).min(columns - 1))),
            "enter" | "space" => {
                if !event.is_held {
                    self.flip(current, cx);
                }
                None
            }
            "h" if self.hold_mismatches && matches!(self.board().turn(), Turn::Mismatch { .. }) => {
                self.conceal(cx);
                None
            }
            _ => return,
        };
        cx.stop_propagation();
        if let Some((row, column)) = next {
            self.cursor = row * columns + column;
            self.focus[self.cursor].focus(window, cx);
            cx.notify();
        }
    }
}

fn layout_seed(layout: Layout, seed: u64) -> u64 {
    seed ^ (layout.slot() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn resume_cursor(board: &Board) -> usize {
    match board.turn() {
        Turn::One { first } => first,
        Turn::Mismatch { second, .. } => second,
        Turn::Ready => (0..board.layout().card_count())
            .find(|&index| board.card(index) == Some(Card::Hidden))
            .unwrap_or(0),
    }
}
