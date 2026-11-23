use corner_table_jigsaw::{Assembly, Placement, Puzzle, Size};
use serde::{Deserialize, Serialize};

use super::{GALLERY, GameView, Mode, PuzzleKey, Reference, Round, Tray};

#[derive(Deserialize, Serialize)]
pub(super) struct SavedGallery {
    active: SavedKey,
    mode: Mode,
    reference: Reference,
    seed: u64,
    rounds: Vec<SavedRound>,
}

#[derive(Deserialize, Serialize)]
struct SavedKey {
    painting: String,
    pieces: usize,
}

impl SavedKey {
    fn new(key: PuzzleKey) -> Self {
        Self {
            painting: GALLERY[key.painting].id.into(),
            pieces: key.size.piece_count(),
        }
    }

    fn resolve(&self) -> Option<PuzzleKey> {
        let painting = GALLERY
            .iter()
            .position(|painting| painting.id == self.painting)?;
        let size = Size::ALL
            .into_iter()
            .find(|size| size.piece_count() == self.pieces)?;
        Some(PuzzleKey { painting, size })
    }
}

#[derive(Deserialize, Serialize)]
struct SavedRound {
    key: SavedKey,
    assembly: SavedAssembly,
    swap: SavedSwap,
}

#[derive(Deserialize, Serialize)]
struct SavedAssembly {
    seed: u64,
    placements: Vec<usize>,
    hints_used: usize,
}

#[derive(Deserialize, Serialize)]
struct SavedSwap {
    seed: u64,
    tiles: Vec<usize>,
    moves: usize,
}

impl SavedRound {
    fn new(key: PuzzleKey, round: &Round) -> Self {
        Self {
            key: SavedKey::new(key),
            assembly: SavedAssembly {
                seed: round.assembly_deal,
                placements: round.assembly.placements().to_vec(),
                hints_used: round.tray.hints_used,
            },
            swap: SavedSwap {
                seed: round.swap_deal,
                tiles: round.swap.pieces().to_vec(),
                moves: round.swap.moves(),
            },
        }
    }

    fn restore(self) -> Option<(PuzzleKey, Round)> {
        let key = self.key.resolve()?;
        let mut assembly = Assembly::new(key.size, self.assembly.seed);
        let valid = self.assembly.placements.into_iter().all(|piece| {
            matches!(
                assembly.place(piece, piece),
                Placement::Placed | Placement::Completed
            )
        });
        let (assembly, hints_used) = if valid {
            (assembly, self.assembly.hints_used)
        } else {
            (Assembly::new(key.size, self.assembly.seed), 0)
        };
        let swap = Puzzle::restore(key.size, self.swap.tiles, self.swap.moves)
            .unwrap_or_else(|| Puzzle::with_size(key.size, self.swap.seed));
        let round = Round {
            assembly,
            swap,
            assembly_deal: self.assembly.seed,
            swap_deal: self.swap.seed,
            tray: Tray {
                hints_used,
                ..Tray::default()
            },
        };
        Some((key, round))
    }
}

impl GameView {
    pub(super) fn restore(&mut self, saved: SavedGallery) {
        if let Some(active) = saved.active.resolve() {
            self.key = active;
        }
        self.mode = saved.mode;
        self.reference = saved.reference;
        self.seed = saved.seed;
        self.rounds = saved
            .rounds
            .into_iter()
            .filter_map(SavedRound::restore)
            .collect();
    }

    pub(super) fn save(&self) {
        let rounds = (0..GALLERY.len())
            .flat_map(|painting| Size::ALL.map(|size| PuzzleKey { painting, size }))
            .filter_map(|key| Some(SavedRound::new(key, self.rounds.get(&key)?)))
            .collect();
        self.storage.save(&SavedGallery {
            active: SavedKey::new(self.key),
            mode: self.mode,
            reference: self.reference,
            seed: self.seed,
            rounds,
        });
    }
}

#[cfg(test)]
mod tests {
    use corner_table_jigsaw::{Selection, TrayFilter};

    use super::*;

    #[test]
    fn round_trip_keeps_both_modes_and_the_assembly_undo_order() {
        let key = PuzzleKey {
            painting: 4,
            size: Size::FortyEight,
        };
        let mut round = Round::new(key.size, 32);
        round.assembly.place(18, 18);
        round.assembly.place(3, 3);
        round.tray.hints_used = 1;
        round.swap_deal = 99;
        round.swap = Puzzle::with_size(key.size, round.swap_deal);
        assert_eq!(round.swap.swap(2, 17), Selection::Swapped);
        let saved = serde_json::to_string(&SavedRound::new(key, &round)).unwrap();
        assert!(saved.contains("orchard-market"));
        let (loaded_key, mut loaded) = serde_json::from_str::<SavedRound>(&saved)
            .unwrap()
            .restore()
            .unwrap();

        assert!(loaded_key == key);
        assert_eq!(loaded.tray.hints_used, 1);
        assert_eq!(loaded.assembly_deal, round.assembly_deal);
        assert!(
            loaded
                .assembly
                .tray(TrayFilter::All)
                .eq(round.assembly.tray(TrayFilter::All))
        );
        assert_eq!(loaded.assembly.undo(), Some(3));
        assert_eq!(loaded.assembly.undo(), Some(18));
        assert_eq!(loaded.swap_deal, round.swap_deal);
        assert_eq!(loaded.swap.pieces(), round.swap.pieces());
        assert_eq!(loaded.swap.moves(), 1);
    }

    #[test]
    fn an_invalid_board_keeps_the_other_mode_playable() {
        let key = PuzzleKey {
            painting: 2,
            size: Size::TwentyFour,
        };
        let mut round = Round::new(key.size, 17);
        round.assembly.place(0, 0);
        round.swap.swap(0, 1);

        let mut saved = SavedRound::new(key, &round);
        saved.assembly.placements.push(0);
        let (_, restored) = saved.restore().unwrap();
        assert_eq!(restored.assembly.placed_count(), 0);
        assert_eq!(restored.swap.moves(), 1);

        let mut saved = SavedRound::new(key, &round);
        saved.swap.tiles[1] = saved.swap.tiles[0];
        let (_, restored) = saved.restore().unwrap();
        assert_eq!(restored.assembly.placed_count(), 1);
        assert_eq!(restored.swap.moves(), 0);
    }
}
