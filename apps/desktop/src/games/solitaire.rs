use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_solitaire::{Card, Game, Move, Place, Suit, Variant};
use gpui::{Context, Div, FontWeight, Render, SharedString, Window, div, prelude::*, px, rgb};

use crate::{
    games::{GameEntry, GameLaunch},
    theme as t,
};

const FELT: u32 = 0x244b3c;
const SLOT: u32 = 0x1a3a30;
const LINE: u32 = 0x608171;
const LIGHT: u32 = 0xd9e7dc;

pub struct GameView {
    game: Game,
    seed: u64,
    selected: Option<Place>,
    history: Vec<Game>,
    status: &'static str,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _: &mut Context<Self>) -> Self {
        let (variant, seed) = match &launch.entry {
            GameEntry::Mode(id) => (Variant::from_id(id), fresh_seed()),
            GameEntry::Challenge(id) => (
                Variant::Klondike,
                id.bytes().fold(0u64, |value, byte| {
                    value.wrapping_mul(31).wrapping_add(u64::from(byte))
                }),
            ),
            GameEntry::Classic => (Variant::Klondike, fresh_seed()),
        };
        Self {
            game: Game::new(variant, seed),
            seed,
            selected: None,
            history: Vec::new(),
            status: "",
        }
    }

    fn restart(&mut self, seed: u64) {
        self.seed = seed;
        self.game = Game::new(self.game.variant, seed);
        self.selected = None;
        self.history.clear();
        self.status = "";
    }

    fn apply(&mut self, action: Move) -> bool {
        let before = self.game.clone();
        if self.game.play(action) {
            self.history.push(before);
            self.selected = None;
            self.status = if self.game.won() {
                "You cleared the table!"
            } else {
                ""
            };
            true
        } else {
            false
        }
    }

    fn play_at(&mut self, place: Place) {
        if self.game.won() {
            return;
        }
        if place == Place::Stock {
            if !self.apply(Move::Draw) {
                self.status = if self.game.variant == Variant::Spider && !self.game.stock.is_empty()
                {
                    "Fill all empty columns before dealing."
                } else {
                    "The stock is empty."
                };
            }
            return;
        }
        if self.game.variant == Variant::TriPeaks {
            if !self.apply(Move::Clear(place)) {
                self.status = "Choose an uncovered card one rank above or below the waste.";
            }
            return;
        }
        if self.game.variant == Variant::Pyramid && self.apply(Move::Clear(place)) {
            return;
        }
        if let Some(selected) = self.selected {
            if selected == place {
                self.selected = None;
                self.status = "";
                return;
            }
            if self.apply(Move::Transfer {
                from: selected,
                to: place,
            }) {
                return;
            }
            self.status = if self.game.variant == Variant::Pyramid {
                "Pairs must add up to 13."
            } else {
                "That move does not fit."
            };
        }
        if self.game.selectable(place) {
            self.selected = Some(place);
            self.status = "";
        }
    }

    fn card(
        &self,
        place: Place,
        face: CardFace,
        width: f32,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let selected = self.selected == Some(place);
        let (fill, ink, corner, mark) = match face {
            CardFace::Up(card) => (
                if selected { 0xffe9aa } else { t::SURFACE },
                if card.suit.red() { 0x9e382f } else { t::INK },
                format!("{}{}", rank(card.rank), suit(card.suit)),
                suit(card.suit).to_owned(),
            ),
            CardFace::Back => (0x315f50, 0xc4d9c8, String::new(), "◆".to_owned()),
            CardFace::Empty(mark) => (SLOT, 0xa7c6b2, String::new(), mark.to_owned()),
        };
        let mark_size = match face {
            CardFace::Up(_) => (width * 0.44).min(34.),
            _ => (width * 0.32).clamp(11., 23.),
        };
        div()
            .id(SharedString::from(format!("solitaire-{place:?}")))
            .relative()
            .w(px(width))
            .h(px(width * 1.34))
            .flex_shrink_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(if selected { t::GOLD } else { LINE }))
            .bg(rgb(fill))
            .text_color(rgb(ink))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.border_color(rgb(t::GOLD)))
            .child(
                div()
                    .absolute()
                    .top(px(3.))
                    .left(px(4.))
                    .text_size(px((width * 0.28).clamp(9., 16.)))
                    .font_weight(FontWeight::BOLD)
                    .child(corner),
            )
            .child(
                div()
                    .mt(px(6.))
                    .font_family(t::DISPLAY)
                    .text_size(px(mark_size))
                    .child(mark),
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                view.play_at(place);
                cx.notify();
            }))
    }

    fn stock(&self, width: f32, cx: &Context<Self>) -> Div {
        let face = if self.game.stock.is_empty() {
            CardFace::Empty(
                if self.game.variant == Variant::Klondike && !self.game.waste.is_empty() {
                    "↺"
                } else {
                    "–"
                },
            )
        } else {
            CardFace::Back
        };
        labeled(
            format!("Stock · {}", self.game.stock.len()),
            self.card(Place::Stock, face, width, cx),
        )
    }

    fn waste(&self, width: f32, cx: &Context<Self>) -> Div {
        let face = self
            .game
            .waste
            .last()
            .copied()
            .map_or(CardFace::Empty("–"), CardFace::Up);
        labeled("Waste", self.card(Place::Waste, face, width, cx))
    }

    fn foundations(&self, width: f32, cx: &Context<Self>) -> Div {
        labeled(
            "Foundations",
            div()
                .flex()
                .gap_2()
                .children(Suit::ALL.into_iter().enumerate().map(|(index, kind)| {
                    let face = self.game.foundations[index]
                        .last()
                        .copied()
                        .map_or(CardFace::Empty(suit(kind)), CardFace::Up);
                    self.card(Place::Foundation(index), face, width, cx)
                })),
        )
    }

    fn cells(&self, width: f32, cx: &Context<Self>) -> Div {
        labeled(
            "Free cells",
            div()
                .flex()
                .gap_2()
                .children(
                    self.game
                        .cells
                        .into_iter()
                        .enumerate()
                        .map(|(index, card)| {
                            self.card(
                                Place::Cell(index),
                                card.map_or(CardFace::Empty("–"), CardFace::Up),
                                width,
                                cx,
                            )
                        }),
                ),
        )
    }

    fn tableau(&self, width: f32, cx: &Context<Self>) -> Div {
        let step = (width * 0.35).clamp(16., 25.);
        div()
            .flex()
            .gap_2()
            .children(self.game.tableau.iter().enumerate().map(|(column, stack)| {
                let height = width * 1.34 + (stack.len().saturating_sub(1) as f32) * step;
                let mut frame = div().relative().w(px(width)).h(px(height)).flex_shrink_0();
                if stack.is_empty() {
                    frame = frame.child(self.card(
                        Place::Tableau(column, 0),
                        CardFace::Empty("–"),
                        width,
                        cx,
                    ));
                }
                frame.children(stack.iter().enumerate().map(|(index, entry)| {
                    let face = if entry.face_up {
                        CardFace::Up(entry.card)
                    } else {
                        CardFace::Back
                    };
                    div()
                        .absolute()
                        .top(px(index as f32 * step))
                        .left_0()
                        .child(self.card(Place::Tableau(column, index), face, width, cx))
                }))
            }))
    }

    fn layout(&self, width: f32, cx: &Context<Self>) -> Div {
        let pyramid = self.game.variant == Variant::Pyramid;
        let board_columns = if pyramid { 7. } else { 10. };
        let stride = width + 6.;
        let row_step = width * 0.47;
        let rows = if pyramid { 7 } else { 4 };
        let mut board = div()
            .relative()
            .w(px(board_columns * stride))
            .h(px((rows - 1) as f32 * row_step + width * 1.34));
        let mut offset = 0;
        for row in 0..rows {
            let count = if pyramid { row + 1 } else { [3, 6, 9, 10][row] };
            for column in 0..count {
                let place = Place::Layout(offset + column);
                let Some(card) = self.game.layout[offset + column] else {
                    continue;
                };
                let x = if pyramid {
                    column as f32 + (rows - count) as f32 / 2.
                } else if row == 3 {
                    column as f32
                } else {
                    let per_peak = row + 1;
                    (column / per_peak * 3 + column % per_peak) as f32 + (3 - row) as f32 / 2.
                };
                let uncovered = self.game.selectable(place);
                let face = if uncovered || pyramid {
                    CardFace::Up(card)
                } else {
                    CardFace::Back
                };
                board = board.child(
                    div()
                        .absolute()
                        .top(px(row as f32 * row_step))
                        .left(px(x * stride))
                        .opacity(if pyramid && !uncovered { 0.76 } else { 1. })
                        .child(self.card(place, face, width, cx)),
                );
            }
            offset += count;
        }
        board
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport: f32 = window.viewport_size().width.into();
        let rail = if viewport < 1080. { 174. } else { 220. };
        let columns = match self.game.variant {
            Variant::Klondike | Variant::Pyramid => 7.,
            Variant::FreeCell => 8.,
            Variant::Spider | Variant::TriPeaks => 10.,
        };
        let width = ((viewport - rail - 165.) / columns - 8.).clamp(31., 68.);
        let (rules, progress) = match self.game.variant {
            Variant::Klondike => (
                "Build foundations from ace to king. Stack descending, alternating colors. Only kings start empty columns.",
                format!("{} of 52 in foundations", self.game.foundation_count()),
            ),
            Variant::Spider => (
                "Build descending spade sequences. Complete king-to-ace runs clear automatically. Fill empty columns before dealing.",
                format!("{} of 8 runs cleared", self.game.runs),
            ),
            Variant::FreeCell => (
                "Build foundations from ace to king. Stack descending, alternating colors. Free cells hold one card each.",
                format!("{} of 52 in foundations", self.game.foundation_count()),
            ),
            Variant::Pyramid => (
                "Match uncovered pairs totaling 13, or click a king to clear it alone. Aces count as 1, jacks as 11, and queens as 12.",
                format!(
                    "{} of 28 cards cleared",
                    self.game
                        .layout
                        .iter()
                        .filter(|card| card.is_none())
                        .count()
                ),
            ),
            Variant::TriPeaks => (
                "Clear uncovered cards one rank above or below the waste. Aces connect to kings and twos.",
                format!(
                    "{} of 28 cards cleared",
                    self.game
                        .layout
                        .iter()
                        .filter(|card| card.is_none())
                        .count()
                ),
            ),
        };
        let upper = div().flex().flex_wrap().gap_4().items_end();
        let upper = match self.game.variant {
            Variant::Klondike => upper
                .child(self.stock(width, cx))
                .child(self.waste(width, cx))
                .child(self.foundations(width, cx)),
            Variant::FreeCell => upper
                .child(self.cells(width, cx))
                .child(self.foundations(width, cx)),
            Variant::Spider => upper.child(self.stock(width, cx)),
            Variant::Pyramid | Variant::TriPeaks => upper
                .child(self.stock(width, cx))
                .child(self.waste(width, cx)),
        };
        let board = if self.game.layout.is_empty() {
            self.tableau(width, cx)
        } else {
            self.layout(width, cx)
        };
        let can_undo = !self.history.is_empty();
        let status = if self.status.is_empty() {
            if self.game.variant == Variant::TriPeaks {
                "Click a card to move it to the waste."
            } else if self.selected.is_some() {
                "Choose a destination or click the selected card again to cancel."
            } else {
                "Click a card to select it."
            }
        } else {
            self.status
        };
        div()
            .w_full()
            .max_w(px(1050.))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(t::display_type(self.game.variant.title(), 32.))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(button("solitaire-undo", "Undo", can_undo).on_click(
                                cx.listener(|view, _, _, cx| {
                                    if let Some(previous) = view.history.pop() {
                                        view.game = previous;
                                        view.selected = None;
                                        view.status = "";
                                        cx.notify();
                                    }
                                }),
                            ))
                            .child(button("solitaire-restart", "Restart", true).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.restart(view.seed);
                                    cx.notify();
                                }),
                            ))
                            .child(button("solitaire-new", "New deal", true).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.restart(fresh_seed());
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(
                div()
                    .text_color(rgb(t::MUTED))
                    .text_size(px(14.))
                    .child(rules),
            )
            .child(
                div()
                    .w_full()
                    .min_h(px(400.))
                    .flex()
                    .flex_col()
                    .gap_5()
                    .rounded_xl()
                    .bg(rgb(FELT))
                    .text_color(rgb(t::PAPER))
                    .p_5()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .justify_between()
                            .gap_3()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(progress)
                            .child(format!("Moves · {}", self.game.moves)),
                    )
                    .child(upper)
                    .child(board)
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(LIGHT))
                            .child(status),
                    ),
            )
    }
}

#[derive(Clone, Copy)]
enum CardFace {
    Up(Card),
    Back,
    Empty(&'static str),
}

fn labeled(label: impl Into<SharedString>, content: impl IntoElement) -> Div {
    div().flex().flex_col().gap_1().child(content).child(
        div()
            .text_size(px(12.))
            .text_color(rgb(LIGHT))
            .child(label.into()),
    )
}

fn button(id: &'static str, label: &'static str, enabled: bool) -> gpui::Stateful<Div> {
    let mut button = div()
        .id(id)
        .rounded_md()
        .border_1()
        .border_color(rgb(t::BORDER))
        .bg(rgb(t::SURFACE))
        .px_3()
        .py_2()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label);
    if enabled {
        button = button
            .cursor_pointer()
            .hover(|style| style.bg(rgb(t::SAGE)));
    } else {
        button = button.opacity(0.45);
    }
    button
}

fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn rank(value: u8) -> String {
    match value {
        1 => "A".into(),
        11 => "J".into(),
        12 => "Q".into(),
        13 => "K".into(),
        _ => value.to_string(),
    }
}

fn suit(value: Suit) -> &'static str {
    match value {
        Suit::Spades => "♠",
        Suit::Hearts => "♥",
        Suit::Clubs => "♣",
        Suit::Diamonds => "♦",
    }
}
