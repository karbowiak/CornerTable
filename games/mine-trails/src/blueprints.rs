//! Hand-checked puzzle layouts. The seed chooses a layout and a symmetry.
//! '.' is dirt, '#' is solid rock, 'M' is a mine, 'C' is a visible safe
//! diamond, and 'S' is an already-open entrance.

pub(super) const BLUEPRINTS: [[&[&str]; 2]; 5] = [
    // Entrance: an unbroken face with an open approach.
    [
        &["M.....", "....CM", "S..M..", "......", ".....C", "..M..M"],
        &["M....M", "....CM", "S..M..", "......", ".....C", "..M..."],
    ],
    // Amber Drift: low mine density and tight quarters around the diamonds.
    [
        &["##M.C.", "#.....", "S.....", "...M..", "...M.#", "MCM###"],
        &["##..MC", "#.MM..", "S....M", "......", "..M..#", "..C###"],
    ],
    // Slate Drift: a pillar splits the approach to a dense seam.
    [
        &["#C..C#", "..MMMM", "..##..", "..##.M", ".....M", "M.S..#"],
        &["#.C.C#", "..M.MM", "..##..", "..##.M", "M....M", "M.S..#"],
    ],
    // Floodgate: two openings on either bank, with a long way round.
    [
        &[
            "#M..CM.", ".....M.", "S####..", "..##M..", "..###.S", ".M...C.", "CM..MM#",
        ],
        &[
            "#M...CM", ".....M.", "S####M.", "..##...", "..###.S", "..M..MC", "MC...M#",
        ],
    ],
    // Deep Vault: use the overlap between clues to solve the tight corners.
    [
        &[
            "#M.C.M#", ".M...MM", "..#....", "S.###MC", "....#..", "M.M...M", "#..C..M",
        ],
        &[
            "#.M.C.#", "..MM.MM", "..#....", "S.###.C", "...M#..", "M..MM..", "#...C.M",
        ],
    ],
];
