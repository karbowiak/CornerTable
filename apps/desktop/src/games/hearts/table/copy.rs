use corner_table_hearts::{
    Game, HandResult, Pass, Phase, Seat, Suit, TRICKS_PER_HAND, TrickResult,
};

use super::super::session::TableSession;
use super::cards::suit_name;

pub(super) struct PhaseCopy {
    pub(super) title: String,
    pub(super) detail: String,
}

impl PhaseCopy {
    fn new(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
        }
    }

    pub(super) fn for_table(table: &TableSession) -> Self {
        let game = table.game();
        let direction = game.pass_direction();
        if table.ready_to_open() {
            return if direction == Pass::Keep {
                Self::new(
                    "Keep your cards",
                    "This hand skips the pass. The 2 of clubs opens.",
                )
            } else {
                let from = player(direction.sender(Seat::You));
                Self::new(
                    "Three cards received",
                    format!("{from} sent the marked cards. The 2 of clubs opens."),
                )
            };
        }
        match game.phase() {
            Phase::Passing => {
                let to = player(direction.recipient(Seat::You));
                let heading = match direction {
                    Pass::Left => format!("Pass left to {to}"),
                    Pass::Right => format!("Pass right to {to}"),
                    Pass::Across => format!("Pass across to {to}"),
                    Pass::Keep => unreachable!("keep deals start at the opening lead"),
                };
                let detail = if table.pass_turn().is_some() {
                    "Ready to pass. Select a marked card again to change your choice."
                } else {
                    "Choose three cards to give away before the first trick."
                };
                Self::new(heading, detail)
            }
            Phase::Playing(Seat::You) => Self::your_turn(game),
            Phase::Playing(seat) => {
                let detail = if game.trick_number() == 1 && game.trick().is_empty() {
                    "The 2 of clubs leads the first trick.".into()
                } else if let Some(lead) = game.trick().first() {
                    format!("{} led {}.", player(lead.seat), suit_name(lead.card.suit()))
                } else {
                    game.last_trick().map_or_else(String::new, took_trick)
                };
                Self::new(format!("{} is playing", player(seat)), detail)
            }
            Phase::TrickComplete(result) => {
                let detail = if game.trick_number() == TRICKS_PER_HAND {
                    format!("{} The hand is ready to score.", took_trick(result))
                } else {
                    format!(
                        "{} {} next.",
                        took_trick(result),
                        if result.winner == Seat::You {
                            "You lead".into()
                        } else {
                            format!("{} leads", player(result.winner))
                        },
                    )
                };
                Self::new("Trick complete", detail)
            }
            Phase::HandComplete(result) => Self::score(game, result),
        }
    }

    fn your_turn(game: &Game) -> Self {
        let first = game.trick_number() == 1;
        let hand = game.hand(Seat::You);
        if let Some(lead) = game.trick().first() {
            let suit = lead.card.suit();
            if hand.iter().any(|card| card.suit() == suit) {
                Self::new(
                    format!("Follow {}", suit_name(suit)),
                    "Play a card in the led suit.",
                )
            } else if first && hand.iter().any(|card| card.points() == 0) {
                Self::new(
                    "Discard a safe card",
                    "You have no clubs. The first trick is free of points when possible.",
                )
            } else if first {
                Self::new(
                    "Discard a card",
                    "You have only point cards, so any of them may be played.",
                )
            } else {
                Self::new(
                    "Discard any card",
                    format!("You have no {} to follow.", suit_name(suit)),
                )
            }
        } else if first {
            Self::new(
                "Open with the 2 of clubs",
                "You have the card that starts the hand.",
            )
        } else if !game.hearts_broken() && hand.iter().any(|card| card.suit() != Suit::Hearts) {
            Self::new(
                "Your lead",
                "Choose a club, diamond, or spade until hearts are broken.",
            )
        } else {
            Self::new("Your lead", "Choose any card to start the trick.")
        }
    }

    fn score(game: &Game, result: HandResult) -> Self {
        let moon = result.moon.map(|seat| {
            format!(
                "{} shot the moon. Everyone else adds 26 points.",
                player(seat)
            )
        });
        if let Some(winner) = result.winner {
            let score = game.scores()[winner.index()];
            let title = if winner == Seat::You {
                "You win the match".into()
            } else {
                format!("{} wins the match", player(winner))
            };
            let detail = format!("The lowest score wins with {}.", points(score));
            let detail = moon.map_or(detail.clone(), |moon| format!("{detail} {moon}"));
            Self::new(title, detail)
        } else if result.tied_at_limit {
            let detail = "The lowest scores are tied at the match limit. Deal another hand.";
            Self::new(
                "Play another hand",
                moon.map_or_else(|| detail.to_owned(), |moon| format!("{detail} {moon}")),
            )
        } else {
            let detail = moon.unwrap_or_else(|| {
                format!(
                    "You added {} this hand.",
                    points(u16::from(result.points[0]))
                )
            });
            Self::new("Hand complete", detail)
        }
    }
}

pub(super) fn player(seat: Seat) -> &'static str {
    match seat {
        Seat::You => "You",
        Seat::West => "Ada",
        Seat::North => "Milo",
        Seat::East => "June",
    }
}

pub(super) fn points(count: u16) -> String {
    format!("{count} point{}", if count == 1 { "" } else { "s" })
}

fn took_trick(result: TrickResult) -> String {
    let worth = if result.points == 0 {
        "no points".into()
    } else {
        points(u16::from(result.points))
    };
    format!("{} took {worth}.", player(result.winner))
}
