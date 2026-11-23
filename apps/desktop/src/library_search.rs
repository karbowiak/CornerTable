use std::ops::Range;

use accesskit::{ActionData, Node, TextPosition, TextSelection};
use gpui::{
    AccessibleAction, App, Bounds, ClipboardItem, Context, ElementInputHandler, EntityInputHandler,
    EventEmitter, FocusHandle, KeyBinding, Menu, MenuItem, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, Point, Role, ShapedLine, SharedString, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, canvas, div, fill, point, prelude::*, px, rgb, size,
};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{theme, widgets};

const LABEL: &str = "Search games";
const KEY_CONTEXT: &str = "LibrarySearch";
const MAX_QUERY_CHARS: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    Start,
    End,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(namespace = library_search, no_json)]
enum Edit {
    Move(Motion, bool),
    Delete(Motion),
    SelectAll,
    Copy,
    Cut,
    Paste,
    Dismiss,
}

pub(crate) fn install_shortcuts(cx: &mut App) {
    use Motion::*;

    let movement = [
        ("left", "shift-left", Left),
        ("right", "shift-right", Right),
        ("home", "shift-home", Start),
        ("end", "shift-end", End),
    ];
    #[cfg(target_os = "macos")]
    let platform_movement = [
        ("alt-left", "alt-shift-left", WordLeft),
        ("alt-right", "alt-shift-right", WordRight),
        ("cmd-left", "cmd-shift-left", Start),
        ("cmd-right", "cmd-shift-right", End),
    ];
    #[cfg(not(target_os = "macos"))]
    let platform_movement = [
        ("ctrl-left", "ctrl-shift-left", WordLeft),
        ("ctrl-right", "ctrl-shift-right", WordRight),
    ];
    cx.bind_keys(movement.into_iter().chain(platform_movement).flat_map(
        |(plain, shifted, motion)| {
            [
                KeyBinding::new(plain, Edit::Move(motion, false), Some(KEY_CONTEXT)),
                KeyBinding::new(shifted, Edit::Move(motion, true), Some(KEY_CONTEXT)),
            ]
        },
    ));

    cx.bind_keys(
        [
            ("backspace", "backspace", Edit::Delete(Left)),
            ("delete", "delete", Edit::Delete(Right)),
            ("alt-backspace", "ctrl-backspace", Edit::Delete(WordLeft)),
            ("alt-delete", "ctrl-delete", Edit::Delete(WordRight)),
            ("cmd-a", "ctrl-a", Edit::SelectAll),
            ("cmd-c", "ctrl-c", Edit::Copy),
            ("cmd-x", "ctrl-x", Edit::Cut),
            ("cmd-v", "ctrl-v", Edit::Paste),
            ("escape", "escape", Edit::Dismiss),
        ]
        .into_iter()
        .map(|(mac, other, edit)| {
            let key = if cfg!(target_os = "macos") {
                mac
            } else {
                other
            };
            KeyBinding::new(key, edit, Some(KEY_CONTEXT))
        }),
    );
}

pub(crate) fn edit_menu() -> Menu {
    Menu::new("Edit").items([
        MenuItem::action("Cut", Edit::Cut),
        MenuItem::action("Copy", Edit::Copy),
        MenuItem::action("Paste", Edit::Paste),
        MenuItem::separator(),
        MenuItem::action("Select All", Edit::SelectAll),
    ])
}

pub(crate) struct QueryChanged;

pub(crate) struct SearchInput {
    text: String,
    // Internally these are UTF-8 byte offsets. GPUI's platform input API uses UTF-16.
    anchor: usize,
    caret: usize,
    marked: Option<Range<usize>>,
    dragging: bool,
    layout: Option<SearchLine>,
    focus: FocusHandle,
    return_focus: FocusHandle,
}

impl EventEmitter<QueryChanged> for SearchInput {}

impl SearchInput {
    pub(crate) fn new(
        return_focus: FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle().tab_stop(true);
        cx.on_focus(&focus, window, |_, _, cx| cx.notify()).detach();
        cx.on_blur(&focus, window, |input, window, cx| {
            input.dragging = false;
            input.unmark_text(window, cx);
            cx.notify();
        })
        .detach();
        Self {
            text: String::new(),
            anchor: 0,
            caret: 0,
            marked: None,
            dragging: false,
            layout: None,
            focus,
            return_focus,
        }
    }

    pub(crate) fn query(&self) -> &str {
        &self.text
    }

    pub(crate) fn find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        self.select(0, self.text.len(), window, cx);
    }

    fn selection(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    fn select(&mut self, anchor: usize, caret: usize, window: &Window, cx: &mut Context<Self>) {
        self.anchor = anchor;
        self.caret = caret;
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn destination(&self, motion: Motion) -> usize {
        match motion {
            Motion::Left => self
                .text
                .grapheme_indices(true)
                .rev()
                .find(|(i, _)| *i < self.caret)
                .map_or(0, |(i, _)| i),
            Motion::Right => self
                .text
                .grapheme_indices(true)
                .find(|(i, _)| *i > self.caret)
                .map_or(self.text.len(), |(i, _)| i),
            Motion::WordLeft => self
                .text
                .unicode_word_indices()
                .rev()
                .find(|(i, _)| *i < self.caret)
                .map_or(0, |(i, _)| i),
            Motion::WordRight => self
                .text
                .unicode_word_indices()
                .map(|(i, word)| i + word.len())
                .find(|end| *end > self.caret)
                .unwrap_or(self.text.len()),
            Motion::Start => 0,
            Motion::End => self.text.len(),
        }
    }

    fn edit(&mut self, edit: &Edit, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.selection();
        match *edit {
            Edit::Move(motion, extend) => {
                let caret = match (motion, extend || selected.is_empty()) {
                    (Motion::Left, false) => selected.start,
                    (Motion::Right, false) => selected.end,
                    _ => self.destination(motion),
                };
                self.select(if extend { self.anchor } else { caret }, caret, window, cx);
            }
            Edit::Delete(motion) => {
                let range = if let Some(marked) = &self.marked {
                    marked.clone()
                } else if selected.is_empty() {
                    let end = self.destination(motion);
                    self.caret.min(end)..self.caret.max(end)
                } else {
                    selected
                };
                self.replace(range, "", None, window, cx);
            }
            Edit::SelectAll => self.select(0, self.text.len(), window, cx),
            Edit::Copy | Edit::Cut if !selected.is_empty() => {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    self.text[selected.clone()].to_owned(),
                ));
                if matches!(edit, Edit::Cut) {
                    self.replace(selected, "", None, window, cx);
                }
            }
            Edit::Paste => {
                if let Some(item) = cx.read_from_clipboard() {
                    self.paste(item, window, cx);
                }
            }
            Edit::Dismiss => {
                self.replace(0..self.text.len(), "", None, window, cx);
                self.return_focus.focus(window, cx);
            }
            _ => {}
        }
    }

    fn replace(
        &mut self,
        range: Range<usize>,
        text: &str,
        composition_selection: Option<Range<usize>>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let mut available = MAX_QUERY_CHARS
            - (self.text.chars().count() - self.text[range.clone()].chars().count());
        let clean: String = text
            .chars()
            .filter(|ch| !ch.is_control())
            .take(available + 1)
            .collect();
        let mut end = 0;
        for grapheme in clean.graphemes(true) {
            let chars = grapheme.chars().count();
            if chars > available {
                break;
            }
            available -= chars;
            end += grapheme.len();
        }
        let inserted = &clean[..end];
        let changed = self.text[range.clone()] != *inserted;
        self.text.replace_range(range.clone(), inserted);
        self.marked = composition_selection
            .as_ref()
            .filter(|_| !inserted.is_empty())
            .map(|_| range.start..range.start + inserted.len());
        let selected = composition_selection
            .map(|selected| utf8_range(inserted, selected))
            .unwrap_or(inserted.len()..inserted.len());
        self.select(
            range.start + selected.start,
            range.start + selected.end,
            window,
            cx,
        );
        window.notify_text_input_changed(&self.focus);
        if changed {
            cx.emit(QueryChanged);
        }
    }

    fn replacement_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        range_utf16
            .map(|range| utf8_range(&self.text, range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection())
    }

    fn index_at(&self, x: Pixels) -> usize {
        self.layout
            .as_ref()
            .filter(|layout| layout.line.text.as_ref() == self.text)
            .map_or(self.caret, |layout| {
                layout.line.closest_index_for_x(x - layout.origin().x)
            })
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        window.request_virtual_keyboard();
        self.dragging = true;
        let index = self.index_at(event.position.x);
        let (anchor, caret) = if event.modifiers.shift {
            (self.anchor, index)
        } else if event.click_count >= 3 {
            (0, self.text.len())
        } else if event.click_count == 2 {
            let inside = index.min(self.text.len().saturating_sub(1));
            self.text
                .split_word_bound_indices()
                .find(|(start, word)| *start <= inside && inside < start + word.len())
                .map_or((index, index), |(start, word)| (start, start + word.len()))
        } else {
            (index, index)
        };
        self.select(anchor, caret, window, cx);
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.dragging && event.pressed_button == Some(MouseButton::Left) {
            let caret = self.index_at(event.position.x);
            self.select(self.anchor, caret, window, cx);
        }
    }
}

fn utf16_offset(text: &str, byte: usize) -> usize {
    text[..byte].encode_utf16().count()
}

fn utf8_range(text: &str, range: Range<usize>) -> Range<usize> {
    let offset = |units| {
        let mut index_utf16 = 0;
        for (index, ch) in text.char_indices() {
            if index_utf16 >= units {
                return index;
            }
            index_utf16 += ch.len_utf16();
        }
        text.len()
    };
    offset(range.start)..offset(range.end.max(range.start))
}

impl EntityInputHandler for SearchInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = utf8_range(&self.text, range);
        *actual = Some(utf16_offset(&self.text, range.start)..utf16_offset(&self.text, range.end));
        Some(self.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let range = self.selection();
        Some(UTF16Selection {
            range: utf16_offset(&self.text, range.start)..utf16_offset(&self.text, range.end),
            reversed: self.caret < self.anchor,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = utf8_range(&self.text, range);
        self.select(range.start, range.end, window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.text.encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let range = self.marked.as_ref()?;
        Some(utf16_offset(&self.text, range.start)..utf16_offset(&self.text, range.end))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.marked.take().is_some() {
            window.notify_text_input_changed(&self.focus);
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(self.replacement_range(range), text, None, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = selected.unwrap_or_else(|| {
            let end = text.encode_utf16().count();
            end..end
        });
        self.replace(
            self.replacement_range(range),
            text,
            Some(selected),
            window,
            cx,
        );
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        if self.text.is_empty() {
            return Some(layout.range_bounds(0..0));
        }
        (layout.line.text.as_ref() == self.text)
            .then(|| layout.range_bounds(utf8_range(&self.text, range)))
    }

    fn selection_bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.bounds_for_range(range, bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.layout.as_ref()?.bounds.localize(&position)?;
        Some(utf16_offset(&self.text, self.index_at(position.x)))
    }
}

struct SearchLine {
    line: ShapedLine,
    bounds: Bounds<Pixels>,
    scroll: Pixels,
}

impl SearchLine {
    fn origin(&self) -> Point<Pixels> {
        point(self.bounds.left() - self.scroll, self.bounds.top())
    }

    fn range_bounds(&self, range: Range<usize>) -> Bounds<Pixels> {
        let left = self.origin().x + self.line.x_for_index(range.start);
        let right = self.origin().x + self.line.x_for_index(range.end);
        Bounds::new(
            point(left.min(right), self.bounds.top() + px(2.)),
            size(
                (right - left).abs().max(px(1.)),
                self.bounds.size.height - px(4.),
            ),
        )
    }
}

impl Render for SearchInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::colors(cx);
        let focused = self.focus.is_focused(window);
        let text = SharedString::from(self.text.clone());
        let empty = text.is_empty();
        let anchor = self.anchor;
        let caret = self.caret;
        let set_value = cx.weak_entity();
        let set_selection = set_value.clone();
        let entity = cx.entity();

        let text_line = canvas(
            move |bounds, window, cx| {
                let input = entity.read(cx);
                let style = window.text_style();
                let text: SharedString = if empty {
                    LABEL.into()
                } else {
                    input.text.clone().into()
                };
                let color = rgb(if empty { palette.muted } else { palette.ink }).into();
                let run = TextRun {
                    len: text.len(),
                    font: style.font(),
                    color,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let runs = if let Some(mark) = &input.marked {
                    vec![
                        TextRun {
                            len: mark.start,
                            ..run.clone()
                        },
                        TextRun {
                            len: mark.len(),
                            underline: Some(UnderlineStyle {
                                color: Some(color),
                                thickness: px(1.),
                                wavy: false,
                            }),
                            ..run.clone()
                        },
                        TextRun {
                            len: text.len() - mark.end,
                            ..run
                        },
                    ]
                    .into_iter()
                    .filter(|run| run.len > 0)
                    .collect()
                } else {
                    vec![run]
                };
                let line = window.text_system().shape_line(
                    text,
                    style.font_size.to_pixels(window.rem_size()),
                    &runs,
                    None,
                );
                let width = (bounds.size.width - px(1.)).max(px(0.));
                let max_scroll = if empty {
                    px(0.)
                } else {
                    (line.width - width).max(px(0.))
                };
                let caret_x = line.x_for_index(caret);
                let previous = input.layout.as_ref().map_or(px(0.), |layout| layout.scroll);
                let scroll = previous
                    .clamp((caret_x - width).max(px(0.)), caret_x)
                    .min(max_scroll);
                SearchLine {
                    line,
                    bounds,
                    scroll,
                }
            },
            {
                let entity = cx.entity();
                move |bounds, layout: SearchLine, window, cx| {
                    let input = entity.read(cx);
                    let selection = input.selection();
                    window.handle_input(
                        &input.focus,
                        ElementInputHandler::new(bounds, entity.clone()),
                        cx,
                    );
                    if focused && !selection.is_empty() {
                        window.paint_quad(fill(
                            layout.range_bounds(selection.clone()),
                            rgb(palette.success).opacity(0.28),
                        ));
                    }
                    layout
                        .line
                        .paint(
                            layout.origin(),
                            window.line_height(),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        )
                        .expect("search line can be painted");
                    if focused && selection.is_empty() {
                        window
                            .paint_quad(fill(layout.range_bounds(caret..caret), rgb(palette.ink)));
                    }
                    entity.update(cx, |input, _| input.layout = Some(layout));
                }
            },
        )
        .w_full()
        .h(px(23.));

        theme::panel(cx)
            .h(px(42.))
            .rounded_md()
            .pl_3()
            .pr_2()
            .flex()
            .items_center()
            .gap_1()
            .when(focused, |field| field.border_color(rgb(palette.success)))
            .child(
                div()
                    .id("game-search")
                    .role(Role::SearchInput)
                    .accessibility_id("game-search")
                    .aria_label(LABEL)
                    .aria_placeholder(LABEL)
                    .aria_value(text.clone())
                    .a11y_synthetic_children(move |builder| {
                        let id = builder.synthetic_node_id("query");
                        let mut run = Node::new(Role::TextRun);
                        run.set_value(text.to_string());
                        run.set_character_lengths(
                            text.chars()
                                .map(|ch| ch.len_utf8() as u8)
                                .collect::<Vec<_>>(),
                        );
                        builder.push_child(id, run);
                        let position = |byte| TextPosition {
                            node: id,
                            character_index: text[..byte].chars().count(),
                        };
                        builder.parent_node().set_text_selection(TextSelection {
                            anchor: position(anchor),
                            focus: position(caret),
                        });
                    })
                    .on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                        if let Some(ActionData::Value(value)) = data {
                            let _ = set_value.update(cx, |input, cx| {
                                input.replace(0..input.text.len(), value, None, window, cx)
                            });
                        }
                    })
                    .on_a11y_action(
                        AccessibleAction::SetTextSelection,
                        move |data, window, cx| {
                            if let Some(ActionData::SetTextSelection(selection)) = data {
                                let _ = set_selection.update(cx, |input, cx| {
                                    let byte = |character| {
                                        input
                                            .text
                                            .char_indices()
                                            .nth(character)
                                            .map_or(input.text.len(), |(i, _)| i)
                                    };
                                    input.select(
                                        byte(selection.anchor.character_index),
                                        byte(selection.focus.character_index),
                                        window,
                                        cx,
                                    );
                                });
                            }
                        },
                    )
                    .track_focus(&self.focus)
                    .key_context(KEY_CONTEXT)
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_text()
                    .on_action(cx.listener(Self::edit))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
                    .on_mouse_move(cx.listener(Self::mouse_move))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|input, _, _, _| input.dragging = false),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|input, _, _, _| input.dragging = false),
                    )
                    .child(div().flex_shrink_0().child(widgets::symbol(
                        "search",
                        palette.muted,
                        17.,
                    )))
                    .child(div().flex_1().min_w_0().overflow_hidden().child(text_line)),
            )
            .when(!empty, |field| {
                field.child(
                    div()
                        .id("clear-search")
                        .role(Role::Button)
                        .aria_label("Clear search")
                        .tab_index(0)
                        .size(px(28.))
                        .flex_shrink_0()
                        .rounded_md()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_size(px(19.))
                        .hover(|style| style.bg(rgb(palette.hover)))
                        .focus(|style| style.bg(rgb(palette.hover)))
                        .on_click(cx.listener(|input, _, window, cx| {
                            input.replace(0..input.text.len(), "", None, window, cx);
                            input.focus.focus(window, cx);
                        }))
                        .child("×"),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use corner_table_api::FixtureApi;
    use gpui::TestAppContext;

    use super::*;
    use crate::app::{Action, Desktop, Page};

    #[gpui::test]
    fn word_navigation_edits_search_without_leaving_the_library(cx: &mut TestAppContext) {
        cx.update(crate::install_shortcuts);
        let window = cx.add_window(|window, cx| {
            Desktop::new(
                FixtureApi::load().unwrap(),
                theme::Appearance::Light,
                window,
                cx,
            )
        });
        window
            .update(cx, |desktop, window, cx| {
                desktop.dispatch(Action::Navigate(Page::DailyChallenge), window, cx);
                desktop.dispatch(Action::Navigate(Page::Library), window, cx);
                desktop.search.update(cx, |search, cx| {
                    search.find(window, cx);
                    search.replace_text_in_range(None, "Mine Tails", window, cx);
                });
            })
            .unwrap();
        cx.run_until_parked();

        let word_left = if cfg!(target_os = "macos") {
            "alt-left"
        } else {
            "ctrl-left"
        };
        cx.simulate_keystrokes(window.into(), word_left);
        cx.simulate_keystrokes(window.into(), "right r");
        window
            .read_with(cx, |desktop, cx| {
                assert_eq!(desktop.page, Page::Library);
                assert_eq!(desktop.search.read(cx).query(), "Mine Trails");
            })
            .unwrap();
    }

    #[gpui::test]
    fn composition_replaces_preedit_at_utf16_offsets(cx: &mut TestAppContext) {
        let window = cx.add_window(|window, cx| SearchInput::new(cx.focus_handle(), window, cx));
        window
            .update(cx, |search, window, cx| {
                search.replace_text_in_range(None, "A🌊C", window, cx);
                search.set_selected_text_range(3..3, window, cx);
                search.replace_and_mark_text_in_range(None, "é😀", Some(1..3), window, cx);
                assert_eq!(search.query(), "A🌊é😀C");
                assert_eq!(search.marked_text_range(window, cx), Some(3..6));
                assert_eq!(
                    search.selected_text_range(false, window, cx).unwrap().range,
                    4..6
                );

                search.replace_and_mark_text_in_range(None, "漢", None, window, cx);
                assert_eq!(search.query(), "A🌊漢C");
                assert_eq!(search.marked_text_range(window, cx), Some(3..4));

                search.replace_text_in_range(None, "漢字", window, cx);
                assert_eq!(search.query(), "A🌊漢字C");
                assert_eq!(search.marked_text_range(window, cx), None);
                assert_eq!(
                    search.selected_text_range(false, window, cx).unwrap().range,
                    5..5
                );
            })
            .unwrap();
    }
}
