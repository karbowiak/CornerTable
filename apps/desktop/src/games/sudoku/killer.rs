mod board;
#[cfg(all(test, debug_assertions))]
mod tests;

use corner_table_sudoku::{Cage, CageIssue, Game, Hint, HintReason, KillerPuzzle, SIZE};
use gpui::{Context, Div, FontWeight, div, prelude::*, px, rgb};

use super::{ButtonStyle, GameView, Puzzle, best_result, control, hint_count, stage, t, unit_name};

pub(super) use board::Cell;

impl GameView {
    pub(super) fn killer_rail(
        &self,
        puzzle: KillerPuzzle,
        compact: bool,
        cx: &Context<Self>,
    ) -> Div {
        let position = KillerPuzzle::ALL
            .iter()
            .position(|&entry| entry == puzzle)
            .unwrap();
        let palette = t::colors(cx);
        let round = self.round();
        let game = &round.game;
        let solved = game.is_solved();
        let neighbors = [
            (
                "sudoku-killer-previous",
                "‹",
                "Previous Killer puzzle",
                position
                    .checked_sub(1)
                    .map(|index| KillerPuzzle::ALL[index]),
            ),
            (
                "sudoku-killer-next",
                "›",
                "Next Killer puzzle",
                KillerPuzzle::ALL.get(position + 1).copied(),
            ),
        ];
        let [previous, next] = neighbors.map(|(id, glyph, label, neighbor)| {
            control(
                id,
                glyph,
                ButtonStyle::enabled(neighbor.is_some()),
                cx,
                move |view| {
                    if let Some(puzzle) = neighbor {
                        view.open(Puzzle::Killer(puzzle));
                    }
                    None
                },
            )
            .aria_label(label)
            .w(px(28.))
            .px_0()
            .flex_shrink_0()
            .text_size(px(19.))
        });
        let picker = div()
            .flex()
            .items_center()
            .gap_1()
            .child(previous)
            .child(
                div()
                    .id("sudoku-killer-puzzle-name")
                    .role(gpui::Role::Label)
                    .aria_label(format!(
                        "{}, puzzle {} of {}",
                        puzzle.label(),
                        position + 1,
                        KillerPuzzle::ALL.len()
                    ))
                    .flex_1()
                    .text_center()
                    .text_size(px(13.))
                    .line_height(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .child(puzzle.label()),
            )
            .child(next);

        stage::rail(cx)
            .child(
                stage::rail_section(cx)
                    .child(stage::rail_heading(
                        cx,
                        "Killer Sudoku",
                        Some(
                            "Use 1–9 once per row, column and box. In each dashed cage, \
                             different digits must add to its label."
                                .into(),
                        ),
                    ))
                    .child(picker),
            )
            .when(!solved, |rail| {
                let section = if let Some(hint) = round.visible_hint() {
                    stage::rail_section(cx)
                        .child(self.hint_panel(hint, false, cx))
                        .child(hint_usage(round.hints_used, palette))
                } else {
                    stage::rail_section(cx)
                        .child(self.killer_status(palette))
                        .child(self.keypad(compact, cx))
                };
                rail.child(section.child(self.edit_actions(cx)))
                    .child(stage::rail_section(cx).child(self.reset_button(true, false, cx)))
            })
            .child(stage::controls_help(
                cx,
                &[
                    "Arrows move · 1–9 enter digits",
                    "Space finds a blank · Delete erases",
                    "N toggles notes · H shows a hint",
                ],
            ))
    }

    fn killer_status(&self, palette: t::Palette) -> Div {
        let round = self.round();
        let game = &round.game;
        let index = round.selected;
        let cage = game.cage(index).expect("every Killer square has a cage");
        let issue = game.cage_issue(index);
        let (filled, total) = cage.cells().iter().fold((0, 0), |(filled, sum), &cell| {
            let value = game.value(cell);
            (filled + usize::from(value != 0), sum + value)
        });
        let duplicate = game.has_conflict(index);
        let detail = if let Some(issue) = issue {
            issue_message(issue, cage.sum())
        } else if duplicate {
            "Repeated in a row, column or box.".to_owned()
        } else if filled == cage.cells().len() {
            "Total reached".to_owned()
        } else if filled != 0 {
            format!("{total} entered · {} left", cage.sum() - total)
        } else if cage.cells().len() == 1 {
            "1 square".to_owned()
        } else {
            format!("{} squares", cage.cells().len())
        };
        div()
            .px_1()
            .flex()
            .flex_col()
            .text_size(px(13.))
            .line_height(px(17.))
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(format!("Cage total {}", cage.sum())),
            )
            .child(
                div()
                    .text_color(rgb(if issue.is_some() || duplicate {
                        palette.danger
                    } else {
                        palette.muted
                    }))
                    .child(detail),
            )
            .when_some(round.best, |status, best| {
                status.child(best_result(best, palette))
            })
            .when(round.hints_used != 0, |status| {
                status.child(hint_usage(round.hints_used, palette))
            })
    }
}

fn hint_usage(hints: u32, palette: t::Palette) -> Div {
    div()
        .text_size(px(13.))
        .line_height(px(17.))
        .text_color(rgb(palette.muted))
        .child(format!("{} used", hint_count(hints).to_lowercase()))
}

fn issue_message(issue: CageIssue, target: u8) -> String {
    match issue {
        CageIssue::Repeated(digit) => format!("{digit} repeats in this cage."),
        CageIssue::OverTotal => format!("The sum is over {target}."),
        CageIssue::WrongTotal => format!("The digits must add to {target}."),
        CageIssue::ImpossibleTotal => format!("These entries leave no way to reach {target}."),
    }
}

pub(super) fn hint_message(hint: Hint, cage: &Cage, game: &Game) -> (String, String) {
    let total = cage.sum();
    let (title, detail) = match hint.reason {
        HintReason::CageConflict { issue } => (
            "Check this cage".to_owned(),
            format!(
                "{} Clear the highlighted entry.",
                issue_message(issue, total)
            ),
        ),
        HintReason::CageCandidate { digit } if cage.cells().len() == 1 => (
            "One-square cage".to_owned(),
            format!("This square must equal its cage total of {digit}."),
        ),
        HintReason::CageCandidate { digit } => {
            let filled = cage
                .cells()
                .iter()
                .filter(|&&index| game.value(index) != 0)
                .count();
            let detail = if filled + 1 == cage.cells().len() {
                format!(
                    "The other squares add to {}. Place {digit} to reach {total}.",
                    total - digit
                )
            } else {
                format!(
                    "Only {digit} here lets the shaded cage reach {total} with different digits under Sudoku rules."
                )
            };
            (format!("Only {digit} fits"), detail)
        }
        HintReason::OnlyCandidate { digit } => (
            format!("Only {digit} fits"),
            "The row, column and box already contain the other digits.".to_owned(),
        ),
        HintReason::OnlyPlace { digit, unit } => (
            format!("One place for {digit}"),
            format!(
                "The other squares in {} can’t take {digit} under the Sudoku and cage rules.",
                unit_name(unit)
            ),
        ),
        HintReason::LockedCandidate {
            digit,
            excluded,
            shared,
            ..
        } => (
            format!("{excluded} is locked"),
            format!(
                "The darker group can only place {excluded} where it meets {}. So {digit} is left here.",
                unit_name(shared)
            ),
        ),
        HintReason::Duplicate { digit, unit, .. } => (
            format!("{digit} is repeated"),
            format!(
                "Clear this entry to remove the duplicate in {}.",
                unit_name(unit)
            ),
        ),
        HintReason::IncorrectEntry { digit } => (
            "Revisit this entry".to_owned(),
            format!("Placing {digit} here prevents a valid solution. Clear it to continue."),
        ),
        HintReason::Forced { digit, candidates } => {
            let alternatives = (1..=SIZE as u8)
                .filter(|&other| other != digit && candidates & (1 << other) != 0)
                .map(|other| other.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            (
                format!("{digit} is forced"),
                format!(
                    "The other candidates ({alternatives}) each break a Sudoku or cage rule farther on."
                ),
            )
        }
    };
    (title, detail)
}
