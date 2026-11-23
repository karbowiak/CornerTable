//! Rules and deterministic deals for layered Mahjong solitaire.

pub const DAILY_SEED: u64 = 0x434f_524e_4552_0001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Dots(u8),
    Bamboo(u8),
    Characters(u8),
    Wind(char),
    Dragon(char),
    Flower,
    Season,
}

#[derive(Clone, Copy, Debug)]
pub struct Tile {
    /// Horizontal position in half-tile units.
    pub x: u8,
    /// Vertical position in half-tile units.
    pub y: u8,
    pub level: u8,
    pub face: Face,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveResult {
    Blocked,
    Selected,
    Deselected,
    DifferentFace,
    Matched,
}

pub struct Board {
    tiles: Vec<Tile>,
    removed: Vec<bool>,
    history: Vec<[usize; 2]>,
    selected: Option<usize>,
}

impl Board {
    pub fn new(seed: u64) -> Self {
        Self::deal(seed).0
    }

    fn deal(seed: u64) -> (Self, Vec<[usize; 2]>) {
        let mut tiles = Vec::with_capacity(144);
        let mut add_row = |width: u8, y, level| {
            for column in 0..width {
                tiles.push(Tile {
                    x: 14 - width + column * 2,
                    y,
                    level,
                    face: Face::Dots(1),
                });
            }
        };
        for (row, width) in [10, 12, 12, 14, 14, 12, 12, 10].into_iter().enumerate() {
            add_row(width, row as u8 * 2, 0);
        }
        for y in [4, 6, 8, 10] {
            add_row(8, y, 1);
        }
        for y in [5, 7, 9] {
            add_row(4, y, 2);
        }
        for y in [6, 8] {
            add_row(2, y, 3);
        }

        let mut rng = Random(seed);
        let mut faces = Vec::with_capacity(72);
        for rank in 1..=9 {
            for face in [Face::Dots(rank), Face::Bamboo(rank), Face::Characters(rank)] {
                faces.extend([face; 2]);
            }
        }
        for wind in ['E', 'S', 'W', 'N'] {
            faces.extend([Face::Wind(wind); 2]);
        }
        for dragon in ['R', 'G', 'W'] {
            faces.extend([Face::Dragon(dragon); 2]);
        }
        faces.extend([Face::Flower, Face::Flower, Face::Season, Face::Season]);

        let mut board = Self {
            removed: vec![false; tiles.len()],
            history: Vec::new(),
            selected: None,
            tiles,
        };
        let mut pairs = Vec::with_capacity(faces.len());
        // Try randomized legal removal orders. If one leaves a lone free tile,
        // fall back to peeling mirrored row ends from the highest layers down.
        for _ in 0..16 {
            pairs.clear();
            board.removed.fill(false);
            while pairs.len() < faces.len() {
                let mut available = board.free_tiles();
                if available.len() < 2 {
                    break;
                }
                let first = available.swap_remove(rng.index(available.len()));
                let second = available[rng.index(available.len())];
                pairs.push([first, second]);
                board.removed[first] = true;
                board.removed[second] = true;
            }
            if pairs.len() == faces.len() {
                break;
            }
        }
        if pairs.len() != faces.len() {
            pairs.clear();
            for level in (0..=3).rev() {
                for y in 0..=14 {
                    let row: Vec<_> = board
                        .tiles
                        .iter()
                        .enumerate()
                        .filter(|(_, tile)| tile.level == level && tile.y == y)
                        .map(|(id, _)| id)
                        .collect();
                    for index in 0..row.len() / 2 {
                        pairs.push([row[index], row[row.len() - index - 1]]);
                    }
                }
            }
        }
        board.removed.fill(false);
        for &[first, second] in &pairs {
            let face = faces.swap_remove(rng.index(faces.len()));
            board.tiles[first].face = face;
            board.tiles[second].face = face;
        }
        (board, pairs)
    }

    pub fn tiles(&self) -> impl Iterator<Item = (usize, &Tile)> {
        self.tiles
            .iter()
            .enumerate()
            .filter(|(id, _)| !self.removed[*id])
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn remaining(&self) -> usize {
        self.tiles.len() - self.history.len() * 2
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn is_free(&self, id: usize) -> bool {
        let Some(tile) = self.tiles.get(id) else {
            return false;
        };
        if self.removed[id] {
            return false;
        }
        let mut left = false;
        let mut right = false;
        for (_, other) in self.tiles() {
            if tile.y.abs_diff(other.y) >= 2 {
                continue;
            }
            if other.level > tile.level && tile.x.abs_diff(other.x) < 2 {
                return false;
            }
            if tile.level == other.level {
                left |= other.x + 2 == tile.x;
                right |= tile.x + 2 == other.x;
            }
        }
        !(left && right)
    }

    fn free_tiles(&self) -> Vec<usize> {
        self.tiles()
            .map(|(id, _)| id)
            .filter(|&id| self.is_free(id))
            .collect()
    }

    pub fn hint(&self) -> Option<[usize; 2]> {
        let free = self.free_tiles();
        if let Some(first) = self.selected {
            if let Some(&second) = free
                .iter()
                .find(|&&id| id != first && self.tiles[id].face == self.tiles[first].face)
            {
                return Some([first, second]);
            }
        }
        for (index, &first) in free.iter().enumerate() {
            if let Some(&second) = free[index + 1..]
                .iter()
                .find(|&&id| self.tiles[id].face == self.tiles[first].face)
            {
                return Some([first, second]);
            }
        }
        None
    }

    pub fn select(&mut self, id: usize) -> MoveResult {
        if !self.is_free(id) {
            return MoveResult::Blocked;
        }
        match self.selected.replace(id) {
            Some(previous) if previous == id => {
                self.selected = None;
                MoveResult::Deselected
            }
            Some(previous) if self.tiles[previous].face == self.tiles[id].face => {
                self.removed[previous] = true;
                self.removed[id] = true;
                self.history.push([previous, id]);
                self.selected = None;
                MoveResult::Matched
            }
            Some(_) => MoveResult::DifferentFace,
            None => MoveResult::Selected,
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some([first, second]) = self.history.pop() else {
            return false;
        };
        self.removed[first] = false;
        self.removed[second] = false;
        self.selected = None;
        true
    }

    pub fn restart(&mut self) {
        self.removed.fill(false);
        self.history.clear();
        self.selected = None;
    }
}

struct Random(u64);

impl Random {
    fn index(&mut self, length: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((value ^ (value >> 31)) % length as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covered_and_surrounded_tiles_are_blocked() {
        let board = Board::new(1);
        let id_at = |x, y, level| {
            board
                .tiles()
                .find(|(_, tile)| (tile.x, tile.y, tile.level) == (x, y, level))
                .unwrap()
                .0
        };
        assert!(board.is_free(id_at(4, 0, 0)));
        assert!(!board.is_free(id_at(6, 0, 0)));
        assert!(!board.is_free(id_at(10, 4, 1)));
        assert!(board.is_free(id_at(12, 6, 3)));
    }

    #[test]
    fn matches_can_be_undone_and_the_same_deal_restarted() {
        let mut board = Board::new(DAILY_SEED);
        let original = board.hint();
        let [first, second] = original.unwrap();
        assert_eq!(board.select(first), MoveResult::Selected);
        assert_eq!(board.select(second), MoveResult::Matched);
        assert_eq!(board.remaining(), 142);
        assert!(board.undo());
        assert_eq!(board.hint(), original);
        board.select(first);
        board.select(second);
        board.restart();
        assert_eq!(board.remaining(), 144);
        assert_eq!(board.hint(), original);
        assert!(!board.undo());
    }

    #[test]
    fn seeded_deals_have_a_complete_solution() {
        for seed in [0, 1, 2, DAILY_SEED] {
            let (mut board, solution) = Board::deal(seed);
            for [first, second] in solution {
                assert_eq!(board.select(first), MoveResult::Selected);
                assert_eq!(board.select(second), MoveResult::Matched);
            }
            assert_eq!(board.remaining(), 0);
        }
    }
}
