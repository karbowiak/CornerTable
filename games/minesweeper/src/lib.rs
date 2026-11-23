//! Seeded Minesweeper rules independent of the desktop view.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Meadow,
    Highlands,
}

impl Difficulty {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Meadow => "Meadow",
            Self::Highlands => "Highlands",
        }
    }

    pub const fn dimensions(self) -> (usize, usize) {
        match self {
            Self::Meadow => (9, 9),
            Self::Highlands => (16, 12),
        }
    }

    pub const fn mines(self) -> usize {
        match self {
            Self::Meadow => 10,
            Self::Highlands => 30,
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

pub struct Board {
    difficulty: Difficulty,
    seed: u64,
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
            status: Status::Ready,
            cells: vec![Cell::default(); width * height],
            detonated: None,
            safe_remaining: width * height - difficulty.mines(),
            flags: 0,
        }
    }

    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
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
        if cell.open || matches!(self.status, Status::Won | Status::Lost) {
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
        if self.cells[index].flagged || matches!(self.status, Status::Won | Status::Lost) {
            return false;
        }
        if self.status == Status::Ready {
            self.place_mines(index);
            self.status = Status::Playing;
        }
        if self.cells[index].open {
            return false;
        }
        if self.cells[index].mine {
            self.status = Status::Lost;
            self.detonated = Some(index);
            return true;
        }

        self.open_area(index);
        if self.safe_remaining == 0 {
            self.status = Status::Won;
            self.flags = self.difficulty.mines();
        }
        true
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
        for index in 0..self.difficulty.mines() {
            let offset = (next_random(&mut random) % (candidates.len() - index) as u64) as usize;
            candidates.swap(index, index + offset);
            let mine = candidates[index];
            self.cells[mine].mine = true;
            for neighbor in neighbors(mine, width, height) {
                self.cells[neighbor].adjacent_mines += 1;
            }
        }
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

fn neighbors(index: usize, width: usize, height: usize) -> impl Iterator<Item = usize> {
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
        for difficulty in [Difficulty::Meadow, Difficulty::Highlands] {
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
}
