use corner_table_solitaire::{Game, Hint, HintReason, Move, Place, Suit, Variant};
use gpui::{Div, FontWeight, div, prelude::*, px, rgb};

use super::{CardFace, CardSize, Slot, TablePalette, card_label, suit_name};

pub(super) enum Suggestion {
    Move(Hint),
    NoneAvailable,
}

impl Suggestion {
    pub(super) fn marker(&self, game: &Game, place: Place) -> Option<Marker> {
        let Self::Move(hint) = self else {
            return None;
        };
        let (from, to) = match hint.action {
            Move::Draw => (Place::Stock, None),
            Move::Clear(from) => (
                from,
                (game.variant == Variant::TriPeaks).then_some(Place::Waste),
            ),
            Move::Transfer { from, to } => (from, Some(to)),
        };
        if place == from {
            Some(Marker::Source)
        } else if Some(place) == to {
            Some(Marker::Target)
        } else {
            None
        }
    }

    pub(super) fn message(&self, game: &Game) -> String {
        use HintReason::*;
        let Self::Move(hint) = self else {
            return "No helpful move found. Try Undo or a new deal.".into();
        };
        match hint.action {
            Move::Draw => match hint.reason {
                Recycle => "Recycle the waste into stock.".into(),
                DealRow => "Deal one card onto each column.".into(),
                _ => "Draw from the stock.".into(),
            },
            Move::Clear(from) => {
                let card = describe(game, from);
                if hint.reason == Adjacent {
                    let waste = describe(game, Place::Waste);
                    format!("Clear {card}. It is one rank away from {waste} on the waste.")
                } else {
                    format!("Clear {card} on its own.")
                }
            }
            Move::Transfer { from, to } => {
                let source = describe(game, from);
                let target = describe(game, to);
                if hint.reason == Pair {
                    return format!("Pair {source} with {target} to make 13.");
                }
                let reason = match hint.reason {
                    Reveal => " to turn over a card",
                    SafeFoundation | Foundation => "",
                    EmptyColumn => " to open a column",
                    FreeCell => " to open a free cell",
                    JoinRun => " to build a sequence",
                    UseWaste => "",
                    SetUp => " to set up another move",
                    OpenDeal => " so you can deal another row",
                    CompleteRun => " to clear a full suited sequence",
                    _ => "",
                };
                format!("Move {source} onto {target}{reason}.")
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Marker {
    Source,
    Target,
}

impl Marker {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Source => "Hint source",
            Self::Target => "Hint destination",
        }
    }

    pub(super) fn mark(self, card: Div, size: CardSize, palette: &TablePalette) -> Div {
        let (number, fill, ink, outline, selector) = match self {
            Self::Source => (
                "1",
                palette.accent,
                palette.frame,
                palette.frame,
                "solitaire-hint-source",
            ),
            Self::Target => (
                "2",
                palette.frame,
                palette.accent,
                palette.accent,
                "solitaire-hint-target",
            ),
        };
        let height = (size.face_step - 4.).clamp(10., 15.);
        card.border_color(rgb(if matches!(self, Self::Source) {
            palette.accent
        } else {
            palette.slot_border
        }))
        .child(
            div()
                .absolute()
                .top(px(1.))
                .right(px(2.))
                .w(px(height + 3.))
                .h(px(height))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(outline))
                .bg(rgb(fill))
                .text_color(rgb(ink))
                .font_weight(FontWeight::BOLD)
                .text_size(px((height - 2.).max(9.)))
                .line_height(px(height))
                .when(cfg!(test), |marker| {
                    marker.debug_selector(move || selector.into())
                })
                .child(number),
        )
    }
}

fn describe(game: &Game, place: Place) -> String {
    let card = match place {
        Place::Tableau(column, index) => game.tableau[column].get(index).map(|entry| entry.card),
        Place::Layout(index) => game.layout[index],
        Place::Waste => game.waste.last().copied(),
        Place::Cell(index) => game.cells[index],
        Place::Foundation(index) => {
            return format!("the {} foundation", suit_name(Suit::ALL[index]));
        }
        Place::Stock => return "the stock".into(),
    };
    card_label(
        place,
        card.map_or(CardFace::Empty(Slot::Plain), CardFace::Up),
        game,
    )
}
