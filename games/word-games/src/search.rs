use crate::search_catalog::{LAYOUTS_PER_THEME, SearchTheme, THEMES};

type Layout = (&'static [u8], &'static [(usize, usize)]);
const BOARDS: &[[Layout; LAYOUTS_PER_THEME]] =
    include!(concat!(env!("OUT_DIR"), "/word_search_boards.rs"));
// Keep the original seeded rotation stable when themes are added to the collection.
const SEEDED_THEME_COUNT: u64 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchLine {
    pub start: usize,
    pub end: usize,
}

impl SearchLine {
    pub fn between(side: usize, start: usize, end: usize) -> Option<Self> {
        if side == 0 || start == end || start / side >= side || end / side >= side {
            return None;
        }
        let rows = (start / side).abs_diff(end / side);
        let columns = (start % side).abs_diff(end % side);
        (rows == 0 || columns == 0 || rows == columns).then_some(Self { start, end })
    }

    fn same_span(self, other: Self) -> bool {
        self == other || (self.start == other.end && self.end == other.start)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchRejection {
    NotStraight,
    NoMatch,
    AlreadyFound,
    Complete,
}

pub struct WordSearch {
    theme: SearchTheme,
    letters: Vec<u8>,
    lines: Vec<SearchLine>,
    found: Vec<bool>,
}

impl WordSearch {
    pub fn themes() -> &'static [SearchTheme] {
        THEMES
    }

    pub fn collection(index: usize) -> Self {
        Self::layout(index % THEMES.len(), 0, 0)
    }

    pub fn seeded(seed: u64) -> Self {
        let index = (seed % SEEDED_THEME_COUNT) as usize;
        // Rotate through themes in order and mix the remaining bits to choose
        // a layout and an independent reflection/rotation for each seed.
        let mut mixed = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^= mixed >> 31;
        let layout = 1 + (mixed % (LAYOUTS_PER_THEME as u64 - 1)) as usize;
        Self::layout(index, layout, ((mixed >> 32) & 7) as u8)
    }

    /// Generate a reproducible board for one collection theme. This generator
    /// is part of the v1 generated-save format, along with the theme indices.
    pub fn generated(index: usize, seed: u64) -> Self {
        let index = index % THEMES.len();
        let theme = THEMES[index];
        let seed = seed ^ 0xc401_7b94_2e16_0f85 ^ ((index as u64) << 32);
        match crate::search_generate::generate(theme, seed) {
            Some(layout) => Self::from_layout(theme, &layout.grid, &layout.paths, 0),
            None => Self::layout(
                index,
                (seed % LAYOUTS_PER_THEME as u64) as usize,
                (seed >> 32) as u8 & 7,
            ),
        }
    }

    fn layout(index: usize, layout: usize, transform: u8) -> Self {
        let (template, paths) = BOARDS[index][layout];
        Self::from_layout(THEMES[index], template, paths, transform)
    }

    fn from_layout(
        theme: SearchTheme,
        template: &[u8],
        paths: &[(usize, usize)],
        transform: u8,
    ) -> Self {
        let mut letters = vec![0; template.len()];
        let map = |i| transform_cell(theme.side, i, transform);
        for (i, &letter) in template.iter().enumerate() {
            letters[map(i)] = letter;
        }
        Self {
            theme,
            letters,
            lines: paths
                .iter()
                .map(|&(start, end)| SearchLine {
                    start: map(start),
                    end: map(end),
                })
                .collect(),
            found: vec![false; paths.len()],
        }
    }

    pub fn theme(&self) -> SearchTheme {
        self.theme
    }

    pub fn letters(&self) -> &[u8] {
        &self.letters
    }

    /// The endpoints in reading order, including the first letter for hints.
    pub fn line(&self, word_index: usize) -> SearchLine {
        self.lines[word_index]
    }

    pub fn found(&self, word_index: usize) -> bool {
        self.found[word_index]
    }

    pub fn found_count(&self) -> usize {
        self.found.iter().filter(|&&found| found).count()
    }

    pub fn complete(&self) -> bool {
        self.found.iter().all(|&found| found)
    }

    pub fn hint(&self) -> Option<usize> {
        self.found.iter().position(|&found| !found)
    }

    pub fn submit(&mut self, start: usize, end: usize) -> Result<usize, SearchRejection> {
        if self.complete() {
            return Err(SearchRejection::Complete);
        }
        let line =
            SearchLine::between(self.theme.side, start, end).ok_or(SearchRejection::NotStraight)?;
        let index = self
            .lines
            .iter()
            .position(|&solution| solution.same_span(line))
            .ok_or(SearchRejection::NoMatch)?;
        if self.found[index] {
            return Err(SearchRejection::AlreadyFound);
        }
        self.found[index] = true;
        Ok(index)
    }

    /// Store words as stable identifiers and reconstruct their strokes from
    /// the puzzle. Unknown entries have no effect.
    pub fn restore(&mut self, words: &[String]) {
        for (found, word) in self.found.iter_mut().zip(self.theme.words) {
            *found = words.iter().any(|saved| saved.eq_ignore_ascii_case(word));
        }
    }
}

fn transform_cell(side: usize, index: usize, transform: u8) -> usize {
    let (mut row, mut column) = (index / side, index % side);
    if transform & 1 != 0 {
        std::mem::swap(&mut row, &mut column);
    }
    if transform & 2 != 0 {
        row = side - 1 - row;
    }
    if transform & 4 != 0 {
        column = side - 1 - column;
    }
    row * side + column
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_layout_has_exactly_one_straight_span_for_each_listed_word() {
        for (index, theme) in THEMES.iter().enumerate() {
            let side = theme.side;
            let mut grids = BTreeSet::new();
            for layout in 0..LAYOUTS_PER_THEME {
                let mut board = WordSearch::layout(index, layout, layout as u8);
                let mut directions = BTreeSet::new();
                assert!(grids.insert(board.letters.clone()));
                for (word_index, word) in theme.words.iter().enumerate() {
                    let expected = board.line(word_index);
                    let mut occurrences = BTreeSet::new();
                    for start in 0..board.letters.len() {
                        for dy in -1isize..=1 {
                            for dx in -1isize..=1 {
                                if (dx, dy) == (0, 0) {
                                    continue;
                                }
                                let mut end = start;
                                let matches = word.bytes().enumerate().all(|(i, letter)| {
                                    let row = (start / side) as isize + dy * i as isize;
                                    let col = (start % side) as isize + dx * i as isize;
                                    if !(0..side as isize).contains(&row)
                                        || !(0..side as isize).contains(&col)
                                    {
                                        return false;
                                    }
                                    end = row as usize * side + col as usize;
                                    board.letters[end] == letter
                                });
                                if matches {
                                    occurrences.insert((start.min(end), start.max(end)));
                                }
                            }
                        }
                    }
                    assert_eq!(
                        occurrences,
                        BTreeSet::from([(
                            expected.start.min(expected.end),
                            expected.start.max(expected.end)
                        )]),
                        "{} layout {layout}, {word}",
                        theme.title,
                    );
                    directions.insert((
                        (expected.end / side).cmp(&(expected.start / side)),
                        (expected.end % side).cmp(&(expected.start % side)),
                    ));
                    assert_eq!(board.letters[expected.start], word.as_bytes()[0]);
                    assert_eq!(board.submit(expected.end, expected.start), Ok(word_index));
                }
                assert_eq!(directions.len(), 8);
                assert!(board.complete());
            }
        }
    }

    #[test]
    fn collection_templates_and_seeded_rotation_are_stable_and_distinct() {
        assert_eq!(
            WordSearch::collection(0).letters(),
            b"NDBVQNNBYSIMWIQAATMQPHXRKBEVUCLNGPAPTRSAUZSLMUVCSTCTOIXLSPEVSNLEWVKELPEGENOMENARX",
        );
        let boards_for_one_theme =
            (0..30).map(|cycle| WordSearch::seeded(cycle * SEEDED_THEME_COUNT));
        assert!(
            boards_for_one_theme
                .map(|board| {
                    assert_eq!(board.theme().title, "Tidal Pools");
                    board.letters
                })
                .collect::<BTreeSet<_>>()
                .len()
                > 10
        );
    }

    #[test]
    fn live_generation_repeats_by_seed_and_uses_all_collection_themes() {
        for (index, theme) in THEMES.iter().enumerate() {
            let mut board = WordSearch::generated(index, 21);
            assert_eq!(board.theme(), *theme);
            assert_eq!(board.letters(), WordSearch::generated(index, 21).letters());
            assert_ne!(board.letters(), WordSearch::generated(index, 22).letters());
            for word in 0..theme.words.len() {
                let line = board.line(word);
                assert_eq!(board.submit(line.end, line.start), Ok(word));
            }
            assert!(board.complete());
        }
    }

    #[test]
    fn same_seeds_resume_and_wrong_or_duplicate_selections_keep_progress() {
        let mut board = WordSearch::seeded(u64::MAX);
        let same = WordSearch::seeded(u64::MAX);
        assert_eq!(board.letters(), same.letters());
        assert_eq!(board.lines, same.lines);
        assert_ne!(board.letters(), WordSearch::seeded(u64::MAX - 1).letters());
        let side = board.theme().side;
        assert_eq!(board.submit(0, side + 2), Err(SearchRejection::NotStraight));
        assert_eq!(
            board.submit(0, side * side),
            Err(SearchRejection::NotStraight)
        );
        assert_eq!(board.submit(0, 1), Err(SearchRejection::NoMatch));
        let line = board.line(0);
        assert_eq!(board.submit(line.start, line.end), Ok(0));
        assert_eq!(
            board.submit(line.end, line.start),
            Err(SearchRejection::AlreadyFound)
        );
        assert_eq!(board.hint(), Some(1));
        let mut restored = same;
        restored.restore(&[board.theme().words[0].to_lowercase(), "unknown".into()]);
        assert_eq!(restored.found_count(), 1);
        assert_eq!(restored.hint(), Some(1));
    }
}
