use corner_table_kakuro::{Direction, Hint, HintKind, RunConflict, Size};
use gpui::{Context, Div, FontWeight, Role, SharedString, Stateful, div, prelude::*, px, rgb};

use super::GameView;
use crate::{audio::Effect, games::round_result, storage::Outcome, theme};

impl GameView {
    pub(super) fn toolbar(&self, width: f32, cx: &Context<Self>) -> Div {
        let size = self.round.game.puzzle().size();
        let number = self.round.number;
        let count = size.puzzle_count();
        let palette = theme::colors(cx);
        if self.fixed_puzzle {
            return div().font_weight(FontWeight::SEMIBOLD).child(size.label());
        }
        div()
            .w(px(width))
            .flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap_2()
            .child(div().flex().gap_1().children(Size::ALL.map(|option| {
                let id = match option {
                    Size::Compact => "kakuro-compact",
                    Size::Classic => "kakuro-classic",
                };
                control(id, option.label(), true, size == option, cx, move |view| {
                    view.open_size(option);
                    None
                })
                .aria_selected(size == option)
            })))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        control("kakuro-prev", "Previous", number != 0, false, cx, |view| {
                            view.open_neighbor(-1);
                            None
                        })
                        .aria_label("Previous puzzle"),
                    )
                    .child(
                        div()
                            .id("kakuro-puzzle-number")
                            .role(Role::Label)
                            .aria_label(format!("Puzzle {} of {count}", number + 1))
                            .w(px(59.))
                            .text_center()
                            .text_size(px(13.))
                            .text_color(rgb(palette.muted))
                            .child(format!("{:02} / {count}", number + 1)),
                    )
                    .child(
                        control(
                            "kakuro-next",
                            "Next",
                            number + 1 < count,
                            false,
                            cx,
                            |view| {
                                view.open_neighbor(1);
                                None
                            },
                        )
                        .aria_label("Next puzzle"),
                    ),
            )
    }

    pub(super) fn controls(&self, width: f32, stacked: bool, cx: &Context<Self>) -> Div {
        let round = &self.round;
        let game = &round.game;
        if game.is_solved() {
            return self.completion(width, cx);
        }
        let palette = theme::colors(cx);
        let empty = game.empty_count();
        let status = if empty == 1 {
            "1 square left".to_string()
        } else {
            format!("{empty} squares left")
        };
        let status_line = div()
            .flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap_x_2()
            .gap_y_1()
            .child(div().font_weight(FontWeight::BOLD).child(status))
            .when(game.has_conflicts(), |line| {
                line.child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(palette.danger))
                        .child("Check red clues"),
                )
            });
        let clues = div().flex().gap_2().children(
            [Direction::Across, Direction::Down]
                .map(|direction| self.clue_summary(direction, palette)),
        );
        let keypad = self.keypad(stacked, cx);
        let actions = self.actions(width < 224., cx);
        let mut panel = theme::panel(cx)
            .w(px(width))
            .flex_shrink_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_3();
        if stacked {
            panel = panel
                .child(keypad)
                .child(actions)
                .child(status_line)
                .child(clues);
        } else {
            panel = panel
                .child(status_line)
                .child(clues)
                .child(keypad)
                .child(actions);
        }
        panel
            .when_some(round.hint, |panel, hint| panel.child(self.hint_card(hint, cx)))
            .child(
                div()
                    .pt_2()
                    .border_t_1()
                    .border_color(rgb(palette.border))
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(rgb(palette.muted))
                    .child("Digits 1–9 must add to each run’s clue without repeats. Read upper-right clues across and lower-left clues down."),
            )
    }

    fn completion(&self, width: f32, cx: &Context<Self>) -> Div {
        let round = &self.round;
        let puzzle = round.game.puzzle();
        let title = match round.used_hint {
            Some(true) => "Solved with hints",
            Some(false) => "Clean solve",
            None => "Puzzle solved",
        };
        let moves = round.moves;
        let detail = format!(
            "{moves} {} · {} squares filled",
            if moves == 1 { "move" } else { "moves" },
            puzzle.entries().len(),
        );
        let has_next = !self.fixed_puzzle && round.number + 1 < puzzle.size().puzzle_count();
        let actions = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .when(has_next, |row| {
                row.child(control(
                    "kakuro-result-next",
                    "Next puzzle",
                    true,
                    true,
                    cx,
                    |view| {
                        view.open_neighbor(1);
                        None
                    },
                ))
            })
            .child(control(
                "kakuro-replay",
                "Replay puzzle",
                true,
                !has_next,
                cx,
                |view| {
                    view.replay();
                    None
                },
            ))
            .child(self.undo_button(cx));
        div().w(px(width)).flex_shrink_0().child(round_result(
            cx,
            "kakuro-result",
            Outcome::Won,
            title,
            detail,
            actions,
        ))
    }

    fn clue_summary(&self, direction: Direction, palette: theme::Palette) -> Div {
        let (index, run) = self.focused_run(direction);
        let game = &self.round.game;
        let sum: u16 = run
            .cells
            .iter()
            .map(|&cell| u16::from(game.value(cell)))
            .sum();
        let blanks = run
            .cells
            .iter()
            .filter(|&&cell| game.value(cell) == 0)
            .count();
        let conflict = game.run_conflict(index);
        let label = match direction {
            Direction::Across => "Across",
            Direction::Down => "Down",
        };
        let tally = match conflict {
            Some(RunConflict::RepeatedDigit) => "Repeated digit".to_string(),
            Some(RunConflict::UnreachableSum) => "Sum conflict".to_string(),
            None if blanks == 0 => "Total matched".to_string(),
            None => format!("{} left to add", u16::from(run.sum) - sum),
        };
        let ink = if conflict.is_some() {
            palette.danger
        } else {
            match direction {
                Direction::Across => palette.gold_text,
                Direction::Down => palette.success,
            }
        };
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(palette.paper))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(ink))
                    .child(div().text_size(px(12.)).child(label))
                    .child(div().text_size(px(19.)).child(run.sum.to_string())),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(rgb(if conflict.is_some() {
                        palette.danger
                    } else {
                        palette.muted
                    }))
                    .child(tally),
            )
    }

    fn keypad(&self, stacked: bool, cx: &Context<Self>) -> Div {
        const KEYS: [&str; 9] = [
            "kakuro-digit-1",
            "kakuro-digit-2",
            "kakuro-digit-3",
            "kakuro-digit-4",
            "kakuro-digit-5",
            "kakuro-digit-6",
            "kakuro-digit-7",
            "kakuro-digit-8",
            "kakuro-digit-9",
        ];
        let round = &self.round;
        let value = round.game.value(round.selected);
        let notes = round.game.notes(round.selected);
        let enabled = !round.game.is_solved() && (!round.pencil || value == 0);
        let across = if stacked { 9 } else { 3 };
        div()
            .flex()
            .flex_col()
            .gap(px(5.))
            .children(KEYS.chunks(across).enumerate().map(|(row, ids)| {
                div()
                    .flex()
                    .gap(px(5.))
                    .children(ids.iter().enumerate().map(|(column, id)| {
                        let digit = (row * across + column + 1) as u8;
                        let active = if round.pencil {
                            notes & (1 << digit) != 0
                        } else {
                            value == digit
                        };
                        control(id, digit.to_string(), enabled, active, cx, move |view| {
                            view.round.edit(digit, view.round.pencil)
                        })
                        .flex_1()
                        .min_w_0()
                        .px_0()
                        .text_size(px(if stacked { 16. } else { 20. }))
                    }))
            }))
    }

    fn actions(&self, slim: bool, cx: &Context<Self>) -> Div {
        let round = &self.round;
        let solved = round.game.is_solved();
        let can_erase = !solved
            && (round.game.value(round.selected) != 0 || round.game.notes(round.selected) != 0);
        let pencil = control(
            "kakuro-pencil",
            "Pencil",
            !solved,
            round.pencil,
            cx,
            |view| {
                view.round.pencil = !view.round.pencil;
                None
            },
        )
        .aria_description("Toggle pencil marks")
        .aria_keyshortcuts("N")
        .aria_toggled(if round.pencil {
            accesskit::Toggled::True
        } else {
            accesskit::Toggled::False
        });
        let erase = control("kakuro-erase", "Erase", can_erase, false, cx, |view| {
            view.round.edit(0, false)
        })
        .aria_keyshortcuts("Backspace");
        let undo = self.undo_button(cx);
        let hint = control(
            "kakuro-hint",
            "Hint",
            !solved,
            round.hint.is_some(),
            cx,
            |view| {
                view.round.hint();
                None
            },
        )
        .aria_keyshortcuts("H");
        let [pencil, erase, undo, hint] =
            [pencil, erase, undo, hint].map(|button| button.flex_1().min_w_0().px_1());
        if slim {
            div()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(div().flex().gap(px(5.)).child(pencil).child(erase))
                .child(div().flex().gap(px(5.)).child(undo).child(hint))
        } else {
            div()
                .flex()
                .gap(px(5.))
                .children([pencil, erase, undo, hint])
        }
    }

    fn undo_button(&self, cx: &Context<Self>) -> Stateful<Div> {
        control(
            "kakuro-undo",
            "Undo",
            self.round.game.can_undo(),
            false,
            cx,
            |view| view.round.undo(),
        )
    }

    fn hint_card(&self, hint: Hint, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let title = match hint.kind {
            HintKind::Clear => "Check this square",
            HintKind::Forced => "Only one digit fits",
            HintKind::Reveal => "One square revealed",
        };
        let message = match hint.kind {
            HintKind::Clear => "Clear this entry to get back to the solution.".to_string(),
            HintKind::Forced => format!(
                "{} is the only digit that fits both the {} across and {} down clues.",
                hint.digit,
                self.focused_run(Direction::Across).1.sum,
                self.focused_run(Direction::Down).1.sum,
            ),
            HintKind::Reveal => format!(
                "The solution uses {} in the highlighted square.",
                hint.digit
            ),
        };
        let action = if hint.digit == 0 {
            "Clear square".to_string()
        } else {
            format!("Place {}", hint.digit)
        };
        div()
            .bg(rgb(palette.hover))
            .rounded_md()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.ink))
                    .child(title),
            )
            .child(div().text_size(px(12.)).line_height(px(17.)).child(message))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        control("kakuro-apply", action, true, true, cx, |view| {
                            view.round.apply_hint()
                        })
                        .aria_keyshortcuts("Enter"),
                    )
                    .child(
                        control("kakuro-dismiss", "Dismiss", true, false, cx, |view| {
                            view.round.hint = None;
                            None
                        })
                        .aria_keyshortcuts("Escape"),
                    ),
            )
    }
}

fn control(
    id: &'static str,
    label: impl Into<SharedString>,
    enabled: bool,
    active: bool,
    cx: &Context<GameView>,
    action: impl Fn(&mut GameView) -> Option<Effect> + 'static,
) -> Stateful<Div> {
    let palette = theme::colors(cx);
    super::super::themed_control(cx, id, label, enabled)
        .debug_selector(move || id.into())
        .justify_center()
        .min_h(px(33.))
        .px_2()
        .when(active, |button| {
            button
                .bg(rgb(palette.hover))
                .border_color(rgb(palette.success))
        })
        .when(enabled, |button| {
            button.on_click(cx.listener(move |view, _, window, cx| {
                let effect = action(view);
                view.focus.focus(window, cx);
                view.update_round(effect, cx);
            }))
        })
}
