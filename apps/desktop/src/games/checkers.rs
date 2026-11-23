mod board;
mod table;

use std::time::Duration;

use corner_table_checkers::{Difficulty, Draw, Game, Outcome, Piece, Side, Square, Step};
use gpui::{
    AnyElement, Context, Div, FocusHandle, FontWeight, KeyDownEvent, Role, SharedString, Stateful,
    Task, Window, div, prelude::*, px, rgb,
};

use self::{
    board::{checker, side_name, square_name},
    table::{DIFFICULTIES, Mode, Table},
};
use crate::{audio, games::round_result, storage::Outcome as RecordOutcome, theme as t};

use super::GameLaunch;

pub struct GameView {
    table: Table,
    board_focus: FocusHandle,
    cursor: Square,
    selected: Option<Square>,
    arrival: Option<Step>,
    notice: Option<&'static str>,
    confirm_restart: bool,
    cancel_restart_focus: FocusHandle,
    computer: Option<Task<()>>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let table = Table::open(launch);
        Self {
            cursor: first_focus(table.game()),
            table,
            board_focus: cx.focus_handle().tab_stop(true),
            selected: None,
            arrival: None,
            notice: None,
            confirm_restart: false,
            cancel_restart_focus: cx.focus_handle().tab_stop(true),
            computer: None,
        }
    }

    fn player_turn(&self) -> bool {
        self.table.game().outcome() == Outcome::Playing
            && (self.table.mode == Mode::Local || self.table.game().side() == Side::Red)
    }

    fn selected_square(&self) -> Option<Square> {
        self.table.game().continuation().or(self.selected)
    }

    fn activate(&mut self, square: Square, keyboard: bool, cx: &mut Context<Self>) {
        if !self.player_turn() {
            return;
        }
        self.confirm_restart = false;
        self.notice = None;
        let legal = self.table.game().legal_steps();
        let selected = self.selected_square();
        if let Some(&step) = legal
            .iter()
            .find(|step| Some(step.from) == selected && step.to == square)
        {
            let effect = self.table.play(step).expect("a highlighted move is legal");
            self.arrival = Some(step);
            let game = self.table.game();
            self.selected = None;
            self.cursor = if keyboard { first_focus(game) } else { step.to };
            audio::play(
                cx,
                if game.outcome() != Outcome::Playing {
                    audio::Effect::Complete
                } else if effect.captured.is_some() || effect.crowned {
                    audio::Effect::Clear
                } else {
                    audio::Effect::Move
                },
            );
            self.ensure_computer(cx);
        } else if self.table.game().continuation().is_some() {
            self.notice = Some("Finish the jump with the same piece.");
        } else if selected == Some(square) {
            self.selected = None;
        } else if let Some(step) = legal.iter().find(|step| step.from == square) {
            self.selected = Some(square);
            if keyboard {
                self.cursor = step.to;
            }
        } else {
            self.notice = Some(if selected.is_some() {
                "Choose a marked landing square."
            } else if capture_required(&legal) {
                "A capture is required. Pick a marked piece."
            } else {
                "Pick one of the pieces marked with a gold dot."
            });
        }
        cx.notify();
    }

    fn ensure_computer(&mut self, cx: &Context<Self>) {
        if self.computer.is_some()
            || self.table.mode != Mode::Computer
            || self.table.game().side() != Side::Light
            || self.table.game().outcome() != Outcome::Playing
        {
            return;
        }

        let game = self.table.game().clone();
        let difficulty = self.table.difficulty;
        let executor = cx.background_executor();
        let pace = executor.timer(Duration::from_millis(240));
        let search = executor.spawn(async move { game.computer_turn(difficulty) });
        self.computer = Some(cx.spawn(async move |view, cx| {
            let turn = search.await;
            pace.await;
            let _ = view.update(cx, |view, cx| {
                view.computer = None;
                view.arrival = turn.last().copied();
                view.table.play_computer(turn, difficulty);
                let game = view.table.game();
                view.cursor = first_focus(game);
                if game.outcome() != Outcome::Playing {
                    view.confirm_restart = false;
                }
                cx.notify();
            });
        }));
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        match key.key.as_str() {
            "z" if (key.modifiers.platform || key.modifiers.control) && !key.modifiers.shift => {
                self.undo(cx);
            }
            _ if key.modifiers.platform || key.modifiers.control || key.modifiers.alt => return,
            "left" => self.move_cursor(0, -1),
            "right" => self.move_cursor(0, 1),
            "up" => self.move_cursor(-1, 0),
            "down" => self.move_cursor(1, 0),
            "enter" | "space" if !event.is_held => self.activate(self.cursor, true, cx),
            "u" if !event.is_held => self.undo(cx),
            "escape" => {
                if let Some(selected) = self.selected.take() {
                    self.cursor = selected;
                }
                self.notice = None;
                self.confirm_restart = false;
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn move_cursor(&mut self, rows: i8, files: i8) {
        let row = self.cursor.row().saturating_add_signed(rows).min(7);
        let file = (self.cursor.column() / 2)
            .saturating_add_signed(files)
            .min(3);
        self.cursor = Square::from_index(row * 4 + file).unwrap();
        self.notice = None;
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        self.computer = None;
        if self.table.undo() {
            self.reset_selection();
            cx.notify();
        }
    }

    fn reset_selection(&mut self) {
        self.cursor = first_focus(self.table.game());
        self.selected = None;
        self.arrival = None;
        self.notice = None;
        self.confirm_restart = false;
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        self.computer = None;
        self.table.restart();
        self.reset_selection();
        cx.notify();
    }

    fn switch_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.table.mode != mode {
            self.computer = None;
            self.table.mode = mode;
            self.reset_selection();
            self.table.save();
            self.ensure_computer(cx);
            cx.notify();
        }
    }

    fn change_difficulty(&mut self, difficulty: Difficulty, cx: &mut Context<Self>) {
        if self.table.difficulty != difficulty {
            self.computer = None;
            self.table.difficulty = difficulty;
            self.confirm_restart = false;
            self.table.save();
            self.ensure_computer(cx);
            cx.notify();
        }
    }

    fn status(&self, width: f32, legal: &[Step], cx: &Context<Self>) -> AnyElement {
        let game = self.table.game();
        if let Some((outcome, title, detail)) = result_copy(game, self.table.mode) {
            return round_result(
                cx,
                "checkers-result",
                outcome,
                title,
                detail,
                self.actions(cx),
            );
        }
        let colors = t::colors(cx);
        let (title, detail) = self.status_copy(legal);
        t::panel(cx)
            .id("checkers-status")
            .role(Role::Status)
            .aria_label(format!("{title}. {detail}"))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(t::display_type(title, if width < 400. { 19. } else { 22. }))
                    .child(
                        div()
                            .mt_1()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(rgb(colors.muted))
                            .child(detail),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap_1()
                    .children([Side::Red, Side::Light].map(|side| {
                        let (pieces, kings) = game.material(side);
                        side_count(side, pieces, kings, game.side() == side, colors)
                    })),
            )
            .into_any_element()
    }

    fn status_copy(&self, legal: &[Step]) -> (&'static str, &'static str) {
        let game = self.table.game();
        let title = match (self.table.mode, game.side()) {
            (Mode::Computer, Side::Red) => "Your turn",
            (Mode::Computer, Side::Light) => {
                return ("Computer’s turn", "Maple is choosing a move.");
            }
            (Mode::Local, Side::Red) => "Red to move",
            (Mode::Local, Side::Light) => "Maple to move",
        };
        let detail = self.notice.unwrap_or_else(|| {
            match (game.continuation(), self.selected, capture_required(legal)) {
                (Some(_), _, _) => "Jump again with the same piece.",
                (None, Some(_), true) => "Jump to a ringed square.",
                (None, Some(_), false) => "Move to a marked square.",
                (None, None, true) => "A capture is required. Pick a marked piece.",
                _ => "Pick a marked piece to move.",
            }
        });
        (title, detail)
    }

    fn actions(&self, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let game = self.table.game();
        let progress = !game.steps().is_empty();
        let row = div().flex().flex_wrap().items_center().gap_2();
        if self.confirm_restart {
            return row
                .child(div().mr_1().text_size(px(13.)).child("Restart this match?"))
                .child(
                    button("checkers-restart-confirm", "Restart match", true, true, cx).on_click(
                        cx.listener(|view, _, window, cx| {
                            view.restart(cx);
                            view.board_focus.focus(window, cx);
                        }),
                    ),
                )
                .child(
                    button("checkers-restart-cancel", "Keep playing", false, true, cx)
                        .track_focus(&self.cancel_restart_focus)
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.confirm_restart = false;
                            view.board_focus.focus(window, cx);
                            cx.notify();
                        })),
                );
        }
        let ended = game.outcome() != Outcome::Playing;
        let undo = button("checkers-undo", "Undo", false, progress, cx)
            .aria_description(if self.table.mode == Mode::Computer {
                "Undo your last turn and any computer reply."
            } else {
                "Undo the last turn."
            })
            .when(progress, |button| {
                button.on_click(cx.listener(|view, _, window, cx| {
                    view.undo(cx);
                    view.board_focus.focus(window, cx);
                }))
            });
        let restart = button(
            "checkers-restart",
            if ended { "Play again" } else { "Restart" },
            ended,
            progress,
            cx,
        )
        .when(progress, |button| {
            button.on_click(cx.listener(move |view, _, window, cx| {
                if ended {
                    view.restart(cx);
                    view.board_focus.focus(window, cx);
                } else {
                    view.confirm_restart = true;
                    cx.on_next_frame(window, |view, window, cx| {
                        if view.confirm_restart {
                            view.cancel_restart_focus.focus(window, cx);
                        }
                    });
                    cx.notify();
                }
            }))
        });
        row.children(if ended {
            [restart, undo]
        } else {
            [undo, restart]
        })
        .when_some(move_record(game.last_steps()), |row, path| {
            let lead = if game.continuation().is_some() {
                "Jump so far"
            } else {
                "Last move"
            };
            row.child(
                div()
                    .id("checkers-move-record")
                    .ml_auto()
                    .text_size(px(12.))
                    .text_color(rgb(colors.muted))
                    .child(format!("{lead}  {path}")),
            )
        })
    }

    fn match_panel(&self, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let computer = self.table.mode == Mode::Computer;
        t::panel(cx)
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(t::display_type("Match", 20.))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .children([
                        (Mode::Computer, "checkers-computer", "Computer"),
                        (Mode::Local, "checkers-local", "Two players"),
                    ].map(|(mode, id, label)| {
                        let active = mode == self.table.mode;
                        button(id, label, active, true, cx)
                            .flex_1()
                            .aria_toggled(active.into())
                            .on_click(cx.listener(move |view, _, _, cx| view.switch_mode(mode, cx)))
                    })),
            )
            .child(
                div()
                    .text_color(rgb(colors.muted))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .child(if computer {
                        "You play Red and move first."
                    } else {
                        "Share the board and take turns. Red opens."
                    }),
            )
            .when(computer, |panel| {
                panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("Difficulty"))
                        .child(div().flex().gap_1().children(DIFFICULTIES.into_iter().map(|difficulty| {
                            let (id, label, description) = difficulty_label(difficulty);
                            let active = self.table.difficulty == difficulty;
                            button(id, label, active, true, cx)
                                .flex_1()
                                .px_1()
                                .aria_toggled(active.into())
                                .aria_description(description)
                                .on_click(cx.listener(move |view, _, _, cx| view.change_difficulty(difficulty, cx)))
                        }))),
                )
            })
            .child(
                div()
                    .border_t_1()
                    .border_color(rgb(colors.border))
                    .pt_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(t::display_type("How to play", 17.))
                    .child(
                        rule_note(colors)
                            .child("Move one square diagonally forward. Take any available jump, then keep jumping with that piece while you can."),
                    )
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_2()
                            .child(checker(Piece { side: Side::Light, king: true }, 29.))
                            .child(
                                rule_note(colors)
                                    .flex_1()
                                    .min_w_0()
                                    .child("Reach the far edge to crown a king and end your turn. Kings move and jump both ways."),
                            ),
                    ),
            )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_computer(cx);
        let colors = t::colors(cx);
        let available = super::board_space(window) + 30.;
        let height = f32::from(window.viewport_size().height);
        let panel = if available < 690. { 244. } else { 268. };
        let side_by_side = available - panel - 24.;
        let wide = side_by_side >= 322.;
        let board_room = if wide { side_by_side } else { available };
        let side = board_room.min((height - 210.).max(322.)).min(630.).floor();
        let playing = self.table.game().outcome() == Outcome::Playing;
        let legal = if self.player_turn() {
            self.table.game().legal_steps()
        } else {
            Vec::new()
        };

        div()
            .w_full()
            .flex()
            .items_start()
            .gap_6()
            .justify_center()
            .when(!wide, |layout| layout.flex_col().items_center().gap_4())
            .child(
                div()
                    .w(px(side))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.status(side, &legal, cx))
                    .child(self.board(side, &legal, window, cx))
                    .when(playing, |column| {
                        column.child(div().mt_1().child(self.actions(cx)))
                    })
                    .child(
                        div()
                            .text_color(rgb(colors.muted))
                            .text_size(px(12.))
                            .line_height(px(17.))
                            .child(if playing {
                                "Arrow keys to move · Enter or Space to act · U to undo"
                            } else {
                                "Arrow keys to review · U to undo"
                            }),
                    ),
            )
            .child(
                div()
                    .w(px(if wide { panel } else { side }))
                    .flex_none()
                    .child(self.match_panel(cx)),
            )
    }
}

fn result_copy(game: &Game, mode: Mode) -> Option<(RecordOutcome, &'static str, String)> {
    let (outcome, title, reason) = match game.outcome() {
        Outcome::Playing => return None,
        Outcome::Won(side) => {
            let (outcome, title) = match (mode, side) {
                (Mode::Computer, Side::Red) => (RecordOutcome::Won, "You win"),
                (Mode::Computer, Side::Light) => (RecordOutcome::Lost, "You lose"),
                (Mode::Local, Side::Red) => (RecordOutcome::Finished, "Red wins"),
                (Mode::Local, Side::Light) => (RecordOutcome::Finished, "Maple wins"),
            };
            let loser = side_name(side.other());
            let reason = if game.material(side.other()).0 == 0 {
                format!("{loser} has no pieces left.")
            } else {
                format!("{loser} has no legal move.")
            };
            (outcome, title, reason)
        }
        Outcome::Draw(Draw::Repetition) => (
            RecordOutcome::Draw,
            "Draw",
            "The same position occurred three times.".into(),
        ),
        Outcome::Draw(Draw::NoProgress) => (
            RecordOutcome::Draw,
            "Draw",
            "40 turns per side passed without a capture or a regular piece advancing.".into(),
        ),
    };
    let (red, _) = game.material(Side::Red);
    let (maple, _) = game.material(Side::Light);
    Some((
        outcome,
        title,
        format!("{reason} Pieces left — Red {red}, Maple {maple}."),
    ))
}

fn capture_required(legal: &[Step]) -> bool {
    legal.first().is_some_and(|step| step.captured().is_some())
}

fn first_focus(game: &Game) -> Square {
    if let Some(step) = game.legal_steps().first() {
        if game.continuation().is_some() {
            step.to
        } else {
            step.from
        }
    } else {
        game.last_steps()
            .last()
            .map_or(Square::from_index(20).unwrap(), |step| step.to)
    }
}

fn move_record(steps: &[Step]) -> Option<String> {
    let first = steps.first()?;
    let mut path = square_name(first.from);
    for step in steps {
        path.push_str(if step.captured().is_some() {
            " × "
        } else {
            "–"
        });
        path.push_str(&square_name(step.to));
    }
    Some(path)
}

fn side_count(
    side: Side,
    pieces: usize,
    kings: usize,
    active: bool,
    colors: t::Palette,
) -> Stateful<Div> {
    let name = side_name(side);
    let piece_label = if pieces == 1 { "piece" } else { "pieces" };
    let king_label = if kings == 1 { "king" } else { "kings" };
    div()
        .id(("checkers-count", usize::from(side == Side::Light)))
        .role(Role::Group)
        .aria_label(format!(
            "{name}, {pieces} {piece_label}, {kings} {king_label}"
        ))
        .flex_none()
        .flex()
        .flex_col()
        .items_center()
        .rounded_md()
        .px_2()
        .py_1()
        .border_1()
        .border_color(rgb(if active {
            colors.border
        } else {
            colors.surface
        }))
        .bg(rgb(if active { colors.paper } else { colors.surface }))
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .text_size(px(15.))
                .line_height(px(18.))
                .font_weight(FontWeight::BOLD)
                .child(checker(Piece { side, king: false }, 22.))
                .child(pieces.to_string()),
        )
        .child(
            div()
                .text_color(rgb(colors.muted))
                .text_size(px(11.))
                .line_height(px(13.))
                .child(name),
        )
}

fn difficulty_label(difficulty: Difficulty) -> (&'static str, &'static str, &'static str) {
    match difficulty {
        Difficulty::Easy => ("checkers-easy", "Easy", "Plays one turn ahead."),
        Difficulty::Standard => (
            "checkers-standard",
            "Standard",
            "Looks ahead for replies and captures.",
        ),
        Difficulty::Expert => (
            "checkers-expert",
            "Expert",
            "Searches deeper for longer combinations.",
        ),
    }
}

fn rule_note(colors: t::Palette) -> Div {
    div()
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(rgb(colors.muted))
}

fn button(
    id: &'static str,
    label: &'static str,
    active: bool,
    enabled: bool,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    let colors = t::colors(cx);
    let label = SharedString::from(label);
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .min_h(px(34.))
        .px_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(if active {
            colors.gold_text
        } else {
            colors.border
        }))
        .bg(rgb(if active {
            colors.hover
        } else if enabled {
            colors.surface
        } else {
            colors.paper
        }))
        .text_color(rgb(if enabled { colors.ink } else { colors.muted }))
        .text_size(px(12.))
        .line_height(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |button| {
            button
                .role(Role::Button)
                .aria_label(label.clone())
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(colors.hover)))
                .focus_visible(|style| style.border_color(rgb(colors.ink)))
        })
        .child(label)
}

#[cfg(test)]
mod tests {
    use corner_table_api::GameId;
    use gpui::{KeyUpEvent, Keystroke, PlatformInput, TestAppContext, size};

    use super::*;
    use crate::games::GameEntry;

    #[gpui::test]
    fn local_result_shows_final_material_and_undo_restores_play(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId("checkers-result-test".into()),
            entry: GameEntry::Mode("local".into()),
        };
        let window = cx.open_window(size(px(812.), px(575.)), |window, cx| {
            cx.set_reduce_motion(true);
            let mut view = GameView::new(&launch, cx);
            while let Some(step) = view.table.game().legal_steps().first().copied() {
                view.table.play(step).unwrap();
            }
            let game = view.table.game();
            let Outcome::Won(winner) = game.outcome() else {
                panic!("expected the fixture to end by capture or blockade");
            };
            let (outcome, title, detail) = result_copy(game, Mode::Local).unwrap();
            assert_eq!(outcome, RecordOutcome::Finished);
            assert_eq!(title, format!("{} wins", side_name(winner)));
            assert!(detail.contains(&format!(
                "Red {}, Maple {}",
                game.material(Side::Red).0,
                game.material(Side::Light).0
            )));
            view.board_focus.focus(window, cx);
            view
        });
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| window.focus_prev(cx))
            .unwrap();
        let enter = Keystroke::parse("enter").unwrap();
        cx.dispatch_keystroke(window.into(), enter.clone());
        cx.update_window(window.into(), |_, window, cx| {
            window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: enter }), cx);
        })
        .unwrap();
        window
            .update(cx, |view, window, _| {
                assert_eq!(view.table.game().outcome(), Outcome::Playing);
                assert!(view.board_focus.is_focused(window));
                assert!(result_copy(view.table.game(), Mode::Local).is_none());
            })
            .unwrap();
    }
}
