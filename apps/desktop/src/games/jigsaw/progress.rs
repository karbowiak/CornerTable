use corner_table_jigsaw::{Assembly, Placement, Puzzle, Size};
use serde::{Deserialize, Serialize};

use super::{AttemptId, GALLERY, GameView, Mode, PuzzleKey, Reference, Round, Tray};

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
    fn new(key: PuzzleKey, gallery: &super::Gallery) -> Self {
        Self {
            painting: gallery.painting(key.painting).id.to_string(),
            pieces: key.size.piece_count(),
        }
    }

    fn resolve(&self, gallery: &super::Gallery) -> Option<PuzzleKey> {
        let painting = GALLERY
            .iter()
            .chain(gallery.photos())
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
    #[serde(default)]
    attempt: AttemptId,
    #[serde(default)]
    moves: u64,
}

#[derive(Deserialize, Serialize)]
struct SavedSwap {
    seed: u64,
    tiles: Vec<usize>,
    moves: usize,
    #[serde(default)]
    attempt: AttemptId,
}

impl SavedRound {
    fn new(key: PuzzleKey, round: &Round, gallery: &super::Gallery) -> Self {
        Self {
            key: SavedKey::new(key, gallery),
            assembly: SavedAssembly {
                seed: round.assembly_deal,
                placements: round.assembly.placements().to_vec(),
                hints_used: round.tray.hints_used,
                attempt: round.assembly_attempt,
                moves: round.assembly_moves,
            },
            swap: SavedSwap {
                seed: round.swap_deal,
                tiles: round.swap.pieces().to_vec(),
                moves: round.swap.moves(),
                attempt: round.swap_attempt,
            },
        }
    }

    fn restore(self, gallery: &super::Gallery) -> Option<(PuzzleKey, Round)> {
        let key = self.key.resolve(gallery)?;
        let mut assembly = Assembly::new(key.size, self.assembly.seed);
        let valid = self.assembly.placements.into_iter().all(|piece| {
            matches!(
                assembly.place(piece, piece),
                Placement::Placed | Placement::Completed
            )
        });
        let (assembly, hints_used, assembly_moves, assembly_attempt) = if valid {
            let moves = self.assembly.moves.max(assembly.placed_count() as u64);
            (
                assembly,
                self.assembly.hints_used,
                moves,
                self.assembly.attempt,
            )
        } else {
            (
                Assembly::new(key.size, self.assembly.seed),
                0,
                0,
                AttemptId::default(),
            )
        };
        let (swap, swap_attempt) = match Puzzle::restore(key.size, self.swap.tiles, self.swap.moves)
        {
            Some(swap) => (swap, self.swap.attempt),
            None => (
                Puzzle::with_size(key.size, self.swap.seed),
                AttemptId::default(),
            ),
        };
        let round = Round {
            assembly,
            swap,
            assembly_deal: self.assembly.seed,
            swap_deal: self.swap.seed,
            assembly_attempt,
            swap_attempt,
            assembly_moves,
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
        if let Some(active) = saved.active.resolve(&self.gallery) {
            self.key = active;
        }
        self.mode = saved.mode;
        self.reference = saved.reference;
        self.seed = saved.seed;
        self.rounds = saved
            .rounds
            .into_iter()
            .filter_map(|round| round.restore(&self.gallery))
            .collect();
    }

    pub(super) fn save(&self) {
        let rounds = (0..GALLERY.len() + self.gallery.photos().len())
            .flat_map(|painting| Size::ALL.map(|size| PuzzleKey { painting, size }))
            .filter_map(|key| Some(SavedRound::new(key, self.rounds.get(&key)?, &self.gallery)))
            .collect();
        self.storage.save(&SavedGallery {
            active: SavedKey::new(self.key, &self.gallery),
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

    use super::super::Gallery;
    use super::*;

    #[test]
    fn round_trip_keeps_both_modes_and_the_assembly_undo_order() {
        let gallery = Gallery::default();
        let key = PuzzleKey {
            painting: 4,
            size: Size::FortyEight,
        };
        let mut round = Round::new(key.size, 32);
        round.assembly.place(18, 18);
        round.assembly.place(3, 3);
        round.assembly_moves = 4;
        round.tray.hints_used = 1;
        round.swap_deal = 99;
        round.swap = Puzzle::with_size(key.size, round.swap_deal);
        assert_eq!(round.swap.swap(2, 17), Selection::Swapped);
        let saved = serde_json::to_string(&SavedRound::new(key, &round, &gallery)).unwrap();
        assert!(saved.contains("orchard-market"));
        let (loaded_key, mut loaded) = serde_json::from_str::<SavedRound>(&saved)
            .unwrap()
            .restore(&gallery)
            .unwrap();

        assert!(loaded_key == key);
        assert_eq!(loaded.tray.hints_used, 1);
        assert_eq!(loaded.assembly_deal, round.assembly_deal);
        assert_eq!(loaded.assembly_moves, 4);
        assert!(loaded.assembly_attempt == round.assembly_attempt);
        assert!(loaded.swap_attempt == round.swap_attempt);
        assert!(loaded.assembly_attempt != loaded.swap_attempt);
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
    fn existing_painting_saves_start_with_their_already_placed_moves() {
        let gallery = Gallery::default();
        let key = PuzzleKey {
            painting: 1,
            size: Size::Twelve,
        };
        let mut round = Round::new(key.size, 7);
        round.assembly.place(5, 5);
        let mut saved = serde_json::to_value(SavedRound::new(key, &round, &gallery)).unwrap();
        let assembly = saved["assembly"].as_object_mut().unwrap();
        assembly.remove("attempt");
        assembly.remove("moves");
        saved["swap"].as_object_mut().unwrap().remove("attempt");

        let (loaded_key, loaded) = serde_json::from_value::<SavedRound>(saved)
            .unwrap()
            .restore(&gallery)
            .unwrap();
        assert!(loaded_key == key);
        assert_eq!(loaded.assembly.placements(), &[5]);
        assert_eq!(loaded.assembly_moves, 1);
        assert!(loaded.assembly_attempt != loaded.swap_attempt);
    }

    #[test]
    fn an_invalid_board_keeps_the_other_mode_playable() {
        let gallery = Gallery::default();
        let key = PuzzleKey {
            painting: 2,
            size: Size::TwentyFour,
        };
        let mut round = Round::new(key.size, 17);
        round.assembly.place(0, 0);
        round.swap.swap(0, 1);

        let mut saved = SavedRound::new(key, &round, &gallery);
        saved.assembly.placements.push(0);
        let (_, restored) = saved.restore(&gallery).unwrap();
        assert_eq!(restored.assembly.placed_count(), 0);
        assert_eq!(restored.swap.moves(), 1);

        let mut saved = SavedRound::new(key, &round, &gallery);
        saved.swap.tiles[1] = saved.swap.tiles[0];
        let (_, restored) = saved.restore(&gallery).unwrap();
        assert_eq!(restored.assembly.placed_count(), 1);
        assert_eq!(restored.swap.moves(), 0);
    }

    #[test]
    fn a_photo_round_resolves_by_id_if_its_gallery_position_changes() {
        use super::super::photos;
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let add_photo = |id: &str| {
            let path = directory.path().join(id);
            fs::create_dir(&path).unwrap();
            fs::File::create(path.join(photos::PICTURE)).unwrap();
            fs::File::create(path.join(photos::THUMBNAIL)).unwrap();
        };
        add_photo("photo-002");
        let gallery = Gallery::load(Some(directory.path().into()));
        let key = PuzzleKey {
            painting: GALLERY.len(),
            size: Size::Twelve,
        };
        let mut round = Round::new(key.size, 20);
        round.assembly.place(2, 2);
        assert_eq!(round.swap.swap(0, 1), Selection::Swapped);
        let saved = SavedRound::new(key, &round, &gallery);

        add_photo("photo-001");
        let gallery = Gallery::load(Some(directory.path().into()));
        let (key, restored) = saved.restore(&gallery).unwrap();
        assert_eq!(key.painting, GALLERY.len() + 1);
        assert_eq!(restored.assembly.placements(), &[2]);
        assert_eq!(restored.swap.pieces(), round.swap.pieces());
    }
}
