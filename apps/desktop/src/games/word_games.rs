mod state;

use std::collections::HashMap;

use corner_table_word_games::{WORD_BONUS, WORD_GOAL};
use gpui::{
    Context, Div, ElementId, FocusHandle, FontWeight, KeyDownEvent, Role, SharedString, Stateful,
    Window, div, linear_color_stop, linear_gradient, prelude::*, px, relative, rgb,
};

use self::state::{
    CrosswordPlay, Navigation, Play, PuzzleKey, Shelf, Source, Variant, WordPlay, plural, utc_day,
};
use super::{GameEntry, GameLaunch, board_space};
use crate::{
    audio::{self, Effect},
    storage::{Outcome, Session},
    theme as t,
};

const WOOD: u32 = 0xfff1cc;
const WOOD_SHADE: u32 = 0xe9cfa0;
const WOOD_EDGE: u32 = 0xa58759;
const CERAMIC: u32 = 0xfffbeb;
const CLUE_WASH: u32 = 0xdce7c9;
const HINT_WASH: u32 = 0xcadfe2;
const HINT_EDGE: u32 = 0x376570;
const ERROR_WASH: u32 = 0xf4d3c1;
const ERROR_EDGE: u32 = 0x9b4c3e;
const PAGE_SIZE: usize = 12;

pub struct GameView {
    navigation: Navigation,
    current: PuzzleKey,
    puzzles: HashMap<PuzzleKey, Play>,
    launch: GameLaunch,
    navigation_session: Session,
    challenge: Option<u64>,
    picker: Option<usize>,
    focus: FocusHandle,
    focus_on_open: bool,
}

#[derive(Clone, Copy)]
enum Action {
    Variant(Variant),
    Shelf(Shelf),
    TogglePicker,
    PickerPage(usize),
    Puzzle(usize),
    Tile(usize),
    AddWord,
    UndoTile,
    ClearTiles,
    Shuffle,
    WordHint,
    FinishWords,
    CancelFinish,
    ConfirmFinish,
    CrosswordSquare(usize),
    CrosswordClue { index: usize, down: bool },
    CheckCrossword,
    HintSquare,
    RevealCrossword,
    CancelReveal,
    ConfirmReveal,
    Replay,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let navigation_session = Session::new(launch);
        let mut navigation = navigation_session
            .load::<Navigation>()
            .unwrap_or_else(|| Navigation::initial(&launch.entry));
        let challenge = matches!(launch.entry, GameEntry::Challenge(_)).then(|| launch.seed());
        let current = navigation.resume(utc_day(), challenge);
        if matches!(current.source, Source::Daily(_)) {
            navigation_session.save(&navigation);
        }
        let saved = puzzle_session(launch, current).load();
        Self {
            navigation,
            current,
            puzzles: HashMap::from([(current, Play::load(current, saved))]),
            launch: launch.clone(),
            navigation_session,
            challenge,
            picker: None,
            focus: cx.focus_handle().tab_stop(true),
            focus_on_open: true,
        }
    }

    fn play(&self) -> &Play {
        &self.puzzles[&self.current]
    }

    fn play_mut(&mut self) -> &mut Play {
        self.puzzles
            .get_mut(&self.current)
            .expect("active puzzle is loaded")
    }

    fn switch(&mut self) {
        self.current = self.navigation.resume(utc_day(), self.challenge);
        self.puzzles.entry(self.current).or_insert_with(|| {
            Play::load(
                self.current,
                puzzle_session(&self.launch, self.current).load(),
            )
        });
        self.navigation_session.save(&self.navigation);
        self.picker = None;
    }

    fn save(&self) {
        puzzle_session(&self.launch, self.current).save(&self.play().save());
    }

    fn save_action(&self, was_finished: bool, effect: Option<Effect>, cx: &Context<Self>) {
        self.save();
        let effect = if !was_finished && let Some((attempt, outcome, metric)) = self.play().result()
        {
            puzzle_session(&self.launch, self.current).record_result(
                attempt,
                self.current.result_mode(),
                outcome,
                metric,
            );
            Some(if outcome == Outcome::Won {
                Effect::Complete
            } else {
                Effect::Move
            })
        } else {
            effect
        };
        if let Some(effect) = effect {
            audio::play(cx, effect);
        }
    }

    fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::Variant(variant) => {
                self.navigation.select(variant);
                self.switch();
            }
            Action::Shelf(shelf) => {
                self.navigation.select_shelf(shelf, utc_day());
                self.switch();
            }
            Action::Puzzle(index) => {
                let cursor = self.navigation.edit();
                cursor.index = index;
                cursor.shelf = Shelf::Collection;
                self.switch();
            }
            Action::TogglePicker => {
                self.picker = if self.picker.is_some() {
                    None
                } else {
                    Some(self.navigation.cursor().index / PAGE_SIZE)
                };
            }
            Action::PickerPage(page) => self.picker = Some(page),
            Action::Replay => {
                self.puzzles
                    .insert(self.current, Play::load(self.current, None));
                self.save();
            }
            Action::CrosswordSquare(index) => {
                let focused = self.focus.is_focused(window);
                if let Play::Crossword(play) = self.play_mut() {
                    if focused && play.cursor == index {
                        play.down = !play.down;
                    }
                    play.cursor = index;
                }
                self.save();
            }
            action => {
                let was_finished = self.play().is_finished();
                let effect = match self.play_mut() {
                    Play::Words(play) => act_words(play, action),
                    Play::Crossword(play) => act_crossword(play, action),
                };
                self.save_action(was_finished, effect, cx);
            }
        }
        if !matches!(action, Action::TogglePicker | Action::PickerPage(_)) {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if !self.focus.is_focused(window)
            || key.modifiers.alt
            || key.modifiers.control
            || key.modifiers.platform
        {
            return;
        }
        let key = key.key.as_str();
        let held = event.is_held;
        let was_finished = self.play().is_finished();
        let mut effect = None;
        let handled = match self.play_mut() {
            Play::Words(play) if !play.round.finished() => match key {
                "enter" if !held => {
                    effect = play.submit();
                    true
                }
                "backspace" | "delete" => {
                    play.round.undo();
                    play.notice.clear();
                    true
                }
                "escape" => {
                    play.round.clear();
                    play.notice.clear();
                    play.confirm_finish = false;
                    true
                }
                _ => key_letter(key, held).is_some_and(|letter| {
                    play.notice = if play.round.type_letter(letter) {
                        String::new()
                    } else {
                        format!(
                            "No available {} tile can follow here.",
                            letter.to_ascii_uppercase()
                        )
                    };
                    true
                }),
            },
            Play::Crossword(play) if !play.board.complete() => {
                match key {
                    "left" => play.move_cursor(-1, 0),
                    "right" => play.move_cursor(1, 0),
                    "up" => play.move_cursor(0, -1),
                    "down" => play.move_cursor(0, 1),
                    "space" if !held => play.down = !play.down,
                    "[" if !held => play.step_clue(true, false),
                    "]" if !held => play.step_clue(false, false),
                    "enter" if !held => play.check(),
                    "backspace" => play.erase(true),
                    "delete" => play.erase(false),
                    "escape" => {
                        play.confirm_reveal = false;
                        play.notice.clear();
                    }
                    _ => {
                        let Some(letter) = key_letter(key, held) else {
                            return;
                        };
                        effect = play.type_letter(letter);
                    }
                }
                true
            }
            _ => false,
        };
        if handled {
            self.save_action(was_finished, effect, cx);
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn selector(&self, cx: &Context<Self>) -> Div {
        let variant = self.current.variant;
        let cursor = self.navigation.cursor();
        let count = variant.puzzle_count();
        let collection = matches!(self.current.source, Source::Collection(_));
        let daily = matches!(self.current.source, Source::Daily(_));
        let mut modes = div().flex().flex_wrap().gap_2();
        for (index, option) in Variant::LETTERS_AND_CROSSWORD.into_iter().enumerate() {
            let target = if option == Variant::Mini {
                self.navigation.crossword()
            } else {
                option
            };
            modes = modes.child(button(
                ("word-mode", index),
                option.title(),
                Button::selected(target == variant),
                Action::Variant(target),
                cx,
            ));
        }
        let size = variant.grid().map(|size| {
            div()
                .flex()
                .gap_1()
                .children([Variant::Mini, Variant::Large].into_iter().map(|option| {
                    let side = option.grid().unwrap().side();
                    button(
                        ("word-grid-size", side),
                        format!("{side} × {side}"),
                        Button::selected(size.side() == side),
                        Action::Variant(option),
                        cx,
                    )
                }))
        });
        let archive = div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_1()
            .child(button(
                "word-previous",
                "Previous",
                Button::enabled(cursor.index > 0),
                Action::Puzzle(cursor.index.saturating_sub(1)),
                cx,
            ))
            .child(
                button(
                    "word-collection-picker",
                    format!("Puzzle {} of {count}", cursor.index + 1),
                    Button::selected(self.picker.is_some()),
                    Action::TogglePicker,
                    cx,
                )
                .aria_expanded(self.picker.is_some()),
            )
            .child(button(
                "word-next",
                "Next",
                Button::enabled(cursor.index + 1 < count),
                Action::Puzzle(cursor.index + 1),
                cx,
            ));
        let shelf = div()
            .flex()
            .gap_1()
            .child(button(
                "word-collection",
                "Collection",
                Button::selected(collection),
                Action::Shelf(Shelf::Collection),
                cx,
            ))
            .child(button(
                "word-daily",
                "Daily",
                Button::selected(daily),
                Action::Shelf(Shelf::Daily),
                cx,
            ))
            .when(self.challenge.is_some(), |line| {
                line.child(button(
                    "word-challenge",
                    "Challenge",
                    Button::selected(!collection && !daily),
                    Action::Shelf(Shelf::Challenge),
                    cx,
                ))
            });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .child(modes)
                    .child(shelf),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .children(size)
                    .when(collection, |line| line.child(archive))
                    .when(daily, |line| {
                        let message = match self.current.source {
                            Source::Daily(day) if day != utc_day() => {
                                self.current.source.daily_date().map_or_else(
                                    || "Previous daily".to_owned(),
                                    |date| format!("Daily {date}"),
                                ) + " · Select Daily for today’s puzzle."
                            }
                            _ => "Daily rotation · Midnight UTC".to_owned(),
                        };
                        line.child(caption(message, cx))
                    }),
            )
            .children(
                self.picker
                    .map(|page| self.collection_picker(page, cursor.index, count, cx)),
            )
    }

    fn collection_picker(
        &self,
        page: usize,
        selected: usize,
        count: usize,
        cx: &Context<Self>,
    ) -> Div {
        let start = page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(count);
        t::panel(cx)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(button(
                        "word-picker-prev-page",
                        "Previous page",
                        Button::enabled(page > 0),
                        Action::PickerPage(page.saturating_sub(1)),
                        cx,
                    ))
                    .child(caption(format!("Puzzles {}–{end}", start + 1), cx))
                    .child(button(
                        "word-picker-next-page",
                        "Next page",
                        Button::enabled(end < count),
                        Action::PickerPage(page + 1),
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children((start..end).map(|index| {
                        button(
                            ("word-pick-number", index),
                            (index + 1).to_string(),
                            Button::selected(selected == index),
                            Action::Puzzle(index),
                            cx,
                        )
                        .aria_label(format!("Open puzzle {}", index + 1))
                        .min_w(px(38.))
                    })),
            )
    }

    fn tray(&self, label: &'static str, cx: &Context<Self>) -> Stateful<Div> {
        let palette = t::colors(cx);
        div()
            .id("word-board")
            .role(Role::Group)
            .aria_label(label)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .border_2()
            .border_color(rgb(if palette.paper == t::PAPER {
                0x3c5748
            } else {
                palette.border
            }))
            .focus_visible(|style| style.border_color(rgb(t::GOLD)))
            .rounded_lg()
            .p_2()
            .bg(rgb(if palette.paper == t::PAPER {
                0x345344
            } else {
                0x12251e
            }))
    }

    fn words(&self, play: &WordPlay, wide: bool, cx: &Context<Self>) -> Div {
        let variant = self.current.variant;
        let round = &play.round;
        let grove = variant == Variant::Grove;
        let finished = round.finished();
        let candidate = round.candidate();
        let hint_path = play
            .hint
            .as_ref()
            .filter(|hint| hint.show_path)
            .and_then(|hint| round.path_for(&hint.word))
            .unwrap_or_default();
        let order = play.display_order();
        let size = if grove { 60. } else { 46. };
        let board = self
            .tray(
                if grove {
                    "Letter Grove tiles"
                } else {
                    "Letter Rack tiles"
                },
                cx,
            )
            .flex()
            .flex_col()
            .gap_1p5()
            .children(order.chunks(if grove { 4 } else { 7 }).map(|row| {
                div().flex().gap_1p5().children(row.iter().map(|&index| {
                    let chosen = round.selection().iter().position(|&tile| tile == index);
                    let hint = hint_path.iter().position(|&tile| tile == index);
                    let letter = char::from(round.letters()[index]);
                    let (top, bottom, edge) = if chosen.is_some() {
                        (t::GOLD_HOVER, t::GOLD, 0x976421)
                    } else if hint.is_some() {
                        (HINT_WASH, 0xb5d1d7, HINT_EDGE)
                    } else {
                        (WOOD, WOOD_SHADE, WOOD_EDGE)
                    };
                    material(("letter-tile", index), size, top, bottom, edge)
                        .role(Role::Button)
                        .aria_label(format!(
                            "{letter}, tile {}{}",
                            index + 1,
                            chosen.map_or_else(String::new, |order| format!(
                                ", picked {}",
                                order + 1
                            ))
                        ))
                        .aria_selected(chosen.is_some())
                        .font_family(t::DISPLAY)
                        .text_size(px(if grove { 27. } else { 24. }))
                        .child(letter.to_string())
                        .children(
                            chosen
                                .or(hint)
                                .map(|order| corner_number(order + 1).right_1().top_0()),
                        )
                        .when(!finished, |tile| {
                            tile.cursor_pointer()
                                .when(round.can_choose(index), |tile| {
                                    tile.hover(|style| style.bg(rgb(WOOD)))
                                        .active(|style| style.bg(rgb(t::GOLD_HOVER)))
                                })
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.act(Action::Tile(index), window, cx);
                                }))
                        })
                }))
            }));
        let instruction = if grove {
            "Connect touching tiles in any direction. Use each tile once per word."
        } else {
            "Rearrange the tiles to make words. Use each tile once per word."
        };
        let input = if finished {
            format!(
                "{} {}",
                round.score(),
                plural(round.score(), "point", "points")
            )
        } else if candidate.is_empty() {
            "Type or tap letters".into()
        } else {
            candidate.clone()
        };
        let heading = div()
            .flex()
            .justify_between()
            .items_center()
            .flex_wrap()
            .gap_1()
            .child(t::display_type(variant.title(), 23.))
            .child(caption("3+ letters · Longer words score more", cx));
        let actions = div()
            .flex()
            .flex_wrap()
            .justify_center()
            .gap_1p5()
            .child(button(
                "word-add",
                "Add word",
                if candidate.len() >= 3 {
                    Button::Primary
                } else {
                    Button::Disabled
                },
                Action::AddWord,
                cx,
            ))
            .child(button(
                "word-undo",
                "Undo letter",
                Button::enabled(!candidate.is_empty()),
                Action::UndoTile,
                cx,
            ))
            .child(button(
                "word-clear",
                "Clear",
                Button::enabled(!candidate.is_empty()),
                Action::ClearTiles,
                cx,
            ))
            .when(!grove, |line| {
                line.child(button(
                    "word-shuffle",
                    "Shuffle",
                    Button::Normal,
                    Action::Shuffle,
                    cx,
                ))
            });
        let panel = t::panel(cx)
            .min_w_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(heading)
            .child(caption(instruction, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .py_1()
                    .child(board)
                    .child(
                        div()
                            .id("word-entry")
                            .role(Role::Status)
                            .aria_label(input.clone())
                            .text_color(rgb(if candidate.is_empty() && !finished {
                                t::colors(cx).muted
                            } else {
                                t::colors(cx).ink
                            }))
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(input),
                    )
                    .when(!finished, |board| board.child(actions)),
            )
            .when(!finished, |panel| {
                panel.child(status(
                    &play.notice,
                    "Enter adds a word · Esc clears the tiles",
                    cx,
                ))
            });
        layout(panel, self.score_panel(play, cx), wide)
    }

    fn score_panel(&self, play: &WordPlay, cx: &Context<Self>) -> Div {
        let palette = t::colors(cx);
        let round = &play.round;
        let score = round.score();
        let finished = round.finished();
        let heading = if finished && score >= WORD_GOAL {
            "Round complete"
        } else if score >= WORD_BONUS {
            "Bonus reached"
        } else if score >= WORD_GOAL {
            "Goal reached"
        } else if finished {
            "Round ended"
        } else {
            "Round score"
        };
        let mut panel = t::panel(cx)
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(t::display_type(heading, 20.))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("{score} pts")),
                    ),
            )
            .child(
                div()
                    .id("word-progress")
                    .role(Role::ProgressIndicator)
                    .aria_label(format!(
                        "{score} {}. Goal {WORD_GOAL}. Bonus {WORD_BONUS}.",
                        plural(score, "point", "points")
                    ))
                    .aria_numeric_value(f64::from(score.min(WORD_BONUS)))
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(f64::from(WORD_BONUS))
                    .h(px(7.))
                    .relative()
                    .rounded_full()
                    .overflow_hidden()
                    .bg(rgb(palette.hover))
                    .child(
                        div()
                            .h_full()
                            .w(relative(score.min(WORD_BONUS) as f32 / WORD_BONUS as f32))
                            .bg(rgb(t::GOLD)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(relative(WORD_GOAL as f32 / WORD_BONUS as f32))
                            .h_full()
                            .w(px(2.))
                            .bg(rgb(palette.surface)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_1()
                    .child(caption(format!("Goal {WORD_GOAL}"), cx))
                    .child(caption(format!("Bonus {WORD_BONUS}"), cx)),
            );
        if finished {
            let mut missed: Vec<_> = round.familiar_remaining().collect();
            missed.sort_by_key(|word| (std::cmp::Reverse(word.len()), *word));
            missed.truncate(30);
            panel = panel.child(completion_actions(self.current, cx)).when(
                !missed.is_empty(),
                |panel| {
                    panel
                        .child(
                            div()
                                .mt_1()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("More words you could make"),
                        )
                        .child(caption(missed.join(" · "), cx))
                },
            );
        } else {
            let hint_ack = play.hint.as_ref();
            let hint_step = if hint_ack.is_some_and(|hint| hint.show_path) {
                Button::Disabled
            } else {
                Button::enabled(hint_ack.is_some() || round.hint_word().is_some())
            };
            panel = panel
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .child(button(
                            "word-hint",
                            if hint_ack.is_some() {
                                "Show tiles"
                            } else {
                                "Get a hint"
                            },
                            hint_step,
                            Action::WordHint,
                            cx,
                        ))
                        .child(button(
                            "word-finish",
                            if score >= WORD_GOAL {
                                "Finish round"
                            } else {
                                "End round…"
                            },
                            if score >= WORD_GOAL {
                                Button::Primary
                            } else {
                                Button::Normal
                            },
                            Action::FinishWords,
                            cx,
                        )),
                )
                .children(hint_ack.map(|hint| {
                    let first = hint
                        .word
                        .chars()
                        .next()
                        .unwrap_or_default()
                        .to_ascii_uppercase();
                    let message = if hint.show_path {
                        "Follow the numbered tiles and add the word.".into()
                    } else {
                        format!(
                            "Look for a {}-letter word starting with {first}.",
                            hint.word.len()
                        )
                    };
                    callout("word-hint-message", message, cx)
                }))
                .when(play.confirm_finish, |panel| {
                    panel.child(confirm(
                        "End this round and review possible words?",
                        "End round",
                        Action::ConfirmFinish,
                        Action::CancelFinish,
                        cx,
                    ))
                });
        }
        let words = div()
            .id("word-ledger")
            .max_h(px(170.))
            .overflow_y_scroll()
            .children(round.found().iter().rev().map(|(word, points)| {
                divider(cx)
                    .flex()
                    .justify_between()
                    .gap_1()
                    .py_1p5()
                    .text_size(px(13.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(word.clone()))
                    .child(caption(format!("+{points}"), cx))
            }));
        panel
            .child(
                divider(cx)
                    .mt_1()
                    .pt_2()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap_1()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                        "{} {} found",
                        round.found().len(),
                        if round.found().len() == 1 {
                            "word"
                        } else {
                            "words"
                        }
                    )))
                    .child(caption(
                        format!(
                            "{} {} used",
                            play.hints_used,
                            plural(play.hints_used, "hint", "hints")
                        ),
                        cx,
                    )),
            )
            .when(!round.found().is_empty(), |panel| panel.child(words))
    }

    fn crossword(&self, play: &CrosswordPlay, wide: bool, cx: &Context<Self>) -> Div {
        let board = &play.board;
        let size = board.size();
        let side = size.side();
        let line = size.line_at(play.cursor, play.down);
        let active = board.puzzle().clues(play.down)[line];
        let squares = self
            .tray("Crossword grid", cx)
            .flex()
            .flex_col()
            .gap_1()
            .children((0..side).map(|row| {
                div().flex().gap_1().children((0..side).map(|column| {
                    let index = row * side + column;
                    let in_clue = !board.complete() && size.line_at(index, play.down) == line;
                    let selected = play.cursor == index && !board.complete();
                    let marked = play.checked & (1 << index) != 0;
                    let hinted = board.hinted_at(index);
                    let (face, edge) = if selected {
                        (t::GOLD_HOVER, 0x86551d)
                    } else if marked {
                        (ERROR_WASH, ERROR_EDGE)
                    } else if hinted {
                        (HINT_WASH, HINT_EDGE)
                    } else if in_clue {
                        (CLUE_WASH, 0xa4b393)
                    } else {
                        (CERAMIC, WOOD_EDGE)
                    };
                    let number = size.square_number(index);
                    let letter = board.letters()[index].map(char::from);
                    material(("crossword-square", index), 57., face, face, edge)
                        .role(Role::GridCell)
                        .aria_selected(selected)
                        .aria_row_index(row)
                        .aria_column_index(column)
                        .aria_label(format!(
                            "Row {}, column {}, {}{}{}",
                            row + 1,
                            column + 1,
                            letter.map_or_else(|| "blank".into(), |letter| letter.to_string()),
                            if marked { ", check this letter" } else { "" },
                            if hinted { ", hinted" } else { "" }
                        ))
                        .when(selected, |tile| tile.aria_active_descendant())
                        .text_size(px(25.))
                        .child(letter.map(|letter| letter.to_string()).unwrap_or_default())
                        .children(number.map(|number| corner_number(number).top_0().left_1()))
                        .when(marked, |tile| {
                            tile.child(
                                div()
                                    .absolute()
                                    .right_1()
                                    .bottom_1()
                                    .w(px(9.))
                                    .h(px(2.))
                                    .rounded_full()
                                    .bg(rgb(ERROR_EDGE)),
                            )
                        })
                        .when(hinted, |tile| {
                            tile.child(
                                div()
                                    .absolute()
                                    .right_1()
                                    .bottom_1()
                                    .size(px(5.))
                                    .rounded_full()
                                    .bg(rgb(HINT_EDGE)),
                            )
                        })
                        .when(!board.complete(), |tile| {
                            tile.cursor_pointer()
                                .hover(|style| style.border_color(rgb(0x825309)))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.act(Action::CrosswordSquare(index), window, cx);
                                }))
                        })
                }))
            }));
        let panel = t::panel(cx)
            .p_4()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .gap_1()
                    .child(t::display_type("Mini Crossword", 23.))
                    .child(caption(format!("{side} × {side} grid"), cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .py_1()
                    .child(squares),
            )
            .when(!board.complete(), |panel| {
                panel
                    .child(callout(
                        "crossword-active-clue",
                        format!(
                            "{} {} · {}",
                            size.clue_number(line, play.down),
                            direction(play.down),
                            active.text
                        ),
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_1p5()
                            .child(button(
                                "crossword-check",
                                "Check letters",
                                Button::Normal,
                                Action::CheckCrossword,
                                cx,
                            ))
                            .child(button(
                                "crossword-hint",
                                "Reveal square",
                                Button::enabled(!board.hinted_at(play.cursor)),
                                Action::HintSquare,
                                cx,
                            ))
                            .child(button(
                                "crossword-reveal",
                                "Show solution…",
                                Button::Normal,
                                Action::RevealCrossword,
                                cx,
                            )),
                    )
                    .child(status(
                        &play.notice,
                        "Type letters · Arrows move · Space changes direction · [ / ] switch clues",
                        cx,
                    ))
            })
            .when(play.confirm_reveal && !board.complete(), |panel| {
                panel.child(confirm(
                    "Show the full solution and end this puzzle?",
                    "Show solution",
                    Action::ConfirmReveal,
                    Action::CancelReveal,
                    cx,
                ))
            })
            .when(board.complete(), |panel| {
                let hints = board.hinted_squares().count_ones();
                let message = if board.revealed() {
                    "Solution shown".into()
                } else if hints == 0 {
                    "Crossword solved".into()
                } else {
                    format!("Solved with {hints} {}.", plural(hints, "hint", "hints"))
                };
                panel
                    .child(t::display_type(message, 19.))
                    .child(completion_actions(self.current, cx))
            });
        let clues =
            t::panel(cx)
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .children([false, true].into_iter().map(|down| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(t::display_type(direction(down), 19.))
                        .children(board.puzzle().clues(down).iter().enumerate().map(
                            |(index, clue)| {
                                let number = size.clue_number(index, down);
                                let selected =
                                    !board.complete() && play.down == down && line == index;
                                let palette = t::colors(cx);
                                div()
                                    .id(("crossword-clue", usize::from(down) * side + index))
                                    .role(Role::Button)
                                    .aria_label(format!(
                                        "{number} {}. {}",
                                        direction(down),
                                        clue.text
                                    ))
                                    .aria_selected(selected)
                                    .flex()
                                    .items_start()
                                    .gap_2()
                                    .min_h(px(30.))
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(if selected {
                                        palette.gold_text
                                    } else {
                                        palette.surface
                                    }))
                                    .bg(rgb(if selected {
                                        palette.hover
                                    } else {
                                        palette.surface
                                    }))
                                    .text_size(px(14.))
                                    .when(!board.complete(), |clue| {
                                        clue.tab_index(0)
                                            .cursor_pointer()
                                            .hover(move |style| style.bg(rgb(palette.hover)))
                                            .focus(move |style| {
                                                style.border_color(rgb(palette.gold_text))
                                            })
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.act(
                                                    Action::CrosswordClue { index, down },
                                                    window,
                                                    cx,
                                                );
                                            }))
                                    })
                                    .child(
                                        div()
                                            .w(px(17.))
                                            .flex_shrink_0()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(palette.gold_text))
                                            .child(number.to_string()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .whitespace_normal()
                                            .child(clue.text),
                                    )
                            },
                        ))
                }));
        layout(panel, clues, wide)
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_on_open) {
            let focus = self.focus.clone();
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        let wide = board_space(window) >= 740.;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .max_w(px(1000.))
            .child(self.selector(cx))
            .child(match self.play() {
                Play::Words(play) => self.words(play, wide, cx),
                Play::Crossword(play) => self.crossword(play, wide, cx),
            })
    }
}

fn act_words(play: &mut WordPlay, action: Action) -> Option<Effect> {
    if play.round.finished() {
        return None;
    }
    match action {
        Action::Tile(index) => {
            if !play.round.choose(index) {
                play.notice = "Choose an unused tile that can follow this one.".into();
                return Some(Effect::InvalidMove);
            }
            play.notice.clear();
        }
        Action::AddWord => return play.submit(),
        Action::UndoTile => {
            play.round.undo();
            play.notice.clear();
        }
        Action::ClearTiles => {
            play.round.clear();
            play.notice.clear();
        }
        Action::Shuffle => {
            play.shuffle = play.shuffle.wrapping_add(1);
            play.notice.clear();
            return Some(Effect::Move);
        }
        Action::WordHint => play.hint(),
        Action::FinishWords if play.round.score() >= WORD_GOAL => play.finish(),
        Action::FinishWords => play.confirm_finish = true,
        Action::CancelFinish => play.confirm_finish = false,
        Action::ConfirmFinish => play.finish(),
        _ => {}
    }
    None
}

fn act_crossword(play: &mut CrosswordPlay, action: Action) -> Option<Effect> {
    match action {
        Action::CrosswordClue { index, down } => play.select_clue(index, down),
        Action::CheckCrossword => play.check(),
        Action::HintSquare => return play.hint(),
        Action::RevealCrossword => play.confirm_reveal = true,
        Action::CancelReveal => play.confirm_reveal = false,
        Action::ConfirmReveal => {
            play.board.reveal();
            play.checked = 0;
            play.confirm_reveal = false;
        }
        _ => {}
    }
    None
}

fn puzzle_session(launch: &GameLaunch, key: PuzzleKey) -> Session {
    Session::new(&GameLaunch {
        game: launch.game.clone(),
        entry: GameEntry::Mode(key.storage_id()),
    })
}

fn key_letter(key: &str, held: bool) -> Option<char> {
    let [byte] = key.as_bytes() else { return None };
    (!held && byte.is_ascii_alphabetic()).then(|| char::from(*byte))
}

fn direction(down: bool) -> &'static str {
    if down { "Down" } else { "Across" }
}

fn layout(play: Div, aside: Div, wide: bool) -> Div {
    let layout = div().flex().gap_3().items_start();
    if wide {
        layout
            .child(play.flex_1())
            .child(aside.w(px(320.)).flex_shrink_0())
    } else {
        layout.flex_col().child(play.w_full()).child(aside.w_full())
    }
}

fn material(
    id: (&'static str, usize),
    size: f32,
    top: u32,
    bottom: u32,
    edge: u32,
) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .size(px(size))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_b_2()
        .border_color(rgb(edge))
        .rounded_md()
        .bg(linear_gradient(
            180.,
            linear_color_stop(rgb(top), 0.),
            linear_color_stop(rgb(bottom), 1.),
        ))
        .text_color(rgb(t::INK))
        .font_weight(FontWeight::BOLD)
}

fn corner_number(text: usize) -> Div {
    div()
        .absolute()
        .font_family(t::BODY)
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .child(text.to_string())
}

fn caption(text: impl Into<SharedString>, cx: &Context<GameView>) -> Div {
    div()
        .text_size(px(13.))
        .text_color(rgb(t::colors(cx).muted))
        .child(text.into())
}

fn divider(cx: &Context<GameView>) -> Div {
    div().border_t_1().border_color(rgb(t::colors(cx).border))
}

fn status(notice: &str, fallback: &'static str, cx: &Context<GameView>) -> Stateful<Div> {
    let text: SharedString = if notice.is_empty() {
        fallback.into()
    } else {
        notice.to_owned().into()
    };
    caption(text.clone(), cx)
        .id("word-status")
        .role(Role::Status)
        .aria_label(text)
}

fn callout(id: &'static str, text: String, cx: &Context<GameView>) -> Stateful<Div> {
    let palette = t::colors(cx);
    div()
        .id(id)
        .role(Role::Status)
        .aria_label(text.clone())
        .px_2()
        .py_1p5()
        .rounded_md()
        .bg(rgb(palette.hover))
        .text_color(rgb(palette.ink))
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

fn confirm(
    message: &'static str,
    label: &'static str,
    yes: Action,
    cancel: Action,
    cx: &Context<GameView>,
) -> Div {
    divider(cx)
        .pt_2()
        .flex()
        .flex_col()
        .gap_1()
        .child(caption(message, cx))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(button("word-confirm", label, Button::Primary, yes, cx))
                .child(button(
                    "word-cancel",
                    "Keep playing",
                    Button::Normal,
                    cancel,
                    cx,
                )),
        )
}

fn completion_actions(key: PuzzleKey, cx: &Context<GameView>) -> Div {
    let next = match key.source {
        Source::Collection(index) if index + 1 < key.variant.puzzle_count() => {
            Some(("Next puzzle", Action::Puzzle(index + 1)))
        }
        Source::Daily(_) | Source::Challenge(_) => {
            Some(("Browse collection", Action::Shelf(Shelf::Collection)))
        }
        _ => None,
    };
    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .children(next.map(|(label, action)| {
            button("word-complete-next", label, Button::Primary, action, cx)
        }))
        .child(button(
            "word-replay",
            "Replay",
            Button::Normal,
            Action::Replay,
            cx,
        ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Button {
    Normal,
    Selected,
    Primary,
    Disabled,
}

impl Button {
    fn selected(selected: bool) -> Self {
        if selected {
            Self::Selected
        } else {
            Self::Normal
        }
    }

    fn enabled(enabled: bool) -> Self {
        if enabled {
            Self::Normal
        } else {
            Self::Disabled
        }
    }
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: Button,
    action: Action,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    let label = label.into();
    let palette = t::colors(cx);
    let (background, border, ink) = match variant {
        Button::Primary => (t::GOLD, t::GOLD, t::INK),
        Button::Selected => (palette.hover, palette.gold_text, palette.ink),
        Button::Normal => (palette.surface, palette.border, palette.ink),
        Button::Disabled => (palette.paper, palette.border, palette.muted),
    };
    div()
        .id(id)
        .when(variant == Button::Selected, |button| {
            button.aria_selected(true)
        })
        .flex()
        .items_center()
        .justify_center()
        .min_h(px(33.))
        .px_2p5()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(border))
        .bg(rgb(background))
        .text_color(rgb(ink))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(variant != Button::Disabled, |button| {
            button
                .role(Role::Button)
                .aria_label(label.clone())
                .tab_index(0)
                .cursor_pointer()
                .hover(move |style| {
                    style.bg(rgb(if variant == Button::Primary {
                        t::GOLD_HOVER
                    } else {
                        palette.hover
                    }))
                })
                .focus(move |style| style.border_color(rgb(palette.gold_text)))
                .on_click(cx.listener(move |view, _, window, cx| view.act(action, window, cx)))
        })
        .child(label)
}
