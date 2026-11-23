//! Rules and seeded deals for a three-dimensional matching board.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suit {
    Dots,
    Diamonds,
    Leaves,
    Blossoms,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    pub suit: Suit,
    pub rank: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub x: u8,
    pub y: u8,
    pub level: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cube {
    pub position: Position,
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
    cubes: Vec<Cube>,
    removed: Vec<bool>,
    selected: Option<usize>,
}

// Each row specifies y, the starting x, and an even number of cubes.
const TERRACE: [&[(u8, u8, u8)]; 3] = [
    &[(0, 1, 4), (1, 0, 6), (2, 0, 6), (3, 0, 6), (4, 1, 4)],
    &[(1, 1, 4), (2, 1, 4), (3, 1, 4)],
    &[(2, 2, 2)],
];
const RIDGE: [&[(u8, u8, u8)]; 3] = [
    &[(0, 1, 4), (1, 0, 6), (2, 0, 6), (3, 1, 4), (4, 1, 4)],
    &[(0, 1, 4), (1, 1, 4), (2, 1, 4), (4, 2, 2)],
    &[(1, 2, 2)],
];

impl Board {
    pub fn new(seed: u64) -> Self {
        let mut random = Random(seed);
        let layers = [TERRACE, RIDGE][random.index(2)];
        let mut cubes = Vec::new();
        let mut rows = Vec::new();
        let blank = Face {
            suit: Suit::Dots,
            rank: 1,
        };
        for (level, layer) in layers.into_iter().enumerate() {
            for &(y, start_x, width) in layer {
                let start = cubes.len();
                for x in start_x..start_x + width {
                    cubes.push(Cube {
                        position: Position {
                            x,
                            y,
                            level: level as u8,
                        },
                        face: blank,
                    });
                }
                rows.push(start..cubes.len());
            }
        }
        let mut board = Self {
            removed: vec![false; cubes.len()],
            cubes,
            selected: None,
        };

        let mut pairs = Vec::with_capacity(board.cubes.len() / 2);
        for _ in 0..12 {
            pairs.clear();
            board.restart();
            while pairs.len() < board.cubes.len() / 2 {
                let mut free: Vec<_> = board.free_ids().collect();
                if free.len() < 2 {
                    break;
                }
                let first = free.swap_remove(random.index(free.len()));
                let second = free[random.index(free.len())];
                board.removed[first] = true;
                board.removed[second] = true;
                pairs.push([first, second]);
            }
            if pairs.len() == board.cubes.len() / 2 {
                break;
            }
        }
        if pairs.len() != board.cubes.len() / 2 {
            // With all higher layers removed, both ends of each row are free.
            pairs = rows
                .into_iter()
                .rev()
                .flat_map(|row| {
                    (0..row.len() / 2).map(move |offset| [row.start + offset, row.end - 1 - offset])
                })
                .collect();
        }
        let mut faces: Vec<_> = [Suit::Dots, Suit::Diamonds, Suit::Leaves, Suit::Blossoms]
            .into_iter()
            .flat_map(|suit| (1..=5).map(move |rank| Face { suit, rank }))
            .collect();
        // Each marking belongs to one pair. Taking a legal match early leaves
        // the other pairs intact and can only expose more cubes.
        for [first, second] in pairs {
            let face = faces.swap_remove(random.index(faces.len()));
            board.cubes[first].face = face;
            board.cubes[second].face = face;
        }
        board.restart();
        board
    }

    pub fn layout(&self) -> impl Iterator<Item = Position> + '_ {
        self.cubes.iter().map(|cube| cube.position)
    }

    pub fn cubes(&self) -> impl Iterator<Item = (usize, &Cube)> {
        self.cubes
            .iter()
            .enumerate()
            .filter(|(id, _)| !self.removed[*id])
    }

    pub fn remaining(&self) -> usize {
        self.cubes().count()
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn is_free(&self, id: usize) -> bool {
        let Some(cube) = self.cubes.get(id) else {
            return false;
        };
        if self.removed[id] {
            return false;
        }
        let position = cube.position;
        let mut sides = [false; 4];
        for (_, other) in self.cubes() {
            let other = other.position;
            let dx = i16::from(other.x) - i16::from(position.x);
            let dy = i16::from(other.y) - i16::from(position.y);
            if dx == 0 && dy == 0 && other.level > position.level {
                return false;
            }
            if other.level == position.level {
                match (dx, dy) {
                    (-1, 0) => sides[0] = true,
                    (1, 0) => sides[1] = true,
                    (0, -1) => sides[2] = true,
                    (0, 1) => sides[3] = true,
                    _ => {}
                }
            }
        }
        sides.contains(&false)
    }

    fn free_ids(&self) -> impl Iterator<Item = usize> + '_ {
        self.cubes()
            .map(|(id, _)| id)
            .filter(|&id| self.is_free(id))
    }

    pub fn hint(&self) -> Option<[usize; 2]> {
        let free: Vec<_> = self.free_ids().collect();
        if let Some(selected) = self.selected
            && let Some(&partner) = free.iter().find(|&&id| self.matches(selected, id))
        {
            return Some([selected, partner]);
        }
        for (index, &first) in free.iter().enumerate() {
            if let Some(&second) = free[index + 1..]
                .iter()
                .find(|&&id| self.matches(first, id))
            {
                return Some([first, second]);
            }
        }
        None
    }

    fn matches(&self, first: usize, second: usize) -> bool {
        first != second && self.cubes[first].face == self.cubes[second].face
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
            Some(previous) if self.matches(previous, id) => {
                self.removed[previous] = true;
                self.removed[id] = true;
                self.selected = None;
                MoveResult::Matched
            }
            Some(_) => MoveResult::DifferentFace,
            None => MoveResult::Selected,
        }
    }

    pub fn restart(&mut self) {
        self.removed.fill(false);
        self.selected = None;
    }
}

struct Random(u64);

impl Random {
    fn index(&mut self, len: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((value ^ (value >> 31)) % len as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_deals_clear_by_matching_any_free_pair_and_restart() {
        for seed in 0..24 {
            let mut board = Board::new(seed);
            let expected = Board::new(seed);
            assert_eq!(board.remaining(), 40);
            assert!(board.cubes().eq(expected.cubes()));
            while board.remaining() > 0 {
                let [first, second] = board.hint().expect("each deal has a legal match");
                assert_eq!(board.select(first), MoveResult::Selected);
                assert_eq!(board.hint(), Some([first, second]));
                assert_eq!(board.select(second), MoveResult::Matched);
                assert!(!board.is_free(first));
            }
            assert_eq!(board.hint(), None);
            board.restart();
            assert!(board.cubes().eq(expected.cubes()));
            assert_eq!(board.selected(), None);
        }
    }

    #[test]
    fn eligibility_requires_clear_top_and_an_open_horizontal_side() {
        let mut board = Board::new(0);
        let target = board
            .cubes()
            .find(|(_, cube)| {
                cube.position
                    == Position {
                        x: 2,
                        y: 2,
                        level: 0,
                    }
            })
            .unwrap()
            .0;
        let position = board.cubes[target].position;
        assert_eq!(board.select(target), MoveResult::Blocked);
        for (id, cube) in board.cubes.iter().enumerate() {
            if cube.position.x == position.x
                && cube.position.y == position.y
                && cube.position.level > 0
            {
                board.removed[id] = true;
            }
        }
        assert!(!board.is_free(target), "all four sides are still closed");
        let neighbor = board
            .cubes()
            .find(|(_, cube)| {
                cube.position
                    == Position {
                        x: 2,
                        y: 3,
                        level: 0,
                    }
            })
            .unwrap()
            .0;
        board.removed[neighbor] = true;
        assert_eq!(board.select(target), MoveResult::Selected);
        assert_eq!(board.select(target), MoveResult::Deselected);
    }
}
