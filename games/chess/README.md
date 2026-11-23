# Chess

Two players can share a board, or one player can play White against an offline computer. The two modes save separate matches. Click or drag pieces to move. With the board focused, use arrow keys to navigate, Enter or Space to select and move, U to undo, and F to flip the board. When a pawn reaches the far rank, choose Queen, Rook, Bishop, or Knight with the picker or Q, R, B, or N. Escape cancels a selection or prompt.

Checkmate, stalemate, common insufficient-material positions, fivefold repetition, and the 75-move rule end the game automatically. Players can claim a threefold repetition or a 50-move draw in the current position or by announcing a qualifying move. Chess also supports resignation and mutual draw agreements in two-player mode. Rare dead positions that require analyzing future moves, such as permanently blocked pawn formations, are outside the automatic draw detector. Games are untimed.

The MIT-licensed cozy-chess crate supplies standard chess move generation and position equivalence. Corner Table tracks history and draw decisions. It saves a versioned replay using standard UCI coordinates for moves and derives algebraic notation for the move list. The computer uses a cancellable, node-bounded search on a background thread. [cozy-chess source](https://github.com/analog-hors/cozy-chess) · [MIT license](COZY-CHESS-LICENSE.txt)
