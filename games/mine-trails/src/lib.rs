//! Deterministic mine-clue rooms and tunnel progression for Mine Trails.

pub const SIDE: usize = 6;
const CELL_COUNT: usize = SIDE * SIDE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Room {
    Entrance,
    AmberDrift,
    SlateDrift,
    DeepVault,
}

impl Room {
    pub const ALL: [Self; 4] = [
        Self::Entrance,
        Self::AmberDrift,
        Self::SlateDrift,
        Self::DeepVault,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Entrance => "Entrance",
            Self::AmberDrift => "Amber Drift",
            Self::SlateDrift => "Slate Drift",
            Self::DeepVault => "Deep Vault",
        }
    }

    pub fn connections(self) -> &'static [Self] {
        match self {
            Self::Entrance | Self::DeepVault => &[Self::AmberDrift, Self::SlateDrift],
            Self::AmberDrift | Self::SlateDrift => &[Self::Entrance, Self::DeepVault],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomStatus {
    Exploring,
    Surveyed,
    Collapsed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Covered { cache: bool },
    Flagged,
    Open { clue: u8, cache: bool },
    Mine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expedition {
    rooms: [RoomBoard; 4],
    current: Room,
}

impl Expedition {
    pub fn new(seed: u64) -> Self {
        Self {
            rooms: Room::ALL.map(|room| {
                let variant = mix(seed.wrapping_add(room as u64)) as usize & 15;
                RoomBoard::new(room, variant)
            }),
            current: Room::Entrance,
        }
    }

    pub fn current_room(&self) -> Room {
        self.current
    }

    pub fn status(&self, room: Room) -> RoomStatus {
        self.rooms[room as usize].status()
    }

    pub fn is_complete(&self) -> bool {
        self.status(Room::DeepVault) == RoomStatus::Surveyed
    }

    pub fn cache_progress(&self, room: Room) -> (usize, usize) {
        self.rooms[room as usize].cache_progress()
    }

    pub fn mine_count(&self) -> usize {
        self.board().features.iter().filter(|&&f| f == b'M').count()
    }

    pub fn flag_count(&self) -> usize {
        self.board()
            .cells
            .iter()
            .filter(|&&cell| cell == Cell::Flagged)
            .count()
    }

    pub fn tile(&self, x: usize, y: usize) -> Option<Tile> {
        Some(self.board().tile(index(x, y)?))
    }

    pub fn can_dig(&self, x: usize, y: usize) -> bool {
        let board = self.board();
        index(x, y).is_some_and(|cell| {
            board.status() == RoomStatus::Exploring
                && board.cells[cell] == Cell::Covered
                && neighbors(cell, false).any(|neighbor| board.cells[neighbor] == Cell::Open)
        })
    }

    pub fn dig(&mut self, x: usize, y: usize) -> bool {
        if !self.can_dig(x, y) {
            return false;
        }
        let cell = y * SIDE + x;
        let board = self.board_mut();
        if board.features[cell] == b'M' {
            board.cells[cell] = Cell::Hit;
        } else {
            board.reveal(cell);
        }
        true
    }

    pub fn toggle_flag(&mut self, x: usize, y: usize) -> bool {
        let Some(cell) = index(x, y) else {
            return false;
        };
        let board = self.board_mut();
        if board.status() != RoomStatus::Exploring || board.features[cell] == b'C' {
            return false;
        }
        board.cells[cell] = match board.cells[cell] {
            Cell::Covered => Cell::Flagged,
            Cell::Flagged => Cell::Covered,
            _ => return false,
        };
        true
    }

    pub fn is_unlocked(&self, room: Room) -> bool {
        match room {
            Room::Entrance => true,
            Room::AmberDrift | Room::SlateDrift => {
                self.status(Room::Entrance) == RoomStatus::Surveyed
            }
            Room::DeepVault => {
                self.status(Room::AmberDrift) == RoomStatus::Surveyed
                    && self.status(Room::SlateDrift) == RoomStatus::Surveyed
            }
        }
    }

    pub fn can_travel(&self, room: Room) -> bool {
        self.current.connections().contains(&room) && self.is_unlocked(room)
    }

    pub fn travel(&mut self, room: Room) -> bool {
        if !self.can_travel(room) {
            return false;
        }
        self.current = room;
        true
    }

    pub fn retry_room(&mut self) -> bool {
        let board = self.board_mut();
        if board.status() != RoomStatus::Collapsed {
            return false;
        }
        board.reset();
        true
    }

    fn board(&self) -> &RoomBoard {
        &self.rooms[self.current as usize]
    }

    fn board_mut(&mut self) -> &mut RoomBoard {
        &mut self.rooms[self.current as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    Covered,
    Flagged,
    Open,
    Hit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RoomBoard {
    features: [u8; CELL_COUNT],
    cells: [Cell; CELL_COUNT],
    entry: usize,
}

impl RoomBoard {
    fn new(room: Room, variant: usize) -> Self {
        let mut features = [b'.'; CELL_COUNT];
        let mut entry = 0;
        for (y, row) in BLUEPRINTS[room as usize][variant / 8].iter().enumerate() {
            for (x, feature) in row.bytes().enumerate() {
                let mut x = if variant & 4 == 0 { x } else { SIDE - 1 - x };
                let mut y = y;
                for _ in 0..variant % 4 {
                    (x, y) = (SIDE - 1 - y, x);
                }
                let cell = y * SIDE + x;
                features[cell] = feature;
                if feature == b'S' {
                    entry = cell;
                }
            }
        }
        let mut board = Self {
            features,
            cells: [Cell::Covered; CELL_COUNT],
            entry,
        };
        board.reveal(entry);
        board
    }

    fn reset(&mut self) {
        self.cells.fill(Cell::Covered);
        self.reveal(self.entry);
    }

    fn reveal(&mut self, cell: usize) {
        let mut pending = vec![cell];
        while let Some(next) = pending.pop() {
            if self.cells[next] != Cell::Covered {
                continue;
            }
            self.cells[next] = Cell::Open;
            if self.clue(next) == 0 {
                pending.extend(neighbors(next, true));
            }
        }
    }

    fn clue(&self, cell: usize) -> u8 {
        neighbors(cell, true)
            .filter(|&neighbor| self.features[neighbor] == b'M')
            .count() as u8
    }

    fn tile(&self, cell: usize) -> Tile {
        let cache = self.features[cell] == b'C';
        match self.cells[cell] {
            Cell::Covered => Tile::Covered { cache },
            Cell::Flagged => Tile::Flagged,
            Cell::Open => Tile::Open {
                clue: self.clue(cell),
                cache,
            },
            Cell::Hit => Tile::Mine,
        }
    }

    fn cache_progress(&self) -> (usize, usize) {
        let mut found = 0;
        let mut total = 0;
        for (feature, cell) in self.features.iter().zip(self.cells) {
            if *feature == b'C' {
                total += 1;
                found += usize::from(cell == Cell::Open);
            }
        }
        (found, total)
    }

    fn status(&self) -> RoomStatus {
        let (found, total) = self.cache_progress();
        if self.cells.contains(&Cell::Hit) {
            RoomStatus::Collapsed
        } else if found == total {
            RoomStatus::Surveyed
        } else {
            RoomStatus::Exploring
        }
    }
}

fn index(x: usize, y: usize) -> Option<usize> {
    (x < SIDE && y < SIDE).then(|| y * SIDE + x)
}

fn neighbors(cell: usize, diagonals: bool) -> impl Iterator<Item = usize> {
    let offsets = [
        (0, -1),
        (1, 0),
        (0, 1),
        (-1, 0),
        (-1, -1),
        (1, -1),
        (1, 1),
        (-1, 1),
    ];
    offsets
        .into_iter()
        .take(if diagonals { 8 } else { 4 })
        .filter_map(move |(dx, dy)| {
            let x = (cell % SIDE).checked_add_signed(dx)?;
            let y = (cell / SIDE).checked_add_signed(dy)?;
            index(x, y)
        })
}

fn mix(seed: u64) -> u64 {
    let mut value = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

// Each blueprint can be solved from its starting clues using single-clue
// deductions. The seed selects a blueprint and rotates or reflects it.
const BLUEPRINTS: [[[&str; SIDE]; 2]; 4] = [
    [
        ["M.....", "....CM", "S..M..", "......", ".....C", "..M..M"],
        ["M....M", "....CM", "S..M..", "......", ".....C", "..M..."],
    ],
    [
        ["M.....", "...C..", "M...MM", "...M..", "S....C", ".....M"],
        ["M.....", "M..C.M", "...M..", "....M.", "S....C", "..M..."],
    ],
    [
        ["M..S..", "M.....", "C.M.MM", "......", "M...C.", ".....M"],
        ["MM.S..", ".M....", "C...M.", "M...M.", "....C.", ".....M"],
    ],
    [
        ["M.S..M", "M.....", "...M..", "C..MM.", "M....M", "....C."],
        ["..S.M.", "......", "MM....", "C....M", ".MMM..", "....CM"],
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_room_variants_can_be_surveyed_from_clues() {
        for variant in 0..16 {
            for room in Room::ALL {
                let mut expedition = Expedition::new(0);
                expedition.current = room;
                expedition.rooms[room as usize] = RoomBoard::new(room, variant);
                assert_eq!(expedition.cache_progress(room), (0, 2));
                survey(&mut expedition);
            }
        }
    }

    #[test]
    fn vault_requires_both_drifts_and_retry_preserves_progress() {
        let mut expedition = Expedition::new(42);
        assert_eq!(expedition, Expedition::new(42));
        assert_ne!(expedition, Expedition::new(43));
        assert!(!expedition.travel(Room::AmberDrift));
        survey(&mut expedition);
        assert!(expedition.travel(Room::AmberDrift));
        let starting_room = expedition.board().clone();
        let mine = (0..CELL_COUNT)
            .find(|&cell| {
                expedition.board().features[cell] == b'M'
                    && expedition.can_dig(cell % SIDE, cell / SIDE)
            })
            .expect("amber starting frontier has a mine");
        assert!(expedition.toggle_flag(mine % SIDE, mine / SIDE));
        assert!(!expedition.dig(mine % SIDE, mine / SIDE));
        assert!(expedition.toggle_flag(mine % SIDE, mine / SIDE));
        assert!(expedition.dig(mine % SIDE, mine / SIDE));
        assert_eq!(expedition.status(Room::AmberDrift), RoomStatus::Collapsed);
        assert!(expedition.retry_room());
        assert_eq!(*expedition.board(), starting_room);
        assert_eq!(expedition.status(Room::Entrance), RoomStatus::Surveyed);

        survey(&mut expedition);
        assert!(!expedition.travel(Room::DeepVault));
        assert!(!expedition.travel(Room::SlateDrift));
        assert!(expedition.travel(Room::Entrance));
        assert!(expedition.travel(Room::SlateDrift));
        survey(&mut expedition);
        assert!(expedition.travel(Room::DeepVault));
        assert!(!expedition.is_complete());
        survey(&mut expedition);
        assert!(expedition.is_complete());
    }

    fn survey(expedition: &mut Expedition) {
        let room = expedition.current_room();
        while expedition.status(room) == RoomStatus::Exploring {
            let mut changed = false;
            for cell in 0..CELL_COUNT {
                let Some(Tile::Open { clue, .. }) = expedition.tile(cell % SIDE, cell / SIDE)
                else {
                    continue;
                };
                let mut covered = Vec::new();
                let mut flagged = 0;
                for neighbor in neighbors(cell, true) {
                    match expedition.tile(neighbor % SIDE, neighbor / SIDE).unwrap() {
                        Tile::Covered { .. } => covered.push(neighbor),
                        Tile::Flagged => flagged += 1,
                        _ => (),
                    }
                }
                let unmarked_mines = usize::from(clue) - flagged;
                for neighbor in covered.iter().copied() {
                    let (x, y) = (neighbor % SIDE, neighbor / SIDE);
                    if unmarked_mines == 0 {
                        changed |= expedition.dig(x, y);
                    } else if unmarked_mines == covered.len() {
                        changed |= expedition.toggle_flag(x, y);
                    }
                }
            }
            assert!(changed, "{room:?} has no clue-based move");
        }
        assert_eq!(expedition.status(room), RoomStatus::Surveyed);
    }
}
