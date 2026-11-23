# Corner Table development handoff

Corner Table is an offline desktop games collection built in Rust and GPUI. The current app launches twenty-one game families from the illustrated library. It supports category filters, native search, Surprise me, System, Light, and Dark appearances, favorites, Last Played, and Back and Forward navigation with mouse buttons and keyboard shortcuts. The Daily Challenge calendar marks completed puzzles, and Records shows local results and personal bests and can open the recorded game or dated challenge.

The games present a result when a round ends. Results include the actual score or completion statistics and actions that fit the rules, such as replay, undo, a new deal, or the next stage. The app follows macOS Reduce Motion for gameplay effects, and View → Always Reduce Motion can reduce effects in Corner Table at any time.

## Playable games

| Game | Current features |
| --- | --- |
| Solitaire | Klondike, Spider, FreeCell, Pyramid, and TriPeaks. Animated stock deals, card and stack dragging, arrow-key play, twelve original court illustrations and four engraved aces, three table styles, 1,024 Verified Draw 1 Klondike deals, hints, safe foundation collection, and per-mode progress and undo. Spider detects when no legal draw or transfer remains. |
| Mahjong | Six solvable layouts from a 24-tile Tea Tray to a 144-tile Koi. Thirty-six original ceramic faces, keyboard controls, shareable practice deal codes, Clean Clear, hints, undo, and saved practice and Daily boards. |
| Minesweeper | Three carved fields, animated reveals and flags, a clock and time trial, chording, exact-layout practice replays, visible-clue hints, deduction-certified Logic only, and restored progress and bests. |
| Sudoku | 9×9 and 6×6 practice, three uniquely solvable Killer puzzles, daily clean solves, deduction hints, notes, undo, and saved rounds. |
| Jigsaw | Nine original scenes plus a gallery for local photo imports. Interlocking assembly and square swaps at three sizes, drag and keyboard controls, filters, hints, and gallery saves. |
| Word Games | Letter Grove, Letter Rack, two crossword sizes, and Word Search with 96 themed layouts. Collection, daily puzzles, hints, and separate saves. |
| Match Three | Seven campaign stages with points, collection, chains, and marked-cell objectives. Animated cascades, hints, keyboard controls, and exact resume. |
| Block Puzzle | Classic, Ten Trays, and nine verified Puzzle Boards. Exact tray dragging, click and keyboard placement, animated line clears, and saved progress and bests. |
| Mine Trails | Five rooms, branching routes, illustrated excavation, a finale, and saved expeditions. |
| Bubble Shooter | Animated bank shots, six stages, four verified bank-shot puzzles, keyboard fine aim, challenge scoring, and resumable progress. |
| Gem Cascade | Timed falling pairs, untimed practice, five distinct gem cuts, groups of four and chain reactions, pause, and saved runs and bests. |
| Mahjong Cubes | Five solvable 3D stacks, free camera orbit with drag or keyboard, depth-aware face picking, a daily deal, undo, and saved progress. |
| Reversi | Offline opponents or two people at the same board. Disc flips, automatic passes, saved matches, undo, and local results. |
| Checkers | American checkers with mandatory jumps and crowned kings. Three offline difficulties, hot-seat and exact resume. |
| Hearts | Four-player trick-taking against three offline opponents on wine-colored felt with an original card reverse. Animated passes and tricks, shoot-the-moon scoring, keyboard play, and match resume. |
| Picture Logic | Fifteen uniquely solvable nonograms. Drag painting, three tools, deductions, undo, and per-picture saves. |
| Memory Match | An illustrated tidepool deck with three sizes, adjustable mismatch handling, separate saves and records. |
| Kakuro | Ninety-six unique cross-sums puzzles at two sizes. Pencil marks, clue highlighting, hints, undo, and per-board progress. |
| Dominoes | Double-six Draw Dominoes with a wrapping chain, offline opponents, covered hot-seat handoffs, and saved match scores. |
| Chess | Offline and hot-seat play, original pieces, drag and keyboard moves, promotion, draw claims, and undo with saved replays. |
| Peg Solitaire | Classic 33-hole cross with copper pegs and teal enamel, pointer and keyboard play, center finishes, hints, undo, replay, and saved rounds. |

## Development

- `cargo run --locked` starts the normal desktop app. The macOS build compiles Metal shaders at startup and works with Apple’s Command Line Tools.
- `cargo xtask dev` uses Dioxus CLI 0.7.10 for GPUI in-process hot patching. Press `r` for a full build after changes to Rust data layouts, dependencies, or bundled assets.
- `cargo xtask dev --restart` rebuilds and relaunches after a successful build. For checks, use the GPUI harness or set `CORNER_TABLE_DATA_DIR` to a separate directory in a normal development process.
- `cargo xtask bundle` builds a locally signed release app at `target/Corner Table.app`.
- Use `cargo fmt --all -- --check`, `cargo test --locked --workspace`, and `cargo clippy --locked --workspace --all-targets -- -D warnings` for workspace checks. Recent native checks were performed on macOS.
- Start new work from current `main`. The original `game/*` branches are historical checkpoints.

## Integration boundaries

- Independent rules crates live in `games/<game-id>/`. Keep GPUI and desktop I/O out of rules crates.
- Views expose `GameView::new(&GameLaunch, &mut Context<Self>)` in `apps/desktop/src/games/`. Store unique game art in `apps/desktop/assets/games/<game-id>/`.
- A new game needs a workspace member, desktop dependency, registry entry, and catalog entry in `crates/api/fixtures/v1/library.json`. The integration pass updates `Cargo.lock` after combining worktrees.
- Session files are versioned and use a per-launch key. Preserve older payloads when changing a game's save format. Daily challenge IDs carry the UTC date and a schedule version.
- The app runs offline. Bundled leaderboard examples remain fixture data, and public rankings and score submission are future work.
