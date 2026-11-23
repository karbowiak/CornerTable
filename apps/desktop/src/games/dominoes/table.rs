use corner_table_dominoes::{Action, End, Finish, Outcome as RoundOutcome, Phase, Player, Tile};
use gpui::{
    AnyElement, BoxShadow, Context, Div, FontWeight, Role, Stateful, Window, div,
    linear_color_stop, linear_gradient, prelude::*, px, rgb, rgba,
};

use crate::{
    games::round_result,
    storage::Outcome,
    theme::{self, Palette},
};

use super::{
    GameView,
    chain::{LEFT_TONE, RIGHT_TONE, end_tone},
    pieces::{self, BONE, Pose},
    state::Mode,
};

const WALNUT: u32 = 0x392827;
const LINEN: u32 = 0xd9c5ae;
const INK: u32 = 0x241f2c;

impl GameView {
    pub(super) fn round_result(&self, result: RoundOutcome, cx: &Context<Self>) -> AnyElement {
        let (outcome, title, detail) = result_copy(self.mode, result);
        let palette = theme::colors(cx);
        let actions = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                control("dominoes-next", "Next round", true, palette)
                    .on_click(cx.listener(|view, _, window, cx| view.restart(window, cx))),
            )
            .child(
                control("dominoes-review", "Review tiles", false, palette)
                    .aria_toggled(self.review_tiles.into())
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.review_tiles = !view.review_tiles;
                        cx.notify();
                    })),
            );
        round_result(cx, "dominoes-result", outcome, title, detail, actions)
    }

    pub(super) fn remaining_tiles(&self, compact: bool, cx: &Context<Self>) -> Stateful<Div> {
        let palette = theme::colors(cx);
        theme::panel(cx)
            .id("dominoes-final-hands")
            .role(Role::Group)
            .aria_label("Remaining tiles")
            .flex()
            .flex_wrap()
            .gap_4()
            .p_3()
            .children([Player::One, Player::Two].map(|player| {
                let hand = self.round().hand(player);
                let name = self.player_name(player);
                div()
                    .id(("dominoes-final-hand", player.index()))
                    .role(Role::Group)
                    .aria_label(name)
                    .flex_1()
                    .min_w(px(140.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().font_weight(FontWeight::BOLD).child(name))
                    .when(hand.is_empty(), |column| {
                        column.child(div().text_color(rgb(palette.muted)).child("No tiles left"))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(hand.iter().enumerate().map(|(index, tile)| {
                                let [a, b] = tile.pips();
                                div()
                                    .id(("dominoes-final-tile", index))
                                    .role(Role::Label)
                                    .aria_label(format!("{a} and {b}"))
                                    .child(pieces::ivory(
                                        [a, b],
                                        if compact { 25. } else { 31. },
                                        Pose::Vertical,
                                        false,
                                    ))
                            })),
                    )
            }))
    }

    pub(super) fn toolbar(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let finished = self.round().outcome().is_some();
        let confirm = self.confirm_restart;
        let modes = div()
            .id("dominoes-modes")
            .role(Role::Group)
            .aria_label("Play mode")
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(7.))
            .border_1()
            .border_color(rgb(palette.border))
            .children(Mode::ALL.map(|mode| {
                let selected = self.mode == mode;
                div()
                    .id(("dominoes-mode", mode.index()))
                    .role(Role::Button)
                    .aria_label(mode.label())
                    .aria_toggled(selected.into())
                    .tab_index(0)
                    .px_3()
                    .h(px(29.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(if selected { WALNUT } else { palette.paper }))
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(if selected { BONE } else { palette.ink }))
                    .bg(rgb(if selected { WALNUT } else { palette.paper }))
                    .cursor_pointer()
                    .hover(|item| item.bg(rgb(if selected { 0x594039 } else { palette.hover })))
                    .focus_visible(|item| item.border_color(rgb(palette.gold_text)))
                    .child(mode.label())
                    .on_click(
                        cx.listener(move |view, _, window, cx| view.switch_mode(mode, window, cx)),
                    )
            }));
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .child(modes)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .when(!finished, |row| {
                        row.child(
                            control(
                                "dominoes-restart",
                                if confirm {
                                    "Confirm restart"
                                } else {
                                    "Restart round"
                                },
                                confirm,
                                palette,
                            )
                            .on_click(cx.listener(|view, _, window, cx| view.restart(window, cx))),
                        )
                    })
                    .when(confirm, |row| {
                        row.child(
                            control("dominoes-cancel", "Cancel", false, palette).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.confirm_restart = false;
                                    cx.notify();
                                }),
                            ),
                        )
                    }),
            )
            .when(confirm, |row| {
                let prompt = "Replace this unfinished round? Your points from earlier rounds stay.";
                row.child(
                    div()
                        .id("dominoes-restart-prompt")
                        .role(Role::Status)
                        .aria_label(prompt)
                        .w_full()
                        .text_size(px(12.))
                        .text_color(rgb(palette.muted))
                        .child(prompt),
                )
            })
    }

    pub(super) fn table_surface(
        &self,
        width: f32,
        narrow: bool,
        short: bool,
        cx: &Context<Self>,
    ) -> Div {
        let round = self.round();
        let count = round.boneyard_count();
        let scores = self.table().scores();
        let current = round.outcome().is_none().then(|| round.player());
        let compact = narrow || short;
        div()
            .p(px(5.))
            .rounded(px(13.))
            .bg(linear_gradient(
                153.,
                linear_color_stop(rgb(0x624336), 0.),
                linear_color_stop(rgb(WALNUT), 1.),
            ))
            .shadow(vec![
                BoxShadow::new(px(0.), px(3.), rgba(0x20152158).into()).blur_radius(px(12.)),
                BoxShadow::new(px(0.), px(1.), rgba(0xd7a77e8a).into()).inset(),
            ])
            .text_color(rgb(BONE))
            .child(
                div()
                    .flex()
                    .items_center()
                    .px(px(if compact { 10. } else { 16. }))
                    .gap(px(if compact { 6. } else { 14. }))
                    .min_h(px(if short {
                        44.
                    } else if narrow {
                        48.
                    } else {
                        55.
                    }))
                    .child(score(
                        self.player_name(Player::One),
                        round.hand(Player::One).len(),
                        scores[0],
                        current == Some(Player::One),
                        false,
                    ))
                    .child(boneyard(count))
                    .child(score(
                        self.player_name(Player::Two),
                        round.hand(Player::Two).len(),
                        scores[1],
                        current == Some(Player::Two),
                        true,
                    )),
            )
            .child(
                div()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(rgb(0x171721))
                    .bg(linear_gradient(
                        164.,
                        linear_color_stop(rgb(0x303f54), 0.),
                        linear_color_stop(rgb(0x242b40), 1.),
                    ))
                    .shadow(vec![
                        BoxShadow::new(px(0.), px(2.), rgba(0x1214217d).into())
                            .blur_radius(px(6.))
                            .inset(),
                    ])
                    .child(self.chain(width - 12., narrow, short, cx))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .border_t_1()
                            .border_color(rgb(0x475268))
                            .px(px(if compact { 7. } else { 10. }))
                            .py(px(if short { 4. } else { 6. }))
                            .child(self.end_control(End::Left, short, cx))
                            .child(self.end_control(End::Right, short, cx)),
                    ),
            )
    }

    fn end_control(&self, end: End, short: bool, cx: &Context<Self>) -> Stateful<Div> {
        let tone = end_tone(end);
        let pip = self.round().ends()[end.index()];
        let enabled = self.valid_end(end);
        let name = match end {
            End::Left => "Left end",
            End::Right => "Right end",
        };
        let exposed = self
            .selected
            .filter(|_| enabled)
            .and_then(|tile| tile.other(pip));
        let detail = if let Some(next) = exposed {
            format!("{pip} → {next}")
        } else if pip == 0 {
            "Blank (0)".to_owned()
        } else {
            format!("{pip} {}", if pip == 1 { "pip" } else { "pips" })
        };
        let aria = if let Some(next) = exposed {
            format!("Play at {name}, matching {pip} and exposing {next}")
        } else {
            format!("{name}, {detail}")
        };
        div()
            .id(("dominoes-end", end.index()))
            .role(if enabled { Role::Button } else { Role::Label })
            .aria_label(aria)
            .min_w(px(112.))
            .min_h(px(if short { 38. } else { 40. }))
            .flex()
            .items_center()
            .justify_center()
            .when(end == End::Right, |button| button.flex_row_reverse())
            .gap(px(8.))
            .px(px(8.))
            .rounded(px(5.))
            .border_1()
            .border_color(rgb(if enabled { tone } else { 0x58657a }))
            .bg(rgb(if enabled { tone } else { 0x242a3c }))
            .text_color(rgb(if enabled { INK } else { BONE }))
            .child(
                div()
                    .size(px(27.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .bg(rgb(if enabled { 0xfcf0d9 } else { tone }))
                    .child(pieces::face(pip, 23., INK)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .when(end == End::Right, |label| label.items_end())
                    .line_height(px(15.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(if enabled { INK } else { 0xd0d9e5 }))
                            .child(detail),
                    ),
            )
            .when(enabled, |button| {
                button
                    .tab_index(0)
                    .cursor_pointer()
                    .hover(|button| button.bg(rgb(BONE)))
                    .active(|button| button.bg(rgb(0xf2dcb4)))
                    .focus_visible(|button| button.border_color(rgb(INK)))
                    .on_click(cx.listener(move |view, _, window, cx| view.place(end, window, cx)))
            })
    }

    pub(super) fn rack(&self, compact: bool, window: &Window, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let keyboard_focus = self.focus.is_focused(window);
        let headline = self.headline();
        let instruction = self.instruction();
        let last_action = self.last_action();
        let announcement = match &instruction {
            Some(instruction) => format!("{headline}. {instruction} {last_action}"),
            None => format!("{headline}. {last_action}"),
        };
        let status = div()
            .id("dominoes-status")
            .role(Role::Status)
            .aria_label(announcement)
            .flex_1()
            .min_w(px(if compact { 205. } else { 245. }))
            .min_h(px(if compact { 59. } else { 64. }))
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(theme::display_type(
                headline,
                if compact { 18. } else { 21. },
            ))
            .children(instruction.map(|instruction| {
                div()
                    .text_size(px(13.))
                    .line_height(px(17.))
                    .text_color(rgb(palette.ink))
                    .child(instruction)
            }))
            .child(
                div()
                    .text_color(rgb(palette.muted))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .child(last_action),
            );
        let rack = theme::panel(cx)
            .rounded(px(10.))
            .p(px(if compact { 10. } else { 13. }))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(status)
                    .child(self.turn_controls(palette, cx)),
            );

        if self.handoff() {
            return rack.child(
                div()
                    .flex()
                    .justify_center()
                    .gap(px(7.))
                    .py(px(6.))
                    .children((0..5).map(|_| pieces::back(23., 41.))),
            );
        }
        let unit = if compact { 28. } else { 34. };
        let player = if self.mode == Mode::Computer {
            Player::One
        } else {
            self.round().player()
        };
        let name = if self.mode == Mode::Computer {
            "Your hand".into()
        } else {
            format!("{}’s hand", self.player_name(player))
        };
        rack.child(
            div()
                .id("dominoes-rack")
                .role(Role::Group)
                .aria_label(name)
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(if compact { 4. } else { 6. }))
                .p(px(5.))
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(0x725546))
                .bg(linear_gradient(
                    143.,
                    linear_color_stop(rgb(0x48322d), 0.),
                    linear_color_stop(rgb(0x665040), 1.),
                ))
                .shadow(vec![
                    BoxShadow::new(px(0.), px(2.), rgba(0x1a100f85).into())
                        .blur_radius(px(5.))
                        .inset(),
                ])
                .children(
                    self.visible_hand()
                        .iter()
                        .enumerate()
                        .map(|(index, &tile)| {
                            self.rack_tile(tile, index, unit, keyboard_focus, cx)
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(12.))
                .text_color(rgb(palette.muted))
                .text_size(px(11.))
                .line_height(px(16.))
                .children([
                    "← → Browse",
                    "Enter Select",
                    "L / R Play at end",
                    "D Draw",
                    "P Pass",
                ]),
        )
    }

    fn turn_controls(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let buttons = div().flex().flex_wrap().items_center().gap_2();
        if self.handoff() {
            return buttons.child(
                control("dominoes-reveal", "Reveal hand", true, palette)
                    .on_click(cx.listener(|view, _, window, cx| view.reveal(window, cx))),
            );
        }
        if !self.human_turn() {
            return buttons;
        }
        let action = match self.round().phase() {
            Phase::Draw => Some(("dominoes-draw", "Draw tiles", Action::Draw, "D")),
            Phase::Pass => Some(("dominoes-pass", "Pass turn", Action::Pass, "P")),
            _ => None,
        };
        buttons
            .children(action.map(|(id, label, action, key)| {
                control(id, label, true, palette)
                    .child(
                        div()
                            .ml_2()
                            .text_size(px(11.))
                            .text_color(rgb(LINEN))
                            .child(key),
                    )
                    .on_click(
                        cx.listener(move |view, _, window, cx| {
                            view.turn_action(action, window, cx)
                        }),
                    )
            }))
            .when(self.mode == Mode::HotSeat, |buttons| {
                buttons.child(
                    control("dominoes-hide", "Hide hand", false, palette)
                        .on_click(cx.listener(|view, _, window, cx| view.hide(window, cx))),
                )
            })
    }

    fn rack_tile(
        &self,
        tile: Tile,
        index: usize,
        unit: f32,
        focused: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let enabled = self.human_turn();
        let selected = self.selected == Some(tile);
        let cursor = focused && enabled && index == self.cursor;
        let [a, b] = tile.pips();
        let fits = self.round().fits(tile);
        let quiet = enabled && !fits.contains(&true) && !selected;
        let description = if enabled {
            match fits {
                [true, true] => "Matches either end",
                [true, false] => "Matches the left end",
                [false, true] => "Matches the right end",
                [false, false] => "No matching end",
            }
        } else {
            ""
        };
        div()
            .id(("dominoes-rack-tile", usize::from(a) * 7 + usize::from(b)))
            .role(if enabled { Role::Button } else { Role::Label })
            .aria_label(format!("{a} and {b}"))
            .aria_description(description)
            .rounded(px(7.))
            .border_1()
            .border_color(if selected {
                rgb(LEFT_TONE)
            } else if cursor {
                rgb(RIGHT_TONE)
            } else {
                rgba(0x00000000)
            })
            .p(px(3.))
            .when(enabled, |slot| {
                slot.aria_toggled(selected.into())
                    .cursor_pointer()
                    .hover(|tile| tile.bg(rgb(0x826449)))
                    .on_click(
                        cx.listener(move |view, _, window, cx| {
                            view.activate_tile(index, window, cx)
                        }),
                    )
            })
            .child(pieces::ivory(tile.pips(), unit, Pose::Vertical, quiet))
    }
}

fn result_copy(mode: Mode, result: RoundOutcome) -> (Outcome, &'static str, String) {
    let (outcome, title) = match (mode, result.winner) {
        (_, None) => (Outcome::Draw, "Draw"),
        (Mode::Computer, Some(Player::One)) => (Outcome::Won, "You win the round"),
        (Mode::Computer, Some(Player::Two)) => (Outcome::Lost, "You lose the round"),
        (Mode::HotSeat, Some(Player::One)) => (Outcome::Finished, "Player 1 wins the round"),
        (Mode::HotSeat, Some(Player::Two)) => (Outcome::Finished, "Player 2 wins the round"),
    };
    let [first, second] = result.pips;
    let one = mode.player_name(Player::One);
    let two = mode.player_name(Player::Two);
    let points = result.points;
    let unit = if points == 1 { "point" } else { "points" };
    let award = match result.winner {
        Some(Player::One) if mode == Mode::Computer => format!("You earn {points} {unit}."),
        Some(winner) => format!("{} earns {points} {unit}.", mode.player_name(winner)),
        None => "No points awarded.".into(),
    };
    let reason = match result.finish {
        Finish::WentOut => "Last tile played.",
        Finish::Blocked => "Both players passed.",
    };
    (
        outcome,
        title,
        format!("Pips left — {one} {first}, {two} {second}. {reason} {award}"),
    )
}

fn score(
    label: &'static str,
    count: usize,
    score: u64,
    active: bool,
    right: bool,
) -> Stateful<Div> {
    let tiles = if count == 1 { "tile" } else { "tiles" };
    div()
        .id(if right {
            "dominoes-score-two"
        } else {
            "dominoes-score-one"
        })
        .role(Role::Label)
        .aria_label(format!(
            "{label}, {count} {tiles}, {score} points{}",
            if active { ", current turn" } else { "" }
        ))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .when(right, |player| player.items_end())
        .gap(px(2.))
        .child(
            div()
                .flex()
                .items_center()
                .when(right, |player| player.flex_row_reverse())
                .gap(px(6.))
                .text_size(px(14.))
                .line_height(px(17.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(if active { LEFT_TONE } else { BONE }))
                .child(label)
                .when(active, |player| {
                    player.child(div().size(px(5.)).rounded_full().bg(rgb(LEFT_TONE)))
                }),
        )
        .child(
            div()
                .text_size(px(11.))
                .line_height(px(15.))
                .text_color(rgb(LINEN))
                .child(format!("{count} {tiles} · {score} pts")),
        )
}

fn boneyard(count: usize) -> Stateful<Div> {
    let label = if count == 0 {
        "Empty".to_owned()
    } else {
        format!("{count} left")
    };
    div()
        .id("dominoes-boneyard")
        .role(Role::Label)
        .aria_label(format!("Boneyard, {count} tiles left"))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(7.))
        .child(
            div()
                .relative()
                .w(px(28.))
                .h(px(25.))
                .children((0..count.min(3)).map(|i| {
                    div()
                        .absolute()
                        .left(px(i as f32 * 6.))
                        .top(px(0.))
                        .child(pieces::back(15., 25.))
                }))
                .when(count == 0, |pile| {
                    pile.child(
                        div()
                            .mx_auto()
                            .w(px(15.))
                            .h_full()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(rgb(0x917662)),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .line_height(px(15.))
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Boneyard"),
                )
                .child(div().text_size(px(11.)).text_color(rgb(LINEN)).child(label)),
        )
}

fn control(
    id: &'static str,
    label: &'static str,
    primary: bool,
    palette: Palette,
) -> Stateful<Div> {
    let palette = if primary {
        Palette {
            border: WALNUT,
            surface: WALNUT,
            ink: BONE,
            hover: 0x5c4036,
            ..palette
        }
    } else {
        palette
    };
    super::super::styled_control(palette, id, label, true)
        .min_h(px(34.))
        .when(primary, |button| {
            button.active(|button| button.bg(rgb(0x2e2425)))
        })
}

#[cfg(test)]
mod tests {
    use corner_table_api::GameId;
    use corner_table_dominoes::computer_action;
    use gpui::{KeyUpEvent, Keystroke, PlatformInput, TestAppContext, size};

    use super::*;
    use crate::games::{GameEntry, GameLaunch, dominoes::state::Table};

    #[test]
    fn primary_action_builds_without_conflicting_interaction_styles() {
        let _ = control("test-action", "Draw tiles", true, theme::LIGHT);
    }

    #[gpui::test]
    fn hot_seat_can_review_only_final_tiles_and_advance_the_scored_round(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId("dominoes-result-test".into()),
            entry: GameEntry::Mode("hot-seat".into()),
        };
        let window = cx.open_window(size(px(812.), px(575.)), |window, cx| {
            cx.set_reduce_motion(true);
            let mut view = GameView::new(&launch, cx);
            view.tables[Mode::HotSeat.index()] = Table::new(35);
            while let Some(action) = computer_action(view.round()) {
                view.apply(action, cx);
            }
            view.focus.focus(window, cx);
            view
        });
        let (attempt, scores) = window
            .read_with(cx, |view, _| {
                let result = view.round().outcome().unwrap();
                let winner = result.winner.unwrap();
                let (outcome, title, detail) = result_copy(Mode::HotSeat, result);
                assert_eq!(outcome, Outcome::Finished);
                assert_eq!(
                    title,
                    format!("Player {} wins the round", winner.index() + 1)
                );
                assert!(detail.contains(&format!(
                    "Player 1 {}, Player 2 {}",
                    result.pips[0], result.pips[1]
                )));
                assert_eq!(
                    view.table().scores()[winner.index()],
                    u64::from(result.points)
                );
                assert!(view.visible_hand().is_empty());
                (view.table().attempt(), view.table().scores())
            })
            .unwrap();
        let activate_focused = |cx: &mut TestAppContext| {
            let enter = Keystroke::parse("enter").unwrap();
            cx.dispatch_keystroke(window.into(), enter.clone());
            cx.update_window(window.into(), |_, window, cx| {
                window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke: enter }), cx);
            })
            .unwrap();
            cx.run_until_parked();
        };
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| {
            for _ in 0..4 {
                window.focus_next(cx);
            }
        })
        .unwrap();
        activate_focused(cx);
        window
            .read_with(cx, |view, _| {
                assert!(view.review_tiles);
                assert!(view.table().attempt() == attempt);
                assert_eq!(view.table().scores(), scores);
            })
            .unwrap();

        cx.update_window(window.into(), |_, window, cx| window.focus_prev(cx))
            .unwrap();
        activate_focused(cx);
        window
            .update(cx, |view, window, _| {
                assert!(view.table().attempt() != attempt);
                assert_eq!(view.table().scores(), scores);
                assert!(view.handoff());
                assert!(!view.review_tiles);
                assert!(view.focus.is_focused(window));
            })
            .unwrap();
    }
}
