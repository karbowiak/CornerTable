use std::{sync::Arc, time::Duration};

use corner_table_memory_match::{Card, Layout, Turn};
use gpui::{
    Animation, AnimationExt, App, BoxShadow, Context, Div, FontWeight, ObjectFit, Render,
    RenderImage, Role, Stateful, StyledImage as _, Window, div, img, prelude::*, px, rgb, rgba,
};

use super::{GameView, Notice, art};
use crate::{
    games::{board_space, styled_control, themed_control},
    theme,
};

const TABLE: u32 = 0x173f45;
const FIELD: u32 = 0x28535a;
const RULE: u32 = 0x567c78;
const CREAM: u32 = 0xfff4db;
const PALE: u32 = 0xcadbd1;
const GOLD: u32 = 0xffd99b;
const CARD_ASPECT: f32 = 1.125;
const MAX_WIDTH: f32 = 818.;
const AXIS: f32 = 18.;

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let available = board_space(window).min(MAX_WIDTH);
        let height = f32::from(window.viewport_size().height);
        let compact = height < 635.;
        let gap = if compact {
            6.
        } else if available < 570. {
            8.
        } else {
            10.
        };
        let (columns, rows) = self.layout.dimensions();
        let width_limit = (available - 72. - gap * (columns - 1) as f32) / columns as f32;
        let reserved_height = if compact { 277. } else { 315. };
        let height_limit =
            (height - reserved_height - gap * (rows - 1) as f32) / (rows as f32 * CARD_ASPECT);
        let size = width_limit.min(height_limit.clamp(56., 112.)).floor();
        let palette = theme::colors(cx);

        div()
            .id("memory-match")
            .w_full()
            .max_w(px(MAX_WIDTH))
            .flex()
            .flex_col()
            .gap(px(if compact { 7. } else { 12. }))
            .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && view.confirm_new_deal {
                    view.cancel_new_deal(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.toolbar(cx))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .rounded(px(14.))
                    .overflow_hidden()
                    .bg(rgb(TABLE))
                    .text_color(rgb(CREAM))
                    .shadow(vec![
                        BoxShadow::new(px(0.), px(4.), rgba(0x0c232936).into()).blur_radius(px(14.)),
                    ])
                    .child(self.counter(compact))
                    .child(
                        div()
                            .bg(rgb(FIELD))
                            .border_y_1()
                            .border_color(rgb(RULE))
                            .py(px(if compact { 4. } else { 12. }))
                            .flex()
                            .justify_center()
                            .child(self.grid(size, gap, window, cx)),
                    )
                    .child(self.status_bar(compact, cx)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(rgb(palette.muted))
                    .child(if self.hold_mismatches {
                        "Tab to the board · Arrow keys move · Enter or Space turns a card · H hides a mismatch"
                    } else {
                        "Tab to the board · Arrow keys move · Enter or Space turns a card"
                    }),
            )
    }
}

impl GameView {
    fn toolbar(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .id("memory-match-layouts")
                            .role(Role::Group)
                            .aria_label("Board size")
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(5.))
                            .child(
                                div()
                                    .mr_1()
                                    .text_size(px(13.))
                                    .text_color(rgb(palette.muted))
                                    .child("Board size"),
                            )
                            .children(Layout::ALL.map(|layout| {
                                let selected = self.layout == layout;
                                let label = format!("{} cards", layout.card_count());
                                let control_palette = if selected {
                                    theme::Palette {
                                        surface: TABLE,
                                        border: TABLE,
                                        ink: CREAM,
                                        hover: FIELD,
                                        ..palette
                                    }
                                } else {
                                    palette
                                };
                                styled_control(control_palette, layout.id(), label, true)
                                    .min_h(px(34.))
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
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                themed_control(
                                    cx,
                                    "memory-match-new",
                                    if self.confirm_new_deal {
                                        "Deal new cards"
                                    } else {
                                        "New deal"
                                    },
                                    true,
                                )
                                .min_h(px(34.))
                                .on_click(cx.listener(
                                    |view, _, window, cx| {
                                        view.request_new_deal(window, cx);
                                    },
                                )),
                            )
                            .children(self.confirm_new_deal.then(|| {
                                themed_control(cx, "memory-match-cancel", "Cancel", true)
                                    .min_h(px(34.))
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.cancel_new_deal(window, cx);
                                    }))
                            })),
                    ),
            )
            .children(self.confirm_new_deal.then(|| {
                div()
                    .id("memory-match-confirm")
                    .role(Role::Status)
                    .aria_label("Deal new cards and discard this board’s progress?")
                    .a11y_synthetic_children(|builder| {
                        builder.parent_node().set_live(accesskit::Live::Polite);
                    })
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(rgb(palette.ink))
                    .child("Deal new cards and discard this board’s progress?")
            }))
    }

    fn cancel_new_deal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_new_deal = false;
        self.focus[self.cursor].focus(window, cx);
        cx.notify();
    }

    fn counter(&self, compact: bool) -> Div {
        let pair_count = self.layout.pair_count();
        let pairs = self.board().pairs();
        let attempts = self.board().attempts();
        let best = self.round().best;

        div()
            .w_full()
            .min_h(px(if compact { 41. } else { 55. }))
            .px(px(17.))
            .py(px(7.))
            .flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap_x_3()
            .gap_y_1()
            .child(theme::display_type("Tidepool deck", 19.).text_color(rgb(CREAM)))
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
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(counter_value("Pairs", format!("{pairs}/{pair_count}")))
                    .child(counter_value("Attempts", attempts.to_string()))
                    .child(counter_value(
                        "Best",
                        best.map_or_else(|| "—".into(), |best| best.to_string()),
                    )),
            )
    }

    fn grid(&self, size: f32, gap: f32, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let (columns, rows) = self.layout.dimensions();
        let height = (size * CARD_ASPECT).floor();
        let focused = self.focused_card(window);
        let tab = focused.unwrap_or(self.cursor);
        let turning = self.last_flip.filter(|_| animation_enabled(cx));

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
            .child(
                div()
                    .flex()
                    .gap(px(gap))
                    .child(div().w(px(AXIS)).h(px(13.)))
                    .children(
                        (0..columns)
                            .map(|column| axis(size, 13., column_letter(column).to_string())),
                    ),
            )
            .children((0..rows).map(|row| {
                div()
                    .id(("memory-match-row", row))
                    .role(Role::Row)
                    .aria_row_index(row)
                    .flex()
                    .items_center()
                    .gap(px(gap))
                    .child(axis(AXIS, height, (row + 1).to_string()))
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
                    Animation::new(Duration::from_millis(138)),
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

    fn status_bar(&self, compact: bool, cx: &Context<Self>) -> Div {
        let pending = matches!(self.board().turn(), Turn::Mismatch { .. });
        let message = self.status();
        div()
            .w_full()
            .min_h(px(if compact { 43. } else { 54. }))
            .px(px(16.))
            .py(px(if compact { 5. } else { 7. }))
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_x_3()
            .gap_y_1()
            .child(
                div()
                    .id("memory-match-status")
                    .accessibility_id("memory-match-status")
                    .role(Role::Status)
                    .aria_label(message.clone())
                    .a11y_synthetic_children(|builder| {
                        builder.parent_node().set_live(accesskit::Live::Polite);
                    })
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(if self.board().is_complete() {
                        GOLD
                    } else {
                        CREAM
                    }))
                    .child(message),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .children((pending && self.hold_mismatches).then(|| {
                        let button = table_button("memory-match-hide", "Hide cards", true);
                        button.on_click(cx.listener(|view, _, window, cx| {
                            view.conceal(cx);
                            view.focus[view.cursor].focus(window, cx);
                        }))
                    }))
                    .child(
                        table_button("memory-match-hold", "Hold mismatches", self.hold_mismatches)
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
        if board.is_complete() {
            let qualifier = if board.attempts() == board.pairs() as u64 {
                "Perfect deal"
            } else if Some(board.attempts()) == self.round().best {
                "Personal best"
            } else {
                "All pairs found"
            };
            return format!(
                "{qualifier} · {} pairs in {} attempts",
                board.pairs(),
                board.attempts()
            );
        }

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

#[cfg(target_os = "macos")]
fn animation_enabled(cx: &App) -> bool {
    use objc2::{
        class, msg_send,
        runtime::{AnyObject, Bool},
    };

    if cx.reduce_motion() {
        return false;
    }
    // The view runs on the AppKit thread. NSWorkspace owns its shared instance.
    let workspace: &AnyObject = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
    let reduce_motion: Bool =
        unsafe { msg_send![workspace, accessibilityDisplayShouldReduceMotion] };
    !reduce_motion.as_bool()
}

#[cfg(not(target_os = "macos"))]
fn animation_enabled(_: &App) -> bool {
    false
}

fn column_letter(column: usize) -> char {
    char::from(b'A' + column as u8)
}

fn axis(width: f32, height: f32, text: String) -> Div {
    div()
        .w(px(width))
        .h(px(height))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .text_size(px(11.))
        .line_height(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(PALE))
        .child(text)
}

fn counter_value(label: &'static str, value: String) -> Div {
    div()
        .flex()
        .items_baseline()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(rgb(PALE))
                .child(label),
        )
        .child(
            div()
                .text_size(px(15.))
                .line_height(px(18.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(CREAM))
                .child(value),
        )
}

fn table_button(id: &'static str, label: &'static str, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .min_h(px(32.))
        .px_2()
        .rounded(px(6.))
        .border_1()
        .border_color(rgb(if selected { GOLD } else { RULE }))
        .bg(rgb(if selected { GOLD } else { TABLE }))
        .text_color(rgb(if selected { TABLE } else { CREAM }))
        .text_size(px(12.))
        .line_height(px(17.))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(move |style| style.bg(rgb(if selected { 0xffe6b5 } else { FIELD })))
        .focus_visible(|style| style.border_color(rgb(0xffffff)))
        .child(label)
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
        return card.bg(rgb(TABLE)).child(
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
