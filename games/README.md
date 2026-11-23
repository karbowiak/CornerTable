# Game directory

Each game title has a stable catalog ID and its own future Rust crate. Add a crate when implementing rules, with seeded puzzle generation, move validation, and scoring where applicable.

| Catalog ID | Planned rules path | Planned desktop module |
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
