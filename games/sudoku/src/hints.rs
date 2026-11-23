use super::{CELLS, Game, Occupancy, Unit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hint {
    pub index: usize,
    pub reason: HintReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintReason {
    OnlyCandidate { digit: u8 },
    OnlyPlace { digit: u8, unit: Unit },
    Duplicate { digit: u8, peer: usize, unit: Unit },
    IncorrectEntry { digit: u8 },
    Forced { digit: u8, candidates: u16 },
}

impl Hint {
    /// The suggested edit. Zero means the selected entry should be erased.
    pub fn entry(self) -> u8 {
        match self.reason {
            HintReason::OnlyCandidate { digit }
            | HintReason::OnlyPlace { digit, .. }
            | HintReason::Forced { digit, .. } => digit,
            HintReason::Duplicate { .. } | HintReason::IncorrectEntry { .. } => 0,
        }
    }
}

impl Game {
    /// Explains one move without changing the board or using the player's notes.
    pub fn hint(&self, preferred: usize) -> Option<Hint> {
        let cells = self.format.cell_count();
        let indices = || {
            Some(preferred)
                .into_iter()
                .filter(move |&index| index < cells)
                .chain((0..cells).filter(move |&index| index != preferred))
        };

        for index in indices().filter(|&index| !self.is_given(index)) {
            if let Some(peer) = self.conflict_peer(index) {
                let unit = self
                    .format
                    .units(index)
                    .into_iter()
                    .find(|unit| unit.contains(self.format, peer))
                    .expect("conflicting cells share a unit");
                return Some(Hint {
                    index,
                    reason: HintReason::Duplicate {
                        digit: self.value(index),
                        peer,
                        unit,
                    },
                });
            }
        }

        if let Some(index) = indices()
            .find(|&index| self.value(index) != 0 && self.value(index) != self.solution[index])
        {
            return Some(Hint {
                index,
                reason: HintReason::IncorrectEntry {
                    digit: self.value(index),
                },
            });
        }

        let used = Occupancy::from_values(self.format, &self.board.values);
        let mut candidates = [0; CELLS];
        let empty = || indices().filter(|&index| self.value(index) == 0);
        for index in empty() {
            let allowed = used.allowed(self.format, index);
            if allowed.count_ones() == 1 {
                return Some(Hint {
                    index,
                    reason: HintReason::OnlyCandidate {
                        digit: allowed.trailing_zeros() as u8,
                    },
                });
            }
            candidates[index] = allowed;
        }

        for index in empty() {
            for unit in self.format.units(index) {
                let other_candidates = unit
                    .cells(self.format)
                    .filter(|&other| other != index)
                    .fold(0, |mask, other| mask | candidates[other]);
                let unique_digits = candidates[index] & !other_candidates;
                if unique_digits != 0 {
                    return Some(Hint {
                        index,
                        reason: HintReason::OnlyPlace {
                            digit: unique_digits.trailing_zeros() as u8,
                            unit,
                        },
                    });
                }
            }
        }

        // Generation established uniqueness, and every current entry agrees with that solution.
        // Each other legal candidate here therefore leads to a contradiction further ahead.
        let index = empty().min_by_key(|&index| candidates[index].count_ones())?;
        Some(Hint {
            index,
            reason: HintReason::Forced {
                digit: self.solution[index],
                candidates: candidates[index],
            },
        })
    }
}
