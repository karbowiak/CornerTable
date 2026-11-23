mod board;
mod controls;
mod state;

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use corner_table_chess::{
    Color, ComputerTurn, Decision, DrawClaim, Game, Move, Piece, Square, choose_turn,
};
use gpui::{Context, FocusHandle, KeyDownEvent, Render, Task, Window, div, prelude::*};

use crate::audio::{self, Effect};

use super::{
    GameLaunch,
    stage::{self, Layout, Stage},
};
use state::{Mode, Table};

const PROMOTIONS: [Piece; 4] = [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight];

pub(crate) const GUIDE_SECTIONS: &[(&str, &str)] = &[
    (
        "Checkmate the king",
        "White moves first. If your king is in check, move it to safety, block the attack, or capture the attacking piece. If there is no legal way out of check, the other side wins. A player with no legal moves and no check is in stalemate, which is a draw.",
    ),
    (
        "Move a piece",
        "Click or drag a piece to a marked destination. With the board focused, use the arrows to move the cursor and Enter or Space to select and play. To castle, move the king two squares toward the rook. When a pawn reaches the far rank, choose its new piece.",
    ),
    (
        "Choose a match",
        "Two players share one board. In Computer mode you play White. Each mode saves its own match. Undo takes back your last action, including the computer’s reply in solo play. Claim draw appears when the position or an announced move qualifies.",
    ),
];

#[derive(Clone, Copy)]
struct Promotion {
    movement: Move,
    choice: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirmation {
    Restart,
    Resign,
    OfferDraw,
}

struct Computer {
    cancelled: Arc<AtomicBool>,
    _task: Task<()>,
}

impl Drop for Computer {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

pub struct GameView {
    table: Table,
    legal: Vec<Move>,
    claim_now: Option<DrawClaim>,
    claim_moves: Vec<Move>,
    claim_mode: bool,
    selected: Option<Square>,
    cursor: Square,
    revision: usize,
    arrival: Option<Move>,
    computer: Option<Computer>,
    board_focus: FocusHandle,
    promotion_focus: FocusHandle,
    confirmation_focus: FocusHandle,
    confirmation: Option<Confirmation>,
    promotion: Option<Promotion>,
    notice: Option<String>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            table: Table::open(launch),
            legal: Vec::new(),
            claim_now: None,
            claim_moves: Vec::new(),
            claim_mode: false,
            selected: None,
            cursor: Square::E2,
            revision: 0,
            arrival: None,
            computer: None,
            board_focus: cx.focus_handle().tab_stop(true),
            promotion_focus: cx.focus_handle().tab_stop(true),
            confirmation_focus: cx.focus_handle().tab_stop(true),
            confirmation: None,
            promotion: None,
            notice: None,
        };
        view.read_position();
        view.reset_cursor();
        view.queue_computer(cx);
        view
    }

    fn game(&self) -> &Game {
        self.table.game()
    }

    fn human_turn(&self) -> bool {
        self.game().ending().is_none()
            && (self.table.mode == Mode::Local || self.game().side_to_move() == Color::White)
    }

    fn moves_for_input(&self) -> &[Move] {
        if !self.human_turn() || self.promotion.is_some() || self.confirmation.is_some() {
            &[]
        } else if self.claim_mode {
            &self.claim_moves
        } else {
            &self.legal
        }
    }

    fn read_position(&mut self) {
        self.legal = self.game().legal_moves();
        self.claim_now = None;
        self.claim_moves.clear();
        if self.human_turn() {
            self.claim_now = self.game().claim(None);
            if self.claim_now.is_none() {
                self.claim_moves = self.game().claim_moves();
            }
        }
    }

    fn reset_cursor(&mut self) {
        let pawn = if self.game().side_to_move() == Color::White {
            Square::E2
        } else {
            Square::E7
        };
        let cursor = self
            .game()
            .moves()
            .next_back()
            .map_or(pawn, |play| play.movement.to);
        self.cursor = cursor;
    }

    fn queue_computer(&mut self, cx: &mut Context<Self>) {
        if self.table.mode != Mode::Computer
            || self.game().side_to_move() != Color::Black
            || self.game().ending().is_some()
            || self.computer.is_some()
            || self.confirmation.is_some()
        {
            return;
        }
        let game = self.game().clone();
        let revision = self.revision;
        let cancelled = Arc::new(AtomicBool::new(false));
        let search_cancellation = cancelled.clone();
        let executor = cx.background_executor();
        let pause = executor.timer(Duration::from_millis(420));
        let search = executor.spawn(async move { choose_turn(&game, &search_cancellation) });
        let task = cx.spawn(async move |view, cx| {
            let turn = search.await;
            pause.await;
            let _ = view.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                view.computer = None;
                match turn {
                    Some(ComputerTurn::Play(movement)) => view.play(movement, cx),
                    Some(ComputerTurn::ClaimDraw { intended }) => {
                        view.decide(Decision::ClaimDraw { intended }, cx);
                    }
                    None => cx.notify(),
                }
            });
        });
        self.computer = Some(Computer {
            cancelled,
            _task: task,
        });
    }

    fn refresh(&mut self, notice: Option<String>, cx: &mut Context<Self>) {
        self.computer = None;
        self.revision = self.revision.wrapping_add(1);
        self.arrival = None;
        self.selected = None;
        self.promotion = None;
        self.confirmation = None;
        self.claim_mode = false;
        self.notice = notice;
        self.read_position();
        self.table.save();
        self.queue_computer(cx);
        cx.notify();
    }

    fn play(&mut self, movement: Move, cx: &mut Context<Self>) {
        if self.table.game_mut().play(movement).is_none() {
            return;
        }
        self.refresh(None, cx);
        self.arrival = Some(movement);
        let effect = if self.game().ending().is_some() {
            Effect::Complete
        } else {
            Effect::Move
        };
        audio::play(cx, effect);
    }

    fn decide(&mut self, decision: Decision, cx: &mut Context<Self>) {
        if self.table.game_mut().decide(decision) {
            self.refresh(None, cx);
            audio::play(cx, Effect::Complete);
        }
    }

    fn choose_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.table.mode != mode {
            self.table.mode = mode;
            self.refresh(None, cx);
            self.reset_cursor();
        }
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let undo_to = (self.table.mode == Mode::Computer).then_some(Color::White);
        if self.table.game_mut().undo(undo_to) {
            self.refresh(Some("Last action undone.".into()), cx);
            self.reset_cursor();
            self.board_focus.focus(window, cx);
        }
    }

    fn restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        *self.table.game_mut() = Game::default();
        self.refresh(None, cx);
        self.reset_cursor();
        self.board_focus.focus(window, cx);
    }

    fn ask_for_confirmation(
        &mut self,
        confirmation: Confirmation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirmation = Some(confirmation);
        self.computer = None;
        self.revision = self.revision.wrapping_add(1);
        self.arrival = None;
        self.notice = None;
        self.confirmation_focus.focus(window, cx);
        cx.notify();
    }

    fn flip(&mut self, cx: &mut Context<Self>) {
        self.table.flipped = !self.table.flipped;
        self.arrival = None;
        self.table.save();
        cx.notify();
    }

    fn enter_claim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.claim_now.is_some() {
            self.decide(Decision::ClaimDraw { intended: None }, cx);
            self.board_focus.focus(window, cx);
        } else if !self.claim_moves.is_empty() {
            self.claim_mode = true;
            self.selected = None;
            self.notice = None;
            self.cursor = self.claim_moves[0].from;
            self.board_focus.focus(window, cx);
            cx.notify();
        }
    }

    fn activate(&mut self, square: Square, window: &mut Window, cx: &mut Context<Self>) {
        self.cursor = square;
        self.notice = None;
        if self.selected == Some(square) {
            self.selected = None;
        } else if let Some(from) = self.selected
            && self.try_destination(from, square, window, cx)
        {
            return;
        } else if self
            .moves_for_input()
            .iter()
            .any(|movement| movement.from == square)
        {
            self.selected = Some(square);
        } else if self.selected.is_some() && !self.claim_mode {
            self.notice = Some(format!(
                "{square} is outside the selected piece’s legal moves."
            ));
            audio::play(cx, Effect::InvalidMove);
        } else if self
            .game()
            .piece(square)
            .is_some_and(|(color, _)| color == self.game().side_to_move())
            && !self.claim_mode
        {
            self.notice = Some(format!("The piece on {square} has no legal move."));
        }
        cx.notify();
    }

    fn try_destination(
        &mut self,
        from: Square,
        to: Square,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(movement) = self
            .moves_for_input()
            .iter()
            .find(|m| m.from == from && m.to == to)
            .copied()
        else {
            return false;
        };
        self.cursor = to;
        if movement.promotion.is_some() {
            self.promotion = Some(Promotion {
                movement,
                choice: 0,
            });
            self.promotion_focus.focus(window, cx);
            cx.notify();
        } else {
            self.resolve(movement, cx);
        }
        true
    }

    fn resolve(&mut self, movement: Move, cx: &mut Context<Self>) {
        if self.claim_mode {
            self.decide(
                Decision::ClaimDraw {
                    intended: Some(movement),
                },
                cx,
            );
        } else {
            self.play(movement, cx);
        }
    }

    fn promote(&mut self, piece: Piece, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(promotion) = self.promotion {
            self.resolve(
                Move {
                    promotion: Some(piece),
                    ..promotion.movement
                },
                cx,
            );
            self.board_focus.focus(window, cx);
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keys = &event.keystroke;
        if keys.modifiers.platform || keys.modifiers.control || keys.modifiers.alt {
            return;
        }
        if self.promotion.is_some() || self.confirmation.is_some() {
            return;
        }
        let direction = if self.table.flipped { -1 } else { 1 };
        let offset = match keys.key.as_str() {
            "up" => (0, direction),
            "down" => (0, -direction),
            "left" => (-direction, 0),
            "right" => (direction, 0),
            _ => (0, 0),
        };
        if offset != (0, 0) {
            if let Some(square) = self.cursor.try_offset(offset.0, offset.1) {
                self.cursor = square;
                self.notice = None;
            }
        } else {
            match keys.key.as_str() {
                "enter" | "space" if self.human_turn() => self.activate(self.cursor, window, cx),
                "u" => self.undo(window, cx),
                "f" => self.flip(cx),
                _ => return,
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn promotion_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(promotion) = &mut self.promotion else {
            return;
        };
        let key = &event.keystroke;
        if key.modifiers.platform || key.modifiers.control || key.modifiers.alt {
            return;
        }
        let piece = match key.key.as_str() {
            "q" => Some(Piece::Queen),
            "r" => Some(Piece::Rook),
            "b" => Some(Piece::Bishop),
            "n" => Some(Piece::Knight),
            "enter" | "space" if self.promotion_focus.is_focused(window) => {
                Some(PROMOTIONS[promotion.choice])
            }
            "left" if self.promotion_focus.is_focused(window) => {
                promotion.choice = (promotion.choice + 3) % 4;
                None
            }
            "right" if self.promotion_focus.is_focused(window) => {
                promotion.choice = (promotion.choice + 1) % 4;
                None
            }
            _ => return,
        };
        if let Some(piece) = piece {
            self.promote(piece, window, cx);
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(pending) = self.promotion.take() {
            self.cursor = pending.movement.from;
        } else if self.confirmation.take().is_some() || std::mem::take(&mut self.claim_mode) {
            self.selected = None;
        } else if !self.board_focus.is_focused(window) || self.selected.take().is_none() {
            return false;
        }
        self.notice = None;
        self.board_focus.focus(window, cx);
        self.queue_computer(cx);
        cx.notify();
        true
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stage = Stage::measure(window, Layout::Rail, false);
        let tile = ((stage.board_width.min(stage.board_height) - 2. * board::FRAME) / 8.)
            .floor()
            .clamp(34., 96.);
        let rail = stage::rail(cx)
            .child(stage::rail_section(cx).child(self.status(cx)))
            .child(stage::rail_section(cx).child(self.controls(cx)))
            .children(
                self.journal(cx)
                    .map(|journal| stage::rail_section(cx).child(journal)),
            )
            .child(stage::controls_help(
                cx,
                &[
                    "Click a piece, then a marked square",
                    "Tab to the board · Arrows move the cursor",
                    "Enter selects or plays · Esc cancels",
                ],
            ));
        div()
            .id("chess")
            .w_full()
            .capture_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && view.cancel(window, cx) {
                    cx.stop_propagation();
                } else if view.promotion.is_some() {
                    view.promotion_key(event, window, cx);
                }
            }))
            .child(stage.render(
                cx,
                None,
                self.board(tile, window, cx),
                self.result_card(cx),
                rail,
            ))
    }
}

fn side_name(color: Color) -> &'static str {
    match color {
        Color::White => "White",
        Color::Black => "Black",
    }
}

fn piece_name(piece: Piece) -> &'static str {
    match piece {
        Piece::Pawn => "pawn",
        Piece::Knight => "knight",
        Piece::Bishop => "bishop",
        Piece::Rook => "rook",
        Piece::Queen => "queen",
        Piece::King => "king",
    }
}

#[cfg(test)]
mod tests {
    use corner_table_api::GameId;
    use gpui::{KeyUpEvent, Keystroke, PlatformInput, TestAppContext, px, size};

    use super::*;
    use crate::games::GameEntry;

    #[gpui::test]
    fn escape_from_a_confirmation_restores_keyboard_navigation(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId("chess".into()),
            entry: GameEntry::Mode("keyboard-test".into()),
        };
        let window = cx.open_window(size(px(812.), px(575.)), |_, cx| GameView::new(&launch, cx));
        cx.run_until_parked();
        window
            .update(cx, |view, window, cx| {
                view.ask_for_confirmation(Confirmation::Restart, window, cx);
                assert!(view.confirmation_focus.is_focused(window));
            })
            .unwrap();
        cx.run_until_parked();
        cx.simulate_keystrokes(window.into(), "escape enter up");
        window
            .update(cx, |view, window, _| {
                assert!(view.confirmation.is_none());
                assert!(view.board_focus.is_focused(window));
                assert_eq!(view.selected, Some(Square::E2));
                assert_eq!(view.cursor, Square::E3);
            })
            .unwrap();
    }

    #[gpui::test]
    fn checkmate_result_actions_undo_and_start_a_new_match(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId("chess-result-test".into()),
            entry: GameEntry::Classic,
        };
        let window = cx.open_window(size(px(1360.), px(760.)), |window, cx| {
            cx.set_reduce_motion(true);
            let mut view = GameView::new(&launch, cx);
            for movement in ["f2f3", "e7e5", "g2g4", "d8h4"] {
                view.play(movement.parse().unwrap(), cx);
            }
            assert_eq!(
                view.game().ending(),
                Some(corner_table_chess::Ending::Checkmate {
                    winner: Color::Black,
                })
            );
            view.board_focus.focus(window, cx);
            view
        });
        let use_result_action = |cx: &mut TestAppContext, tab_stops| {
            cx.run_until_parked();
            window
                .update(cx, |_, window, cx| {
                    for _ in 0..tab_stops {
                        window.focus_next(cx);
                    }
                })
                .unwrap();
            let keystroke = Keystroke::parse("enter").unwrap();
            cx.dispatch_keystroke(window.into(), keystroke.clone());
            cx.update_window(window.into(), |_, window, cx| {
                window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
            })
            .unwrap();
            window
                .update(cx, |view, window, _| {
                    assert!(view.board_focus.is_focused(window));
                    assert_eq!(view.game().ending(), None);
                    view.game().moves().len()
                })
                .unwrap()
        };
        assert_eq!(use_result_action(cx, 2), 3);
        window
            .update(cx, |view, _, cx| view.play("d8h4".parse().unwrap(), cx))
            .unwrap();
        assert_eq!(use_result_action(cx, 1), 0);
    }
}
