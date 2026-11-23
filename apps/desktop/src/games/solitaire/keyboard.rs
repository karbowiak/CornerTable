use std::cmp::Ordering;

use corner_table_solitaire::{Game, Place, Variant};

use super::{BoardFrame, fan_len};

#[derive(Default)]
pub(super) struct Cursor {
    current: Option<Stop>,
    stops: Vec<Stop>,
}

impl Cursor {
    pub(super) fn update(&mut self, game: &Game, board: &BoardFrame<'_>) {
        self.stops.clear();
        if !board.interactive {
            // Keep the last place so undo can return to it from the result card.
            return;
        }
        collect_stops(game, board, &mut self.stops);
        self.current = self
            .current
            .and_then(|old| {
                self.stops
                    .iter()
                    .find(|stop| stop.place == old.place)
                    .or_else(|| {
                        self.stops
                            .iter()
                            .min_by(|a, b| old.recovery_score(**a).cmp(old.recovery_score(**b)))
                    })
            })
            .or_else(|| self.stops.first())
            .copied();
    }

    pub(super) fn place(&self) -> Option<Place> {
        if self.stops.is_empty() {
            return None;
        }
        self.current.map(|stop| stop.place)
    }

    pub(super) fn set(&mut self, place: Place) -> bool {
        let Some(stop) = self.stops.iter().find(|stop| stop.place == place) else {
            return false;
        };
        self.current = Some(*stop);
        true
    }

    pub(super) fn step(&mut self, direction: Direction) {
        if let Some(current) = self.current
            && let Some((_, next)) = self
                .stops
                .iter()
                .filter_map(|&stop| {
                    current
                        .direction_score(stop, direction)
                        .map(|score| (score, stop))
                })
                .min_by(|(a, _), (b, _)| a.cmp(*b))
        {
            self.current = Some(next);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub(super) fn from_key(key: &str) -> Option<Self> {
        match key {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct Stop {
    place: Place,
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
struct Score {
    fallback: bool,
    distance: f32,
    alignment: f32,
}

impl Score {
    fn cmp(self, other: Self) -> Ordering {
        self.fallback
            .cmp(&other.fallback)
            .then_with(|| self.distance.total_cmp(&other.distance))
            .then_with(|| self.alignment.total_cmp(&other.alignment))
    }
}

#[derive(PartialEq, Eq)]
enum Area {
    Rack,
    Tableau,
    Layout,
}

impl Stop {
    fn area(self) -> Area {
        match self.place {
            Place::Tableau(..) => Area::Tableau,
            Place::Layout(_) => Area::Layout,
            _ => Area::Rack,
        }
    }

    fn direction_score(self, target: Self, direction: Direction) -> Option<Score> {
        let dx = target.x - self.x;
        let dy = target.y - self.y;
        let (ahead, across) = match direction {
            Direction::Left => (-dx, dy),
            Direction::Right => (dx, dy),
            Direction::Up => (-dy, dx),
            Direction::Down => (dy, dx),
        };
        if ahead < 0.5 {
            return None;
        }
        let horizontal = matches!(direction, Direction::Left | Direction::Right);
        let same_area = self.area() == target.area();
        if horizontal && !same_area {
            return None;
        }
        if !same_area && (self.area() == Area::Tableau || target.area() == Area::Tableau) {
            // Enter the closest column or rack slot before comparing depths.
            return Some(Score {
                fallback: true,
                distance: across.abs(),
                alignment: ahead,
            });
        }
        if self.area() == Area::Tableau {
            if horizontal {
                return Some(Score {
                    fallback: false,
                    distance: ahead,
                    alignment: across.abs(),
                });
            }
            if across.abs() >= 0.5 {
                return None;
            }
        }
        Some(Score {
            fallback: false,
            distance: ahead.powi(2) + 4. * across.powi(2),
            alignment: across.abs(),
        })
    }

    fn recovery_score(self, target: Self) -> Score {
        let same_pile = match (self.place, target.place) {
            (Place::Tableau(a, _), Place::Tableau(b, _)) => a == b,
            (Place::Waste, Place::Stock) => true,
            (Place::Waste, _) => false,
            _ => self.area() == target.area(),
        };
        Score {
            fallback: !same_pile,
            distance: (self.x - target.x).powi(2) + (self.y - target.y).powi(2),
            alignment: 0.,
        }
    }
}

fn collect_stops(game: &Game, board: &BoardFrame<'_>, stops: &mut Vec<Stop>) {
    let size = board.size;
    let stride = size.stride();
    let layout = !game.layout.is_empty();
    let waste_step = size.waste_step(game.mode());
    let rack_x = if layout {
        (size.span() - stride - size.width - 2. * waste_step) / 2.
    } else {
        0.
    };
    let rack_y = if layout {
        size.layout_height(game.variant) + size.field_gap()
    } else {
        0.
    };
    let mut push = |place, x, y| stops.push(Stop { place, x, y });
    if game.variant == Variant::FreeCell {
        for index in 0..4 {
            push(Place::Cell(index), index as f32 * stride, rack_y);
        }
    } else {
        push(Place::Stock, rack_x, rack_y);
        if board.waste_top_visible(game) {
            let fan_offset = fan_len(game.mode(), game.waste.len()).saturating_sub(1);
            push(
                Place::Waste,
                rack_x + stride + fan_offset as f32 * waste_step,
                rack_y,
            );
        }
    }
    if matches!(game.variant, Variant::Klondike | Variant::FreeCell) {
        for index in 0..4 {
            push(
                Place::Foundation(index),
                (size.columns - 4 + index) as f32 * stride,
                rack_y,
            );
        }
    }
    let tableau_y = size.height() + 21. + size.field_gap(); // Rack label and gap.
    for (column, stack) in game.tableau.iter().enumerate() {
        if board
            .flight
            .as_ref()
            .is_some_and(|flight| flight.previous_column(column).is_some())
        {
            continue;
        }
        let x = column as f32 * stride;
        let mut y = tableau_y;
        if stack.is_empty() {
            push(Place::Tableau(column, 0), x, y);
        }
        for (index, entry) in stack.iter().enumerate() {
            let place = Place::Tableau(column, index);
            if game.selectable(place) {
                push(place, x, y);
            }
            y += size.stack_step(entry.face_up);
        }
    }
    if layout {
        for (index, x, y) in size.layout_positions(game.variant) {
            let place = Place::Layout(index);
            if game.selectable(place) {
                push(place, x, y);
            }
        }
    }
}
