use super::{Size, shuffled_order};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrayFilter {
    #[default]
    All,
    Edges,
    Interior,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Ignored,
    Missed,
    Placed,
    Completed,
}

pub struct Assembly {
    size: Size,
    tray: Vec<usize>,
    placements: Vec<usize>,
    placed: Vec<bool>,
}

impl Assembly {
    pub fn new(size: Size, seed: u64) -> Self {
        Self {
            size,
            tray: shuffled_order(size, seed),
            placements: Vec::new(),
            placed: vec![false; size.piece_count()],
        }
    }

    pub fn tray(&self, filter: TrayFilter) -> impl Iterator<Item = usize> + '_ {
        self.tray.iter().copied().filter(move |&piece| {
            !self.is_placed(piece)
                && match filter {
                    TrayFilter::All => true,
                    TrayFilter::Edges => self.is_edge(piece),
                    TrayFilter::Interior => !self.is_edge(piece),
                }
        })
    }

    pub fn is_edge(&self, piece: usize) -> bool {
        let (columns, rows) = self.size.dimensions();
        let (column, row) = (piece % columns, piece / columns);
        row < rows && (column == 0 || column + 1 == columns || row == 0 || row + 1 == rows)
    }

    pub fn is_placed(&self, piece: usize) -> bool {
        self.placed.get(piece).copied().unwrap_or(false)
    }

    pub fn placed_count(&self) -> usize {
        self.placements.len()
    }

    pub fn placements(&self) -> &[usize] {
        &self.placements
    }

    pub fn is_complete(&self) -> bool {
        self.placed_count() == self.placed.len()
    }

    pub fn place(&mut self, piece: usize, target: usize) -> Placement {
        if piece >= self.placed.len() || target >= self.placed.len() || self.is_placed(piece) {
            return Placement::Ignored;
        }
        if piece != target {
            return Placement::Missed;
        }

        self.placed[piece] = true;
        self.placements.push(piece);
        if self.is_complete() {
            Placement::Completed
        } else {
            Placement::Placed
        }
    }

    pub fn undo(&mut self) -> Option<usize> {
        let piece = self.placements.pop()?;
        self.placed[piece] = false;
        Some(piece)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tray_filters_partition_the_remaining_pieces() {
        for size in Size::ALL {
            let mut game = Assembly::new(size, 17);
            let (columns, rows) = size.dimensions();
            let edges: Vec<_> = game.tray(TrayFilter::Edges).collect();
            let interior: Vec<_> = game.tray(TrayFilter::Interior).collect();
            let all: Vec<_> = game.tray(TrayFilter::All).collect();
            assert_eq!(edges.len(), columns * 2 + (rows - 2) * 2);
            assert_eq!(interior.len(), (columns - 2) * (rows - 2));
            assert_eq!(edges.len() + interior.len(), all.len());
            assert!(edges.iter().all(|piece| !interior.contains(piece)));

            let first = all[0];
            assert_eq!(game.place(first, first), Placement::Placed);
            assert!(all[1..].iter().copied().eq(game.tray(TrayFilter::All)));
            assert_eq!(game.undo(), Some(first));
            assert!(all.into_iter().eq(game.tray(TrayFilter::All)));
        }
    }

    #[test]
    fn pieces_snap_only_at_their_own_position_and_finish_once() {
        let mut game = Assembly::new(Size::Twelve, 0);
        assert_eq!(game.place(0, 1), Placement::Missed);
        assert_eq!(
            game.place(0, Size::Twelve.piece_count()),
            Placement::Ignored
        );
        assert_eq!(
            game.place(Size::Twelve.piece_count(), 0),
            Placement::Ignored
        );
        assert_eq!(game.placed_count(), 0);
        assert_eq!(game.undo(), None);

        let pieces: Vec<_> = game.tray(TrayFilter::All).collect();
        for (index, &piece) in pieces.iter().enumerate() {
            let expected = if index + 1 == pieces.len() {
                Placement::Completed
            } else {
                Placement::Placed
            };
            assert_eq!(game.place(piece, piece), expected);
            assert_eq!(game.place(piece, piece), Placement::Ignored);
        }
        assert!(game.is_complete());
        assert_eq!(game.tray(TrayFilter::All).next(), None);
        assert_eq!(game.undo(), pieces.last().copied());
        assert!(!game.is_complete());
    }
}
