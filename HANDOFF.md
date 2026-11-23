# Corner Table development handoff

Corner Table is an original Linux-first casual games suite built in Rust and GPUI. The first practical goal is a polished replacement for the Windows casual games someone plays every day. Keep the desktop app usable offline with large readable boards, obvious controls, and quick resume. The long-term plan adds original daily puzzles and challenge modes, a public site for guest competition, and potentially an ad-free paid mobile edition.

## Current checkpoint

`main` contains the desktop shell and five playable game families. Individual game branches preserve each agent's work. The remaining branches are checkpoints and include some code that is ready only for rules-level testing.

| Game | Branch | State and next task |
| --- | --- | --- |
| Solitaire | `game/solitaire` | On `main`. Klondike draw one, single-suit Spider, FreeCell, Pyramid, and TriPeaks. Click to move, undo, restart, and new deals. Add drag and drop, more difficulty variants, and solvable deals where practical. |
| Mahjong | `game/mahjong` | On `main`. 144 tiles, solvable seeded boards, hints, undo, and a fixed daily deal. Add more original layouts and challenge objectives. |
| Minesweeper | `game/minesweeper` | On `main`. Two board sizes, safe first reveal and surroundings, flags, flood fill, and win/loss. Add more challenge objectives and a timed mode. |
| Sudoku | `game/sudoku` | On `main`. Three difficulties, uniquely solvable generated puzzles, notes, undo, and conflict highlighting. Add further puzzle formats and challenge objectives. |
| Jigsaw | `game/jigsaw` | Lighthouse puzzle on `main`. Swap 12 square tiles and toggle the reference. The branch also contains a generated harbor painting to wire into a second puzzle. Add piece counts and shaped pieces or drag and drop. |
| Word Games | `game/word-games` | Rules and GPUI view checkpoint for a letter grid, letter rack, and mini crossword. Includes an English dictionary and its license. Register the crate and view, then verify the desktop and rules before merging. |
| Match Three | `game/match-three` | Seeded board, swaps, match/cascade scoring, and end-state rules. Implement and register the GPUI board. |
| Block Puzzle | `game/block-puzzle` | Seeded placement and scoring rules. Implement and register the GPUI board. |
| Mine Trails | `game/mine-trails` | Original mine entrance painting saved. Implement rooms with mine clues, progression, and the GPUI board. |
| Bubble Shooter | `game/bubble-shooter` | Starter branch. Implement aiming, wall bounces, cluster matching, and dropping disconnected bubbles. |
| Gem Cascade | `game/gem-cascade` | Starter branch. Implement dropping pairs, group clears, gravity, and chain scoring. |
| Mahjong Cubes | `game/mahjong-cubes` | Starter branch. Implement cube eligibility and layer rules, then project and hit-test the frontmost cube faces on GPUI's canvas. |

Game sessions are kept in memory when navigating away and back during the same app run. Disk saves, automatic daily rotation, remote score submission, and live rankings are future work. The current challenge, guest profile, and leaderboards come from bundled JSON fixtures.

## Recreate worktrees on your computer

From a new checkout, use the HTTPS or SSH URL for `karbowiak/games`. If you already have a clone, start at `git fetch origin`.

```sh
git clone git@github.com:karbowiak/games.git corner-table
cd corner-table
git fetch origin
mkdir -p ../corner-worktrees
for game in solitaire mahjong minesweeper sudoku jigsaw word-games match-three bubble-shooter mine-trails block-puzzle gem-cascade mahjong-cubes; do
    git worktree add --track -b "game/$game" "../corner-worktrees/$game" "origin/game/$game" || break
done
```

Git worktree registrations are local to each clone. The commands recreate the branch checkouts on your machine. If a local game branch already exists, use `git worktree add ../corner-worktrees/<game> game/<game>` for that branch.

The game branches descend from the desktop scaffold. Before further development, move an unfinished branch onto the current app by branching from `main` and cherry-picking its game-specific commits. For Jigsaw, take only the harbor artwork commit since `main` already has the lighthouse puzzle. The branch histories show which changes to pick. Keep each agent in its own worktree and let the integration pass own the shared registry and `Cargo.lock`.

```sh
# Example from the Word Games worktree
git switch -c resume/word-games main
git cherry-pick origin/game/word-games
```

## Integration and verification

- Game rules live under `games/<game-id>/`. Keep them independent of GPUI and the API so they can also validate replays on a future server.
- The desktop view lives under `apps/desktop/src/games/` with any assets under `apps/desktop/assets/games/<game-id>/`. Follow the existing `GameView::new(&GameLaunch, &mut Context<Self>)` convention.
- Add the crate to the root workspace, the dependency to `apps/desktop/Cargo.toml`, and the route to `apps/desktop/src/games/mod.rs`. Run Cargo without `--locked` once to record new workspace members. Then run the locked checks.
- Run `cargo fmt --all -- --check`, `cargo test --locked --workspace`, and `cargo clippy --locked --workspace --all-targets -- -D warnings`. Run `cargo run --locked` and open each game through its actual launcher entry. For Mahjong also check the Daily Challenge route.

Once the boards are usable, prioritize disk saves and a versioned daily puzzle format with reproducible moves. The eventual server can issue a guest ID and daily puzzle ID, replay moves with the same rules crate, and publish rankings. Keep play local and offline by default. Treat public competition and mobile packaging as separate milestones.
