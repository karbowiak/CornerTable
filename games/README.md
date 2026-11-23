# Game directory

Each game family has a stable catalog ID and its own Rust rules crate. Keep seeded generation, move validation, and scoring in the rules crate so they can be used independently of the desktop UI.

| Catalog ID | Rules path | Desktop module |
| --- | --- | --- |
| `solitaire` | `games/solitaire/` | `games::solitaire` |
| `mahjong` | `games/mahjong/` | `games::mahjong` |
| `minesweeper` | `games/minesweeper/` | `games::minesweeper` |
| `sudoku` | `games/sudoku/` | `games::sudoku` |
| `jigsaw` | `games/jigsaw/` | `games::jigsaw` |
| `word-games` | `games/word-games/` | `games::word_games` |
| `match-three` | `games/match-three/` | `games::match_three` |
| `bubble-shooter` | `games/bubble-shooter/` | `games::bubble_shooter` |
| `mine-trails` | `games/mine-trails/` | `games::mine_trails` |
| `block-puzzle` | `games/block-puzzle/` | `games::block_puzzle` |
| `gem-cascade` | `games/gem-cascade/` | `games::gem_cascade` |
| `mahjong-cubes` | `games/mahjong-cubes/` | `games::mahjong_cubes` |

Keep Solitaire variants in separate modules within the Solitaire crate so they can share the card model. The current launch values are `klondike`, `spider`, `freecell`, `pyramid`, and `tripeaks`.

Desktop launches distinguish a classic game, a named game mode, and a daily challenge ID. UI code translates clicks and keyboard events into game moves. The rule crate should accept those moves explicitly so a replay can exercise it without a window.
