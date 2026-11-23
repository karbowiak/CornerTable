use corner_table_word_games::{
    ACROSS_NUMBERS, Crossword, DOWN_NUMBERS, Mode, Rejection, WordRound,
};
use gpui::{
    ClickEvent, Context, Div, ElementId, FocusHandle, FontWeight, KeyDownEvent, SharedString,
    Window, div, prelude::*, px, rgb,
};

use crate::games::{GameEntry, GameLaunch};
use crate::theme as t;

const FOREST: u32 = 0x214a39;
const MOSS: u32 = 0xcfdfbd;
const TILE: u32 = 0xf8edcc;
const TILE_EDGE: u32 = 0xd6be91;

enum Puzzle {
    Words(WordRound),
    Crossword(Crossword),
}

pub struct GameView {
    mode: Mode,
    seed: u64,
    puzzle: Puzzle,
    status: String,
    focus: FocusHandle,
    square: usize,
    down: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let mode = match &launch.entry {
            GameEntry::Mode(id) if matches!(id.as_str(), "rack" | "letter-rack") => Mode::Rack,
            GameEntry::Mode(id) if matches!(id.as_str(), "crossword" | "mini-crossword") => {
                Mode::Crossword
            }
            _ => Mode::Grove,
        };
        let seed = if let GameEntry::Challenge(id) = &launch.entry {
            id.bytes().fold(0u64, |seed, byte| {
                seed.wrapping_mul(31).wrapping_add(u64::from(byte))
            })
        } else {
            0
        };
        Self {
            mode,
            seed,
            puzzle: Self::make_puzzle(mode, seed),
            status: String::new(),
            focus: cx.focus_handle().tab_stop(true),
            square: 0,
            down: false,
        }
    }

    fn make_puzzle(mode: Mode, seed: u64) -> Puzzle {
        match mode {
            Mode::Grove | Mode::Rack => Puzzle::Words(WordRound::new(mode, seed)),
            Mode::Crossword => Puzzle::Crossword(Crossword::new(seed)),
        }
    }

    fn reset(&mut self, mode: Mode) {
        self.mode = mode;
        self.puzzle = Self::make_puzzle(mode, self.seed);
        self.status.clear();
        self.square = 0;
        self.down = false;
    }

    fn pick_letter(&mut self, index: usize) {
        if let Puzzle::Words(round) = &mut self.puzzle {
            if round.revealed() || round.found().len() == round.answer_count() {
                return;
            }
            if round.choose(index) {
                self.status.clear();
            } else {
                self.status = match self.mode {
                    Mode::Grove => "Choose a neighboring tile you haven’t used.",
                    Mode::Rack => "That tile is already in this word.",
                    Mode::Crossword => unreachable!(),
                }
                .into();
            }
        }
    }

    fn submit(&mut self) {
        if let Puzzle::Words(round) = &mut self.puzzle {
            if round.revealed() || round.found().len() == round.answer_count() {
                return;
            }
            self.status = match round.submit() {
                Ok(_) if round.found().len() == round.answer_count() => {
                    "You found every word!".into()
                }
                Ok(points) => format!(
                    "Word added for {points} {}.",
                    if points == 1 { "point" } else { "points" }
                ),
                Err(Rejection::TooShort) => "Choose at least three letters.".into(),
                Err(Rejection::UnknownWord) => "That word isn’t in the game dictionary.".into(),
                Err(Rejection::AlreadyFound) => "You already found that word.".into(),
            };
        }
    }

    fn check_crossword(&mut self) {
        if let Puzzle::Crossword(crossword) = &self.puzzle {
            let incorrect = crossword.incorrect_count();
            self.status = if crossword.revealed() {
                "Solution shown.".into()
            } else if crossword.complete() {
                "You solved the crossword!".into()
            } else if incorrect != 0 {
                format!(
                    "{incorrect} {} to revisit.",
                    if incorrect == 1 { "letter" } else { "letters" }
                )
            } else {
                "The letters entered so far are correct.".into()
            };
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = &event.keystroke.modifiers;
        if !self.focus.is_focused(window)
            || modifiers.control
            || modifiers.alt
            || modifiers.platform
        {
            return;
        }
        let key = event.keystroke.key.as_str();
        match &mut self.puzzle {
            Puzzle::Words(round) => match key {
                "enter" => self.submit(),
                "backspace" => {
                    round.undo();
                    self.status.clear();
                }
                "escape" => {
                    round.clear();
                    self.status.clear();
                }
                _ => return,
            },
            Puzzle::Crossword(crossword) => {
                let row = self.square / 3;
                let column = self.square % 3;
                match key {
                    "left" => self.square = row * 3 + column.saturating_sub(1),
                    "right" => self.square = row * 3 + (column + 1).min(2),
                    "up" => self.square = row.saturating_sub(1) * 3 + column,
                    "down" => self.square = (row + 1).min(2) * 3 + column,
                    "space" => self.down = !self.down,
                    "enter" => self.check_crossword(),
                    "backspace" | "delete" if !crossword.revealed() => {
                        crossword.put(self.square, None);
                        self.status.clear();
                    }
                    key if key.len() == 1 && !crossword.revealed() => {
                        let letter = char::from(key.as_bytes()[0]);
                        if !crossword.put(self.square, Some(letter)) {
                            return;
                        }
                        self.status.clear();
                        if crossword.complete() {
                            self.check_crossword();
                        } else if self.down {
                            self.square = (row + 1).min(2) * 3 + column;
                        } else {
                            self.square = row * 3 + (column + 1).min(2);
                        }
                    }
                    _ => return,
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn words(&self, round: &WordRound, cx: &Context<Self>) -> Div {
        let grove = self.mode == Mode::Grove;
        let title = if grove { "Letter Grove" } else { "Letter Rack" };
        let instructions = if grove {
            "Join touching tiles, including diagonals. Use each tile once per word."
        } else {
            "Choose tiles in any order. Use each tile once per word."
        };
        let board = div().flex().flex_col().gap_2().children(
            round
                .letters()
                .chunks(if grove { 4 } else { 7 })
                .enumerate()
                .map(|(row, letters)| {
                    div()
                        .flex()
                        .gap_2()
                        .flex_wrap()
                        .children(letters.iter().enumerate().map(|(column, &letter)| {
                            let index = row * 4 + column;
                            let chosen = round.selection().iter().position(|&tile| tile == index);
                            div()
                                .id(("word-tile", index))
                                .relative()
                                .tab_index(0)
                                .cursor_pointer()
                                .w(px(if grove { 61. } else { 43. }))
                                .h(px(57.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(TILE_EDGE))
                                .bg(rgb(if chosen.is_some() { t::GOLD } else { TILE }))
                                .text_color(rgb(t::INK))
                                .font_family(t::DISPLAY)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(25.))
                                .hover(|style| style.bg(rgb(t::GOLD_HOVER)))
                                .focus_visible(|style| style.border_2().border_color(rgb(FOREST)))
                                .child(char::from(letter).to_string())
                                .children(chosen.map(|order| {
                                    div()
                                        .absolute()
                                        .top_0()
                                        .right_1()
                                        .font_family(t::BODY)
                                        .text_size(px(11.))
                                        .child((order + 1).to_string())
                                }))
                                .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                                    view.focus.focus(window);
                                    view.pick_letter(index);
                                    cx.notify();
                                }))
                        }))
                }),
        );
        let candidate = round.candidate();
        let play =
            t::surface()
                .p_5()
                .flex()
                .flex_col()
                .gap_3()
                .flex_grow()
                .child(t::display_type(title, 26.))
                .child(div().text_color(rgb(t::MUTED)).child(instructions))
                .child(div().text_color(rgb(t::MUTED)).child(
                    "Words must have at least three letters. Longer words earn more points.",
                ))
                .child(div().p_3().bg(rgb(FOREST)).rounded_lg().child(board))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .flex_wrap()
                        .child(
                            div()
                                .min_w(px(153.))
                                .text_size(px(20.))
                                .font_weight(FontWeight::BOLD)
                                .child(if candidate.is_empty() {
                                    "Choose letters".into()
                                } else {
                                    candidate
                                }),
                        )
                        .child(button("word-add", "Add word", true, cx, |view| {
                            view.submit()
                        }))
                        .child(button("word-undo", "Undo letter", false, cx, |view| {
                            if let Puzzle::Words(round) = &mut view.puzzle {
                                round.undo();
                            }
                            view.status.clear();
                        }))
                        .child(button("word-clear", "Clear", false, cx, |view| {
                            if let Puzzle::Words(round) = &mut view.puzzle {
                                round.clear();
                            }
                            view.status.clear();
                        })),
                )
                .child(feedback(&self.status));
        let mut words = div().flex().flex_col().gap_1();
        for (word, points) in round.found().iter().rev() {
            words = words.child(
                div()
                    .flex()
                    .justify_between()
                    .gap_3()
                    .child(word.clone())
                    .child(format!("+{points}")),
            );
        }
        if round.found().is_empty() {
            words = words.child(
                div()
                    .text_color(rgb(t::MUTED))
                    .child("Words you find will appear here."),
            );
        }
        if round.revealed() {
            let missed = round.remaining_words().collect::<Vec<_>>();
            words = words
                .child(t::display_type("Missed words", 17.))
                .child(div().text_sm().child(if missed.is_empty() {
                    "None".into()
                } else {
                    missed.join(" · ")
                }));
        }
        let score = t::surface()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .w(px(290.))
            .max_w_full()
            .child(t::display_type("Found words", 23.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .font_weight(FontWeight::BOLD)
                    .child(format!(
                        "{} / {} words",
                        round.found().len(),
                        round.answer_count()
                    ))
                    .child(format!("{} points", round.score())),
            )
            .child(
                div()
                    .id("word-list")
                    .max_h(px(290.))
                    .overflow_y_scroll()
                    .child(words),
            )
            .children(
                (!round.revealed() && round.found().len() < round.answer_count()).then(|| {
                    button("word-reveal", "Reveal remaining words", false, cx, |view| {
                        if let Puzzle::Words(round) = &mut view.puzzle {
                            round.reveal();
                        }
                        view.status = "Round ended. The remaining words are shown.".into();
                    })
                }),
            );
        div()
            .flex()
            .items_start()
            .flex_wrap()
            .gap_4()
            .child(play)
            .child(score)
    }

    fn crossword(&self, crossword: &Crossword, cx: &Context<Self>) -> Div {
        let board = div().flex().flex_col().gap_1().children((0..3).map(|row| {
            div().flex().gap_1().children((0..3).map(|column| {
                let index = row * 3 + column;
                let current = index == self.square;
                let in_clue = if self.down {
                    column == self.square % 3
                } else {
                    row == self.square / 3
                };
                let number = if row == 0 {
                    Some(DOWN_NUMBERS[column])
                } else if column == 0 {
                    Some(ACROSS_NUMBERS[row])
                } else {
                    None
                };
                div()
                    .id(("crossword-square", index))
                    .cursor_pointer()
                    .relative()
                    .w(px(65.))
                    .h(px(65.))
                    .flex()
                    .justify_center()
                    .items_center()
                    .rounded_sm()
                    .bg(rgb(if current {
                        t::GOLD
                    } else if in_clue {
                        MOSS
                    } else {
                        t::SURFACE
                    }))
                    .border_1()
                    .border_color(rgb(if current { FOREST } else { TILE_EDGE }))
                    .text_size(px(26.))
                    .font_weight(FontWeight::BOLD)
                    .child(
                        crossword.letters()[index]
                            .map(|letter| char::from(letter).to_string())
                            .unwrap_or_default(),
                    )
                    .children(number.map(|number| {
                        div()
                            .absolute()
                            .top_0()
                            .left_1()
                            .text_size(px(11.))
                            .child(number.to_string())
                    }))
                    .hover(|style| style.bg(rgb(t::GOLD_HOVER)))
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        view.focus.focus(window);
                        view.square = index;
                        cx.notify();
                    }))
            }))
        }));
        let play = t::surface()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .flex_grow()
            .child(t::display_type("Mini Crossword", 26.))
            .child(div().text_color(rgb(t::MUTED)).child("Select a clue or square, then type. Use the arrow keys to move and Space to change direction."))
            .child(div().p_2().rounded_lg().bg(rgb(FOREST)).self_start().child(board))
            .child(div().flex().gap_3().flex_wrap()
                .child(button("crossword-check", "Check letters", true, cx, |view| view.check_crossword()))
                .child(button("crossword-reveal", "Reveal solution", false, cx, |view| {
                    if let Puzzle::Crossword(crossword) = &mut view.puzzle {
                        if !crossword.complete() {
                            crossword.reveal();
                        }
                    }
                    view.check_crossword();
                })))
            .child(feedback(&self.status));
        let clues = t::surface()
            .p_5()
            .w(px(320.))
            .max_w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children([false, true].into_iter().map(|down| {
                let (heading, clues, numbers) = if down {
                    ("Down", crossword.puzzle().down, DOWN_NUMBERS)
                } else {
                    ("Across", crossword.puzzle().across, ACROSS_NUMBERS)
                };
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(t::display_type(heading, 21.))
                    .children(clues.into_iter().enumerate().map(|(index, clue)| {
                        let active = self.down == down
                            && if down {
                                self.square % 3 == index
                            } else {
                                self.square / 3 == index
                            };
                        button(
                            ("crossword-clue", usize::from(down) * 3 + index),
                            format!("{}  {clue}", numbers[index]),
                            active,
                            cx,
                            move |view| {
                                view.down = down;
                                view.square = if down { index } else { index * 3 };
                            },
                        )
                    }))
            }));
        div()
            .flex()
            .items_start()
            .flex_wrap()
            .gap_4()
            .child(play)
            .child(clues)
    }
}

impl Render for GameView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("word-games")
            .track_focus(&self.focus)
            .tab_group()
            .on_key_down(cx.listener(Self::handle_key))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .flex_wrap()
                    .child(t::heading("Word Games"))
                    .child(button("word-new", "New puzzle", false, cx, |view| {
                        view.seed = view.seed.wrapping_add(1);
                        view.reset(view.mode);
                    })),
            )
            .child(
                div().flex().gap_2().flex_wrap().children(
                    [
                        (Mode::Grove, "Letter Grove"),
                        (Mode::Rack, "Letter Rack"),
                        (Mode::Crossword, "Mini Crossword"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (mode, label))| {
                        button(
                            ("word-mode", index),
                            label,
                            self.mode == mode,
                            cx,
                            move |view| {
                                if view.mode != mode {
                                    view.reset(mode);
                                }
                            },
                        )
                    }),
                ),
            )
            .child(match &self.puzzle {
                Puzzle::Words(round) => self.words(round, cx),
                Puzzle::Crossword(crossword) => self.crossword(crossword, cx),
            })
    }
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    cx: &Context<GameView>,
    action: impl Fn(&mut GameView) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .tab_index(0)
        .cursor_pointer()
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(TILE_EDGE))
        .bg(rgb(if active { t::GOLD } else { t::SURFACE }))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(|style| style.bg(rgb(t::GOLD_HOVER)))
        .focus_visible(|style| style.border_2().border_color(rgb(FOREST)))
        .child(label.into())
        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
            view.focus.focus(window);
            action(view);
            cx.notify();
        }))
}

fn feedback(status: &str) -> Div {
    div()
        .min_h(px(23.))
        .text_color(rgb(t::MUTED))
        .child(status.to_owned())
}
