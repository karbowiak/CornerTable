mod arena;
mod cards;
mod copy;
mod hud;

use corner_table_hearts::{Phase, Seat};
use gpui::{
    Context, Div, FontWeight, ObjectFit, Role, Stateful, Toggled, Window, div, img,
    linear_color_stop, linear_gradient, prelude::*, px, rgb, rgba,
};

use crate::theme;

use super::GameView;
use cards::{Face, card_name, face};

const INK: u32 = 0xfff3e5;
const MUTED: u32 = 0xe6c6c8;
const BRASS: u32 = 0xf5cb88;
const BRASS_HOVER: u32 = 0xffdc9f;
const RIM: u32 = 0x977165;
const RIM_SOFT: u32 = 0xe5bb9c45;
const CARD_BAY: u32 = 0x270f1acd;
const CHIP: u32 = 0x3b1e2b;
const DECK_RATIO: f32 = 1.40;

#[derive(Clone, Copy)]
struct Layout {
    inset: f32,
    compact: bool,
    short: bool,
    hand_cols: usize,
    card_width: f32,
    card_gap: f32,
    mat_width: f32,
    mat_height: f32,
    mat_card: f32,
}

impl Layout {
    fn new(window: &Window) -> Self {
        let width = crate::games::board_space(window) + 36.;
        let compact = width < 700.;
        let short = window.viewport_size().height < px(690.);
        let inset = if compact || short { 13. } else { 22. };
        let inner = width - 2. * inset - 4.;
        let card_gap = if compact { 6. } else { 8. };
        let hand_cols = if inner < 13. * 44. + 12. * card_gap {
            7
        } else {
            13
        };
        let card_max = match (hand_cols, short) {
            (7, true) => 46.,
            (7, false) => 52.,
            (_, true) => 61.,
            _ => 69.,
        };
        let card_width = ((inner - (hand_cols - 1) as f32 * card_gap) / hand_cols as f32)
            .floor()
            .clamp(42., card_max);
        Self {
            inset,
            compact,
            short,
            hand_cols,
            card_width,
            card_gap,
            mat_width: if compact { 278. } else { 348. },
            mat_height: if short {
                124.
            } else if compact {
                222.
            } else {
                261.
            },
            mat_card: if short {
                45.
            } else if compact {
                50.
            } else {
                62.
            },
        }
    }

    fn trick_card(self, seat: Seat) -> (f32, f32) {
        if self.short {
            let edge = (self.mat_width - 4. * self.mat_card) / 5.;
            let step = self.mat_card + edge;
            return match seat {
                Seat::West => (edge, 21.),
                Seat::North => (edge + step, 4.),
                Seat::East => (edge + step * 3., 21.),
                Seat::You => (edge + step * 2., 36.),
            };
        }
        let x = (self.mat_width - self.mat_card) * 0.5;
        let middle = (self.mat_height - self.mat_card * DECK_RATIO - 18.) * 0.5;
        match seat {
            Seat::North => (x, 4.),
            Seat::West => (x - self.mat_card * 1.37, middle),
            Seat::East => (x + self.mat_card * 1.37, middle),
            Seat::You => (x, middle * 2. - 4.),
        }
    }
}

impl GameView {
    pub(super) fn render_table(&self, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let layout = Layout::new(window);
        let game = self.session.game();
        let result = match game.phase() {
            Phase::HandComplete(result) => Some(result),
            _ => None,
        };
        let dark = theme::colors(cx).paper != theme::PAPER;
        let felt = if dark {
            [0x4a2a3d, 0x291726]
        } else {
            [0x612e43, 0x3e1c30]
        };
        div()
            .id("hearts-table")
            .track_focus(&self.focus)
            .tab_index(0)
            .role(Role::Group)
            .aria_label("Hearts table")
            .aria_description(match result {
                Some(result) if result.winner.is_some() => "Return starts a new match. The scorecard shows the final totals.",
                Some(_) => "Return deals the next hand. The scorecard shows the hand and match totals.",
                None => "Left and right arrows choose a card. Space selects or plays it. Return passes selected cards or continues.",
            })
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .min_w_0()
            .rounded_xl()
            .border_1()
            .border_color(rgba(0x00000000))
            .focus_visible(|style| style.border_color(rgb(theme::colors(cx).gold_text)))
            .flex()
            .flex_col()
            .gap_3()
            .child(self.toolbar(cx))
            .when(self.show_help, |root| root.child(self.rules(cx)))
            .when(self.confirm_new, |root| root.child(self.confirmation(cx)))
            .when_some(result, |root, result| root.child(self.hand_result(result, cx)))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .w_full()
                    .overflow_hidden()
                    .rounded_xl()
                    .border_1()
                    .border_color(rgb(RIM))
                    .bg(linear_gradient(
                        157.,
                        linear_color_stop(rgb(felt[0]), 0.),
                        linear_color_stop(rgb(felt[1]), 1.),
                    ))
                    .text_color(rgb(INK))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .opacity(if dark { 0.77 } else { 0.64 })
                            .child(
                                img("games/hearts/felt.jpg")
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset(px(6.))
                            .rounded_lg()
                            .border_1()
                            .border_color(rgba(RIM_SOFT)),
                    )
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_col()
                            .p(px(layout.inset))
                            .gap(px(if layout.compact || layout.short { 9. } else { 15. }))
                            .child(match result {
                                Some(result) => self.score_sheet(result, layout),
                                None => self.arena(layout, cx),
                            })
                            .when(result.is_none(), |felt| felt.child(self.phase_bar(cx)))
                            .when(!game.hand(Seat::You).is_empty(), |felt| {
                                felt.child(self.player_hand(layout, window, cx))
                            }),
                    ),
            )
    }

    fn player_hand(&self, layout: Layout, window: &Window, cx: &Context<Self>) -> Div {
        let game = self.session.game();
        let hand = game.hand(Seat::You);
        let selected = self.session.selected();
        let passing = game.phase() == Phase::Passing;
        let legal = self.keyboard_cards();
        let can_play = !passing && !legal.is_empty();
        let incoming = game.received().filter(|_| self.session.ready_to_open());
        let cursor = self
            .focus
            .is_focused(window)
            .then(|| self.cursor_card())
            .flatten();
        let columns = if hand.len() > layout.hand_cols {
            hand.len().div_ceil(2)
        } else {
            layout.hand_cols
        };
        let cards = hand.chunks(columns).map(|row| {
            div()
                .flex()
                .justify_center()
                .gap(px(layout.card_gap))
                .children(row.iter().copied().map(|card| {
                    let picked = selected.contains(&card);
                    let enabled = !self.confirm_new
                        && (passing && (picked || selected.len() < 3)
                            || can_play && legal.contains(&card));
                    let received = incoming.is_some_and(|cards| cards.contains(&card));
                    let appearance = if picked || received {
                        Face::Selected
                    } else if (can_play || passing) && !enabled {
                        Face::Dimmed
                    } else {
                        Face::Normal
                    };
                    let title = if passing {
                        if picked {
                            format!("{} selected to pass", card_name(card))
                        } else {
                            format!("Select {} to pass", card_name(card))
                        }
                    } else if received {
                        format!("{} received in the pass", card_name(card))
                    } else if enabled {
                        format!("Play {}", card_name(card))
                    } else {
                        card_name(card)
                    };
                    div()
                        .id(("hearts-hand-card", usize::from(card.id())))
                        .accessibility_id(format!("hearts-hand-card-{}", card.id()))
                        .aria_label(title)
                        .role(if enabled { Role::Button } else { Role::Group })
                        .relative()
                        .flex_none()
                        .rounded_md()
                        .border_2()
                        .border_color(if cursor == Some(card) {
                            rgb(BRASS)
                        } else {
                            rgba(0x00000000)
                        })
                        .when(passing, |card| {
                            card.aria_toggled(if picked {
                                Toggled::True
                            } else {
                                Toggled::False
                            })
                        })
                        .when(enabled, |element| {
                            element
                                .tab_index(0)
                                .cursor_pointer()
                                .hover(|style| style.border_color(rgb(BRASS_HOVER)))
                                .focus_visible(|style| style.border_color(rgb(BRASS_HOVER)))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.choose_card(card, cx);
                                    view.focus.focus(window, cx);
                                }))
                        })
                        .child(face(card, layout.card_width - 4., appearance))
                        .when(picked || received, |card| {
                            card.child(
                                div()
                                    .absolute()
                                    .right(px(2.))
                                    .bottom(px(2.))
                                    .rounded_sm()
                                    .px_1()
                                    .bg(rgb(BRASS))
                                    .text_color(rgb(CHIP))
                                    .font_weight(FontWeight::BOLD)
                                    .line_height(px(13.))
                                    .text_size(px(9.))
                                    .child(if picked { "Pass" } else { "New" }),
                            )
                        })
                }))
        });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .text_size(px(13.))
                    .line_height(px(17.))
                    .child(div().font_weight(FontWeight::BOLD).child("Your hand"))
                    .child(div().text_color(rgb(MUTED)).child(format!(
                        "Hand +{} · Total {}",
                        game.hand_points()[0],
                        game.scores()[0],
                    ))),
            )
            .children(cards)
            .child(
                div()
                    .text_size(px(11.))
                    .line_height(px(15.))
                    .text_color(rgb(MUTED))
                    .child(if passing {
                        "← → Choose card   ·   Space Select   ·   Return Pass"
                    } else {
                        "← → Choose legal card   ·   Space Play   ·   Return Continue"
                    }),
            )
    }
}
