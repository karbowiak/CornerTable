//! Rules for the picture-tile and interlocking jigsaw puzzles.

mod assembly;
mod outline;

pub use assembly::{Assembly, Placement, TrayFilter};
pub use outline::{OutlinePoint, piece_outline};

pub const COLUMNS: usize = 4;
pub const ROWS: usize = 3;
pub const PIECE_COUNT: usize = COLUMNS * ROWS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    Twelve,
    TwentyFour,
    FortyEight,
}

impl Size {
    pub const ALL: [Self; 3] = [Self::Twelve, Self::TwentyFour, Self::FortyEight];

    pub fn dimensions(self) -> (usize, usize) {
        match self {
            Self::Twelve => (COLUMNS, ROWS),
            Self::TwentyFour => (6, 4),
            Self::FortyEight => (8, 6),
        }
    }

    pub fn piece_count(self) -> usize {
        let (columns, rows) = self.dimensions();
        columns * rows
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    Ignored,
    Selected,
    Cleared,
    Swapped,
    Completed,
}

pub struct Puzzle {
    pieces: Vec<usize>,
    selected: Option<usize>,
    moves: usize,
}

impl Puzzle {
    pub fn shuffled(seed: u64) -> Self {
        Self::with_size(Size::Twelve, seed)
    }

    pub fn with_size(size: Size, seed: u64) -> Self {
        Self {
            pieces: shuffled_order(size, seed),
            selected: None,
            moves: 0,
        }
    }

    pub fn pieces(&self) -> &[usize] {
        &self.pieces
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn moves(&self) -> usize {
        self.moves
    }

    pub fn placed_count(&self) -> usize {
        self.pieces
            .iter()
            .enumerate()
            .filter(|(position, piece)| *position == **piece)
            .count()
    }

    pub fn is_complete(&self) -> bool {
        self.placed_count() == self.pieces.len()
    }

    pub fn select(&mut self, position: usize) -> Selection {
        let Some(&piece) = self.pieces.get(position) else {
            return Selection::Ignored;
        };

        if piece == position {
            return Selection::Ignored;
        }

        match self.selected.take() {
            None => {
                self.selected = Some(position);
                Selection::Selected
            }
            Some(previous) if previous == position => Selection::Cleared,
            Some(previous) => self.swap(previous, position),
        }
    }

    pub fn swap(&mut self, first: usize, second: usize) -> Selection {
        if first == second
            || !self.pieces.get(first).is_some_and(|&piece| piece != first)
            || !self
                .pieces
                .get(second)
                .is_some_and(|&piece| piece != second)
        {
            return Selection::Ignored;
        }

        self.pieces.swap(first, second);
        self.moves += 1;
        self.selected = None;
        if self.is_complete() {
            Selection::Completed
        } else {
            Selection::Swapped
        }
    }
}

fn shuffled_order(size: Size, mut state: u64) -> Vec<usize> {
    let mut pieces: Vec<_> = (0..size.piece_count()).collect();
    // Swapping with strictly earlier positions makes a single cycle with no fixed pieces.
    for position in (1..pieces.len()).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let other = ((state >> 32) % position as u64) as usize;
        pieces.swap(position, other);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_preserves_all_pieces_and_displaces_each_one() {
        for size in Size::ALL {
            for seed in 0..128 {
                let puzzle = Puzzle::with_size(size, seed);
                let mut pieces = puzzle.pieces().to_vec();
                pieces.sort_unstable();
                assert!(pieces.into_iter().eq(0..size.piece_count()));
                assert_eq!(puzzle.placed_count(), 0);
            }
        }
        assert_eq!(Puzzle::shuffled(62).pieces(), Puzzle::shuffled(62).pieces());
        assert_ne!(Puzzle::shuffled(62).pieces(), Puzzle::shuffled(63).pieces());
    }

    #[test]
    fn reselecting_a_piece_cancels_without_a_move() {
        let mut puzzle = Puzzle::shuffled(0);
        assert_eq!(puzzle.select(PIECE_COUNT), Selection::Ignored);
        assert_eq!(puzzle.select(0), Selection::Selected);
        assert_eq!(puzzle.selected(), Some(0));
        assert_eq!(puzzle.select(0), Selection::Cleared);
        assert_eq!(puzzle.moves(), 0);
        assert_eq!(puzzle.selected(), None);
    }

    #[test]
    fn correct_pieces_lock_and_the_last_swap_completes_the_picture() {
        let mut puzzle = Puzzle::shuffled(0);
        for position in 0..PIECE_COUNT {
            if puzzle.pieces()[position] == position {
                continue;
            }
            let source = puzzle
                .pieces()
                .iter()
                .position(|&piece| piece == position)
                .unwrap();
            assert_eq!(puzzle.select(source), Selection::Selected);
            let result = puzzle.select(position);
            assert_eq!(puzzle.select(position), Selection::Ignored);
            assert_eq!(
                result,
                if puzzle.is_complete() {
                    Selection::Completed
                } else {
                    Selection::Swapped
                }
            );
        }
        assert!(puzzle.is_complete());
        assert_eq!(puzzle.moves(), PIECE_COUNT - 1);
        assert_eq!(puzzle.selected(), None);
    }
}
