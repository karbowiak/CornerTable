use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_solitaire::{Card, DealMode, Game, Move, Place, Suit, Variant};
use gpui::{
    BoxShadow, Context, Div, EntityId, FontWeight, ObjectFit, Render, Role, SharedString, Stateful,
    Window, div, img, prelude::*, px, rgb, rgba, svg,
};

use crate::{
    games::{GameEntry, GameLaunch},
    theme as t,
};

const FELT: u32 = 0x153b30;
const SLOT: u32 = 0x15392f;
const SLOT_LINE: u32 = 0x5d8371;
const FELT_LINE: u32 = 0xc7d7b33b;
const LIGHT: u32 = 0xdae7d6;
const FLAX: u32 = 0xf5dfaa;
const STOCK_ART: &str = "games/solitaire/card-back.webp";
const FELT_ART: &str = "games/solitaire/baize.webp";
const CARD_RATIO: f32 = 1.4;

#[derive(Clone, Copy)]
struct CardSize {
    width: f32,
    gap: f32,
    face_step: f32,
    columns: usize,
}

impl CardSize {
    fn height(self) -> f32 {
        self.width * CARD_RATIO
    }

    fn stride(self) -> f32 {
        self.width + self.gap
    }

    fn span(self) -> f32 {
        self.stride() * self.columns as f32 - self.gap
    }

    fn back_step(self) -> f32 {
        self.width * 0.155
    }
}

struct TableSize {
    card: CardSize,
    padding: f32,
    min_height: f32,
}

impl TableSize {
    fn new(game: &Game, window: &Window, show_title: bool) -> Self {
        let available_width = super::board_space(window) + 36.;
        let padding = (available_width * 0.022).clamp(12., 25.);
        let gap = (available_width * 0.012).clamp(5., 14.);
        let (columns, depth) = match game.variant {
            Variant::Klondike => (7, 3.8),
            Variant::FreeCell => (8, 4.8),
            Variant::Spider => (10, 3.7),
            Variant::Pyramid => (7, 5.4),
            Variant::TriPeaks => (10, 4.1),
        };
        let viewport_height = f32::from(window.viewport_size().height);
        let min_height = (viewport_height - if show_title { 137. } else { 129. }).max(326.);
        let available_height = min_height - if game.layout.is_empty() { 128. } else { 152. };
        let width = ((available_width - 2. * padding - gap * (columns - 1) as f32)
            / columns as f32)
            .min((available_height / depth).max(44.))
            .min(124.)
            .floor();
        let mut card = CardSize {
            width,
            gap,
            face_step: width * 0.35,
            columns,
        };
        for stack in &game.tableau {
            let backs = stack.iter().take_while(|entry| !entry.face_up).count();
            let exposed = stack.len().saturating_sub(backs + 1);
            if exposed > 0 {
                let room = available_height - card.height() * 2. - card.back_step() * backs as f32;
                card.face_step = card
                    .face_step
                    .min((room / exposed as f32).max(width * 0.29));
            }
        }
        Self {
            card,
            padding,
            min_height,
        }
    }
}

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
        self.deal.status = "";
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
            if self.deal.game.selectable(place) {
                self.deal.selected = Some(place);
            }
            return;
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
        size: CardSize,
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
            size,
        })
    }

    fn card(
        &self,
        place: Place,
        face: CardFace,
        size: CardSize,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let owner = cx.entity_id();
        let view = cx.weak_entity();
        let game = &self.deal.game;
        let selected = self.deal.selected == Some(place);
        let clickable = !game.won()
            && match place {
                Place::Stock | Place::Foundation(_) | Place::Cell(_) => true,
                Place::Tableau(..) => {
                    game.selectable(place)
                        || self.deal.selected.is_some()
                        || matches!(face, CardFace::Empty(_))
                }
                Place::Layout(_) | Place::Waste => game.selectable(place),
            };
        let drag = self.drag_source(place, face, size, owner);
        let draggable = drag.is_some();
        card_surface(face, size, selected)
            .id(SharedString::from(format!("solitaire-{place:?}")))
            .block_mouse_except_scroll()
            .when(clickable, |element| {
                element
                    .role(Role::Button)
                    .aria_label(card_label(place, face, game))
                    .aria_selected(selected)
                    .when(
                        !matches!(face, CardFace::Back) || place == Place::Stock,
                        |element| {
                            element
                                .tab_index(0)
                                .focus(|style| style.border_color(rgb(FLAX)))
                        },
                    )
                    .cursor_pointer()
                    .hover(move |style| {
                        let style = style.border_color(rgb(0xcbaf79));
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
            })
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
                        let highlight = style.border_color(rgb(0xffcf73)).shadow(vec![
                            BoxShadow::new(px(0.), px(2.), rgba(0x020c08b8).into())
                                .blur_radius(px(7.))
                                .spread_radius(px(2.)),
                        ]);
                        return if matches!(face, CardFace::Empty(_)) {
                            highlight.bg(rgb(0x44745a)).text_color(rgb(0xfff4d7))
                        } else {
                            highlight
                        };
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

    fn stock(&self, size: CardSize, cx: &Context<Self>) -> Div {
        let game = &self.deal.game;
        let face = if game.stock.is_empty() {
            CardFace::Empty(
                if game.variant == Variant::Klondike && !game.waste.is_empty() {
                    Slot::Recycle
                } else {
                    Slot::Plain
                },
            )
        } else {
            CardFace::Back
        };
        labeled(
            format!("Stock  {}", game.stock.len()),
            self.card(Place::Stock, face, size, cx),
        )
    }

    fn waste(&self, size: CardSize, cx: &Context<Self>) -> Div {
        let game = &self.deal.game;
        let face = game
            .waste
            .last()
            .copied()
            .map_or(CardFace::Empty(Slot::Plain), CardFace::Up);
        let top = self.card(Place::Waste, face, size, cx);
        if game.mode() != DealMode::KlondikeDrawThree {
            return labeled("Waste", top);
        }

        let step = size.width * 0.46;
        let waste = &game.waste[game.waste.len().saturating_sub(3)..];
        let past_cards = waste.len().saturating_sub(1);
        let fan = div()
            .relative()
            .w(px(size.width + step * 2.))
            .h(px(size.height()))
            .children(
                waste[..past_cards]
                    .iter()
                    .enumerate()
                    .map(|(index, &card)| {
                        div()
                            .absolute()
                            .left(px(index as f32 * step))
                            .child(card_surface(CardFace::Up(card), size, false))
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

    fn foundations(&self, size: CardSize, cx: &Context<Self>) -> Div {
        labeled(
            "Foundations",
            div()
                .flex()
                .gap(px(size.gap))
                .children(Suit::ALL.into_iter().enumerate().map(|(index, kind)| {
                    let face = self.deal.game.foundations[index]
                        .last()
                        .copied()
                        .map_or(CardFace::Empty(Slot::Foundation(kind)), CardFace::Up);
                    self.card(Place::Foundation(index), face, size, cx)
                })),
        )
    }

    fn cells(&self, size: CardSize, cx: &Context<Self>) -> Div {
        labeled(
            "Free cells",
            div().flex().gap(px(size.gap)).children(
                self.deal
                    .game
                    .cells
                    .into_iter()
                    .enumerate()
                    .map(|(index, card)| {
                        self.card(
                            Place::Cell(index),
                            card.map_or(CardFace::Empty(Slot::Plain), CardFace::Up),
                            size,
                            cx,
                        )
                    }),
            ),
        )
    }

    fn tableau(&self, size: CardSize, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .gap(px(size.gap))
            .children(
                self.deal
                    .game
                    .tableau
                    .iter()
                    .enumerate()
                    .map(|(column, stack)| {
                        let frame = div().relative().w(px(size.width)).flex_shrink_0();
                        if stack.is_empty() {
                            return frame.h(px(size.height())).child(self.card(
                                Place::Tableau(column, 0),
                                CardFace::Empty(Slot::Plain),
                                size,
                                cx,
                            ));
                        }

                        let mut offset = 0.;
                        let cards = stack.iter().enumerate().map(|(index, entry)| {
                            let top = offset;
                            let face = if entry.face_up {
                                CardFace::Up(entry.card)
                            } else {
                                CardFace::Back
                            };
                            if index + 1 < stack.len() {
                                offset += if entry.face_up {
                                    size.face_step
                                } else {
                                    size.back_step()
                                };
                            }
                            div().absolute().top(px(top)).left_0().child(self.card(
                                Place::Tableau(column, index),
                                face,
                                size,
                                cx,
                            ))
                        });
                        let frame = frame.children(cards);
                        frame.h(px(size.height() + offset))
                    }),
            )
    }

    fn layout(&self, size: CardSize, cx: &Context<Self>) -> Div {
        let game = &self.deal.game;
        let pyramid = game.variant == Variant::Pyramid;
        let row_step = size.width * if pyramid { 0.43 } else { 0.51 };
        let rows = if pyramid { 7 } else { 4 };
        let mut board = div()
            .relative()
            .w(px(size.span()))
            .h(px((rows - 1) as f32 * row_step + size.height()));
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
                let face = match (uncovered, pyramid) {
                    (true, _) => CardFace::Up(card),
                    (false, true) => CardFace::Covered(card),
                    (false, false) => CardFace::Back,
                };
                board = board.child(
                    div()
                        .absolute()
                        .top(px(row as f32 * row_step))
                        .left(px(x * size.stride()))
                        .child(self.card(place, face, size, cx)),
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
                .gap(px(2.))
                .p(px(3.))
                .rounded_lg()
                .border_1()
                .border_color(rgb(t::BORDER))
                .bg(rgb(0xebe7dc))
                .children(modes.iter().map(|&mode| {
                    let label = mode_label(mode);
                    let active = mode == game.mode();
                    div()
                        .id(label)
                        .role(Role::Button)
                        .aria_label(label)
                        .aria_toggled(if active {
                            gpui::Toggled::True
                        } else {
                            gpui::Toggled::False
                        })
                        .tab_index(0)
                        .flex()
                        .items_center()
                        .h(px(29.))
                        .px_3()
                        .rounded_md()
                        .border_1()
                        .border_color(rgba(0x00000000))
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .focus(|style| style.border_color(rgb(t::INK)))
                        .when(active, |button| {
                            button.bg(rgb(t::SURFACE)).border_color(rgb(t::BORDER))
                        })
                        .when(!active, |button| {
                            button
                                .text_color(rgb(t::MUTED))
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(t::SAGE)))
                        })
                        .child(label)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.switch_mode(mode);
                            cx.notify();
                        }))
                })),
        )
    }

    fn undo_button(&self, cx: &Context<Self>) -> Stateful<Div> {
        let can_undo = !self.deal.history.is_empty();
        super::control("solitaire-undo", "Undo", can_undo)
            .role(Role::Button)
            .aria_label("Undo move")
            .when(can_undo, |button| {
                button.on_click(cx.listener(|view, _, _, cx| {
                    if let Some(previous) = view.deal.history.pop() {
                        view.deal.game = previous;
                        view.deal.selected = None;
                        view.deal.status = "";
                        view.revision += 1;
                        cx.notify();
                    }
                }))
            })
    }

    fn replay_button(id: &'static str, cx: &Context<Self>) -> Stateful<Div> {
        super::control(id, "Replay", true)
            .role(Role::Button)
            .aria_label("Replay this deal")
            .on_click(cx.listener(|view, _, _, cx| {
                view.restart(view.deal.seed);
                cx.notify();
            }))
    }

    fn new_deal_button(id: &'static str, cx: &Context<Self>) -> Stateful<Div> {
        div()
            .id(id)
            .role(Role::Button)
            .aria_label("New deal")
            .tab_index(0)
            .flex()
            .items_center()
            .min_h(px(36.))
            .px_3()
            .rounded_md()
            .border_1()
            .bg(rgb(t::GOLD))
            .border_color(rgb(0xc79543))
            .text_color(rgb(t::INK))
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .hover(|style| style.bg(rgb(t::GOLD_HOVER)))
            .focus(|style| style.border_color(rgb(t::INK)))
            .child("New deal")
            .on_click(cx.listener(|view, _, _, cx| {
                view.restart(fresh_seed());
                cx.notify();
            }))
    }

    fn rack(&self, size: CardSize, cx: &Context<Self>) -> Div {
        let variant = self.deal.game.variant;
        let draws = || {
            div()
                .flex()
                .gap(px(size.gap))
                .child(self.stock(size, cx))
                .child(self.waste(size, cx))
        };
        let rack = div()
            .w(px(size.span()))
            .flex()
            .items_start()
            .justify_between()
            .gap(px(size.gap));
        match variant {
            Variant::Klondike => rack.child(draws()).child(self.foundations(size, cx)),
            Variant::FreeCell => rack
                .child(self.cells(size, cx))
                .child(self.foundations(size, cx)),
            Variant::Spider => rack.child(self.stock(size, cx)),
            Variant::Pyramid | Variant::TriPeaks => rack.justify_center().child(draws()),
        }
    }

    fn finish(&self, cx: &Context<Self>) -> Div {
        let moves = self.deal.game.moves;
        div()
            .flex_1()
            .w_full()
            .min_h(px(176.))
            .py_4()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .text_center()
            .child(
                svg()
                    .path("icons/solitaire.svg")
                    .size(px(30.))
                    .text_color(rgb(FLAX)),
            )
            .child(t::display_type("Table cleared", 31.).text_color(rgb(0xfff3d9)))
            .child(
                div()
                    .text_color(rgb(LIGHT))
                    .child(format!("Won in {moves} moves")),
            )
            .child(
                div()
                    .mt_2()
                    .flex()
                    .gap_2()
                    .child(Self::replay_button("solitaire-finish-replay", cx))
                    .child(Self::new_deal_button("solitaire-finish-new", cx)),
            )
    }

    fn note(&self, stalled: bool) -> &'static str {
        if stalled {
            return "No moves left. Undo a move or start a new deal.";
        }
        if !self.deal.status.is_empty() {
            return self.deal.status;
        }
        if self.deal.selected.is_some() {
            return "Choose a destination. Select the same card again to cancel.";
        }
        match self.deal.game.variant {
            Variant::Klondike => {
                "Build foundations from ace to king. Stack in descending order and alternate colors. Only kings start empty columns."
            }
            Variant::Spider => {
                "Stack in descending order. Only sequences of one suit move together or clear from king to ace. Fill empty columns before dealing."
            }
            Variant::FreeCell => {
                "Build foundations from ace to king. Stack in descending order and alternate colors. Each free cell holds one card."
            }
            Variant::Pyramid => {
                "Pair uncovered cards that add up to 13. Clear kings on their own. Aces are 1, jacks are 11, and queens are 12."
            }
            Variant::TriPeaks => {
                "Clear uncovered cards one rank above or below the waste. Aces connect to kings and twos."
            }
        }
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let game = &self.deal.game;
        let table = TableSize::new(game, window, self.show_title);
        let won = game.won();
        let stalled = game.layout_stalled();
        let has_layout = !game.layout.is_empty();
        let field = div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .when(has_layout, |field| field.justify_center())
            .gap(px((table.card.width * 0.17).clamp(12., 21.)));
        let field = match (has_layout, won) {
            (true, true) => field.child(self.finish(cx)),
            (true, false) => field
                .child(self.layout(table.card, cx))
                .child(self.rack(table.card, cx)),
            (false, _) => field.child(self.rack(table.card, cx)).child(if won {
                self.finish(cx)
            } else {
                self.tableau(table.card, cx)
            }),
        };
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .when(self.show_title, |row| {
                        row.child(t::display_type(game.variant.title(), 24.).mr_2())
                    })
                    .children(self.mode_picker(cx))
                    .child(
                        div()
                            .flex()
                            .ml_auto()
                            .gap_2()
                            .child(self.undo_button(cx))
                            .child(Self::replay_button("solitaire-replay", cx))
                            .child(Self::new_deal_button("solitaire-new", cx)),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .min_h(px(table.min_height))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded_xl()
                    .border_1()
                    .border_color(rgb(0x0d2d24))
                    .bg(rgb(FELT))
                    .child(
                        img(FELT_ART)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .rounded_xl()
                            .object_fit(ObjectFit::Fill),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset(px(5.))
                            .rounded_lg()
                            .border_1()
                            .border_color(rgba(FELT_LINE)),
                    )
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .px(px(table.padding))
                            .pt(px(14.))
                            .pb(px(11.))
                            .text_color(rgb(t::SURFACE))
                            .child(score_strip(game))
                            .child(field)
                            .when(!won, |table| {
                                table.child(
                                    div()
                                        .border_t_1()
                                        .border_color(rgba(FELT_LINE))
                                        .pt_2()
                                        .text_size(px(13.))
                                        .line_height(px(19.))
                                        .text_color(rgb(
                                            if stalled || !self.deal.status.is_empty() {
                                                FLAX
                                            } else {
                                                LIGHT
                                            },
                                        ))
                                        .child(self.note(stalled)),
                                )
                            }),
                    ),
            )
    }
}

fn score_strip(game: &Game) -> Div {
    const METER_WIDTH: f32 = 75.;
    let (label, completed, total) = match game.variant {
        Variant::Klondike | Variant::FreeCell => ("Foundations", game.foundation_count(), 52),
        Variant::Spider => ("Runs cleared", game.runs, 8),
        Variant::Pyramid | Variant::TriPeaks => (
            "Cards cleared",
            game.layout.iter().filter(|card| card.is_none()).count(),
            game.layout.len(),
        ),
    };
    div()
        .w_full()
        .flex()
        .justify_between()
        .items_center()
        .gap_2()
        .pb_3()
        .border_b_1()
        .border_color(rgba(FELT_LINE))
        .text_size(px(13.))
        .line_height(px(21.))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_color(rgb(LIGHT)).child(label))
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(FLAX))
                        .child(format!("{completed} / {total}")),
                )
                .child(
                    div()
                        .ml_1()
                        .w(px(METER_WIDTH))
                        .h(px(4.))
                        .rounded_full()
                        .overflow_hidden()
                        .bg(rgb(0x426052))
                        .child(
                            div()
                                .h_full()
                                .w(px(METER_WIDTH * completed as f32 / total as f32))
                                .bg(rgb(0xd9ba75)),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_color(rgb(LIGHT)).child("Moves"))
                .child(
                    div()
                        .px_2()
                        .min_w(px(26.))
                        .rounded_md()
                        .bg(rgb(0x305747))
                        .text_center()
                        .font_weight(FontWeight::BOLD)
                        .child(game.moves.to_string()),
                ),
        )
}

#[derive(Clone, Copy)]
enum Slot {
    Plain,
    Recycle,
    Foundation(Suit),
}

#[derive(Clone, Copy)]
enum CardFace {
    Up(Card),
    Covered(Card),
    Back,
    Empty(Slot),
}

#[derive(Clone)]
struct CardDrag {
    owner: EntityId,
    revision: u64,
    from: Place,
    cards: Vec<Card>,
    size: CardSize,
}

impl Render for CardDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let step = self.size.face_step;
        div().pl(px(3.)).pt(px(4.)).child(
            div()
                .relative()
                .w(px(self.size.width))
                .h(px(
                    self.size.height() + self.cards.len().saturating_sub(1) as f32 * step
                ))
                .children(self.cards.iter().enumerate().map(|(index, &card)| {
                    div()
                        .absolute()
                        .top(px(index as f32 * step))
                        .left_0()
                        .child(
                            card_surface(CardFace::Up(card), self.size, false)
                                .border_color(rgb(0xe4c27c))
                                .shadow(vec![
                                    BoxShadow::new(px(1.), px(5.), rgba(0x070d09a8).into())
                                        .blur_radius(px(14.)),
                                ]),
                        )
                })),
        )
    }
}

fn card_surface(face: CardFace, size: CardSize, selected: bool) -> Div {
    let width = size.width;
    let card = div()
        .relative()
        .w(px(width))
        .h(px(size.height()))
        .flex_shrink_0()
        .rounded(px((width * 0.074).clamp(4., 9.)))
        .border_2();
    let floating = |card: Div| {
        card.shadow(vec![
            BoxShadow::new(px(0.), px(2.), rgba(0x080e0a6b).into()).blur_radius(px(3.)),
        ])
    };

    match face {
        CardFace::Empty(mark) => card
            .bg(rgb(SLOT))
            .border_color(rgb(SLOT_LINE))
            .text_color(rgb(0xaac2aa))
            .flex()
            .items_center()
            .justify_center()
            .child(match mark {
                Slot::Plain => div().w(px(width * 0.16)).h(px(1.)).bg(rgb(SLOT_LINE)),
                Slot::Recycle => card_symbol("↺", width * 0.36),
                Slot::Foundation(suit) => card_symbol(suit_mark(suit), width * 0.32),
            }),
        CardFace::Back => {
            let radius = px((width * 0.053).clamp(2., 7.));
            floating(card)
                .bg(rgb(0xe7d4ac))
                .border_color(rgb(0xccb38a))
                .child(
                    div()
                        .absolute()
                        .inset(px((width * 0.025).clamp(1., 3.)))
                        .rounded(radius)
                        .bg(rgb(0x214b52))
                        .child(
                            img(STOCK_ART)
                                .size_full()
                                .rounded(radius)
                                .object_fit(ObjectFit::Fill),
                        ),
                )
        }
        CardFace::Up(value) | CardFace::Covered(value) => {
            let covered = matches!(face, CardFace::Covered(_));
            let ink = if value.suit.red() { 0x9d3028 } else { 0x18342d };
            let (paper, edge) = if selected {
                (0xffefc7, t::GOLD)
            } else if covered {
                (0xd7cdb2, 0xc1b699)
            } else {
                (0xfff9e9, 0xe6d8bb)
            };
            let index_offset = px((width * 0.04).max(2.));
            let court = match value.rank {
                11 => Some("games/solitaire/court-jack.webp"),
                12 => Some("games/solitaire/court-queen.webp"),
                13 => Some("games/solitaire/court-king.webp"),
                _ => None,
            };
            floating(card)
                .bg(rgb(paper))
                .border_color(rgb(edge))
                .text_color(rgb(ink))
                .child(match court {
                    Some(art) => court_panel(art, size, covered),
                    None => pip_panel(value, size),
                })
                .child(
                    card_index(value, width, false)
                        .absolute()
                        .left(index_offset)
                        .top(index_offset),
                )
                .child(
                    card_index(value, width, true)
                        .absolute()
                        .right(index_offset)
                        .bottom(index_offset),
                )
        }
    }
}

fn court_panel(art: &'static str, size: CardSize, covered: bool) -> Div {
    let width = size.width * 0.58;
    let height = width * 1.5;
    div()
        .absolute()
        .left(px((size.width - width) * 0.5 - 2.))
        .top(px((size.height() - height) * 0.5 - 2.))
        .w(px(width))
        .h(px(height))
        .overflow_hidden()
        .rounded(px(2.))
        .border_1()
        .border_color(rgb(0xa88758))
        .child(
            img(art)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .opacity(if covered { 0.68 } else { 1. }),
        )
}

fn card_index(card: Card, width: f32, lower: bool) -> Div {
    let text_size = if lower {
        (width * 0.185).clamp(9.5, 22.)
    } else {
        (width * 0.218).clamp(10.5, 26.)
    };
    div()
        .flex()
        .items_center()
        .gap(px((width * 0.028).clamp(1., 3.)))
        .font_family(t::DISPLAY)
        .font_weight(FontWeight::BOLD)
        .line_height(px(text_size))
        .child(div().text_size(px(text_size)).child(rank(card.rank)))
        .child(
            div()
                .text_size(px(text_size * 0.80))
                .child(suit_mark(card.suit)),
        )
}

fn card_symbol(mark: &'static str, size: f32) -> Div {
    div()
        .font_family(t::DISPLAY)
        .text_size(px(size))
        .line_height(px(size))
        .child(mark)
}

fn pip_panel(card: Card, size: CardSize) -> Div {
    let pip_size = size.width
        * match card.rank {
            1 => 0.50,
            2..=5 => 0.25,
            _ => 0.205,
        };
    div()
        .absolute()
        .inset_0()
        .children(pip_positions(card.rank).iter().map(|&(x, y)| {
            div()
                .absolute()
                .left(px((size.width - 4.) * x - pip_size * 0.5))
                .top(px((size.height() - 4.) * y - pip_size * 0.5))
                .size(px(pip_size))
                .flex()
                .items_center()
                .justify_center()
                .child(card_symbol(suit_mark(card.suit), pip_size))
        }))
}

fn pip_positions(rank: u8) -> &'static [(f32, f32)] {
    const L: f32 = 0.32;
    const C: f32 = 0.50;
    const R: f32 = 0.68;
    match rank {
        1 => &[(C, 0.50)],
        2 => &[(C, 0.32), (C, 0.68)],
        3 => &[(C, 0.30), (C, 0.50), (C, 0.70)],
        4 => &[(L, 0.30), (R, 0.30), (L, 0.70), (R, 0.70)],
        5 => &[(L, 0.30), (R, 0.30), (C, 0.50), (L, 0.70), (R, 0.70)],
        6 => &[
            (L, 0.28),
            (R, 0.28),
            (L, 0.50),
            (R, 0.50),
            (L, 0.72),
            (R, 0.72),
        ],
        7 => &[
            (L, 0.28),
            (R, 0.28),
            (C, 0.39),
            (L, 0.50),
            (R, 0.50),
            (L, 0.72),
            (R, 0.72),
        ],
        8 => &[
            (L, 0.28),
            (R, 0.28),
            (C, 0.39),
            (L, 0.50),
            (R, 0.50),
            (C, 0.61),
            (L, 0.72),
            (R, 0.72),
        ],
        9 => &[
            (L, 0.26),
            (R, 0.26),
            (L, 0.42),
            (R, 0.42),
            (C, 0.50),
            (L, 0.58),
            (R, 0.58),
            (L, 0.74),
            (R, 0.74),
        ],
        10 => &[
            (L, 0.26),
            (R, 0.26),
            (C, 0.34),
            (L, 0.42),
            (R, 0.42),
            (L, 0.58),
            (R, 0.58),
            (C, 0.66),
            (L, 0.74),
            (R, 0.74),
        ],
        _ => &[],
    }
}

fn card_label(place: Place, face: CardFace, game: &Game) -> String {
    match (place, face) {
        (Place::Stock, CardFace::Empty(Slot::Recycle)) => "Recycle the waste into stock".into(),
        (Place::Stock, _) => format!("Stock, {} cards", game.stock.len()),
        (_, CardFace::Up(card) | CardFace::Covered(card)) => {
            let rank = match card.rank {
                1 => "Ace",
                11 => "Jack",
                12 => "Queen",
                13 => "King",
                value => rank(value),
            };
            format!("{rank} of {}", suit_name(card.suit))
        }
        (_, CardFace::Empty(Slot::Foundation(suit))) => {
            format!("Foundation for {}", suit_name(suit))
        }
        (Place::Cell(index), CardFace::Empty(_)) => format!("Empty free cell {}", index + 1),
        (Place::Tableau(column, _), CardFace::Empty(_)) => format!("Empty column {}", column + 1),
        (_, CardFace::Empty(_)) => "Empty pile".into(),
        (_, CardFace::Back) => "Face-down card".into(),
    }
}

fn suit_name(suit: Suit) -> &'static str {
    match suit {
        Suit::Spades => "spades",
        Suit::Hearts => "hearts",
        Suit::Clubs => "clubs",
        Suit::Diamonds => "diamonds",
    }
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
    div().flex().flex_col().gap(px(5.)).child(content).child(
        div()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(rgb(LIGHT))
            .child(label.into()),
    )
}

fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn rank(value: u8) -> &'static str {
    match value {
        1 => "A",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        8 => "8",
        9 => "9",
        10 => "10",
        11 => "J",
        12 => "Q",
        13 => "K",
        _ => "",
    }
}

fn suit_mark(value: Suit) -> &'static str {
    match value {
        Suit::Spades => "♠",
        Suit::Hearts => "♥",
        Suit::Clubs => "♣",
        Suit::Diamonds => "♦",
    }
}
