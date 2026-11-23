use corner_table_solitaire::Variant;
use gpui::{
    Context, Div, FocusHandle, FontWeight, MouseButton, Role, ScrollHandle, Stateful, Window, div,
    point, prelude::*, px, rgb,
};

use crate::{
    app::{Action, Desktop, FocusNext, FocusPrevious},
    games::{GameEntry, GameLaunch},
    theme as t, widgets,
};

pub struct GuideOverlay {
    close_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    scroll: ScrollHandle,
}

impl GuideOverlay {
    pub fn new(cx: &Context<Desktop>) -> Self {
        Self {
            close_focus: cx.focus_handle().tab_stop(true),
            return_focus: None,
            scroll: ScrollHandle::new(),
        }
    }

    pub fn open(
        &mut self,
        launch: &GameLaunch,
        fallback: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Desktop>,
    ) {
        if self.return_focus.is_none() && Guide::for_launch(launch).is_some() {
            self.return_focus = Some(window.focused(cx).unwrap_or_else(|| fallback.clone()));
            self.scroll.set_offset(point(px(0.), px(0.)));
            self.close_focus.focus(window, cx);
        }
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Desktop>) {
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
    }

    pub fn dismiss(&mut self) {
        self.return_focus = None;
    }

    pub fn is_open(&self) -> bool {
        self.return_focus.is_some()
    }

    pub fn button(&self, launch: &GameLaunch, cx: &Context<Desktop>) -> Option<Stateful<Div>> {
        Guide::for_launch(launch)?;
        Some(
            widgets::button("game-guide", "How to Play", Action::OpenGuide, false, cx)
                .aria_expanded(self.is_open())
                .aria_keyshortcuts("F1")
                .min_h(px(34.))
                .px_3()
                .text_size(px(13.))
                .whitespace_nowrap()
                .capture_any_mouse_down(|event, window, _| {
                    // Keep the board's focus until opening so it can be restored on close.
                    if event.button == MouseButton::Left {
                        window.prevent_default();
                    }
                }),
        )
    }

    pub fn render(
        &self,
        launch: &GameLaunch,
        window: &Window,
        cx: &Context<Desktop>,
    ) -> Option<Stateful<Div>> {
        self.return_focus.as_ref()?;
        let guide = Guide::for_launch(launch)?;
        let title = format!("How to play {}", guide.title);
        let palette = t::colors(cx);
        let scroll = self.scroll.clone();
        Some(
            div()
                .id("game-guide-backdrop")
                .absolute()
                .inset_0()
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .p_4()
                .bg(rgb(palette.rail).opacity(0.62))
                .child(
                    t::panel(cx)
                        .id("game-guide-dialog")
                        .role(Role::Dialog)
                        .aria_label(title.clone())
                        .w_full()
                        .max_w(px(572.))
                        .max_h(window.viewport_size().height - px(64.))
                        .flex()
                        .flex_col()
                        // Close is the only control in the guide.
                        .on_action(|_: &FocusNext, _, _| {})
                        .on_action(|_: &FocusPrevious, _, _| {})
                        .capture_key_down(move |event, window, cx| {
                            let stroke = &event.keystroke;
                            if !stroke.modifiers.modified() && stroke.key == "escape" {
                                // Dismiss on key-up so Escape repeats stay out of the game.
                                cx.stop_propagation();
                            } else if !stroke.modifiers.modified()
                                && scroll_guide(&scroll, &stroke.key)
                            {
                                window.refresh();
                                cx.stop_propagation();
                            }
                        })
                        .on_key_up(
                            cx.listener(|desktop, event: &gpui::KeyUpEvent, window, cx| {
                                let stroke = &event.keystroke;
                                if stroke.key == "escape" && !stroke.modifiers.modified() {
                                    cx.stop_propagation();
                                    desktop.dispatch(Action::CloseGuide, window, cx);
                                }
                            }),
                        )
                        .child(
                            div()
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .px_5()
                                .py_3()
                                .border_b_1()
                                .border_color(rgb(palette.border))
                                .child(t::display_type(title, 23.))
                                .child(
                                    widgets::button(
                                        "game-guide-close",
                                        "Close",
                                        Action::CloseGuide,
                                        false,
                                        cx,
                                    )
                                    .track_focus(&self.close_focus)
                                    .aria_keyshortcuts("Escape")
                                    .min_h(px(34.))
                                    .px_3()
                                    .text_size(px(13.))
                                    .flex_none(),
                                ),
                        )
                        .child(
                            div()
                                .id("game-guide-copy")
                                .min_h_0()
                                .overflow_y_scroll()
                                .track_scroll(&self.scroll)
                                .child(div().px_5().py_4().flex().flex_col().gap_3().children(
                                    guide.sections.iter().map(|&(heading, text)| {
                                        div()
                                            .id(heading)
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .id("heading")
                                                    .role(Role::Heading)
                                                    .aria_level(2)
                                                    .aria_label(heading)
                                                    .text_size(px(14.))
                                                    .line_height(px(19.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(heading),
                                            )
                                            .child(
                                                div()
                                                    .id("instructions")
                                                    .role(Role::Label)
                                                    .aria_label(text)
                                                    .text_size(px(14.))
                                                    .line_height(px(21.))
                                                    .text_color(rgb(palette.muted))
                                                    .child(text),
                                            )
                                    }),
                                )),
                        ),
                ),
        )
    }
}

fn scroll_guide(scroll: &ScrollHandle, key: &str) -> bool {
    let offset = scroll.offset().y;
    let bottom = -scroll.max_offset().y;
    let page = scroll.bounds().size.height - px(28.);
    let next = match key {
        "up" => offset + px(42.),
        "down" => offset - px(42.),
        "pageup" => offset + page,
        "pagedown" => offset - page,
        "home" => px(0.),
        "end" => bottom,
        _ => return false,
    };
    scroll.set_offset(point(px(0.), next.clamp(bottom, px(0.))));
    true
}

struct Guide {
    title: &'static str,
    sections: &'static [(&'static str, &'static str)],
}

impl Guide {
    fn for_launch(launch: &GameLaunch) -> Option<Self> {
        let guide = match launch.game.0.as_str() {
            "block-puzzle" => Self {
                title: "Block Puzzle",
                sections: &[
                    (
                        "Make room for the next tray",
                        "Fit the three shapes onto the grid. Completed rows and columns disappear, making space for the next pieces. A fresh tray arrives after you place all three.",
                    ),
                    (
                        "Place a shape",
                        "Drag a piece to the grid, or click a piece and then its landing square. On the keyboard, press 1–3 to pick a piece, move it with the arrows, and press Enter to place it. Escape cancels the selection.",
                    ),
                    (
                        "Finish the round",
                        "Classic ends when no piece fits. Ten Trays is complete after you place all 30 pieces. Puzzle Boards give you a fixed set of pieces and a goal. Undo takes back one placement, and Restart replays the same board.",
                    ),
                ],
            },
            "bubble-shooter" => Self {
                title: "Bubble Shooter",
                sections: &[
                    (
                        "Pop the whole field",
                        "Land your bubble beside others of the same color. A connected group of three or more pops, and any bubbles cut off from the ceiling drop away too.",
                    ),
                    (
                        "Aim and shoot",
                        "Move the pointer over the field to see the shot path and click to fire. You can bank a shot off a side wall. Check the Next color to plan a follow-up.",
                    ),
                    (
                        "Watch the danger line",
                        "You win by clearing every bubble. The round ends if a bubble reaches the dashed line or you use all your shots. Tab to Restart and press Enter to retry, or choose New board when available.",
                    ),
                    (
                        "Try a bank-shot puzzle",
                        "Choose Puzzles to clear a small field in a fixed number of shots. A bank match must bounce off a side wall and then pop a group. Check the goal above each field.",
                    ),
                ],
            },
            "checkers" => Self {
                title: "Checkers",
                sections: &[
                    (
                        "Walk or jump",
                        "Select a piece and then a highlighted square. Uncrowned pieces move diagonally forward. Jumping an opposing piece captures it, and a capture is compulsory when available. If that piece can keep jumping, finish the chain.",
                    ),
                    (
                        "Crown a king",
                        "Reach the far side of the board to crown the piece and end its turn. Kings can move and jump in both directions. Win when your opponent has no legal move left.",
                    ),
                    (
                        "Choose an opponent",
                        "Play the computer at one of three difficulties or share the board with a friend. Use the arrows to move the cursor, Enter or Space to select and move, and U to undo.",
                    ),
                ],
            },
            "chess" => Self {
                title: "Chess",
                sections: crate::games::chess::GUIDE_SECTIONS,
            },
            "dominoes" => Self {
                title: "Dominoes",
                sections: corner_table_dominoes::GUIDE_SECTIONS,
            },
            "gem-cascade" => Self {
                title: "Gem Cascade",
                sections: &[
                    (
                        "Build chains",
                        "Drop pairs so four or more gems of one color touch edge to edge. When a group clears, the gems above it fall. A second clear from that fall starts a higher-scoring chain.",
                    ),
                    (
                        "Move, rotate and drop",
                        "Use the on-screen buttons or focus the board. Left and Right shift the pair, Up and Z rotate in opposite directions, Down lowers a timed pair, and Space or Enter drops it. P pauses or resumes.",
                    ),
                    (
                        "Keep the entrance clear",
                        "Timed play speeds up as you place pairs. Practice lets you plan each drop at your own pace. Beat your best score before the entry cells block the next pair. Retry replays the deal.",
                    ),
                ],
            },
            "hearts" => Self {
                title: "Hearts",
                sections: &[
                    (
                        "Pass three cards",
                        "Choose three cards to give away at the start of each passing hand. The direction changes each hand, and every fourth hand keeps its cards. The 2 of clubs opens the first trick.",
                    ),
                    (
                        "Follow suit",
                        "Play a card in the suit led if you have one. The highest card of that suit takes the trick and leads next. On the first trick, avoid point cards if possible. You can lead with a heart once someone has played one, or when your hand only holds hearts.",
                    ),
                    (
                        "Keep your score low",
                        "Each captured heart adds one point, and the queen of spades adds thirteen. If one player takes every penalty card, each other player adds 26. The lowest score wins when someone reaches 100. A tie for lowest goes to another hand.",
                    ),
                    (
                        "Controls and pacing",
                        "Tab to the table and use Left and Right to choose a card. Space selects or plays it. Enter passes the three selected cards, plays the focused card, or continues. Turn on Pause after each trick to advance at your own pace.",
                    ),
                ],
            },
            "jigsaw" => Self {
                title: "Jigsaw",
                sections: &[
                    (
                        "Assemble",
                        "Drag a piece from the tray, or select it and click its spot. Tab to a piece and press Enter to select it, then use the arrows and Enter to place it. Escape returns the piece to the tray.",
                    ),
                    (
                        "Swap",
                        "Drag one square tile onto another, or select the two in turn. Tab and Enter can select each tile. Pieces lock when they reach their correct positions.",
                    ),
                    (
                        "Complete the picture",
                        "Place every piece to finish. Use Photo for a reference. In Assemble, Ghost shows the image beneath the pieces, Hint marks a fitting spot, and Undo removes the last placement. Restart starts the puzzle again.",
                    ),
                    (
                        "Use your own picture",
                        "Open My photos and choose Add Photo to turn a JPEG, PNG, or WebP into a puzzle. The gallery keeps a cropped copy on this device. Choose either puzzle mode and size as you would with a painting.",
                    ),
                ],
            },
            "kakuro" => Self {
                title: "Kakuro",
                sections: &[
                    (
                        "Add up the clues",
                        "Fill each empty square with a digit from 1 to 9. The upper-right number in a clue square is the sum across. The lower-left number is the sum down. A digit can appear only once in any run.",
                    ),
                    (
                        "Make notes as you go",
                        "Click a square or use the arrows to move, then type a digit. Press N to switch Pencil on or off. Backspace or Delete clears an entry. The two runs through the selected square share its digit.",
                    ),
                    (
                        "Check your deductions",
                        "Conflict markings flag a repeated number or a sum that can’t be reached. Press H for a hint and Enter to apply it. Your progress is saved separately for each grid.",
                    ),
                ],
            },
            "mahjong" => Self {
                title: "Mahjong",
                sections: &[
                    (
                        "Clear every tile",
                        "Remove matching faces in pairs. A tile is free when nothing overlaps it from above and at least one of its left or right edges is open.",
                    ),
                    (
                        "Choose a pair",
                        "Click two free tiles with the same face. On the board, use the arrows to move and Enter or Space to select. Escape clears the selection.",
                    ),
                    (
                        "Find another route",
                        "Use Hint (H) to find a pair or Undo (U) to try a different order. For a Clean Clear, remove all tiles without using either. Restart replays the same layout.",
                    ),
                    (
                        "Share a practice deal",
                        "Use Copy deal code to share the exact layout and starting tiles. Copy a code you received, then choose Play from clipboard in Practice to start that deal.",
                    ),
                ],
            },
            "mahjong-cubes" => Self {
                title: "Mahjong Cubes",
                sections: &[
                    (
                        "Clear the stack",
                        "Find pairs with identical marks. A cube is free when nothing sits directly above it and at least one of its four sides has no neighbor at the same level.",
                    ),
                    (
                        "Orbit the stack and choose",
                        "Drag the board to turn or tilt it, then click visible faces on two free cubes to match them. Use Q and E to turn, Shift with Up or Down to tilt, and R to reset the view. Use the arrows to browse exposed free cubes and Enter to select.",
                    ),
                    (
                        "Find the last pairs",
                        "You win when every cube is gone. Press H for a hint and orbit to find a highlighted cube hidden from your current angle. Press U to undo, or use Restart to replay the deal.",
                    ),
                ],
            },
            "match-three" => Self {
                title: "Match Three",
                sections: &[
                    (
                        "Meet the stage goals",
                        "Match rows or columns of three or more identical jewels. Check the stage meters for targets such as points, collected jewels, chains, or marked cells. To clear a marked cell, include its jewel in a match.",
                    ),
                    (
                        "Swap neighbors",
                        "Click two neighboring jewels, or drag one toward the other. On the board, use the arrows to move and Enter or Space to choose each jewel. Only swaps that make a match spend a move.",
                    ),
                    (
                        "Advance or try again",
                        "Meet every goal within the move limit to unlock the next stage. Hint (H) marks a swap. If you run out of moves, Retry replays the layout and New board gives the stage a fresh deal.",
                    ),
                ],
            },
            "memory-match" => Self {
                title: "Memory Match",
                sections: &[
                    (
                        "Turn over two cards",
                        "Click a card to reveal it. To play with the keyboard, Tab into the board, use the arrow keys to move, and press Enter or Space to flip. Each choice of two distinct cards counts as one attempt.",
                    ),
                    (
                        "Find the matching illustrations",
                        "Pairs stay face up. A mismatch shows both cards briefly. Turn on Hold mismatches to keep both visible until you choose Hide cards or press H on the board.",
                    ),
                    (
                        "Complete each board",
                        "Find all the pairs in as few attempts as you can. Each board size keeps its own progress and personal best. Choose New deal to shuffle that board again.",
                    ),
                ],
            },
            "mine-trails" => Self {
                title: "Mine Trails",
                sections: &[
                    (
                        "Survey the tunnels",
                        "Recover both diamonds in each room. Survey the Entrance to open the two drifts, then survey both drifts to unlock the Deep Vault. Recover the vault diamonds to finish the expedition.",
                    ),
                    (
                        "Dig and flag",
                        "Numbers count mines in the eight surrounding squares. Dig by clicking a covered square that shares an edge with an open square. Right-click to flag suspected mines, or choose Flag and then a square. Tab and Enter activate available squares and rooms.",
                    ),
                    (
                        "Recover from a mine",
                        "Use the tunnel map to travel between connected rooms. A mine collapses the current room. Retry room resets that room and keeps the others you’ve surveyed.",
                    ),
                ],
            },
            "minesweeper" => Self {
                title: "Minesweeper",
                sections: &[
                    (
                        "Reveal every safe square",
                        "Your first reveal is safe. Each clue counts mines in its eight neighboring squares. Flag the squares you suspect and use the clues to find safe squares.",
                    ),
                    (
                        "Reveal, flag and chord",
                        "Click to reveal and right-click to flag, or turn on Flag mode to place flags with a click. Click a revealed clue with the indicated number of adjacent flags to open the other neighbors. Use arrows to move, Enter or Space to open, F to flag, and H for a deduction based on the visible clues and flags.",
                    ),
                    (
                        "Finish the field",
                        "Clear every safe square to win. After hitting a mine, choose Replay field to practice the same layout. New field gives you a fresh layout, and Restart begins the daily board again. In Time trial you can still finish the field after the countdown reaches zero.",
                    ),
                    (
                        "Share a field",
                        "After your first reveal in Classic or Logic only, use Copy field code to share the layout and opening square. Copy a code you received and choose Play from clipboard to practice that field. Use Return to my field to resume your own round.",
                    ),
                ],
            },
            "peg-solitaire" => Self {
                title: "Peg Solitaire",
                sections: &[
                    (
                        "Jump and remove a peg",
                        "Choose a peg and jump over an adjacent peg into an empty hole. Jumps go up, down, left, or right. The jumped peg comes off the board.",
                    ),
                    (
                        "Leave a single peg",
                        "Make jumps until one peg remains. For a center finish, leave it in the brass ring. If no jumps remain, undo a move or replay the board.",
                    ),
                    (
                        "Choose a move",
                        "Click a peg and a highlighted landing hole, or move the focus with the arrows and press Enter or Space at each end. Press H for a legal jump or U to undo.",
                    ),
                ],
            },
            "picture-logic" => Self {
                title: "Picture Logic",
                sections: &[
                    (
                        "Read the runs",
                        "Each clue describes an unbroken run of filled squares, in order from left to right or top to bottom. Leave at least one empty square between runs. Use the clues from crossing lines to work out where the runs fit.",
                    ),
                    (
                        "Paint the grid",
                        "Choose Fill, Mark, or Erase and click or drag over the squares. On the keyboard, move with the arrows and press F, X, or E to pick a tool. Space applies the tool. Mark squares you know are empty.",
                    ),
                    (
                        "Finish the picture",
                        "Press Z to undo, C to check, or H for a deduction. Find another print in the gallery with G. The gallery keeps your progress for each picture.",
                    ),
                ],
            },
            "reversi" => Self {
                title: "Reversi",
                sections: &[
                    (
                        "Trap a line of discs",
                        "Place a disc in an empty square so it brackets an unbroken line of your opponent’s discs between two of yours. Any line bracketed horizontally, vertically, or diagonally flips to your color.",
                    ),
                    (
                        "Choose a square",
                        "Click a legal square, or move the board cursor with the arrow keys and press Enter or Space. Play against the computer or share the board with another person.",
                    ),
                    (
                        "Count the discs",
                        "A player without a legal move passes automatically. When neither player can move, whoever has more discs wins. Use Undo to take back the previous turn.",
                    ),
                ],
            },
            "solitaire" => return Some(Self::solitaire(launch)),
            "sudoku" => Self {
                title: "Sudoku",
                sections: &[
                    (
                        "Fill the grid",
                        "Put each digit exactly once in each row, column, and outlined box. Standard and daily grids use 1–9. Mini uses 1–6 with boxes two rows high and three columns wide.",
                    ),
                    (
                        "Enter digits and notes",
                        "Click a square or move with the arrows, then type a digit or use the number pad on screen. Notes (N) toggles pencil marks. Backspace or Delete clears an entry, and Space moves to the next blank.",
                    ),
                    (
                        "Resolve mistakes",
                        "Underlines mark repeated digits. Undo takes back an edit. Hint (H) explains a deduction, and Enter applies it. Solve every square to finish. For the daily goal, finish without hints. Retry puzzle resets the daily grid.",
                    ),
                    (
                        "Solve Killer cages",
                        "In Killer, each dashed cage has a sum. Use a different digit in every square of the cage and make the digits add up to that sum. The regular row, column, and box rules still apply.",
                    ),
                ],
            },
            "word-games" => Self {
                title: "Word Games",
                sections: &[
                    (
                        "Letter Grove",
                        "Build a word by choosing touching tiles, including diagonals. Use each tile once per word.",
                    ),
                    (
                        "Letter Rack",
                        "Choose tiles in any order. Each tile can be used once per word, so repeating a letter takes another copy of that tile.",
                    ),
                    (
                        "Submit and finish a word round",
                        "Click tiles, or select them with Tab and Enter. Use Add word or Enter to submit words of three or more letters. Backspace undoes a letter and Escape clears the word. Find every answer to finish, or choose Reveal remaining words to end the round.",
                    ),
                    (
                        "Mini Crossword",
                        "Click a clue or square and type. Move with the arrows and switch between Across and Down with Space. Enter checks letters and Backspace erases. Fill all nine squares correctly to solve it. Use Reveal solution if stuck. New puzzle starts a fresh round in any mode.",
                    ),
                    (
                        "Word Search",
                        "Find each listed word in a straight line. Drag between its first and last letters in either direction, or click both endpoints. On the keyboard, move with the arrows and press Space or Enter at each end. Press H for a first-letter hint.",
                    ),
                ],
            },
            _ => return None,
        };
        Some(guide)
    }

    fn solitaire(launch: &GameLaunch) -> Self {
        let variant = match &launch.entry {
            GameEntry::Mode(id) => Variant::from_id(id),
            _ => Variant::Klondike,
        };
        const MOVE_CARDS: (&str, &str) = (
            "Move cards",
            "Click a card and then its destination, or drag it there. With the keyboard, Tab to the table, move between cards and piles with the arrows, and press Enter or Space to activate the focused spot.",
        );
        const TRY_AGAIN: (&str, &str) = (
            "Try another move",
            "Press H for a hint, U to undo, or Escape to cancel a selection. D draws where there is a stock. Replay starts this deal again, and New deal shuffles a fresh board.",
        );
        const FINISH: (&str, &str) = (
            "Collect the last cards",
            "When Finish appears, you can send all remaining cards to their foundations in one action. Undo takes back the whole collection.",
        );
        const SHARE: (&str, &str) = (
            "Share a deal",
            "Copy deal code shares the starting cards and rules. Copy a code you received, then choose Play copied deal to open it. Use Return to my deal to resume your own table.",
        );
        let sections: &'static [(&str, &str)] = match variant {
            Variant::Klondike => &[
                (
                    "Build four foundations",
                    "Send every card to its suit’s foundation, from ace through king. Arrange the table columns in descending order with alternating colors. An empty column starts with a king.",
                ),
                (
                    "Choose a deal",
                    "Draw 1 and Draw 3 shuffle the deck. Verified Draw 1 starts from a shuffle with a proven route to a win. Play only the top card of the waste. When the stock runs out, click it again to recycle the waste.",
                ),
                MOVE_CARDS,
                FINISH,
                TRY_AGAIN,
                SHARE,
            ],
            Variant::Spider => &[
                (
                    "Clear eight runs",
                    "Build descending sequences from king to ace. A run clears automatically when all 13 cards share a suit. Different suits can share a column. Only same-suit runs move together. Any movable card or run can start an empty column.",
                ),
                (
                    "Deal another row",
                    "Choose 1, 2, or 4 suits for the deal. Stock adds one card to each column. Fill every empty column before dealing a row.",
                ),
                MOVE_CARDS,
                TRY_AGAIN,
                SHARE,
            ],
            Variant::FreeCell => &[
                (
                    "Build four foundations",
                    "Send all 52 cards to their suit foundations, from ace through king. Build down in alternating colors in the columns. Any card can start an empty column.",
                ),
                (
                    "Make working space",
                    "Each free cell can hold one card. The more free cells and spare columns you leave empty, the longer the runs you can move.",
                ),
                MOVE_CARDS,
                FINISH,
                TRY_AGAIN,
                SHARE,
            ],
            Variant::Pyramid => &[
                (
                    "Clear the pyramid",
                    "Remove uncovered cards in pairs that add up to 13. Ace is 1, jack is 11, and queen is 12. A king clears on its own.",
                ),
                (
                    "Choose a pair or draw",
                    "Select two cards or drag one onto the other. Draw from Stock to use the top waste card as a partner. Double-click an exposed card to find a partner on Easy or Medium.",
                ),
                (
                    "Choose a difficulty",
                    "Easy lets you recycle the waste into Stock as often as you like. Medium allows three recycles. Hard gives you one pass through the stock.",
                ),
                MOVE_CARDS,
                TRY_AGAIN,
                SHARE,
            ],
            Variant::TriPeaks => &[
                (
                    "Clear all three peaks",
                    "Choose an uncovered card of any suit one rank above or below the top waste card. Ace connects to both king and two.",
                ),
                (
                    "Play a card or draw",
                    "Click a card or drag it to the waste. If no exposed card fits, draw from Stock. The Solvable mode gives you a deal with a known solution.",
                ),
                MOVE_CARDS,
                TRY_AGAIN,
                SHARE,
            ],
        };
        Self {
            title: variant.title(),
            sections,
        }
    }
}

#[cfg(test)]
mod tests {
    use corner_table_api::{FixtureApi, GameId};
    use gpui::{KeyUpEvent, Keystroke, PlatformInput, TestAppContext, size};

    use super::*;
    use crate::app::Page;

    #[test]
    fn every_catalog_game_and_solitaire_route_has_a_guide() {
        let library = FixtureApi::load().unwrap().library;
        for game in library.games {
            let launch = GameLaunch {
                game: game.id,
                entry: GameEntry::Classic,
            };
            let guide = Guide::for_launch(&launch).unwrap_or_else(|| panic!("{}", game.title));
            assert!(!guide.sections.is_empty());
        }
        for mode in library.solitaire_modes {
            let launch = GameLaunch {
                game: GameId("solitaire".into()),
                entry: GameEntry::Mode(mode.id),
            };
            assert_eq!(Guide::for_launch(&launch).unwrap().title, mode.title);
        }
    }

    #[gpui::test]
    fn escape_restores_focus_after_tab_navigation_in_the_guide(cx: &mut TestAppContext) {
        cx.update(crate::install_shortcuts);
        for appearance in [t::Appearance::Light, t::Appearance::Dark] {
            let window = cx.open_window(size(px(760.), px(540.)), |window, cx| {
                let mut desktop = Desktop::new(FixtureApi::load().unwrap(), appearance, window, cx);
                desktop.page = Page::Game(GameLaunch {
                    game: GameId("minesweeper".into()),
                    entry: GameEntry::Classic,
                });
                desktop.focus.focus(window, cx);
                desktop
            });
            cx.run_until_parked();
            window
                .update(cx, |desktop, window, cx| {
                    desktop.dispatch(Action::OpenGuide, window, cx);
                    assert!(desktop.guide.close_focus.is_focused(window));
                })
                .unwrap();

            cx.simulate_keystrokes(window.into(), "tab shift-tab escape");
            window
                .update(cx, |desktop, window, _| {
                    assert!(desktop.guide.close_focus.is_focused(window));
                })
                .unwrap();
            cx.update_window(window.into(), |_, window, cx| {
                window.dispatch_event(
                    PlatformInput::KeyUp(KeyUpEvent {
                        keystroke: Keystroke::parse("escape").unwrap(),
                    }),
                    cx,
                );
            })
            .unwrap();
            window
                .update(cx, |desktop, window, _| {
                    assert!(desktop.guide.return_focus.is_none());
                    assert!(desktop.focus.is_focused(window));
                })
                .unwrap();
        }
    }
}
