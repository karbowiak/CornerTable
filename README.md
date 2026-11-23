# Corner Table

Corner Table is a native Rust and GPUI collection of twelve tabletop and arcade game families. Play offline, browse an illustrated library, favorite games, or open a deterministic daily puzzle from a week of challenges. The app includes its fonts, artwork, and sample data in the binary. See the [game directory](games/README.md) for the games and the [development handoff](HANDOFF.md) for integration notes.

## Run on Linux

Install stable Rust. On Debian or Ubuntu, install the build tools and desktop libraries.

```sh
sudo apt install build-essential pkg-config libfontconfig1-dev libfreetype-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libx11-dev libx11-xcb-dev libxcb1-dev libvulkan1
cargo run --locked
```

GPUI uses your desktop's Wayland or X11 session and a Vulkan driver. Mesa users generally get the driver from `mesa-vulkan-drivers`. The mock API runs in process. You can run the app offline.

## Run on macOS

Install Rust and Apple's Command Line Tools, then run `cargo run --locked`.

## Develop with hot reload

Install [Dioxus CLI 0.7.10](https://github.com/DioxusLabs/dioxus/releases/tag/v0.7.10)
and run `cargo xtask dev` from the repository root. Set `CORNER_TABLE_DX`
to the absolute path of the executable if multiple tools supply `dx`.

GPUI applies compatible Rust view edits in the running app, preserving the
current game and its in-memory state. Changes to data layouts, startup code,
dependencies, or bundled assets can require a full build. Press `r` in the
interactive Dioxus terminal for a full rebuild, or restart the dev command.

Run `python3 scripts/dev.py` for automatic rebuilds and fresh launches after
source or asset edits. The current window stays open if a build fails. Press
Ctrl+C in either watcher terminal to stop it and its game client.

## Checks

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

## Workspace layout

| Path | Owns |
| --- | --- |
| `apps/desktop/src/pages/` | The library, Solitaire collection, daily challenge, and leaderboard |
| `apps/desktop/src/games/` | Game launch context and per-game GPUI views |
| `apps/desktop/src/theme.rs` and `widgets.rs` | Shared typography, colors, and desktop controls |
| `apps/desktop/assets/` | Original prototype illustrations and bundled fonts |
| `crates/api/` | JSON wire types and the fixture-backed API |
| `games/` | Per-game rules, generators, and tests as the games land |

## Mock API

| Planned route | Bundled JSON |
| --- | --- |
| `GET /v1/library` | `crates/api/fixtures/v1/library.json` |
| `GET /v1/home` | `crates/api/fixtures/v1/home.json` |
| `GET /v1/leaderboards?game_id=mahjong&period=today` | The matching item in `crates/api/fixtures/v1/leaderboards.json` |

Game IDs are stable slugs shared by the catalog, launch routes, and leaderboard filters. Periods are `today`, `this-week`, and `all-time`. Missing game and period combinations display an empty state. The featured challenge and guest profile are fixed fixtures. Edits to fixtures take effect on rebuilding.

A future HTTP client can deserialize the same structs from `corner-table-api`. Keep network requests asynchronous in the desktop and let the service calculate official scores. Keep the local fixture source for offline UI development.

## Build the games in worktrees

Pick a single game ID from [the game directory](games/README.md). Put the rules in `games/<game-id>/`, the GPUI board in `apps/desktop/src/games/<game-id>/`, and game-specific artwork in `apps/desktop/assets/games/<game-id>/`. Use underscores for Rust module names. Keep rule crates independent of GPUI and the API. This lets the same seeded rules run locally and later on the server for replay verification.

Use a worktree per game. For example, from a checkout with the initial commit on `main`:

```sh
git worktree add ../corner-table-mahjong -b game/mahjong main
```

Have each worktree own its game's files. The integration pass adds workspace members, desktop dependencies, and dispatch entries. It also regenerates `Cargo.lock` after combining the changes. Keep stable IDs and shared controls together in the integration branch.

The appearance control at the bottom of the sidebar offers System, Light, and Dark. The sidebar Back and Forward controls also respond to mouse navigation buttons and keyboard shortcuts. Last Played, Favorites, appearance, and supported game sessions are saved on this device. Set `CORNER_TABLE_DATA_DIR` when you want to run a separate test profile.
