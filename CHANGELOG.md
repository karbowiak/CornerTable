# Changelog

All notable changes to Corner Table are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **After Dark** adds six painted Jigsaws, five Picture Logic prints, and four Word Search
  themes. Find the paintings under **After Dark** in the Jigsaw gallery.
- Word Games adds eight Letter Grove boards, eight Letter Racks, seven crosswords, and
  122 familiar words for hints and round review. Saved puzzles and seeded daily deals keep
  their original boards.
- Release builds include a Linux AppImage, a per-user Windows setup executable, and an
  Apple silicon disk image. Portable archives and SHA-256 checksums are still available.
- Release builds can check for signed updates and offer to install them. Open
  **Help → Check for Updates** to check at any time.

### Changed

#### Look and feel

- Replaced the green-black dark theme with a warm walnut and charcoal palette that keeps the gold
  accent. Each game's own table colour now carries the screen. The light theme's sidebar and
  text are warm too.
- Selected modes and toggles use the same gold fill in every game.
- **How to Play** and **Add favorite** now match in the game header.

#### Window and navigation

- On macOS the window's title bar is part of the app. The traffic lights sit in the walnut
  chrome, above a header holding:
  - a Corner Table button that opens the Library,
  - round Back and Forward buttons,
  - the game sounds toggle,
  - a System, Light and Dark appearance switch.

  Empty header space drags the window, and double-clicking it zooms. Windows and Linux keep
  their native title bars.
- Pages sit in a rounded card inset from the window edge.
- The sidebar is more compact. The active page gets a rounded highlight with a gold outline,
  and **Last played** shows the game's artwork.
- Keyboard focus reaches the sidebar first, then the page, then the header controls.

#### Shared game layouts

Every game uses one of two shared layouts:

- **Board and rail:** the board is centred on a table that fills the window height. A panel
  on the right, about 30% of the width, holds status, statistics, actions and a collapsible
  **Controls** section. Used by Sudoku and Killer Sudoku, Kakuro, Picture Logic, Chess,
  Checkers, Reversi, Peg Solitaire, Match Three, Gem Cascade, Block Puzzle, Bubble Shooter
  and Memory Match.
- **Bar and table:** status and actions sit in one strip above a full-width table. Used by
  Minesweeper and Mahjong Cubes. Picture Logic switches to it for finished prints and for
  wide grids in small windows.

In both layouts:

- Boards scale to the window instead of sitting at a fixed size in the corner.
- Round results appear as a card over the table, so the board never moves when a round ends.
- On short windows the page scrolls rather than shrinking tiles below a comfortable size.

Solitaire, Mahjong, Hearts, Dominoes, Mine Trails, Jigsaw and Word Games keep their own table
layouts. They now match the shared panel widths and spacing and use the extra space.

#### Games

- **Minesweeper:**
  - Larger squares.
  - Covered squares show a full stud instead of a stray "v".
  - Counters sit in the top bar.
- **Sudoku:** a larger board. Both papers are warm, with brass rules, a gold selection and blue
  for your own entries.
- **Kakuro, Peg Solitaire, Chess, Checkers, Reversi, Match Three and Block Puzzle:** larger
  boards, with status and actions grouped in the side panel.
- **Picture Logic:**
  - Larger grids.
  - A printmaking palette: indigo ink on warm stock in light mode, gold on walnut in dark mode.
- **Gem Cascade:** a larger well with warm tints.
- **Bubble Shooter:** a taller table.
- **Memory Match:**
  - Cards sit on the shared table without the old teal slab or the A–D and 1–4 labels.
  - Pairs, attempts and best score appear as statistics.
- **Mahjong Cubes:** the 3D view fills the table, and a finished stack no longer shrinks it.
- **Solitaire:** cards can grow about 20% larger.
- **Hearts:** a larger trick mat and hand in tall windows.
- **Dominoes:** the chain table fills the free height, and your hand tiles are larger.
- **Jigsaw:** a warm mat, gold selected options and a taller puzzle area.
- **Word Search:** larger letters and warm paper.

#### Pages

- **Records:**
  - Rounds, wins, win rate and games played appear as tiles for the chosen game and period.
  - Filter and view buttons use the compact control size.
- **Daily Challenge:**
  - The archive calendar spans the page with larger days.
  - The selected day shows its game's artwork.
- **Solitaire:** each variant card shows a short description of the game.

### Fixed

- Rebuilding the macOS development client with Dioxus works with Cargo 1.100.

## [0.1.0] - 2026-11-23

### Added

- First public preview with 21 offline card, board, and puzzle games, including five Solitaire
  variants, daily challenges, favorites, and personal records.
- Share Solitaire deals, Mahjong layouts, and Minesweeper fields with a code. Import photos
  into Jigsaw and export your game progress as JSON.
- Downloads for Windows x64, Linux x86-64, and Apple silicon macOS.

[Unreleased]: https://github.com/karbowiak/CornerTable/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/karbowiak/CornerTable/releases/tag/v0.1.0
