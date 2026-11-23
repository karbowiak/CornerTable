//! Rules and deterministic deals for layered Mahjong solitaire.

pub const DAILY_SEED: u64 = 0x434f_524e_4552_0001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Terrace,
    Atrium,
    Crown,
    Lantern,
    TeaTray,
    Koi,
}

impl Layout {
    pub const ALL: [Self; 6] = [
        Self::TeaTray,
        Self::Terrace,
        Self::Atrium,
        Self::Crown,
        Self::Lantern,
        Self::Koi,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Terrace => "terrace",
            Self::Atrium => "atrium",
            Self::Crown => "crown",
            Self::Lantern => "lantern",
            Self::TeaTray => "tea-tray",
            Self::Koi => "koi",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layout| layout.id() == id)
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Terrace => "Terrace",
            Self::Atrium => "Atrium",
            Self::Crown => "Crown",
            Self::Lantern => "Lantern",
            Self::TeaTray => "Tea Tray",
            Self::Koi => "Koi",
        }
    }

    pub const fn tile_count(self) -> usize {
        match self {
            Self::Terrace | Self::Koi => 144,
            Self::Atrium => 116,
            Self::Crown => 92,
            Self::Lantern => 102,
            Self::TeaTray => 24,
        }
    }

    fn tiles(self) -> Vec<Tile> {
        let mut tiles = Vec::with_capacity(self.tile_count());
        match self {
            Self::Terrace => {
                centered_rows(&mut tiles, 0, 0, 14, &[10, 12, 12, 14, 14, 12, 12, 10]);
                centered_rows(&mut tiles, 1, 4, 14, &[8, 8, 8, 8]);
                centered_rows(&mut tiles, 2, 5, 14, &[4, 4, 4]);
                centered_rows(&mut tiles, 3, 6, 14, &[2, 2]);
            }
            Self::Atrium => {
                // Four raised corners frame an open courtyard.
                centered_rows(&mut tiles, 0, 0, 14, &[14, 14]);
                for y in [4, 6, 8, 10] {
                    add_row(&mut tiles, 0, y, 0, 4);
                    add_row(&mut tiles, 0, y, 20, 4);
                }
                centered_rows(&mut tiles, 0, 12, 14, &[14, 14]);
                for y in [2, 4, 10, 12] {
                    add_row(&mut tiles, 1, y, 2, 3);
                    add_row(&mut tiles, 1, y, 20, 3);
                }
                for y in [3, 11] {
                    add_row(&mut tiles, 2, y, 4, 1);
                    add_row(&mut tiles, 2, y, 22, 1);
                }
            }
            Self::Crown => {
                // Three towers share a low, stepped foundation.
                for x in [0, 10, 20] {
                    add_row(&mut tiles, 0, 0, x, 2);
                }
                centered_rows(&mut tiles, 0, 2, 12, &[12, 12, 12, 10, 8]);
                for (level, rows) in [(1, &[2, 4, 6][..]), (2, &[2, 4][..])] {
                    for &y in rows {
                        for x in [0, 10, 20] {
                            add_row(&mut tiles, level, y, x, 2);
                        }
                    }
                }
                centered_rows(&mut tiles, 3, 3, 12, &[2]);
            }
            Self::Lantern => {
                centered_rows(&mut tiles, 0, 0, 10, &[2, 4, 6, 8, 10, 10, 8, 6, 4, 2]);
                centered_rows(&mut tiles, 1, 4, 10, &[2, 4, 6, 6, 4, 2]);
                centered_rows(&mut tiles, 2, 6, 10, &[2, 4, 4, 2]);
                centered_rows(&mut tiles, 3, 8, 10, &[2, 2]);
                centered_rows(&mut tiles, 4, 9, 10, &[2]);
            }
            Self::TeaTray => {
                centered_rows(&mut tiles, 0, 0, 6, &[4, 6, 6, 4]);
                centered_rows(&mut tiles, 1, 3, 6, &[2, 2]);
            }
            Self::Koi => {
                // A forked tail at the left meets the body at the middle.
                // The head rises on the right between the two offset fins.
                // Runs are (row, first column, tile count), from left to right.
                let layers: [&[(u8, u8, u8)]; 3] = [
                    &[
                        (0, 0, 2),
                        (0, 8, 2),
                        (1, 0, 3),
                        (1, 7, 5),
                        (2, 0, 4),
                        (2, 6, 8),
                        (3, 1, 14),
                        (4, 2, 14),
                        (5, 1, 14),
                        (6, 0, 4),
                        (6, 6, 8),
                        (7, 0, 3),
                        (7, 8, 5),
                        (8, 0, 2),
                        (8, 10, 2),
                    ],
                    &[
                        (1, 9, 2),
                        (2, 7, 6),
                        (3, 2, 2),
                        (3, 7, 6),
                        (4, 2, 2),
                        (4, 6, 8),
                        (5, 7, 6),
                        (6, 7, 6),
                        (7, 9, 2),
                    ],
                    &[(2, 9, 2), (3, 9, 4), (4, 10, 4), (5, 10, 2)],
                ];
                for (level, runs) in layers.into_iter().enumerate() {
                    for &(row, column, width) in runs {
                        add_row(&mut tiles, level as u8, row * 2, column * 2, width);
                    }
                }
                add_row(&mut tiles, 3, 7, 22, 2);
            }
        }
        tiles
    }
}

fn centered_rows(tiles: &mut Vec<Tile>, level: u8, y: u8, center: u8, widths: &[u8]) {
    for (row, &width) in widths.iter().enumerate() {
        add_row(tiles, level, y + row as u8 * 2, center - width, width);
    }
}

fn add_row(tiles: &mut Vec<Tile>, level: u8, y: u8, x: u8, width: u8) {
    for column in 0..width {
        tiles.push(Tile {
            x: x + column * 2,
            y,
            level,
            face: Face::Dots(1),
        });
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileState {
    Free,
    Covered,
    SidesBlocked,
}

pub struct Board {
    layout: Layout,
    tiles: Vec<Tile>,
    removed: Vec<bool>,
    history: Vec<[usize; 2]>,
    selected: Option<usize>,
    used_assistance: bool,
}

impl Board {
    pub fn new(seed: u64) -> Self {
        Self::with_layout(Layout::Terrace, seed)
    }

    pub fn with_layout(layout: Layout, seed: u64) -> Self {
        Self::deal(layout, seed).0
    }

    /// Restore a seeded deal by replaying its surviving matches in order.
    pub fn resume(
        layout: Layout,
        seed: u64,
        matches: &[[usize; 2]],
        assisted: bool,
    ) -> Option<Self> {
        let mut board = Self::with_layout(layout, seed);
        if matches.len() > board.remaining() / 2 {
            return None;
        }
        for &[first, second] in matches {
            if board.select(first) != MoveResult::Selected
                || board.select(second) != MoveResult::Matched
            {
                return None;
            }
        }
        board.used_assistance = assisted;
        Some(board)
    }

    fn deal(layout: Layout, seed: u64) -> (Self, Vec<[usize; 2]>) {
        let tiles = layout.tiles();
        let pair_count = tiles.len() / 2;
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
        if layout == Layout::TeaTray {
            // Each face has exactly one partner, so any legal match keeps the
            // remainder solvable. Removing tiles can only open more moves.
            faces.dedup();
        }

        let mut board = Self {
            layout,
            removed: vec![false; tiles.len()],
            history: Vec::new(),
            selected: None,
            used_assistance: false,
            tiles,
        };
        let mut pairs = Vec::with_capacity(pair_count);
        // Try randomized legal removal orders. If one leaves a lone free tile,
        // fall back to peeling mirrored row ends from the highest layers down.
        for _ in 0..16 {
            pairs.clear();
            board.removed.fill(false);
            while pairs.len() < pair_count {
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
            if pairs.len() == pair_count {
                break;
            }
        }
        if pairs.len() != pair_count {
            board.peel_rows(&mut pairs);
        }
        board.removed.fill(false);
        for &[first, second] in &pairs {
            let face = faces.swap_remove(rng.index(faces.len()));
            board.tiles[first].face = face;
            board.tiles[second].face = face;
        }
        (board, pairs)
    }

    fn peel_rows(&self, pairs: &mut Vec<[usize; 2]>) {
        pairs.clear();
        let highest = self.tiles.last().unwrap().level;
        let (_, height) = self.dimensions();
        // Every layer's rows are at least one tile-height apart and contain an
        // even number of tiles, including rows that contain separate runs.
        // Peel the leftmost and rightmost ends after clearing the layers above.
        for level in (0..=highest).rev() {
            for y in 0..height {
                let row: Vec<_> = self
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

    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Full board dimensions in half-tile units, including removed tiles.
    pub fn dimensions(&self) -> (u8, u8) {
        self.tiles.iter().fold((0, 0), |(width, height), tile| {
            (width.max(tile.x + 2), height.max(tile.y + 2))
        })
    }

    pub fn tiles(&self) -> impl Iterator<Item = (usize, &Tile)> {
        self.tiles
            .iter()
            .enumerate()
            .filter(|(id, _)| !self.removed[*id])
    }

    /// The full, unchanging footprint, in tile ID order.
    pub fn all_tiles(&self) -> &[Tile] {
        &self.tiles
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn remaining(&self) -> usize {
        self.tiles.len() - self.history.len() * 2
    }

    pub fn used_assistance(&self) -> bool {
        self.used_assistance
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn matched_pairs(&self) -> &[[usize; 2]] {
        &self.history
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Look up a tile by stable ID, including after it has been matched.
    pub fn tile(&self, id: usize) -> Option<&Tile> {
        self.tiles.get(id)
    }

    pub fn is_free(&self, id: usize) -> bool {
        self.tile_state(id) == Some(TileState::Free)
    }

    /// Explain how a tile can be freed. Removed and unknown tiles have no state.
    pub fn tile_state(&self, id: usize) -> Option<TileState> {
        let tile = self.tiles.get(id)?;
        if self.removed[id] {
            return None;
        }
        let mut left = false;
        let mut right = false;
        for (_, other) in self.tiles() {
            if tile.y.abs_diff(other.y) >= 2 {
                continue;
            }
            if other.level > tile.level && tile.x.abs_diff(other.x) < 2 {
                return Some(TileState::Covered);
            }
            if tile.level == other.level {
                left |= other.x + 2 == tile.x;
                right |= tile.x + 2 == other.x;
            }
        }
        Some(if left && right {
            TileState::SidesBlocked
        } else {
            TileState::Free
        })
    }

    fn free_tiles(&self) -> Vec<usize> {
        self.tiles()
            .map(|(id, _)| id)
            .filter(|&id| self.is_free(id))
            .collect()
    }

    pub fn hint(&self) -> Option<[usize; 2]> {
        let free = self.free_tiles();
        if let Some(first) = self.selected
            && let Some(&second) = free
                .iter()
                .find(|&&id| id != first && self.tiles[id].face == self.tiles[first].face)
        {
            return Some([first, second]);
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

    /// Reveal a hint to the player and record it for the Clean Clear objective.
    pub fn reveal_hint(&mut self) -> Option<[usize; 2]> {
        let pair = self.hint();
        self.used_assistance |= pair.is_some();
        pair
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
        self.used_assistance = true;
        true
    }

    pub fn restart(&mut self) {
        self.removed.fill(false);
        self.history.clear();
        self.selected = None;
        self.used_assistance = false;
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
        assert_eq!(board.tile_state(id_at(4, 0, 0)), Some(TileState::Free));
        assert_eq!(
            board.tile_state(id_at(6, 0, 0)),
            Some(TileState::SidesBlocked)
        );
        assert_eq!(board.tile_state(id_at(10, 4, 1)), Some(TileState::Covered));
        assert_eq!(board.tile_state(id_at(12, 6, 3)), Some(TileState::Free));
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
        for layout in Layout::ALL {
            for seed in (0..32).chain([DAILY_SEED, u64::MAX]) {
                let (mut board, solution) = Board::deal(layout, seed);
                assert_eq!(solution.len() * 2, layout.tile_count(), "{layout:?}");
                for [first, second] in solution {
                    assert_eq!(
                        board.select(first),
                        MoveResult::Selected,
                        "{layout:?} seed {seed}"
                    );
                    assert_eq!(
                        board.select(second),
                        MoveResult::Matched,
                        "{layout:?} seed {seed}"
                    );
                }
                assert_eq!(board.remaining(), 0);
                assert!(!board.used_assistance());
            }
        }
    }

    #[test]
    fn the_fallback_order_can_clear_every_layout() {
        for layout in Layout::ALL {
            let mut board = Board::with_layout(layout, DAILY_SEED);
            board
                .tiles
                .iter_mut()
                .for_each(|tile| tile.face = Face::Flower);
            let mut pairs = Vec::new();
            board.peel_rows(&mut pairs);
            for [first, second] in pairs {
                assert_eq!(board.select(first), MoveResult::Selected, "{layout:?}");
                assert_eq!(board.select(second), MoveResult::Matched, "{layout:?}");
            }
            assert_eq!(board.remaining(), 0, "{layout:?}");
        }
    }

    #[test]
    fn new_layouts_have_supported_stacks_and_the_expected_footprints() {
        for (layout, dimensions, counts) in [
            (Layout::TeaTray, (12, 8), &[20, 4][..]),
            (Layout::Koi, (32, 18), &[90, 40, 12, 2][..]),
        ] {
            let board = Board::with_layout(layout, DAILY_SEED);
            let tiles = board.all_tiles();
            assert_eq!(board.dimensions(), dimensions);
            assert_eq!(tiles.len(), layout.tile_count());
            assert_eq!(Layout::from_id(layout.id()), Some(layout));
            let mut occupied = std::collections::HashSet::new();
            let mut actual_counts = vec![0; counts.len()];
            for tile in tiles {
                actual_counts[usize::from(tile.level)] += 1;
                for x in tile.x..tile.x + 2 {
                    for y in tile.y..tile.y + 2 {
                        assert!(occupied.insert((x, y, tile.level)), "{layout:?}: overlap");
                        assert!(
                            tile.level == 0 || occupied.contains(&(x, y, tile.level - 1)),
                            "{layout:?}: unsupported tile {tile:?}"
                        );
                    }
                }
            }
            assert_eq!(actual_counts, counts);
        }
    }

    #[test]
    fn tea_tray_gives_every_tile_one_partner() {
        for seed in [0, 1, DAILY_SEED, u64::MAX] {
            let board = Board::with_layout(Layout::TeaTray, seed);
            let tiles = board.all_tiles();
            for tile in tiles {
                assert_eq!(
                    tiles.iter().filter(|other| tile.face == other.face).count(),
                    2
                );
            }
        }
    }

    #[test]
    fn original_seeded_tile_ids_and_faces_are_stable() {
        // Fingerprints captured before adding layouts. The dated seed comes
        // from the Daily Challenge ID daily-v1-2026-11-23-mahjong.
        for (layout, seed, expected) in [
            (Layout::Terrace, DAILY_SEED, 0x441a1b301c5ff01d),
            (Layout::Atrium, DAILY_SEED, 0xa90bb810abb58481),
            (Layout::Crown, DAILY_SEED, 0x9950833f9eabb475),
            (Layout::Lantern, DAILY_SEED, 0x903d436a1e8ccf47),
            (Layout::Terrace, 0x02825903f7bd2635, 0x461e1ea0cf1f29f5),
        ] {
            let board = Board::with_layout(layout, seed);
            let bytes = board.all_tiles().iter().flat_map(|tile| {
                let face = match tile.face {
                    Face::Dots(rank) => rank,
                    Face::Bamboo(rank) => 10 + rank,
                    Face::Characters(rank) => 20 + rank,
                    Face::Wind(letter) => letter as u8,
                    Face::Dragon(letter) => letter as u8 + 64,
                    Face::Flower => 200,
                    Face::Season => 201,
                };
                [tile.x, tile.y, tile.level, face]
            });
            let fingerprint = bytes.fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
            assert_eq!(fingerprint, expected, "{layout:?} seed {seed}");
        }
    }

    #[test]
    fn only_revealed_hints_and_successful_undos_use_assistance() {
        let mut board = Board::new(DAILY_SEED);
        let [first, second] = board.hint().unwrap();
        assert!(!board.undo());
        assert!(!board.used_assistance());
        board.select(first);
        board.select(second);
        assert!(board.undo());
        assert!(board.used_assistance());
        board.restart();
        assert!(!board.used_assistance());
        assert!(board.reveal_hint().is_some());
        assert!(board.used_assistance());
    }

    #[test]
    fn resumed_matches_preserve_order_undo_and_assistance() {
        for layout in Layout::ALL {
            let (mut board, solution) = Board::deal(layout, DAILY_SEED);
            for &[first, second] in &solution[..3] {
                board.select(first);
                assert_eq!(board.select(second), MoveResult::Matched);
            }
            assert!(board.undo());
            let mut restored = Board::resume(
                layout,
                DAILY_SEED,
                board.matched_pairs(),
                board.used_assistance(),
            )
            .unwrap();
            assert_eq!(restored.matched_pairs(), &solution[..2]);
            assert_eq!(restored.hint(), board.hint());
            assert!(restored.used_assistance());
            assert!(restored.undo());
            assert_eq!(restored.matched_pairs(), &solution[..1]);
            assert!(Board::resume(layout, DAILY_SEED, &[solution[0]; 2], false).is_none());
            assert!(Board::resume(layout, DAILY_SEED, &[[usize::MAX, 0]], false).is_none());
        }
    }
}
