//! Seeded Minesweeper rules independent of the desktop view.

mod logic;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Meadow,
    Highlands,
    Summit,
}

impl Difficulty {
    pub const ALL: [Self; 3] = [Self::Meadow, Self::Highlands, Self::Summit];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Meadow => "Meadow",
            Self::Highlands => "Highlands",
            Self::Summit => "Summit",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Meadow => "meadow",
            Self::Highlands => "highlands",
            Self::Summit => "summit",
        }
    }

    pub const fn dimensions(self) -> (usize, usize) {
        match self {
            Self::Meadow => (9, 9),
            Self::Highlands => (16, 12),
            Self::Summit => (22, 16),
        }
    }

    pub const fn mines(self) -> usize {
        match self {
            Self::Meadow => 10,
            Self::Highlands => 30,
            Self::Summit => 70,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ready,
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Covered,
    Flagged,
    Open(u8),
    Mine,
    Detonated,
    Misflagged,
}

#[derive(Clone, Copy, Debug, Default)]
struct Cell {
    mine: bool,
    adjacent_mines: u8,
    open: bool,
    flagged: bool,
}

/// A field and its progress as row-major cell indices. Persist the mine layout
/// itself so restored and replayed fields are independent of future generators.
#[derive(Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub seed: u64,
    pub opening: Option<usize>,
    pub mines: Vec<usize>,
    pub open: Vec<usize>,
    pub flags: Vec<usize>,
    pub detonated: Option<usize>,
}

#[derive(Clone)]
pub struct Board {
    difficulty: Difficulty,
    seed: u64,
    opening: Option<usize>,
    status: Status,
    cells: Vec<Cell>,
    detonated: Option<usize>,
    safe_remaining: usize,
    flags: usize,
}

impl Board {
    pub fn new(difficulty: Difficulty, seed: u64) -> Self {
        let (width, height) = difficulty.dimensions();
        Self {
            difficulty,
            seed,
            opening: None,
            status: Status::Ready,
            cells: vec![Cell::default(); width * height],
            detonated: None,
            safe_remaining: width * height - difficulty.mines(),
            flags: 0,
        }
    }

    pub fn checkpoint(&self) -> Checkpoint {
        let mut checkpoint = Checkpoint {
            seed: self.seed,
            opening: self.opening,
            mines: Vec::new(),
            open: Vec::new(),
            flags: Vec::new(),
            detonated: self.detonated,
        };
        for (index, cell) in self.cells.iter().enumerate() {
            if cell.mine {
                checkpoint.mines.push(index);
            }
            if cell.open {
                checkpoint.open.push(index);
            }
            if cell.flagged {
                checkpoint.flags.push(index);
            }
        }
        checkpoint
    }

    pub fn restore(difficulty: Difficulty, saved: Checkpoint) -> Option<Self> {
        let mut board = Self::new(difficulty, saved.seed);
        if let Some(opening) = saved.opening {
            board.install_layout(opening, &saved.mines)?;
        } else if !saved.mines.is_empty() || !saved.open.is_empty() || saved.detonated.is_some() {
            return None;
        }

        for index in saved.open {
            let cell = board.cells.get_mut(index)?;
            if cell.mine || cell.open {
                return None;
            }
            cell.open = true;
            board.safe_remaining -= 1;
        }
        for index in saved.flags {
            let cell = board.cells.get_mut(index)?;
            if cell.open || cell.flagged || board.opening == Some(index) {
                return None;
            }
            cell.flagged = true;
            board.flags += 1;
        }

        if let Some(opening) = board.opening {
            board.status = if board.cells[opening].open {
                Status::Playing
            } else if board.safe_remaining == board.cells.len() - difficulty.mines() {
                Status::Ready
            } else {
                return None;
            };
        }
        if let Some(index) = saved.detonated {
            let cell = board.cells.get(index)?;
            if board.status != Status::Playing
                || board.safe_remaining == 0
                || !cell.mine
                || cell.flagged
            {
                return None;
            }
            board.detonate(index);
        } else {
            board.check_win();
        }
        Some(board)
    }

    /// Start with the same mines and opening, with all tiles covered again.
    pub fn replay(&self) -> Option<Self> {
        let opening = self.opening?;
        let mut board = Self::new(self.difficulty, self.seed);
        for (target, source) in board.cells.iter_mut().zip(&self.cells) {
            target.mine = source.mine;
            target.adjacent_mines = source.adjacent_mines;
        }
        board.opening = Some(opening);
        Some(board)
    }

    pub fn required_opening(&self) -> Option<(usize, usize)> {
        let opening = self.opening.filter(|_| self.status == Status::Ready)?;
        let (width, _) = self.difficulty.dimensions();
        Some((opening % width, opening / width))
    }

    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn safe_remaining(&self) -> usize {
        self.safe_remaining
    }

    pub fn flags(&self) -> usize {
        self.flags
    }

    pub fn tile(&self, x: usize, y: usize) -> Option<Tile> {
        let index = self.index(x, y)?;
        let cell = self.cells[index];
        Some(match cell {
            _ if self.detonated == Some(index) => Tile::Detonated,
            _ if self.status == Status::Lost && cell.flagged && !cell.mine => Tile::Misflagged,
            _ if cell.flagged || (self.status == Status::Won && cell.mine) => Tile::Flagged,
            _ if self.status == Status::Lost && cell.mine => Tile::Mine,
            _ if cell.open => Tile::Open(cell.adjacent_mines),
            _ => Tile::Covered,
        })
    }

    pub fn toggle_flag(&mut self, x: usize, y: usize) -> bool {
        let Some(index) = self.index(x, y) else {
            return false;
        };
        let cell = &mut self.cells[index];
        if cell.open
            || self.opening == Some(index)
            || matches!(self.status, Status::Won | Status::Lost)
        {
            return false;
        }
        cell.flagged = !cell.flagged;
        if cell.flagged {
            self.flags += 1;
        } else {
            self.flags -= 1;
        }
        true
    }

    pub fn reveal(&mut self, x: usize, y: usize) -> bool {
        let Some(index) = self.index(x, y) else {
            return false;
        };
        if self.cells[index].flagged
            || self.cells[index].open
            || matches!(self.status, Status::Won | Status::Lost)
        {
            return false;
        }
        if self.status == Status::Ready {
            match self.opening {
                Some(opening) if opening != index => return false,
                Some(_) => {}
                None => self.place_mines(index),
            }
            self.status = Status::Playing;
        }
        if self.cells[index].mine {
            self.detonate(index);
        } else {
            self.open_area(index);
            self.check_win();
        }
        true
    }

    /// Open the unflagged neighbors of an open clue when its flag count matches.
    /// A misplaced flag can cause the chord to hit an unflagged mine.
    pub fn chord(&mut self, x: usize, y: usize) -> bool {
        let Some(targets) = self.chord_targets(x, y) else {
            return false;
        };
        let covered: Vec<_> = targets.collect();
        if let Some(&mine) = covered.iter().find(|&&i| self.cells[i].mine) {
            self.detonate(mine);
            return true;
        }
        for &neighbor in &covered {
            self.open_area(neighbor);
        }
        self.check_win();
        !covered.is_empty()
    }

    pub fn can_chord(&self, x: usize, y: usize) -> bool {
        self.chord_targets(x, y)
            .is_some_and(|mut targets| targets.next().is_some())
    }

    fn chord_targets(&self, x: usize, y: usize) -> Option<impl Iterator<Item = usize> + '_> {
        let index = self.index(x, y)?;
        let cell = self.cells[index];
        if self.status != Status::Playing || !cell.open || cell.adjacent_mines == 0 {
            return None;
        }

        let (width, height) = self.difficulty.dimensions();
        let adjacent = neighbors(index, width, height);
        let flags = adjacent.clone().filter(|&i| self.cells[i].flagged).count();
        if flags != usize::from(cell.adjacent_mines) {
            return None;
        }
        Some(adjacent.filter(|&i| !self.cells[i].open && !self.cells[i].flagged))
    }

    fn detonate(&mut self, index: usize) {
        self.status = Status::Lost;
        self.detonated = Some(index);
    }

    fn check_win(&mut self) {
        if self.safe_remaining == 0 {
            self.status = Status::Won;
            self.flags = self.difficulty.mines();
        }
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        let (width, height) = self.difficulty.dimensions();
        (x < width && y < height).then(|| y * width + x)
    }

    fn place_mines(&mut self, first: usize) {
        let (width, height) = self.difficulty.dimensions();
        let safe_neighbors: Vec<_> = neighbors(first, width, height).collect();
        let mut candidates: Vec<_> = (0..self.cells.len())
            .filter(|index| *index != first && !safe_neighbors.contains(index))
            .collect();

        let mut random = self.seed;
        let mine_count = self.difficulty.mines();
        for index in 0..mine_count {
            let offset = (next_random(&mut random) % (candidates.len() - index) as u64) as usize;
            candidates.swap(index, index + offset);
        }
        self.install_layout(first, &candidates[..mine_count])
            .expect("generated field has unique mines outside the opening");
    }

    fn install_layout(&mut self, opening: usize, mines: &[usize]) -> Option<()> {
        let (width, height) = self.difficulty.dimensions();
        if opening >= self.cells.len() || mines.len() != self.difficulty.mines() {
            return None;
        }
        for &index in mines {
            let mine = self.cells.get_mut(index)?;
            if mine.mine {
                return None;
            }
            mine.mine = true;
            for neighbor in neighbors(index, width, height) {
                self.cells[neighbor].adjacent_mines += 1;
            }
        }
        let first = self.cells[opening];
        if first.mine || first.adjacent_mines != 0 {
            return None;
        }
        self.opening = Some(opening);
        Some(())
    }

    fn open_area(&mut self, first: usize) {
        let (width, height) = self.difficulty.dimensions();
        let mut pending = vec![first];
        while let Some(index) = pending.pop() {
            let cell = &mut self.cells[index];
            if cell.open || cell.flagged || cell.mine {
                continue;
            }
            cell.open = true;
            self.safe_remaining -= 1;
            if cell.adjacent_mines == 0 {
                pending.extend(neighbors(index, width, height));
            }
        }
    }
}

fn neighbors(index: usize, width: usize, height: usize) -> impl Iterator<Item = usize> + Clone {
    let x = index % width;
    let y = index / width;
    (y.saturating_sub(1)..=(y + 1).min(height - 1)).flat_map(move |row| {
        (x.saturating_sub(1)..=(x + 1).min(width - 1))
            .map(move |column| row * width + column)
            .filter(move |neighbor| *neighbor != index)
    })
}

fn next_random(state: &mut u64) -> u64 {
    // SplitMix64 keeps boards reproducible across platforms and accepts a zero seed.
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_reveal_clears_its_neighborhood_and_places_the_full_mine_count() {
        for difficulty in Difficulty::ALL {
            let (width, height) = difficulty.dimensions();
            for y in 0..height {
                for x in 0..width {
                    let mut board = Board::new(difficulty, 827);
                    assert!(board.reveal(x, y));
                    assert_eq!(board.tile(x, y), Some(Tile::Open(0)));
                    for neighbor in neighbors(y * width + x, width, height) {
                        assert!(board.cells[neighbor].open);
                    }
                    assert_eq!(
                        board.cells.iter().filter(|cell| cell.mine).count(),
                        difficulty.mines()
                    );
                }
            }
        }
    }

    #[test]
    fn same_seed_and_opening_generate_same_mines_and_counts() {
        let mut first = Board::new(Difficulty::Meadow, 0);
        let mut second = Board::new(Difficulty::Meadow, 0);
        first.reveal(8, 8);
        second.reveal(8, 8);
        for (index, (left, right)) in first.cells.iter().zip(&second.cells).enumerate() {
            assert_eq!(left.mine, right.mine);
            let adjacent = neighbors(index, 9, 9).filter(|neighbor| first.cells[*neighbor].mine);
            assert_eq!(left.adjacent_mines as usize, adjacent.count());
        }
    }

    #[test]
    fn flags_block_manual_reveals_and_flood_fill() {
        let mut board = Board::new(Difficulty::Meadow, 2);
        assert!(board.toggle_flag(1, 0));
        assert!(!board.reveal(1, 0));
        assert_eq!(board.status(), Status::Ready);
        assert!(board.reveal(0, 0));
        assert_eq!(board.tile(1, 0), Some(Tile::Flagged));
        assert_eq!(board.flags(), 1);
        assert!(board.toggle_flag(1, 0));
        assert!(board.reveal(1, 0));
        assert!(matches!(board.tile(1, 0), Some(Tile::Open(_))));
        assert!(!board.toggle_flag(1, 0));
        assert!(!board.reveal(usize::MAX, usize::MAX));
    }

    #[test]
    fn chord_opens_safe_neighbors_only_when_the_flag_count_matches() {
        let mut board = Board::new(Difficulty::Meadow, 19);
        assert!(!board.chord(0, 0));
        board.reveal(0, 0);
        assert!(!board.chord(0, 0));
        assert!(!board.chord(9, 0));

        let (clue, mine, safe) = chord_setup(&board);
        let remaining = board.safe_remaining();
        assert!(!board.can_chord(clue % 9, clue / 9));
        assert!(!board.chord(clue % 9, clue / 9));
        assert_eq!(board.safe_remaining(), remaining);

        assert!(board.toggle_flag(mine % 9, mine / 9));
        assert!(board.can_chord(clue % 9, clue / 9));
        assert!(board.chord(clue % 9, clue / 9));
        assert_eq!(board.tile(mine % 9, mine / 9), Some(Tile::Flagged));
        assert!(matches!(
            board.tile(safe % 9, safe / 9),
            Some(Tile::Open(_))
        ));
        assert!(!board.chord(clue % 9, clue / 9));
    }

    #[test]
    fn chord_with_a_misplaced_flag_detonates_the_unflagged_mine() {
        let mut board = Board::new(Difficulty::Meadow, 19);
        board.reveal(0, 0);
        let (clue, mine, safe) = chord_setup(&board);
        let remaining = board.safe_remaining();

        assert!(board.toggle_flag(safe % 9, safe / 9));
        assert!(board.chord(clue % 9, clue / 9));
        assert_eq!(board.status(), Status::Lost);
        assert_eq!(board.safe_remaining(), remaining);
        assert_eq!(board.tile(mine % 9, mine / 9), Some(Tile::Detonated));
        assert_eq!(board.tile(safe % 9, safe / 9), Some(Tile::Misflagged));
        assert!(!board.chord(clue % 9, clue / 9));
    }

    #[test]
    fn chording_the_last_safe_square_wins() {
        let mut board = Board::new(Difficulty::Meadow, 19);
        board.reveal(0, 0);
        let (clue, mine, safe) = chord_setup(&board);
        board.toggle_flag(safe % 9, safe / 9);
        for index in 0..81 {
            if !board.cells[index].mine {
                board.reveal(index % 9, index / 9);
            }
        }
        assert_eq!(board.safe_remaining(), 1);

        assert!(board.toggle_flag(safe % 9, safe / 9));
        assert!(board.toggle_flag(mine % 9, mine / 9));
        assert!(board.chord(clue % 9, clue / 9));
        assert_eq!(board.status(), Status::Won);
        assert_eq!(board.flags(), Difficulty::Meadow.mines());
    }

    fn chord_setup(board: &Board) -> (usize, usize, usize) {
        board
            .cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.open && cell.adjacent_mines == 1)
            .find_map(|(clue, _)| {
                let mine = neighbors(clue, 9, 9).find(|&i| board.cells[i].mine)?;
                let safe = neighbors(clue, 9, 9).find(|&i| {
                    let cell = board.cells[i];
                    !cell.open && !cell.mine
                })?;
                Some((clue, mine, safe))
            })
            .expect("fixture contains an open 1 next to a mine and a covered safe cell")
    }

    #[test]
    fn loss_exposes_mines_and_misflags_and_freezes_the_board() {
        let mut board = Board::new(Difficulty::Meadow, 19);
        board.toggle_flag(1, 0);
        board.reveal(0, 0);
        let mine = board.cells.iter().position(|cell| cell.mine).unwrap();
        assert!(board.reveal(mine % 9, mine / 9));
        assert_eq!(board.status(), Status::Lost);
        assert_eq!(board.tile(mine % 9, mine / 9), Some(Tile::Detonated));
        assert_eq!(board.tile(1, 0), Some(Tile::Misflagged));
        assert!(!board.toggle_flag(1, 0));
        assert!(!board.reveal(1, 0));
    }

    #[test]
    fn clearing_every_safe_tile_wins_and_marks_remaining_mines() {
        let mut board = Board::new(Difficulty::Meadow, 11);
        board.reveal(5, 5);
        for index in 0..81 {
            if !board.cells[index].mine {
                board.reveal(index % 9, index / 9);
            }
        }
        assert_eq!(board.status(), Status::Won);
        assert_eq!(board.safe_remaining(), 0);
        assert_eq!(board.flags(), Difficulty::Meadow.mines());
        for index in 0..board.cells.len() {
            if board.cells[index].mine {
                assert_eq!(board.tile(index % 9, index / 9), Some(Tile::Flagged));
                assert!(!board.reveal(index % 9, index / 9));
            }
        }
    }

    #[test]
    fn checkpoints_restore_exact_state_before_play_during_play_and_after_both_results() {
        let difficulty = Difficulty::Meadow;
        let mut board = Board::new(difficulty, 19);
        assert!(board.toggle_flag(1, 0));
        let mut ready = assert_checkpoint_round_trip(&board);
        assert_eq!(ready.status(), Status::Ready);
        assert!(ready.reveal(0, 0));
        assert!(board.reveal(0, 0));
        assert_eq!(ready.checkpoint(), board.checkpoint());

        let mut playing = assert_checkpoint_round_trip(&board);
        assert_eq!(playing.status(), Status::Playing);
        let (_, mine, _) = chord_setup(&board);
        assert!(playing.reveal(mine % 9, mine / 9));
        assert_eq!(playing.status(), Status::Lost);
        assert_eq!(playing.tile(1, 0), Some(Tile::Misflagged));
        assert_checkpoint_round_trip(&playing);

        assert!(board.toggle_flag(1, 0));
        for index in 0..81 {
            if !board.cells[index].mine {
                board.reveal(index % 9, index / 9);
            }
        }
        assert_eq!(board.status(), Status::Won);
        assert_checkpoint_round_trip(&board);
    }

    #[test]
    fn a_replay_restores_its_marked_opening_and_the_exact_original_field() {
        let mut original = Board::new(Difficulty::Highlands, 191);
        assert!(original.reveal(8, 6));
        let after_opening = original.checkpoint();
        let mine = after_opening.mines[0];
        assert!(original.reveal(mine % 16, mine / 16));
        assert_eq!(original.status(), Status::Lost);

        let replay = original.replay().unwrap();
        let mut replay = assert_checkpoint_round_trip(&replay);
        assert_eq!(replay.status(), Status::Ready);
        assert_eq!(replay.required_opening(), Some((8, 6)));
        assert!(!replay.reveal(0, 0));
        assert!(!replay.toggle_flag(8, 6));
        assert!(replay.reveal(8, 6));
        assert_eq!(replay.required_opening(), None);
        assert_eq!(replay.checkpoint(), after_opening);
    }

    #[test]
    fn restore_rejects_impossible_mines_flags_and_detonations() {
        let mut board = Board::new(Difficulty::Meadow, 19);
        board.reveal(0, 0);
        let mut invalid = board.checkpoint();
        invalid.mines[0] = 0;
        assert!(Board::restore(board.difficulty(), invalid).is_none());
        let mut invalid = board.checkpoint();
        invalid.flags = vec![81];
        assert!(Board::restore(board.difficulty(), invalid).is_none());
        let mut invalid = board.checkpoint();
        invalid.detonated = Some(0);
        assert!(Board::restore(board.difficulty(), invalid).is_none());
    }

    fn assert_checkpoint_round_trip(board: &Board) -> Board {
        let difficulty = board.difficulty();
        let restored = Board::restore(difficulty, board.checkpoint()).expect("valid checkpoint");
        assert_eq!(restored.checkpoint(), board.checkpoint());
        assert_eq!(restored.status(), board.status());
        assert_eq!(restored.flags(), board.flags());
        assert_eq!(restored.safe_remaining(), board.safe_remaining());
        let (width, height) = difficulty.dimensions();
        for y in 0..height {
            for x in 0..width {
                assert_eq!(restored.tile(x, y), board.tile(x, y));
            }
        }
        restored
    }
}
