# Corner Table development handoff

Corner Table is an offline desktop games collection built in Rust and GPUI. The current app launches twelve original game families from the illustrated library. It supports System, Light, and Dark appearances, favorites, Last Played, and Back and Forward navigation with mouse navigation buttons and keyboard shortcuts.

## Playable games

| Game | Current features |
| --- | --- |
| Solitaire | Klondike, Spider, FreeCell, Pyramid, and TriPeaks. Card and stack dragging, click-to-move, illustrated court cards, multiple deal styles, and saved progress and undo. |
| Mahjong | Four solvable layouts, original illustrated ceramic tiles, keyboard controls, Clean Clear, hints, undo, and saved classic boards. |
| Minesweeper | Three fields, a clock and time trial, chording, undo-free replays, deduction-certified Logic only, and restored progress and bests. |
| Sudoku | 9×9 and 6×6 practice, daily clean solves, deduction hints, notes, undo, and saved rounds. |
| Jigsaw | Five original paintings, interlocking assembly and square swaps at three sizes, drag and keyboard controls, filters, hints, and gallery saves. |
| Word Games | Letter Grove, Letter Rack, and two crossword sizes. Bundled dictionary, puzzle collection, UTC daily puzzles, hints, and separate progress saves. |
| Match Three | Animated swaps and cascades, four stages, hints, keyboard controls, and saved rounds. |
| Block Puzzle | Accurate tray dragging, click and keyboard placement, animated line clears, Ten Trays, and saved progress and bests. |
| Mine Trails | Five rooms, branching routes, illustrated excavation, a finale, and saved expeditions. |
| Bubble Shooter | Animated bank shots, six stages, keyboard fine aim, challenge scoring, and resumable progress. |
| Gem Cascade | Timed falling pairs, untimed practice, pause, chains, and saved runs and bests. |
| Mahjong Cubes | Five solvable stacks, drag-to-turn and keyboard controls, precise face picking, a daily deal, undo, and saved progress. |

## Development

- `cargo run --locked` starts the normal desktop app. The macOS build compiles Metal shaders at startup and works with Apple’s Command Line Tools.
- `cargo xtask dev` uses Dioxus CLI 0.7.10 for GPUI in-process hot patching. Press `r` for a full build after changes to Rust data layouts, dependencies, or bundled assets.
- `python3 scripts/dev.py` rebuilds and relaunches after a successful build. Set `CORNER_TABLE_DATA_DIR` to a separate directory for UI checks against clean save data.
- Use `cargo fmt --all -- --check`, `cargo test --locked --workspace`, and `cargo clippy --locked --workspace --all-targets -- -D warnings` for workspace checks. Recent native checks were performed on macOS.
- Start new work from current `main`. The original `game/*` branches are historical checkpoints.

## Integration boundaries

- Independent rules crates live in `games/<game-id>/`. Keep GPUI and desktop I/O out of rules crates.
- Views expose `GameView::new(&GameLaunch, &mut Context<Self>)` in `apps/desktop/src/games/`. Store unique game art in `apps/desktop/assets/games/<game-id>/`.
- A new game needs a workspace member, desktop dependency, registry entry, and catalog entry in `crates/api/fixtures/v1/library.json`. The integration pass updates `Cargo.lock` after combining worktrees.
- Session files are versioned and use a per-launch key. Preserve older payloads when changing a game's save format. Daily challenge IDs carry the UTC date and a schedule version.
- The app runs offline. Bundled leaderboard examples remain fixture data, and public rankings and score submission are future work.
