use corner_table_hearts::{Card, Suit};
use gpui::{BoxShadow, Div, FontWeight, ObjectFit, div, img, prelude::*, px, rgb, rgba};

use crate::{games::card_art, theme};

use super::{BRASS, DECK_RATIO, RIM_SOFT};

const BACK: &str = "games/solitaire/card-back.webp";

#[derive(Clone, Copy)]
pub(super) enum Face {
    Normal,
    Dimmed,
    Selected,
    Winner,
}

pub(super) fn suit_mark(suit: Suit) -> &'static str {
    match suit {
        Suit::Clubs => "♣",
        Suit::Diamonds => "♦",
        Suit::Spades => "♠",
        Suit::Hearts => "♥",
    }
}

pub(super) fn suit_name(suit: Suit) -> &'static str {
    match suit {
        Suit::Clubs => "clubs",
        Suit::Diamonds => "diamonds",
        Suit::Spades => "spades",
        Suit::Hearts => "hearts",
    }
}

fn rank_mark(rank: u8) -> &'static str {
    const RANKS: [&str; 13] = [
        "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A",
    ];
    RANKS[(rank - 2) as usize]
}

pub(super) fn card_name(card: Card) -> String {
    let rank = match card.rank() {
        11 => "jack",
        12 => "queen",
        13 => "king",
        14 => "ace",
        rank => rank_mark(rank),
    };
    format!("{rank} of {}", suit_name(card.suit()))
}

pub(super) fn face(card: Card, width: f32, kind: Face) -> Div {
    let (paper, rim) = match kind {
        Face::Normal => (0xfff6e5, 0xe5c8a4),
        Face::Dimmed => (0xd1b7ae, 0xb68d81),
        Face::Selected | Face::Winner => (0xffecbd, BRASS),
    };
    let ink = match card.suit() {
        Suit::Hearts | Suit::Diamonds => 0x952c39,
        Suit::Spades | Suit::Clubs => 0x253d35,
    };
    let size = (width * 0.31).clamp(13., 19.);
    let height = width * DECK_RATIO;
    let art = card_art::court_portrait(suit_mark(card.suit()), card.rank());
    let index = || {
        div()
            .flex()
            .items_center()
            .gap(px(1.))
            .text_size(px(size))
            .line_height(px(size + 1.))
            .font_weight(FontWeight::BOLD)
            .child(rank_mark(card.rank()))
            .child(
                div()
                    .text_size(px(size * 0.76))
                    .child(suit_mark(card.suit())),
            )
    };
    let top = if width < 46. { 3. } else { 4. };
    let center = if let Some(path) = art {
        let picture_width = width * 0.58;
        let picture_height = height * 0.59;
        div()
            .w(px(picture_width))
            .h(px(picture_height))
            .rounded(px(2.))
            .overflow_hidden()
            .border_1()
            .border_color(rgb(0x9c7753))
            .child(img(path).size_full().object_fit(ObjectFit::Cover).opacity(
                if matches!(kind, Face::Dimmed) {
                    0.54
                } else {
                    1.
                },
            ))
    } else {
        let pip = width * if card.rank() == 14 { 0.58 } else { 0.51 };
        div()
            .text_size(px(pip))
            .line_height(px(pip))
            .child(suit_mark(card.suit()))
    };
    div()
        .relative()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .rounded(px(5.))
        .border_1()
        .when(matches!(kind, Face::Selected | Face::Winner), |face| {
            face.border_2()
        })
        .border_color(rgb(rim))
        .bg(rgb(paper))
        .text_color(rgb(ink))
        .font_family(theme::DISPLAY)
        .shadow(vec![
            BoxShadow::new(px(0.), px(2.), rgba(0x18091465).into()).blur_radius(px(4.)),
        ])
        .child(
            div()
                .absolute()
                .top(px(size))
                .bottom(px(0.))
                .left(px(0.))
                .right(px(0.))
                .flex()
                .items_center()
                .justify_center()
                .child(center),
        )
        .child(index().absolute().top(px(top)).left(px(top)))
        .when(width >= 61. && art.is_none(), |face| {
            face.child(index().absolute().bottom(px(top)).right(px(top)))
        })
}

pub(super) fn slot(width: f32, active: bool) -> Div {
    div()
        .w(px(width))
        .h(px(width * DECK_RATIO))
        .flex_none()
        .rounded(px(5.))
        .border_1()
        .border_color(if active { rgb(BRASS) } else { rgba(RIM_SOFT) })
        .bg(rgba(0x3014216b))
        .flex()
        .justify_center()
        .items_center()
}

pub(super) fn back_stack(count: usize) -> Div {
    div()
        .relative()
        .w(px(34.))
        .h(px(36.))
        .flex_none()
        .children((0..count.min(3)).map(|index| {
            div()
                .absolute()
                .top(px(2. + index as f32 * 2.))
                .left(px(index as f32 * 4.))
                .w(px(21.))
                .h(px(28.))
                .p(px(1.))
                .rounded(px(3.))
                .bg(rgb(0xe6c7a6))
                .child(
                    img(BACK)
                        .size_full()
                        .rounded(px(2.))
                        .object_fit(ObjectFit::Fill),
                )
        }))
        .when(count > 0, |stack| {
            stack.child(
                div()
                    .absolute()
                    .right(px(0.))
                    .bottom(px(0.))
                    .min_w(px(17.))
                    .h(px(15.))
                    .px(px(2.))
                    .rounded(px(3.))
                    .bg(rgb(0x25131e))
                    .text_color(rgb(0xfff3e5))
                    .text_size(px(10.))
                    .line_height(px(15.))
                    .text_center()
                    .child(count.to_string()),
            )
        })
}
