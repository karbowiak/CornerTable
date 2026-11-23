mod board;
mod controls;
mod persistence;

use std::time::{Duration, Instant};

use corner_table_kakuro::{Cell, Direction, Edit, Game, Hint, Size};
use gpui::{Context, FocusHandle, KeyDownEvent, Window, div, prelude::*};

use super::{
    GameEntry, GameLaunch,
    stage::{Layout, Stage},
};
use crate::{
    audio::{self, Effect},
    storage::{AttemptId, Outcome, ResultMetric, Session},
};
use persistence::SavedSession;

const RUN_PULSE: Duration = Duration::from_millis(320);

#[derive(Clone, Copy)]
struct RunPulse {
    runs: [Option<usize>; 2],
    started: Instant,
}

struct Round {
    number: usize,
    game: Game,
    selected: usize,
    pencil: bool,
    hint: Option<Hint>,
    moves: u64,
    attempt: AttemptId,
    used_hint: Option<bool>,
    pulse: Option<RunPulse>,
}

impl Round {
    fn new(size: Size, number: usize) -> Self {
        let game = Game::new(size, number).expect("bundled Kakuro board must have one solution");
        let selected = game.puzzle().entries()[0];
        Self {
            number,
            game,
            selected,
            pencil: false,
            hint: None,
            moves: 0,
            attempt: AttemptId::default(),
            used_hint: Some(false),
            pulse: None,
        }
    }

    fn select(&mut self, cell: usize) {
        if self.selected != cell {
            self.selected = cell;
            self.hint = None;
        }
    }

    fn move_focus(&mut self, row_delta: isize, column_delta: isize) {
        let side = self.game.puzzle().size().side();
        let advance = |(row, column): (usize, usize)| {
            let row = row.checked_add_signed(row_delta)?;
            let column = column.checked_add_signed(column_delta)?;
            (row < side && column < side).then_some((row, column))
        };
        let mut position = (self.selected / side, self.selected % side);
        while let Some(next) = advance(position) {
            position = next;
            let cell = position.0 * side + position.1;
            if self.game.puzzle().cells()[cell] == Cell::Entry {
                self.select(cell);
                break;
            }
        }
    }

    fn cycle_blank(&mut self, backwards: bool) {
        let count = self.game.puzzle().cells().len();
        if let Some(cell) = (1..count)
            .map(|offset| if backwards { count - offset } else { offset })
            .map(|offset| (self.selected + offset) % count)
            .find(|&cell| {
                self.game.puzzle().cells()[cell] == Cell::Entry && self.game.value(cell) == 0
            })
        {
            self.select(cell);
        }
    }

    fn edit(&mut self, digit: u8, pencil: bool) -> Option<Effect> {
        let previous = self.game.value(self.selected);
        if !self.game.edit(Edit {
            cell: self.selected,
            digit,
            pencil,
        }) {
            return None;
        }
        self.hint = None;
        self.pulse = None;
        if self.game.value(self.selected) != previous {
            self.moves = self.moves.saturating_add(1);
            let puzzle = self.game.puzzle();
            let runs = [Direction::Across, Direction::Down].map(|direction| {
                puzzle.run_at(self.selected, direction).filter(|&index| {
                    puzzle
                        .run(index)
                        .cells
                        .iter()
                        .all(|&cell| self.game.value(cell) != 0)
                        && self.game.run_conflict(index).is_none()
                })
            });
            if runs.iter().any(Option::is_some) {
                self.pulse = Some(RunPulse {
                    runs,
                    started: Instant::now(),
                });
            }
        }
        Some(if self.game.is_solved() {
            Effect::Complete
        } else {
            Effect::Move
        })
    }

    fn hint(&mut self) {
        if let Some(hint) = self.game.hint(self.selected) {
            self.selected = hint.cell;
            self.hint = Some(hint);
            self.used_hint = Some(true);
        }
    }

    fn apply_hint(&mut self) -> Option<Effect> {
        self.edit(self.hint?.digit, false)
    }

    fn undo(&mut self) -> Option<Effect> {
        let cell = self.game.undo()?;
        self.selected = cell;
        self.hint = None;
        self.pulse = None;
        Some(Effect::Move)
    }
}

pub struct GameView {
    focus: FocusHandle,
    focus_on_mount: bool,
    session: Session,
    progress: SavedSession,
    round: Round,
    fixed_puzzle: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let session = Session::new(launch);
        let progress = session.load::<SavedSession>().unwrap_or_default();
        let initial_size = match &launch.entry {
            GameEntry::Challenge(_) => Size::Classic,
            GameEntry::Mode(id) if id == "classic" => Size::Classic,
            _ => Size::Compact,
        };
        let fixed_puzzle = matches!(launch.entry, GameEntry::Challenge(_));
        let size = if fixed_puzzle {
            initial_size
        } else {
            progress.size().unwrap_or(initial_size)
        };
        let number = if fixed_puzzle {
            (launch.seed() % size.puzzle_count() as u64) as usize
        } else {
            progress.number(size)
        };
        let round = progress.open(size, number);
        let view = Self {
            focus: cx.focus_handle().tab_stop(true),
            focus_on_mount: true,
            session,
            progress,
            round,
            fixed_puzzle,
        };
        view.record_completed();
        view
    }

    fn open_size(&mut self, size: Size) {
        if self.round.game.puzzle().size() != size {
            self.round = self.progress.open(size, self.progress.number(size));
        }
    }

    fn open_neighbor(&mut self, delta: isize) {
        let size = self.round.game.puzzle().size();
        if let Some(number) = self.round.number.checked_add_signed(delta)
            && number < size.puzzle_count()
        {
            self.round = self.progress.open(size, number);
        }
    }

    fn replay(&mut self) {
        self.round = Round::new(self.round.game.puzzle().size(), self.round.number);
    }

    fn record_completed(&self) {
        if self.round.game.is_solved() {
            self.session.record_result(
                self.round.attempt,
                self.round.game.puzzle().size().label(),
                Outcome::Won,
                ResultMetric::Moves(self.round.moves),
            );
        }
    }

    fn update_round(&mut self, effect: Option<Effect>, cx: &mut Context<Self>) {
        self.progress.remember(&self.round);
        self.session.save(&self.progress);
        if let Some(effect) = effect {
            if matches!(effect, Effect::Complete) {
                self.record_completed();
            }
            audio::play(cx, effect);
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        let key = stroke.key.as_str();
        let modifiers = stroke.modifiers;
        if !self.focus.is_focused(window) || modifiers.alt || modifiers.function {
            return;
        }
        let effect = if modifiers.control || modifiers.platform {
            if key != "z" || modifiers.shift || event.is_held {
                return;
            }
            self.round.undo()
        } else {
            let mut effect = None;
            match key {
                "up" => self.round.move_focus(-1, 0),
                "down" => self.round.move_focus(1, 0),
                "left" => self.round.move_focus(0, -1),
                "right" => self.round.move_focus(0, 1),
                "space" => self.round.cycle_blank(modifiers.shift),
                _ if event.is_held => return,
                "n" if !self.round.game.is_solved() => self.round.pencil = !self.round.pencil,
                "h" => self.round.hint(),
                "escape" if self.round.hint.is_some() => self.round.hint = None,
                "enter" if self.round.hint.is_some() => effect = self.round.apply_hint(),
                "backspace" | "delete" | "0" => effect = self.round.edit(0, false),
                _ => {
                    let Some(digit) = stroke
                        .key_char
                        .as_deref()
                        .and_then(|character| character.parse::<u8>().ok())
                        .or_else(|| key.parse().ok())
                        .filter(|digit| (1..=9).contains(digit))
                    else {
                        return;
                    };
                    effect = self.round.edit(digit, self.round.pencil);
                }
            }
            effect
        };
        cx.stop_propagation();
        self.update_round(effect, cx);
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_on_mount {
            self.focus_on_mount = false;
            cx.defer_in(window, |view, window, cx| view.focus.focus(window, cx));
        }

        let has_toolbar = true;
        let stage = Stage::measure(window, Layout::Rail, has_toolbar);
        let compact = crate::app::content_height(window) < 760.;
        let side = self.round.game.puzzle().size().side() as f32;
        let fit = stage.board_width.min(stage.board_height);
        let cell = ((fit - board::FRAME * 2. + board::GRID) / side - board::GRID)
            .floor()
            .clamp(32., if side == 6. { 104. } else { 76. });
        let result = self.round.game.is_solved().then(|| self.completion(cx));

        div()
            .id("kakuro-game")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .child(stage.render(
                cx,
                Some(self.toolbar(cx).into_any_element()),
                self.board(cell, cx),
                result,
                self.rail(compact, cx),
            ))
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use corner_table_api::GameId;
    use gpui::{Modifiers, TestAppContext, VisualTestContext, px, size};

    use super::*;

    #[gpui::test]
    fn completed_challenge_replays_the_same_board_as_a_fresh_attempt(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId(format!("kakuro-result-{}", std::process::id())),
            entry: GameEntry::Challenge("kakuro-replay".to_owned()),
        };
        let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
            let mut view = GameView::new(&launch, cx);
            while view.round.game.empty_count() > 1 {
                let hint = view.round.game.hint(view.round.selected).unwrap();
                view.round.select(hint.cell);
                view.round.edit(hint.digit, false);
            }
            view
        });
        cx.run_until_parked();
        let mut ui = VisualTestContext::from_window(window.into(), cx);
        ui.simulate_keystrokes("h enter");
        ui.run_until_parked();
        let (number, attempt) = window
            .read_with(&ui, |view, _| {
                let round = &view.round;
                assert!(round.game.is_solved());
                let pulse = round.pulse.unwrap();
                assert!(pulse.runs.iter().all(Option::is_some));
                assert_eq!(round.used_hint, Some(true));
                assert_eq!(round.moves as usize, round.game.puzzle().entries().len());
                (round.number, round.attempt)
            })
            .unwrap();

        assert!(ui.debug_bounds("kakuro-result-next").is_none());
        let replay = ui.debug_bounds("kakuro-replay").unwrap().center();
        ui.simulate_mouse_move(replay, None, Modifiers::default());
        ui.simulate_click(replay, Modifiers::default());
        window
            .read_with(&ui, |view, _| {
                let round = &view.round;
                assert_eq!(round.number, number);
                assert!(round.attempt != attempt);
                assert_eq!(
                    round.game.empty_count(),
                    round.game.puzzle().entries().len()
                );
                assert!(!round.game.can_undo());
            })
            .unwrap();
    }
}
