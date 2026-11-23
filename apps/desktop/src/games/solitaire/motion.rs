use std::{cell::Cell, rc::Rc, time::Instant};

use corner_table_solitaire::{Card, DealMode, Game, TableCard, Variant};
use gpui::{
    AvailableSpace, BoxShadow, IntoElement, Pixels, Point, canvas, point, prelude::*, px, rgba,
};

use super::{CardFace, CardSize, TablePalette, card_surface};

pub(super) struct DealMotion {
    started: Instant,
    route: Route,
}

enum Route {
    Waste {
        previous: Vec<Card>,
        cards: Vec<Card>,
    },
    Spider(Vec<SpiderColumn>),
    Recycle {
        card: Card,
        fan_index: usize,
    },
}

struct SpiderColumn {
    previous: Vec<TableCard>,
    dealt: Card,
    collected: bool,
}

#[derive(Clone, Copy)]
pub(super) enum Anchor {
    Stock,
    Waste,
    Tableau,
}

type Anchors = Rc<Cell<[Option<Point<Pixels>>; 3]>>;

pub(super) struct DealFrame<'a> {
    route: &'a Route,
    elapsed_ms: f32,
    anchors: Anchors,
}

impl DealMotion {
    pub(super) fn from_draw(before: Game, after: &Game, started: Instant) -> Option<Self> {
        let route = if after.variant == Variant::Spider {
            Route::Spider(
                before.stock[after.stock.len()..]
                    .iter()
                    .rev()
                    .zip(before.tableau.into_iter().zip(&after.tableau))
                    .map(|(&dealt, (previous, current))| SpiderColumn {
                        dealt,
                        collected: current.len() <= previous.len(),
                        previous,
                    })
                    .collect(),
            )
        } else if after.waste.len() > before.waste.len() {
            let cards = after.waste[before.waste.len()..].to_vec();
            Route::Waste {
                previous: before.waste,
                cards,
            }
        } else {
            Route::Recycle {
                card: *before.waste.last()?,
                fan_index: fan_len(before.mode(), before.waste.len()) - 1,
            }
        };
        Some(Self { started, route })
    }

    pub(super) fn finished(&self, now: Instant) -> bool {
        let count = match &self.route {
            Route::Waste { cards, .. } => cards.len(),
            Route::Spider(columns) => columns.len(),
            Route::Recycle { .. } => 1,
        };
        let stop_ms = (0..count)
            .map(|index| self.route.timing(index).end())
            .fold(0., f32::max);
        elapsed_ms(self.started, now) >= stop_ms
    }

    pub(super) fn frame(&self, now: Instant) -> DealFrame<'_> {
        DealFrame {
            elapsed_ms: elapsed_ms(self.started, now),
            route: &self.route,
            anchors: Rc::default(),
        }
    }
}

fn elapsed_ms(started: Instant, now: Instant) -> f32 {
    now.saturating_duration_since(started).as_secs_f32() * 1000.
}

impl Route {
    fn timing(&self, index: usize) -> Timing {
        let order = index as f32;
        match self {
            Self::Waste { .. } => Timing {
                delay: order * 55.,
                duration: 235.,
                hold: 0.,
            },
            Self::Spider(columns) => Timing {
                delay: order * 19.,
                duration: 238. + order * 5.,
                // Show the completed run in contact before removing it from the column.
                hold: if columns[index].collected { 65. } else { 0. },
            },
            Self::Recycle { .. } => Timing {
                delay: 0.,
                duration: 255.,
                hold: 0.,
            },
        }
    }
}

struct Timing {
    delay: f32,
    duration: f32,
    hold: f32,
}

impl Timing {
    fn end(&self) -> f32 {
        self.delay + self.duration + self.hold
    }
}

pub(super) fn fan_len(mode: DealMode, count: usize) -> usize {
    count.min(if mode == DealMode::KlondikeDrawThree {
        3
    } else {
        1
    })
}

impl DealFrame<'_> {
    fn pending(&self, index: usize) -> bool {
        self.elapsed_ms < self.route.timing(index).end()
    }

    pub(super) fn recycling(&self) -> bool {
        matches!(self.route, Route::Recycle { .. })
    }

    pub(super) fn previous_waste(&self) -> Option<&[Card]> {
        match self.route {
            Route::Waste { previous, .. } if self.pending(0) => Some(previous),
            _ => None,
        }
    }

    pub(super) fn waste_pending(&self, index: usize) -> bool {
        match self.route {
            Route::Waste { previous, .. } => index
                .checked_sub(previous.len())
                .is_some_and(|index| self.pending(index)),
            _ => false,
        }
    }

    pub(super) fn previous_column(&self, column: usize) -> Option<&[TableCard]> {
        match self.route {
            Route::Spider(columns) if self.pending(column) => Some(&columns[column].previous),
            _ => None,
        }
    }

    pub(super) fn measure(&self, anchor: Anchor) -> impl IntoElement + use<> {
        let anchors = self.anchors.clone();
        canvas(
            move |bounds, _, _| {
                let mut all = anchors.get();
                all[anchor as usize] = Some(bounds.origin);
                anchors.set(all);
            },
            |_, (), _, _| {},
        )
        .absolute()
        .inset_0()
    }

    pub(super) fn layer(
        &self,
        size: CardSize,
        mode: DealMode,
        palette: &'static TablePalette,
    ) -> impl IntoElement + use<> {
        let mut cards = Vec::new();
        let mut push = |index, card, end, offset| {
            if let Some(progress) = self.progress(index) {
                cards.push(Flight {
                    index,
                    card,
                    from: Anchor::Stock,
                    to: end,
                    offset,
                    progress,
                });
            }
        };
        match self.route {
            Route::Waste {
                previous,
                cards: drawn,
            } => {
                let count = previous.len() + drawn.len();
                let first = count - fan_len(mode, count);
                for (index, &card) in drawn.iter().enumerate() {
                    let fan_index = previous.len() + index - first;
                    push(
                        index,
                        card,
                        Anchor::Waste,
                        point(px(fan_index as f32 * size.waste_step(mode)), px(0.)),
                    );
                }
            }
            Route::Spider(columns) => {
                for (column, arrival) in columns.iter().enumerate() {
                    let offset = arrival
                        .previous
                        .iter()
                        .map(|entry| size.stack_step(entry.face_up))
                        .sum::<f32>();
                    push(
                        column,
                        arrival.dealt,
                        Anchor::Tableau,
                        point(px(column as f32 * size.stride()), px(offset)),
                    );
                }
            }
            Route::Recycle { card, fan_index } => {
                if let Some(progress) = self.progress(0) {
                    cards.push(Flight {
                        index: 0,
                        card: *card,
                        from: Anchor::Waste,
                        to: Anchor::Stock,
                        offset: point(px(*fan_index as f32 * size.waste_step(mode)), px(0.)),
                        progress,
                    });
                }
            }
        }

        let anchors = self.anchors.clone();
        // The slots prepaint first, so the sprites can use this frame's actual positions.
        // Painting last lets cards pass over the felt and other piles without taking input.
        canvas(
            move |_, window, cx| {
                let anchors = anchors.get();
                cards
                    .into_iter()
                    .filter_map(|flight| {
                        let (origin, face) = flight.pose(anchors, size)?;
                        let flight_shadow = 1. - flight.progress;
                        let index = flight.index;
                        let mut sprite = card_surface(face, size, false, palette)
                            .shadow(vec![
                                BoxShadow::new(
                                    px(0.),
                                    px(2. + 6. * flight_shadow),
                                    rgba(0x090b0bb0).into(),
                                )
                                .blur_radius(px(4. + 10. * flight_shadow)),
                            ])
                            .when(cfg!(test), |sprite| {
                                sprite.debug_selector(move || format!("solitaire-flight-{index}"))
                            })
                            .into_any_element();
                        sprite.prepaint_as_root(
                            origin,
                            gpui::size(AvailableSpace::MinContent, AvailableSpace::MinContent),
                            window,
                            cx,
                        );
                        Some(sprite)
                    })
                    .collect::<Vec<_>>()
            },
            |_, mut sprites, window, cx| {
                for sprite in &mut sprites {
                    sprite.paint(window, cx);
                }
            },
        )
        .absolute()
        .inset_0()
    }

    fn progress(&self, index: usize) -> Option<f32> {
        let timing = self.route.timing(index);
        (timing.delay..timing.end())
            .contains(&self.elapsed_ms)
            .then(|| ((self.elapsed_ms - timing.delay) / timing.duration).min(1.))
    }
}

struct Flight {
    index: usize,
    card: Card,
    from: Anchor,
    to: Anchor,
    offset: Point<Pixels>,
    progress: f32,
}

impl Flight {
    fn pose(
        &self,
        anchors: [Option<Point<Pixels>>; 3],
        size: CardSize,
    ) -> Option<(Point<Pixels>, CardFace)> {
        let reverse = matches!(self.to, Anchor::Stock);
        let mut start = anchors[self.from as usize]?;
        let mut end = anchors[self.to as usize]?;
        if reverse {
            start += self.offset;
        } else {
            end += self.offset;
        }
        let p = self.progress;
        let travel = p * (2. - p);
        let lift = if matches!(self.to, Anchor::Tableau) {
            (size.width * 0.28).clamp(12., 27.)
        } else {
            (size.width * 0.18).clamp(9., 18.)
        };
        let arc = 4. * p * (1. - p) * lift;
        let origin = start + (end - start) * travel - point(px(0.), px(arc));
        let face = if (p >= 0.38) != reverse {
            CardFace::Up(self.card)
        } else {
            CardFace::Back
        };
        Some((origin, face))
    }
}
