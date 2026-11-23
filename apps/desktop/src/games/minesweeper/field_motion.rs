use std::time::{Duration, Instant};

use corner_table_minesweeper::{Board, Tile};

const UNCOVER: Duration = Duration::from_millis(145);
const STAGGER: Duration = Duration::from_millis(11);
const MAX_STAGGER: u32 = 8;
const FLAG: Duration = Duration::from_millis(170);
const IMPACT: Duration = Duration::from_millis(200);

pub(super) struct FieldMotion {
    started: Instant,
    duration: Duration,
    kind: Kind,
}

enum Kind {
    Reveal {
        width: usize,
        delays: Vec<Option<Duration>>,
    },
    Flag((usize, usize)),
    Impact,
}

#[derive(Clone, Copy)]
pub(super) enum Cue {
    Reveal(f32),
    Flag(f32),
    Impact(f32),
}

impl FieldMotion {
    pub(super) fn reveal(before: &Board, after: &Board, origin: (usize, usize)) -> Self {
        let (width, height) = after.difficulty().dimensions();
        let delays: Vec<_> = (0..width * height)
            .map(|index| {
                let (x, y) = (index % width, index / width);
                (matches!(after.tile(x, y), Some(Tile::Open(_)))
                    && !matches!(before.tile(x, y), Some(Tile::Open(_))))
                .then(|| {
                    let steps = x.abs_diff(origin.0) + y.abs_diff(origin.1);
                    STAGGER * (steps as u32).min(MAX_STAGGER)
                })
            })
            .collect();
        let last_delay = delays.iter().flatten().copied().max().unwrap_or_default();
        Self::new(Kind::Reveal { width, delays }, UNCOVER + last_delay)
    }

    pub(super) fn flag(position: (usize, usize)) -> Self {
        Self::new(Kind::Flag(position), FLAG)
    }

    pub(super) fn impact() -> Self {
        Self::new(Kind::Impact, IMPACT)
    }

    fn new(kind: Kind, duration: Duration) -> Self {
        Self {
            started: Instant::now(),
            duration,
            kind,
        }
    }

    pub(super) fn playing(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) < self.duration
    }

    pub(super) fn cue(&self, (x, y): (usize, usize), now: Instant) -> Option<Cue> {
        let elapsed = now.saturating_duration_since(self.started);
        let progress = |time: Duration, duration: Duration| {
            (time < duration).then(|| time.as_secs_f32() / duration.as_secs_f32())
        };
        match &self.kind {
            Kind::Reveal { width, delays } => {
                let delay = delays[y * width + x]?;
                progress(elapsed.saturating_sub(delay), UNCOVER).map(Cue::Reveal)
            }
            Kind::Flag(position) if *position == (x, y) => progress(elapsed, FLAG).map(Cue::Flag),
            Kind::Impact => progress(elapsed, IMPACT).map(Cue::Impact),
            Kind::Flag(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use corner_table_minesweeper::Difficulty;

    use super::*;

    #[test]
    fn reveal_wave_visits_new_squares_in_order_and_then_clears() {
        let origin = (4, 4);
        let neighbor = (5, 4);
        let mut before = Board::new(Difficulty::Meadow, 19);
        assert!(before.toggle_flag(0, 0));
        let mut after = before.clone();
        assert!(after.reveal(origin.0, origin.1));
        let wave = FieldMotion::reveal(&before, &after, origin);

        let anchor_time = wave.started + STAGGER / 2;
        assert!(matches!(wave.cue(origin, anchor_time), Some(Cue::Reveal(value)) if value > 0.));
        assert!(matches!(
            wave.cue(neighbor, anchor_time),
            Some(Cue::Reveal(0.))
        ));
        assert!(wave.cue((0, 0), anchor_time).is_none());

        let after_motion = wave.started + wave.duration;
        assert!(!wave.playing(after_motion));
        assert!(wave.cue(neighbor, after_motion).is_none());
    }
}
