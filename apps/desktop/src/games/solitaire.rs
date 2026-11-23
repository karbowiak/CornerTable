use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_solitaire::{Card, DealMode, Game, Move, Place, Suit, Variant};
use gpui::{
    Context, Div, EntityId, FontWeight, Render, SharedString, Stateful, Window, div, prelude::*,
    px, rgb,
};

use crate::{
    games::{GameEntry, GameLaunch},
    theme as t,
};

const FELT: u32 = 0x244b3c;
const SLOT: u32 = 0x1a3a30;
const LINE: u32 = 0x608171;
const LIGHT: u32 = 0xd9e7dc;

pub struct GameView {
    deal: Deal,
    other_deals: Vec<Deal>,
    revision: u64,
    show_title: bool,
}

struct Deal {
    game: Game,
    seed: u64,
    selected: Option<Place>,
    history: Vec<Game>,
    status: &'static str,
}

impl Deal {
    fn new(game: Game, seed: u64) -> Self {
        Self {
            game,
            seed,
            selected: None,
            history: Vec::new(),
            status: "",
        }
    }
}

impl GameView {
    pub fn new(launch: &GameLaunch, _: &mut Context<Self>) -> Self {
        let variant = match &launch.entry {
            GameEntry::Mode(id) => Variant::from_id(id),
            GameEntry::Classic | GameEntry::Challenge(_) => Variant::Klondike,
        };
        let seed = launch.seed();
        Self {
            deal: Deal::new(Game::new(variant, seed), seed),
            other_deals: variant.deal_modes()[1..]
                .iter()
                .map(|&mode| Deal::new(Game::with_mode(mode, seed), seed))
                .collect(),
            revision: 0,
            show_title: !matches!(launch.entry, GameEntry::Mode(_)),
        }
    }

    fn restart(&mut self, seed: u64) {
        self.deal = Deal::new(Game::with_mode(self.deal.game.mode(), seed), seed);
        self.revision += 1;
    }

    fn switch_mode(&mut self, mode: DealMode) {
        if let Some(deal) = self
            .other_deals
            .iter_mut()
            .find(|deal| deal.game.mode() == mode)
        {
            std::mem::swap(&mut self.deal, deal);
            self.revision += 1;
        }
    }

    fn apply(&mut self, action: Move) -> bool {
        let mut next = self.deal.game.clone();
        if !next.play(action) {
            return false;
        }
        self.deal.status = if next.won() {
            "You cleared the table!"
        } else {
            ""
        };
        self.deal
            .history
            .push(std::mem::replace(&mut self.deal.game, next));
        self.deal.selected = None;
        self.revision += 1;
        true
    }

    fn play_at(&mut self, place: Place) {
        if self.deal.game.won() {
            return;
        }
        if place == Place::Stock {
            if !self.apply(Move::Draw) {
                self.deal.status = if self.deal.game.variant == Variant::Spider
                    && !self.deal.game.stock.is_empty()
                {
                    "Fill all empty columns before dealing."
                } else {
                    "The stock is empty."
                };
            }
            return;
        }
        if self.deal.game.variant == Variant::TriPeaks {
            if !self.apply(Move::Clear(place)) {
                self.deal.status = "Choose an uncovered card one rank above or below the waste.";
            }
            return;
        }
        if self.deal.game.variant == Variant::Pyramid && self.apply(Move::Clear(place)) {
            return;
        }
        if let Some(selected) = self.deal.selected {
            if selected == place {
                self.deal.selected = None;
                self.deal.status = "";
                return;
            }
            if self.apply(Move::Transfer {
                from: selected,
                to: place,
            }) {
                return;
            }
            self.deal.status = if self.deal.game.variant == Variant::Pyramid {
                "Pairs must add up to 13."
            } else {
                "That move does not fit."
            };
        }
        if self.deal.game.selectable(place) {
            self.deal.selected = Some(place);
            self.deal.status = "";
        }
    }

    fn drop_action(&self, drag: &CardDrag, to: Place, owner: EntityId) -> Option<Move> {
        if drag.owner != owner || drag.revision != self.revision || drag.from == to {
            return None;
        }
        if self.deal.game.variant == Variant::TriPeaks {
            (to == Place::Waste).then_some(Move::Clear(drag.from))
        } else {
            Some(Move::Transfer {
                from: drag.from,
                to,
            })
        }
    }

    fn drag_source(
        &self,
        from: Place,
        face: CardFace,
        width: f32,
        owner: EntityId,
    ) -> Option<CardDrag> {
        let game = &self.deal.game;
        let CardFace::Up(card) = face else {
            return None;
        };
        if game.won()
            || !game.selectable(from)
            || (game.variant == Variant::Pyramid && card.rank == 13)
        {
            return None;
        }
        let cards = if let Place::Tableau(column, index) = from {
            game.tableau[column][index..]
                .iter()
                .map(|entry| entry.card)
                .collect()
        } else {
            vec![card]
        };
        Some(CardDrag {
            owner,
            revision: self.revision,
            from,
            cards,
            width,
        })
    }

    fn card(&self, place: Place, face: CardFace, width: f32, cx: &Context<Self>) -> Stateful<Div> {
        let owner = cx.entity_id();
        let view = cx.weak_entity();
        let drag = self.drag_source(place, face, width, owner);
        let draggable = drag.is_some();
        card_surface(face, width, self.deal.selected == Some(place))
            .id(SharedString::from(format!("solitaire-{place:?}")))
            .block_mouse_except_scroll()
            .cursor_pointer()
            .hover(move |style| {
                let style = style.border_color(rgb(t::GOLD));
                if draggable {
                    style.cursor_grab()
                } else {
                    style
                }
            })
            .on_click(cx.listener(move |view, _, _, cx| {
                view.play_at(place);
                cx.notify();
            }))
            .when_some(drag, |element, drag| {
                element
                    .cursor_grabbing()
                    .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
            })
            .drag_over(move |style, drag: &CardDrag, _, cx| {
                if let Some(view) = view.upgrade() {
                    let view = view.read(cx);
                    if let Some(action) = view.drop_action(drag, place, owner)
                        && view.deal.game.clone().play(action)
                    {
                        return style.border_color(rgb(t::GOLD)).border_2();
                    }
                }
                style
            })
            .on_drop(cx.listener(move |view, drag: &CardDrag, _, cx| {
                if let Some(action) = view.drop_action(drag, place, owner)
                    && view.apply(action)
                {
                    cx.notify();
                }
            }))
    }

    fn stock(&self, width: f32, cx: &Context<Self>) -> Div {
        let game = &self.deal.game;
        let face = if game.stock.is_empty() {
            CardFace::Empty(
                if game.variant == Variant::Klondike && !game.waste.is_empty() {
                    "↺"
                } else {
                    "–"
                },
            )
        } else {
            CardFace::Back
        };
        labeled(
            format!("Stock · {}", game.stock.len()),
            self.card(Place::Stock, face, width, cx),
        )
    }

    fn waste(&self, width: f32, cx: &Context<Self>) -> Div {
        let game = &self.deal.game;
        let face = game
            .waste
            .last()
            .copied()
            .map_or(CardFace::Empty("–"), CardFace::Up);
        let top = self.card(Place::Waste, face, width, cx);
        if game.mode() != DealMode::KlondikeDrawThree {
            return labeled("Waste", top);
        }

        let step = (width * 0.48).max(22.);
        let waste = &game.waste[game.waste.len().saturating_sub(3)..];
        let past_cards = waste.len().saturating_sub(1);
        let fan = div()
            .relative()
            .w(px(width + step * 2.))
            .h(px(width * 1.34))
            .children(
                waste[..past_cards]
                    .iter()
                    .enumerate()
                    .map(|(index, &card)| {
                        div()
                            .absolute()
                            .left(px(index as f32 * step))
                            .child(card_surface(CardFace::Up(card), width, false))
                    }),
            )
            .child(
                div()
                    .absolute()
                    .left(px(past_cards as f32 * step))
                    .child(top),
            );
        labeled("Waste", fan)
    }

    fn foundations(&self, width: f32, cx: &Context<Self>) -> Div {
        labeled(
            "Foundations",
            div()
                .flex()
                .gap_2()
                .children(Suit::ALL.into_iter().enumerate().map(|(index, kind)| {
                    let face = self.deal.game.foundations[index]
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
                    self.deal
                        .game
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
        let step = stack_step(width);
        div()
            .flex()
            .gap_2()
            .children(
                self.deal
                    .game
                    .tableau
                    .iter()
                    .enumerate()
                    .map(|(column, stack)| {
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
                    }),
            )
    }

    fn layout(&self, width: f32, cx: &Context<Self>) -> Div {
        let game = &self.deal.game;
        let pyramid = game.variant == Variant::Pyramid;
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
                let Some(card) = game.layout[offset + column] else {
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
                let uncovered = game.selectable(place);
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

    fn mode_picker(&self, cx: &Context<Self>) -> Option<Div> {
        let game = &self.deal.game;
        let modes = game.variant.deal_modes();
        if modes.len() < 2 {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .children(modes.iter().map(|&mode| {
                    let label = mode_label(mode);
                    let active = mode == game.mode();
                    button(label, label, true)
                        .when(active, |button| {
                            button
                                .bg(rgb(t::SAGE))
                                .border_color(rgb(t::INK))
                                .cursor_default()
                        })
                        .when(!active, |button| {
                            button.on_click(cx.listener(move |view, _, _, cx| {
                                view.switch_mode(mode);
                                cx.notify();
                            }))
                        })
                })),
        )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let game = &self.deal.game;
        let viewport: f32 = window.viewport_size().width.into();
        let rail = if viewport < 1080. { 174. } else { 220. };
        let columns = match game.variant {
            Variant::Klondike | Variant::Pyramid => 7.,
            Variant::FreeCell => 8.,
            Variant::Spider | Variant::TriPeaks => 10.,
        };
        let width = ((viewport - rail - 165.) / columns - 8.).clamp(31., 68.);
        let (rules, progress) = match game.variant {
            Variant::Klondike => (
                "Build foundations from ace to king. Stack descending, alternating colors. Only kings start empty columns.",
                format!("{} of 52 in foundations", game.foundation_count()),
            ),
            Variant::Spider => (
                "Stack cards in descending order. Only runs of one suit move together or clear from king to ace. Fill empty columns before dealing.",
                format!("{} of 8 runs cleared", game.runs),
            ),
            Variant::FreeCell => (
                "Build foundations from ace to king. Stack descending, alternating colors. Free cells hold one card each.",
                format!("{} of 52 in foundations", game.foundation_count()),
            ),
            Variant::Pyramid => (
                "Match uncovered pairs totaling 13, or click a king to clear it alone. Aces count as 1, jacks as 11, and queens as 12.",
                format!(
                    "{} of 28 cards cleared",
                    game.layout.iter().filter(|card| card.is_none()).count()
                ),
            ),
            Variant::TriPeaks => (
                "Clear uncovered cards one rank above or below the waste. Aces connect to kings and twos.",
                format!(
                    "{} of 28 cards cleared",
                    game.layout.iter().filter(|card| card.is_none()).count()
                ),
            ),
        };
        let upper = div().flex().flex_wrap().gap_4().items_end();
        let upper = match game.variant {
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
        let board = if game.layout.is_empty() {
            self.tableau(width, cx)
        } else {
            self.layout(width, cx)
        };
        let can_undo = !self.deal.history.is_empty();
        let status = if self.deal.status.is_empty() {
            if self.deal.selected.is_some() {
                "Choose a destination or click the selected card again to cancel."
            } else {
                match game.variant {
                    Variant::Pyramid => "Drag one card onto its match or click both cards.",
                    Variant::TriPeaks => "Drag a card to the waste or click it to move it there.",
                    _ => "Drag a card to move it or click to select it.",
                }
            }
        } else {
            self.deal.status
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
                    .gap_3()
                    .when(self.show_title, |row| {
                        row.child(t::display_type(game.variant.title(), 32.))
                    })
                    .children(self.mode_picker(cx))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .ml_auto()
                            .gap_2()
                            .child(button("solitaire-undo", "Undo", can_undo).on_click(
                                cx.listener(|view, _, _, cx| {
                                    if let Some(previous) = view.deal.history.pop() {
                                        view.deal.game = previous;
                                        view.deal.selected = None;
                                        view.deal.status = "";
                                        view.revision += 1;
                                        cx.notify();
                                    }
                                }),
                            ))
                            .child(button("solitaire-restart", "Restart", true).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.restart(view.deal.seed);
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
                            .child(format!("Moves · {}", game.moves)),
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

#[derive(Clone)]
struct CardDrag {
    owner: EntityId,
    revision: u64,
    from: Place,
    cards: Vec<Card>,
    width: f32,
}

impl Render for CardDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let step = stack_step(self.width);
        div().pl(px(4.)).pt(px(5.)).child(
            div()
                .relative()
                .w(px(self.width))
                .h(px(self.width * 1.34 + (self.cards.len() - 1) as f32 * step))
                .children(self.cards.iter().enumerate().map(|(index, &card)| {
                    div()
                        .absolute()
                        .top(px(index as f32 * step))
                        .left_0()
                        .child(card_surface(CardFace::Up(card), self.width, false).shadow_md())
                })),
        )
    }
}

fn stack_step(width: f32) -> f32 {
    (width * 0.35).clamp(16., 25.)
}

fn card_surface(face: CardFace, width: f32, selected: bool) -> Div {
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
}

fn mode_label(mode: DealMode) -> &'static str {
    match mode {
        DealMode::KlondikeDrawOne => "Draw 1",
        DealMode::KlondikeDrawThree => "Draw 3",
        DealMode::SpiderOneSuit => "1 suit",
        DealMode::SpiderTwoSuits => "2 suits",
        DealMode::SpiderFourSuits => "4 suits",
        DealMode::FreeCell | DealMode::Pyramid | DealMode::TriPeaks => "Standard",
        DealMode::TriPeaksSolvable => "Solvable",
    }
}

fn labeled(label: impl Into<SharedString>, content: impl IntoElement) -> Div {
    div().flex().flex_col().gap_1().child(content).child(
        div()
            .text_size(px(12.))
            .text_color(rgb(LIGHT))
            .child(label.into()),
    )
}

fn button(id: &'static str, label: &'static str, enabled: bool) -> Stateful<Div> {
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
            .tab_index(0)
            .cursor_pointer()
            .hover(|style| style.bg(rgb(t::SAGE)))
            .focus(|style| style.border_color(rgb(t::INK)));
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
