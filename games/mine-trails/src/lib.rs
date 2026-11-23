//! Deterministic mine-clue rooms and tunnel progression for Mine Trails.

mod blueprints;

pub type Position = (usize, usize);
const MAX_CELLS: usize = 7 * 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Room {
    Entrance,
    AmberDrift,
    SlateDrift,
    Floodgate,
    DeepVault,
}

impl Room {
    pub const ALL: [Self; 5] = [
        Self::Entrance,
        Self::AmberDrift,
        Self::SlateDrift,
        Self::Floodgate,
        Self::DeepVault,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Entrance => "Entrance",
            Self::AmberDrift => "Amber Drift",
            Self::SlateDrift => "Slate Drift",
            Self::Floodgate => "Floodgate",
            Self::DeepVault => "Deep Vault",
        }
    }

    pub fn connections(self) -> &'static [Self] {
        match self {
            Self::Entrance => &[Self::AmberDrift, Self::SlateDrift],
            Self::AmberDrift | Self::SlateDrift => &[Self::Entrance, Self::Floodgate],
            Self::Floodgate => &[Self::AmberDrift, Self::SlateDrift, Self::DeepVault],
            Self::DeepVault => &[Self::Floodgate],
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
    Bedrock,
    Covered { diamond: bool },
    Flagged,
    Open { clue: u8, diamond: bool },
    Mine { hit: bool },
    FalseFlag,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expedition {
    rooms: [RoomBoard; Room::ALL.len()],
    current: Room,
    cave_ins: u32,
}

impl Expedition {
    pub fn new(seed: u64) -> Self {
        Self {
            rooms: Room::ALL.map(|room| {
                let variant = mix(seed.wrapping_add(u64::from(room as u8))) as usize & 15;
                RoomBoard::new(room, variant)
            }),
            current: Room::Entrance,
            cave_ins: 0,
        }
    }

    pub fn current_room(&self) -> Room {
        self.current
    }

    pub fn status(&self, room: Room) -> RoomStatus {
        self.rooms[room as usize].status()
    }

    pub fn surveyed_count(&self) -> usize {
        Room::ALL
            .into_iter()
            .filter(|&room| self.status(room) == RoomStatus::Surveyed)
            .count()
    }

    pub fn is_complete(&self) -> bool {
        self.status(Room::DeepVault) == RoomStatus::Surveyed
    }

    pub fn cave_ins(&self) -> u32 {
        self.cave_ins
    }

    pub fn diamonds(&self, room: Room) -> (usize, usize) {
        self.rooms[room as usize].diamonds()
    }

    pub fn side(&self) -> usize {
        self.board().side
    }

    pub fn entry(&self) -> Position {
        let board = self.board();
        board.position(board.entries[0])
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

    pub fn tile(&self, position: Position) -> Option<Tile> {
        let board = self.board();
        Some(board.tile(board.index(position)?))
    }

    pub fn can_dig(&self, position: Position) -> bool {
        let board = self.board();
        board.status() == RoomStatus::Exploring
            && board
                .index(position)
                .is_some_and(|cell| board.at_edge(cell))
    }

    pub fn dig(&mut self, position: Position) -> bool {
        if !self.can_dig(position) {
            return false;
        }
        let board = self.board_mut();
        let cell = board.index(position).expect("diggable tile");
        if board.features[cell] == b'M' {
            board.cells[cell] = Cell::Hit;
            self.cave_ins = self.cave_ins.saturating_add(1);
        } else {
            board.reveal(cell);
        }
        true
    }

    pub fn toggle_flag(&mut self, position: Position) -> bool {
        let board = self.board_mut();
        let Some(cell) = board.index(position) else {
            return false;
        };
        if board.status() != RoomStatus::Exploring || matches!(board.features[cell], b'#' | b'C') {
            return false;
        }
        board.cells[cell] = match board.cells[cell] {
            Cell::Covered => Cell::Flagged,
            Cell::Flagged => Cell::Covered,
            _ => return false,
        };
        true
    }

    pub fn adjacent_flags(&self, position: Position) -> usize {
        let board = self.board();
        board
            .index(position)
            .map_or(0, |cell| board.adjacent_flags(cell))
    }

    /// A sweep trusts the player's flags. Only edge-reachable tiles are dug.
    pub fn can_sweep(&self, position: Position) -> bool {
        let board = self.board();
        board.status() == RoomStatus::Exploring
            && board.index(position).is_some_and(|cell| {
                board.cells[cell] == Cell::Open
                    && board.adjacent_flags(cell) == usize::from(board.clue(cell))
                    && board
                        .neighbors(cell, true)
                        .any(|neighbor| board.at_edge(neighbor))
            })
    }

    pub fn sweep(&mut self, position: Position) -> bool {
        if !self.can_sweep(position) {
            return false;
        }
        let board = self.board();
        let cell = board.index(position).expect("sweep clue");
        let side = board.side;
        let mut neighbors: Vec<_> = board.neighbors(cell, true).collect();
        // Dig the sides before the corners so the corners can become reachable.
        while !neighbors.is_empty() {
            let count = neighbors.len();
            neighbors.retain(|&neighbor| !self.dig((neighbor % side, neighbor / side)));
            if neighbors.len() == count || self.status(self.current) != RoomStatus::Exploring {
                break;
            }
        }
        true
    }

    pub fn is_unlocked(&self, room: Room) -> bool {
        let surveyed = |room| self.status(room) == RoomStatus::Surveyed;
        match room {
            Room::Entrance => true,
            Room::AmberDrift | Room::SlateDrift => surveyed(Room::Entrance),
            Room::Floodgate => surveyed(Room::AmberDrift) || surveyed(Room::SlateDrift),
            Room::DeepVault => surveyed(Room::Floodgate),
        }
    }

    /// A player can take an open connecting tunnel and pass through surveyed
    /// rooms on the way to a destination.
    pub fn can_travel(&self, room: Room) -> bool {
        if room == self.current || !self.is_unlocked(room) {
            return false;
        }
        let mut pending = vec![self.current];
        let mut visited = [false; Room::ALL.len()];
        visited[self.current as usize] = true;
        while let Some(from) = pending.pop() {
            for &next in from.connections() {
                if next == room {
                    return true;
                }
                if !visited[next as usize] && self.status(next) == RoomStatus::Surveyed {
                    visited[next as usize] = true;
                    pending.push(next);
                }
            }
        }
        false
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
    side: usize,
    features: [u8; MAX_CELLS],
    cells: [Cell; MAX_CELLS],
    entries: Vec<usize>,
}

impl RoomBoard {
    fn new(room: Room, variant: usize) -> Self {
        let rows = blueprints::BLUEPRINTS[room as usize][variant / 8];
        let side = rows.len();
        let mut features = [b'#'; MAX_CELLS];
        let mut entries = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            for (x, feature) in row.bytes().enumerate() {
                let mut x = if variant & 4 == 0 { x } else { side - 1 - x };
                let mut y = y;
                for _ in 0..variant % 4 {
                    (x, y) = (side - 1 - y, x);
                }
                let cell = y * side + x;
                features[cell] = feature;
                if feature == b'S' {
                    entries.push(cell);
                }
            }
        }
        let mut board = Self {
            side,
            features,
            cells: [Cell::Covered; MAX_CELLS],
            entries,
        };
        board.reset();
        board
    }

    fn reset(&mut self) {
        self.cells.fill(Cell::Covered);
        for entry in 0..self.entries.len() {
            self.reveal(self.entries[entry]);
        }
    }

    fn reveal(&mut self, cell: usize) {
        let mut pending = vec![cell];
        while let Some(next) = pending.pop() {
            if self.cells[next] != Cell::Covered {
                continue;
            }
            self.cells[next] = Cell::Open;
            if self.clue(next) == 0 {
                pending.extend(self.neighbors(next, false));
            }
        }
    }

    fn at_edge(&self, cell: usize) -> bool {
        self.features[cell] != b'#'
            && self.cells[cell] == Cell::Covered
            && self
                .neighbors(cell, false)
                .any(|neighbor| self.cells[neighbor] == Cell::Open)
    }

    fn adjacent_flags(&self, cell: usize) -> usize {
        self.neighbors(cell, true)
            .filter(|&neighbor| self.cells[neighbor] == Cell::Flagged)
            .count()
    }

    fn clue(&self, cell: usize) -> u8 {
        self.neighbors(cell, true)
            .filter(|&neighbor| self.features[neighbor] == b'M')
            .count() as u8
    }

    fn tile(&self, cell: usize) -> Tile {
        let feature = self.features[cell];
        match (feature, self.cells[cell]) {
            (b'#', _) => Tile::Bedrock,
            (_, Cell::Hit) => Tile::Mine { hit: true },
            (b'M', _) if self.status() == RoomStatus::Collapsed => Tile::Mine { hit: false },
            (_, Cell::Flagged) if self.status() == RoomStatus::Collapsed => Tile::FalseFlag,
            (_, Cell::Covered) => Tile::Covered {
                diamond: feature == b'C',
            },
            (_, Cell::Flagged) => Tile::Flagged,
            (_, Cell::Open) => Tile::Open {
                clue: self.clue(cell),
                diamond: feature == b'C',
            },
        }
    }

    fn diamonds(&self) -> (usize, usize) {
        let mut found = 0;
        let mut total = 0;
        for (&feature, cell) in self.features.iter().zip(self.cells) {
            if feature == b'C' {
                total += 1;
                found += usize::from(cell == Cell::Open);
            }
        }
        (found, total)
    }

    fn status(&self) -> RoomStatus {
        let (found, total) = self.diamonds();
        if self.cells.contains(&Cell::Hit) {
            RoomStatus::Collapsed
        } else if found == total {
            RoomStatus::Surveyed
        } else {
            RoomStatus::Exploring
        }
    }

    fn index(&self, (x, y): Position) -> Option<usize> {
        (x < self.side && y < self.side).then(|| y * self.side + x)
    }

    fn position(&self, cell: usize) -> Position {
        (cell % self.side, cell / self.side)
    }

    fn neighbors(&self, cell: usize, diagonals: bool) -> impl Iterator<Item = usize> + '_ {
        let (x, y) = self.position(cell);
        const OFFSETS: [(isize, isize); 8] = [
            (0, -1),
            (1, 0),
            (0, 1),
            (-1, 0),
            (-1, -1),
            (1, -1),
            (1, 1),
            (-1, 1),
        ];
        OFFSETS
            .into_iter()
            .take(if diagonals { 8 } else { 4 })
            .filter_map(move |(dx, dy)| {
                let cell = self.index((x.checked_add_signed(dx)?, y.checked_add_signed(dy)?))?;
                (self.features[cell] != b'#').then_some(cell)
            })
    }
}

fn mix(seed: u64) -> u64 {
    let mut value = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests;
