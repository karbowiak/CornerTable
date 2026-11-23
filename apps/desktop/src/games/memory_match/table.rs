use std::sync::Arc;

use corner_table_memory_match::{Card, Layout, Turn};
use gpui::{
    Animation, AnimationExt, AnyElement, BoxShadow, Context, Div, FontWeight, ObjectFit, Render,
    RenderImage, Role, Stateful, StyledImage as _, Window, div, img, prelude::*, px, rgb, rgba,
};

use super::{FLIP_TIME, GameEntry, GameView, Notice, art};
use crate::{
    games::{
        round_result,
        stage::{self, Layout as StageLayout, Stage, Stat},
        themed_choice, themed_control,
    },
    motion,
    storage::Outcome,
    theme,
};

const CARD_BACK: u32 = 0x173f45;
const GOLD: u32 = 0xffd99b;
const CARD_ASPECT: f32 = 1.125;

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stage = Stage::measure(window, StageLayout::Rail, true);
        let (columns, rows) = self.layout.dimensions();
        let gap = if stage.board_height < 460. { 6. } else { 12. };
        let width_limit = (stage.board_width - gap * (columns - 1) as f32) / columns as f32;
        let height_limit =
            (stage.board_height - gap * (rows - 1) as f32) / (rows as f32 * CARD_ASPECT);
        let size = width_limit.min(height_limit).clamp(56., 160.).floor();
        let complete = self.board().is_complete();

        div()
            .id("memory-match")
            .w_full()
            .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && view.confirm_new_deal {
                    view.cancel_new_deal(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(stage.render(
                cx,
                Some(self.toolbar(cx).into_any_element()),
                self.grid(size, gap, window, cx),
                complete.then(|| self.completion(cx)),
                self.rail(cx),
            ))
    }
}

impl GameView {
    fn toolbar(&self, cx: &Context<Self>) -> Stateful<Div> {
        div()
            .id("memory-match-layouts")
            .role(Role::Group)
            .aria_label("Board size")
            .flex()
            .gap_1()
            .children(Layout::ALL.map(|layout| {
                let selected = self.layout == layout;
                themed_choice(
                    cx,
                    layout.id(),
                    format!("{} cards", layout.card_count()),
                    selected,
                )
                .aria_label(format!(
                    "{}, {} cards, {} pairs",
                    layout.name(),
                    layout.card_count(),
                    layout.pair_count(),
                ))
                .aria_toggled(selected.into())
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.select_layout(layout, cx);
                }))
            }))
    }

    fn rail(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let complete = self.board().is_complete();
        let confirm = self.confirm_new_deal.then(|| {
            let message = if matches!(self.launch.entry, GameEntry::Challenge(_)) {
                "Restart this deal and discard the current progress?"
            } else {
                "Deal new cards and discard this board’s progress?"
            };
            div()
                .id("memory-match-confirm")
                .role(Role::Status)
                .aria_label(message)
                .a11y_synthetic_children(|builder| {
                    builder.parent_node().set_live(accesskit::Live::Polite);
                })
                .text_size(px(13.))
                .line_height(px(19.))
                .text_color(rgb(palette.ink))
                .child(message)
        });
        stage::rail(cx)
            .child(self.counter(cx))
            .when(!complete, |rail| {
                rail.child(self.status_section(cx)).child(
                    stage::rail_section(cx).children(confirm).child(
                        stage::actions().child(self.deal_button(cx)).children(
                            self.confirm_new_deal.then(|| {
                                themed_control(cx, "memory-match-cancel", "Cancel", true).on_click(
                                    cx.listener(|view, _, window, cx| {
                                        view.cancel_new_deal(window, cx);
                                    }),
                                )
                            }),
                        ),
                    ),
                )
            })
            .child(stage::controls_help(
                cx,
                if self.hold_mismatches {
                    &[
                        "Click a card to turn it over",
                        "Tab to the board · Arrow keys move",
                        "Enter or Space turns a card · H hides a mismatch",
                    ]
                } else {
                    &[
                        "Click a card to turn it over",
                        "Tab to the board · Arrow keys move",
                        "Enter or Space turns a card",
                    ]
                },
            ))
    }

    fn deal_button(&self, cx: &Context<Self>) -> Stateful<Div> {
        let label = if matches!(self.launch.entry, GameEntry::Challenge(_)) {
            if self.confirm_new_deal {
                "Restart this deal"
            } else {
                "Replay deal"
            }
        } else if self.confirm_new_deal {
            "Deal new cards"
        } else {
            "New deal"
        };
        themed_control(cx, "memory-match-new", label, true)
            .debug_selector(|| "memory-match-new".into())
            .min_h(px(34.))
            .on_click(cx.listener(|view, _, window, cx| view.request_new_deal(window, cx)))
    }

    fn completion(&self, cx: &Context<Self>) -> AnyElement {
        let attempts = self.board().attempts();
        let pairs = self.board().pairs();
        let title = if attempts == pairs as u64 {
            "Perfect deal"
        } else if self.round().best == Some(attempts) {
            "Personal best"
        } else {
            "All pairs found"
        };
        let mut detail = format!("{pairs} pairs in {}", attempt_label(attempts));
        if let Some(best) = self.round().best.filter(|&best| best < attempts) {
            detail.push_str(&format!(" · Personal best {}", attempt_label(best)));
        }
        round_result(
            cx,
            "memory-match-result",
            Outcome::Won,
            title,
            detail,
            div().flex().child(self.deal_button(cx)),
        )
    }

    fn cancel_new_deal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_new_deal = false;
        self.focus[self.cursor].focus(window, cx);
        cx.notify();
    }

    fn counter(&self, cx: &Context<Self>) -> Div {
        let pair_count = self.layout.pair_count();
        let pairs = self.board().pairs();
        let attempts = self.board().attempts();
        let best = self.round().best;
        stage::rail_section(cx)
            .child(stage::rail_heading(
                cx,
                "Tidepool deck",
                Some(self.layout.name().into()),
            ))
            .child(
                div()
                    .id("memory-match-score")
                    .role(Role::Label)
                    .aria_label(format!(
                        "{pairs} of {pair_count} pairs found, {}. {}",
                        attempt_label(attempts),
                        best.map_or_else(
                            || "No personal best yet.".into(),
                            |best| format!("Personal best {best} attempts.")
                        ),
                    ))
                    .child(stage::stat_grid(
                        cx,
                        [
                            Stat::new(
                                "memory-match-pairs",
                                "Pairs",
                                format!("{pairs}/{pair_count}"),
                            ),
                            Stat::new("memory-match-attempts", "Attempts", attempts.to_string()),
                            Stat::new(
                                "memory-match-best",
                                "Best",
                                best.map_or_else(|| "—".into(), |best| best.to_string()),
                            ),
                        ],
                    )),
            )
    }

    fn grid(&self, size: f32, gap: f32, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let (columns, rows) = self.layout.dimensions();
        let height = (size * CARD_ASPECT).floor();
        let focused = self.focused_card(window);
        let tab = focused.unwrap_or(self.cursor);
        let turning = self
            .last_flip
            .filter(|(_, start)| start.elapsed() < FLIP_TIME && motion::enabled(cx))
            .map(|(index, _)| index);

        div()
            .id("memory-match-board")
            .accessibility_id("memory-match-board")
            .role(Role::Grid)
            .aria_label(format!("Memory Match, {} cards", self.layout.card_count()))
            .aria_row_count(rows)
            .aria_column_count(columns)
            .aria_description(
                "Use arrow keys to move between cards, then Enter or Space to turn one over.",
            )
            .capture_key_down(cx.listener(Self::board_key_down))
            .flex()
            .flex_col()
            .items_start()
            .gap(px(gap))
            .children((0..rows).map(|row| {
                div()
                    .id(("memory-match-row", row))
                    .role(Role::Row)
                    .aria_row_index(row)
                    .flex()
                    .items_center()
                    .gap(px(gap))
                    .children((0..columns).map(|column| {
                        let index = row * columns + column;
                        self.card(
                            (size, height),
                            index,
                            tab == index,
                            turning == Some(index),
                            cx,
                        )
                    }))
            }))
    }

    fn card(
        &self,
        dimensions: (f32, f32),
        index: usize,
        tab_stop: bool,
        turning: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let (width, height) = dimensions;
        let (columns, _) = self.layout.dimensions();
        let column = index % columns;
        let position = format!("{}{}", column_letter(column), index / columns + 1);
        let card = self.board().card(index).unwrap();
        let state = match card {
            Card::Hidden => "face down".into(),
            Card::Revealed(face) => format!("{}, face up", art::name(face)),
            Card::Matched(face) => format!("{}, matched", art::name(face)),
        };
        let front = match card {
            Card::Hidden => None,
            Card::Revealed(face) | Card::Matched(face) => Some(PrintedFace {
                image: self.art.face(face),
                name: art::name(face),
                seal: matches!(card, Card::Matched(_)).then(|| self.art.seal.clone()),
            }),
        };
        let back = self.art.back.clone();
        let handle = self.focus[index].clone().tab_stop(tab_stop);

        div()
            .id(("memory-match-card", index))
            .accessibility_id(format!("memory-match-card-{position}"))
            .role(Role::GridCell)
            .aria_label(format!("Card {position}, {state}"))
            .aria_column_index(column)
            .track_focus(&handle)
            .relative()
            .w(px(width))
            .h(px(height))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .border_2()
            .border_color(rgba(0x00000000))
            .when(self.board().can_flip(index), |cell| {
                cell.cursor_pointer()
                    .hover(|style| style.border_color(rgb(0xbcd8c2)))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.focus[index].focus(window, cx);
                        view.flip(index, cx);
                    }))
            })
            .focus(|style| style.border_color(rgb(GOLD)))
            .when(turning, |cell| {
                let back = back.clone();
                let front = front.clone();
                cell.child(div().with_animation(
                    ("memory-match-flip", self.revision),
                    Animation::new(FLIP_TIME),
                    move |slot, fraction| {
                        let (front, spread) = if fraction < 0.44 {
                            (None, 1. - fraction / 0.44)
                        } else {
                            let spread = (fraction - 0.44) / 0.56;
                            (front.as_ref(), 1. - (1. - spread).powi(2))
                        };
                        slot.child(print_card(width, height, spread, front, back.clone()))
                    },
                ))
            })
            .when(!turning, |cell| {
                cell.child(print_card(width, height, 1., front.as_ref(), back))
            })
    }

    fn status_section(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let pending = matches!(self.board().turn(), Turn::Mismatch { .. });
        let message = self.status();
        stage::rail_section(cx)
            .child(
                div()
                    .id("memory-match-status")
                    .accessibility_id("memory-match-status")
                    .role(Role::Status)
                    .aria_label(message.clone())
                    .a11y_synthetic_children(|builder| {
                        builder.parent_node().set_live(accesskit::Live::Polite);
                    })
                    .min_h(px(38.))
                    .text_size(px(14.))
                    .line_height(px(19.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(palette.ink))
                    .child(message),
            )
            .child(
                stage::actions()
                    .children((pending && self.hold_mismatches).then(|| {
                        themed_choice(cx, "memory-match-hide", "Hide cards", true).on_click(
                            cx.listener(|view, _, window, cx| {
                                view.conceal(cx);
                                view.focus[view.cursor].focus(window, cx);
                            }),
                        )
                    }))
                    .child(
                        themed_choice(
                            cx,
                            "memory-match-hold",
                            "Hold mismatches",
                            self.hold_mismatches,
                        )
                        .aria_toggled(self.hold_mismatches.into())
                        .aria_description(
                            "Keep mismatched cards face up until you choose Hide cards.",
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.toggle_hold(cx))),
                    ),
            )
    }

    fn status(&self) -> String {
        let board = self.board();
        let name_at = |index| match board.card(index) {
            Some(Card::Revealed(face)) => art::name(face),
            _ => unreachable!("open turn positions are revealed"),
        };
        match board.turn() {
            Turn::One { first } => format!("{} · Find its pair", name_at(first)),
            Turn::Mismatch { first, second } => {
                format!("No match · {} and {}", name_at(first), name_at(second))
            }
            Turn::Ready => match self.notice {
                Some(Notice::Pair(face)) => format!("{} · Pair found", art::name(face)),
                Some(Notice::Concealed) => "Cards turned back. Try another pair.".into(),
                None => "Turn over two cards to find a pair.".into(),
            },
        }
    }
}

fn attempt_label(attempts: u64) -> String {
    let unit = if attempts == 1 { "attempt" } else { "attempts" };
    format!("{attempts} {unit}")
}

fn column_letter(column: usize) -> char {
    char::from(b'A' + column as u8)
}

#[derive(Clone)]
struct PrintedFace {
    image: Arc<RenderImage>,
    name: &'static str,
    seal: Option<Arc<RenderImage>>,
}

fn print_card(
    width: f32,
    height: f32,
    spread: f32,
    face: Option<&PrintedFace>,
    back: Arc<RenderImage>,
) -> Div {
    let width = width - 4.;
    let height = height - 4.;
    let visible_width = width * (0.10 + 0.90 * spread);
    let card = div()
        .w(px(visible_width))
        .h(px(height))
        .flex_none()
        .relative()
        .rounded(px(6.))
        .overflow_hidden()
        .shadow(vec![
            BoxShadow::new(px(0.), px(2.), rgba(0x0b232d6b).into()).blur_radius(px(4.)),
        ]);
    let Some(face) = face else {
        return card.bg(rgb(CARD_BACK)).child(
            img(back)
                .size_full()
                .rounded(px(6.))
                .object_fit(ObjectFit::Fill),
        );
    };

    let font = (width * 0.135).clamp(10., 13.);
    let label_height = font + 4.;
    let side = if width < 66. { 1. } else { 3. };
    let artwork = div()
        .w(px(width))
        .h_full()
        .absolute()
        .left(px((visible_width - width) / 2.))
        .top_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .p(px(side))
        .child(
            img(face.image.clone())
                .w(px(width - side * 2. - 3.))
                .h(px(height - label_height - side * 2. - 4.))
                .object_fit(ObjectFit::Contain),
        )
        .child(
            div()
                .text_size(px(font))
                .line_height(px(label_height))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x213e3d))
                .child(face.name),
        )
        .children(face.seal.clone().map(|seal| {
            img(seal)
                .absolute()
                .top(px(3.))
                .right(px(3.))
                .size(px((width * 0.20).clamp(13., 20.)))
        }));
    card.bg(rgb(if face.seal.is_some() {
        0xe6eee0
    } else {
        0xfcf0d9
    }))
    .border_1()
    .border_color(rgb(if face.seal.is_some() {
        0x6b967a
    } else {
        0xe4b77e
    }))
    .child(artwork)
}
