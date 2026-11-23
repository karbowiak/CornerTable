use crate::search_catalog::SearchTheme;

pub(crate) const DIRECTIONS: [(isize, isize); 8] = [
    (0, 1),
    (0, -1),
    (1, 0),
    (-1, 0),
    (1, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
];

pub(crate) struct Layout {
    pub grid: Vec<u8>,
    pub paths: Vec<(usize, usize)>,
}

pub(crate) fn generate(theme: SearchTheme, seed: u64) -> Option<Layout> {
    let mut random = Random(seed);
    let mut order: Vec<_> = (0..theme.words.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(theme.words[i].len()));
    let mut candidates = Vec::new();

    for _ in 0..256 {
        let mut grid = vec![0; theme.side * theme.side];
        let mut paths = vec![(0, 0); theme.words.len()];
        let mut directions = DIRECTIONS;
        for index in (1..directions.len()).rev() {
            directions.swap(index, random.index(index + 1));
        }
        for (position, &word_index) in order.iter().enumerate() {
            let word = theme.words[word_index].as_bytes();
            candidates.clear();
            // Seed every direction once before allowing the remaining words
            // to fill gaps in any direction.
            let available = directions
                .get(position)
                .map_or(&directions[..], std::slice::from_ref);
            for &direction in available {
                for start in 0..grid.len() {
                    let Some((end, stride)) = line(theme.side, start, direction, word.len()) else {
                        continue;
                    };
                    let squares = word.iter().enumerate().map(|(offset, letter)| {
                        let index = start.wrapping_add_signed(stride * offset as isize);
                        (grid[index], *letter)
                    });
                    if squares
                        .clone()
                        .all(|(cell, letter)| cell == 0 || cell == letter)
                    {
                        let overlap = squares.filter(|(cell, _)| *cell != 0).count();
                        if overlap < word.len() && overlap <= 2 {
                            candidates.push((start, end, stride));
                        }
                    }
                }
            }
            if candidates.is_empty() {
                break;
            }
            let (start, end, stride) = candidates[random.index(candidates.len())];
            paths[word_index] = (start, end);
            for (offset, &letter) in word.iter().enumerate() {
                grid[start.wrapping_add_signed(stride * offset as isize)] = letter;
            }
        }
        if paths.iter().any(|&(start, end)| start == end) {
            continue;
        }

        let empty: Vec<_> = (0..grid.len()).filter(|&i| grid[i] == 0).collect();
        for _ in 0..12 {
            for &index in &empty {
                grid[index] = b'A' + random.index(26) as u8;
            }
            if theme
                .words
                .iter()
                .all(|word| unique(&grid, theme.side, word.as_bytes()))
            {
                return Some(Layout { grid, paths });
            }
        }
    }
    None
}

fn line(
    side: usize,
    start: usize,
    (dy, dx): (isize, isize),
    length: usize,
) -> Option<(usize, isize)> {
    let steps = (length - 1) as isize;
    let row = (start / side).checked_add_signed(dy * steps)?;
    let col = (start % side).checked_add_signed(dx * steps)?;
    (row < side && col < side).then_some((row * side + col, dy * side as isize + dx))
}

fn unique(grid: &[u8], side: usize, word: &[u8]) -> bool {
    let mut matched = None;
    for (start, _) in grid
        .iter()
        .enumerate()
        .filter(|(_, letter)| **letter == word[0])
    {
        for direction in DIRECTIONS {
            let Some((end, stride)) = line(side, start, direction, word.len()) else {
                continue;
            };
            if word.iter().enumerate().all(|(offset, &letter)| {
                grid[start.wrapping_add_signed(stride * offset as isize)] == letter
            }) {
                let span = (start.min(end), start.max(end));
                if matched.is_some_and(|path| path != span) {
                    return false;
                }
                matched = Some(span);
            }
        }
    }
    matched.is_some()
}

struct Random(u64);

impl Random {
    fn index(&mut self, len: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 32) % len as u64) as usize
    }
}
