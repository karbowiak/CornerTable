use corner_table_mine_trails::{Expedition, Room};
use serde::{Deserialize, Serialize};

const REVISION: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "move", content = "value", rename_all = "kebab-case")]
pub(super) enum Move {
    Dig(u8, u8),
    Flag(u8, u8),
    Sweep(u8, u8),
    Travel(u8),
    Retry,
}

impl Move {
    pub(super) fn apply(self, expedition: &mut Expedition) -> bool {
        let at = |x, y| (usize::from(x), usize::from(y));
        match self {
            Self::Dig(x, y) => expedition.dig(at(x, y)),
            Self::Flag(x, y) => expedition.toggle_flag(at(x, y)),
            Self::Sweep(x, y) => expedition.sweep(at(x, y)),
            Self::Travel(room) => Room::ALL
                .get(usize::from(room))
                .is_some_and(|&destination| expedition.travel(destination)),
            Self::Retry => expedition.retry_room(),
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub(super) struct Best {
    pub(super) rooms: usize,
    pub(super) cave_ins: u32,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Record {
    revision: u8,
    pub(super) seed: u64,
    moves: Vec<Move>,
    pub(super) best: Option<Best>,
}

impl Record {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            revision: REVISION,
            seed,
            moves: Vec::new(),
            best: None,
        }
    }

    pub(super) fn replay(&self, fixed_seed: Option<u64>) -> Option<Expedition> {
        if self.revision != REVISION || fixed_seed.is_some_and(|seed| self.seed != seed) {
            return None;
        }
        let mut expedition = Expedition::new(self.seed);
        self.moves
            .iter()
            .all(|&action| action.apply(&mut expedition))
            .then_some(expedition)
    }

    pub(super) fn remember(&mut self, action: Move, expedition: &Expedition) {
        if matches!(action, Move::Flag(..)) && self.moves.last() == Some(&action) {
            self.moves.pop();
        } else {
            self.moves.push(action);
        }
        if expedition.is_complete() {
            let current = Best {
                rooms: expedition.surveyed_count(),
                cave_ins: expedition.cave_ins(),
            };
            if self.best.is_none_or(|best| {
                current.rooms > best.rooms
                    || (current.rooms == best.rooms && current.cave_ins < best.cave_ins)
            }) {
                self.best = Some(current);
            }
        }
    }

    pub(super) fn restart(&mut self, seed: u64) -> Expedition {
        self.seed = seed;
        self.moves.clear();
        Expedition::new(seed)
    }

    pub(super) fn has_progress(&self) -> bool {
        !self.moves.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corner_table_mine_trails::Tile;

    #[test]
    fn record_replays_stable_moves_and_rejects_invalid_seeds_and_moves() {
        let mut record = Record::new(42);
        let mut expedition = Expedition::new(42);
        let (x, y) = (0..expedition.side())
            .flat_map(|y| (0..expedition.side()).map(move |x| (x, y)))
            .find(|&p| matches!(expedition.tile(p), Some(Tile::Covered { diamond: false })))
            .unwrap();
        let mark = Move::Flag(x as u8, y as u8);
        assert!(mark.apply(&mut expedition));
        record.remember(mark, &expedition);
        let saved = serde_json::to_string(&record).unwrap();
        let mut record: Record = serde_json::from_str(&saved).unwrap();
        assert_eq!(record.replay(Some(42)), Some(expedition.clone()));
        assert!(record.replay(Some(43)).is_none());
        assert!(mark.apply(&mut expedition));
        record.remember(mark, &expedition);
        assert!(!record.has_progress());
        assert_eq!(record.replay(None), Some(expedition));
        record.moves.push(Move::Travel(u8::MAX));
        assert!(record.replay(None).is_none());
    }
}
