mod board;
mod paper;
mod state;

use std::{
    cell::Cell as LayoutCell,
    rc::Rc,
    time::{Duration, Instant},
};

use corner_table_picture_logic::{Cell, Deduction, Series};
use gpui::{
    Bounds, Context, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, ScrollHandle, Window, point, prelude::*,
};

use crate::audio::{self, Effect};

use self::state::Notebook;
use super::GameLaunch;

const REVEAL_DURATION: Duration = Duration::from_millis(480);

pub struct GameView {
    book: Notebook,
    gallery: Option<Series>,
    guide: Option<bool>,
    tool: Tool,
    notice: Option<Notice>,
    hover: Option<usize>,
    stroke: Option<Stroke>,
    plot: Rc<LayoutCell<Option<Bounds<Pixels>>>>,
    grid_scroll: ScrollHandle,
    board_focus: FocusHandle,
    gallery_focus: FocusHandle,
    needs_focus: bool,
    reveal_at: Option<Instant>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Fill,
    Mark,
    Erase,
}

impl Tool {
    const ALL: [Self; 3] = [Self::Fill, Self::Mark, Self::Erase];

    fn label(self) -> &'static str {
        match self {
            Self::Fill => "Fill",
            Self::Mark => "Mark",
            Self::Erase => "Erase",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Fill => "F",
            Self::Mark => "X",
            Self::Erase => "E",
        }
    }

    fn paint(self, current: Cell) -> Cell {
        match self {
            Self::Fill if current != Cell::Filled => Cell::Filled,
            Self::Mark if current != Cell::Marked => Cell::Marked,
            _ => Cell::Blank,
        }
    }
}

#[derive(Clone, Copy)]
enum Notice {
    Checked(usize),
    Hint(Deduction),
    NoDeduction,
}

struct Stroke {
    before: String,
    brush: Cell,
    button: MouseButton,
    last: Option<usize>,
}

#[derive(Clone, Copy)]
enum Action {
    Gallery,
    BackToPlate,
    Series(Series),
    Open(usize),
    Next,
    Guide(bool),
    Tool(Tool),
    Undo,
    Check,
    Hint,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let book = Notebook::load(launch);
        Self {
            book,
            gallery: None,
            guide: None,
            tool: Tool::Fill,
            notice: None,
            hover: None,
            stroke: None,
            plot: Rc::default(),
            grid_scroll: ScrollHandle::new(),
            board_focus: cx.focus_handle().tab_stop(true),
            gallery_focus: cx.focus_handle(),
            needs_focus: true,
            reveal_at: None,
        }
    }

    fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_stroke(cx);
        match action {
            Action::Gallery => {
                self.gallery = Some(self.book.play().board.puzzle().series);
                self.needs_focus = true;
            }
            Action::BackToPlate => {
                self.gallery = None;
                self.needs_focus = true;
            }
            Action::Series(series) => self.gallery = Some(series),
            Action::Open(index) => self.open(index),
            Action::Next => {
                if let Some(index) = self.book.next_unfinished() {
                    self.open(index);
                } else {
                    self.gallery = Some(self.book.play().board.puzzle().series);
                    self.needs_focus = true;
                }
            }
            Action::Guide(show) => self.guide = Some(show),
            Action::Tool(tool) if !self.book.play().solved => {
                self.tool = tool;
                self.board_focus.focus(window, cx);
            }
            Action::Undo if !self.book.play().solved => {
                if self.book.edit().undo() {
                    self.notice = None;
                    self.book.save();
                }
                self.board_focus.focus(window, cx);
            }
            Action::Check if !self.book.play().solved => {
                self.notice = Some(Notice::Checked(self.conflicts()));
                self.board_focus.focus(window, cx);
            }
            Action::Hint if !self.book.play().solved => {
                self.hint(cx);
                self.board_focus.focus(window, cx);
            }
            _ => {}
        }
        cx.notify();
    }

    fn open(&mut self, index: usize) {
        self.book.active = index;
        self.gallery = None;
        self.notice = None;
        self.hover = None;
        self.plot.set(None);
        self.grid_scroll.set_offset(Point::default());
        self.tool = Tool::Fill;
        self.reveal_at = None;
        self.needs_focus = true;
        self.book.save();
    }

    fn conflicts(&self) -> usize {
        let board = &self.book.play().board;
        (0..board.cells().len())
            .filter(|&index| board.has_conflict(index))
            .count()
    }

    fn hint(&mut self, cx: &mut Context<Self>) {
        let conflicts = self.conflicts();
        if conflicts > 0 {
            self.notice = Some(Notice::Checked(conflicts));
            return;
        }
        let play = self.book.edit();
        let Some(hint) = play.board.deduction(play.cursor) else {
            self.notice = Some(Notice::NoDeduction);
            return;
        };
        let before = play.board.snapshot();
        play.cursor = hint.index;
        play.board.paint(hint.index, hint.cell);
        play.mark_hint();
        self.hover = None;
        self.notice = Some(Notice::Hint(hint));
        self.commit(before, cx);
        self.show_cursor();
    }

    fn commit(&mut self, before: String, cx: &mut Context<Self>) {
        if self.book.edit().commit_turn(before) {
            if self.book.play().solved {
                self.book.record_completion();
                self.hover = None;
                self.notice = None;
                self.reveal_at = Some(Instant::now());
                audio::play(cx, Effect::Complete);
            } else {
                audio::play(cx, Effect::Move);
            }
            self.book.save();
        }
    }

    fn index_at(&self, position: Point<Pixels>, step: f32) -> Option<usize> {
        let plot = self.plot.get()?;
        let x = f32::from(position.x - plot.origin.x);
        let y = f32::from(position.y - plot.origin.y);
        let puzzle = self.book.play().board.puzzle();
        let (width, height) = (puzzle.width(), puzzle.height());
        (x >= 0. && y >= 0. && x < step * width as f32 && y < step * height as f32).then(|| {
            let row = ((y / step) as usize).min(height - 1);
            let col = ((x / step) as usize).min(width - 1);
            row * width + col
        })
    }

    fn show_cursor(&self) -> bool {
        let Some(plot) = self.plot.get() else {
            return false;
        };
        let play = self.book.play();
        let puzzle = play.board.puzzle();
        let step = plot.size.height / puzzle.height() as f32;
        let row_top = step * (play.cursor / puzzle.width()) as f32;
        let row_bottom = row_top + step;
        let scroll_top = -self.grid_scroll.offset().y;
        let viewport = self.grid_scroll.bounds().size.height;
        if row_top < scroll_top {
            self.grid_scroll
                .set_offset(point(Default::default(), -row_top));
        } else if row_bottom > scroll_top + viewport {
            self.grid_scroll
                .set_offset(point(Default::default(), viewport - row_bottom));
        } else {
            return false;
        }
        true
    }

    fn pointer_down(
        &mut self,
        event: &MouseDownEvent,
        step: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tool = match event.button {
            MouseButton::Left if event.modifiers.control && cfg!(target_os = "macos") => Tool::Mark,
            MouseButton::Left => self.tool,
            MouseButton::Right => Tool::Mark,
            _ => return,
        };
        self.finish_stroke(cx);
        if self.book.play().solved {
            return;
        }
        let Some(index) = self.index_at(event.position, step) else {
            return;
        };
        self.board_focus.focus(window, cx);
        let play = self.book.edit();
        let brush = tool.paint(play.board.cells()[index]);
        self.stroke = Some(Stroke {
            before: play.board.snapshot(),
            brush,
            button: event.button,
            last: None,
        });
        self.drag_to(index);
        self.hover = Some(index);
        cx.stop_propagation();
        cx.notify();
    }

    fn pointer_move(&mut self, event: &MouseMoveEvent, step: f32, cx: &mut Context<Self>) {
        if self.book.play().solved {
            return;
        }
        let index = self.index_at(event.position, step);
        let mut changed = self.hover != index;
        self.hover = index;
        if let Some(stroke) = &mut self.stroke {
            if event.pressed_button != Some(stroke.button) {
                self.finish_stroke(cx);
                changed = true;
            } else if let Some(index) = index {
                changed |= self.drag_to(index);
            } else {
                stroke.last = None;
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn pointer_up(&mut self, event: &MouseUpEvent, step: Option<f32>, cx: &mut Context<Self>) {
        if self
            .stroke
            .as_ref()
            .is_some_and(|s| s.button == event.button)
        {
            if let Some(index) = step.and_then(|step| self.index_at(event.position, step)) {
                self.drag_to(index);
            }
            self.finish_stroke(cx);
            cx.notify();
        }
    }

    fn pointer_exit(&mut self, cx: &mut Context<Self>) {
        if let Some(stroke) = &mut self.stroke {
            stroke.last = None;
        }
        if self.hover.take().is_some() {
            cx.notify();
        }
    }

    fn drag_to(&mut self, index: usize) -> bool {
        let stroke = self.stroke.as_mut().expect("an open stroke owns the brush");
        if stroke.last == Some(index) {
            return false;
        }
        let start = stroke.last.replace(index).unwrap_or(index);
        let play = self.book.edit();
        let width = play.board.puzzle().width();
        let mut changed = false;
        trace_cells(start, index, width, |index| {
            changed |= play.board.paint(index, stroke.brush);
        });
        play.cursor = index;
        if changed {
            self.notice = None;
        }
        true
    }

    fn finish_stroke(&mut self, cx: &mut Context<Self>) {
        if let Some(stroke) = self.stroke.take() {
            self.commit(stroke.before, cx);
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.board_focus.is_focused(window) {
            return;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if modifiers.alt || modifiers.shift {
            return;
        }
        let undo = key == "z" && !self.book.play().solved;
        if (modifiers.platform || modifiers.control) && !undo {
            return;
        }
        if key == "g" && !event.is_held {
            self.act(Action::Gallery, window, cx);
        } else if self.book.play().solved {
            if key != "enter" || event.is_held {
                return;
            }
            self.act(Action::Next, window, cx);
        } else if let Some((dx, dy)) = match key {
            "left" => Some((-1, 0)),
            "right" => Some((1, 0)),
            "up" => Some((0, -1)),
            "down" => Some((0, 1)),
            _ => None,
        } {
            self.finish_stroke(cx);
            let play = self.book.edit();
            let (width, height) = (play.board.puzzle().width(), play.board.puzzle().height());
            let column = (play.cursor % width)
                .saturating_add_signed(dx)
                .min(width - 1);
            let row = (play.cursor / width)
                .saturating_add_signed(dy)
                .min(height - 1);
            play.cursor = row * width + column;
            self.hover = None;
            self.show_cursor();
            cx.notify();
        } else if !event.is_held {
            match key {
                "f" => self.act(Action::Tool(Tool::Fill), window, cx),
                "x" => self.act(Action::Tool(Tool::Mark), window, cx),
                "e" => self.act(Action::Tool(Tool::Erase), window, cx),
                "z" => self.act(Action::Undo, window, cx),
                "c" => self.act(Action::Check, window, cx),
                "h" => self.act(Action::Hint, window, cx),
                "escape" => {
                    self.notice = None;
                    cx.notify();
                }
                "space" | "backspace" | "delete" => {
                    self.finish_stroke(cx);
                    if self.book.play().solved {
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    let play = self.book.edit();
                    let before = play.board.snapshot();
                    let tool = if key == "space" {
                        self.tool
                    } else {
                        Tool::Erase
                    };
                    let ink = tool.paint(play.board.cells()[play.cursor]);
                    if play.board.paint(play.cursor, ink) {
                        self.notice = None;
                        self.commit(before, cx);
                    }
                    self.hover = None;
                    cx.notify();
                }
                _ => return,
            }
        } else {
            return;
        }
        cx.stop_propagation();
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.needs_focus) {
            let focus = if self.gallery.is_some() {
                &self.gallery_focus
            } else {
                &self.board_focus
            }
            .clone();
            cx.on_next_frame(window, move |view, window, cx| {
                focus.focus(window, cx);
                if view.gallery.is_none() && !view.book.play().solved && view.show_cursor() {
                    cx.notify();
                }
            });
        }
        let width = super::board_space(window).max(280.);
        let colors = paper::Colors::new(cx);
        if let Some(series) = self.gallery {
            self.render_gallery(series, width, colors, cx)
                .into_any_element()
        } else {
            self.render_plate(colors, window, cx).into_any_element()
        }
    }
}

fn plate_name(index: usize) -> String {
    format!("Plate {:02}", index + 1)
}

/// Connect consecutive sampled pointer positions even when motion skips tiles.
fn trace_cells(from: usize, to: usize, width: usize, mut visit: impl FnMut(usize)) {
    let (mut x, mut y) = ((from % width) as isize, (from / width) as isize);
    let (end_x, end_y) = ((to % width) as isize, (to / width) as isize);
    let (dx, dy) = ((end_x - x).abs(), -(end_y - y).abs());
    let (step_x, step_y) = ((end_x - x).signum(), (end_y - y).signum());
    let mut error = dx + dy;
    loop {
        visit(y as usize * width + x as usize);
        if (x, y) == (end_x, end_y) {
            break;
        }
        let doubled = error * 2;
        if doubled >= dy {
            error += dy;
            x += step_x;
        }
        if doubled <= dx {
            error += dx;
            y += step_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use corner_table_api::GameId;
    use gpui::{Modifiers, PlatformInput, TestAppContext, px, size};

    use super::*;
    use crate::games::GameEntry;

    #[cfg(debug_assertions)]
    #[gpui::test]
    fn completed_print_reports_turns_and_enter_opens_the_next_plate(cx: &mut TestAppContext) {
        use gpui::VisualTestContext;

        let launch = GameLaunch {
            game: GameId(format!("picture-logic-result-{}", std::process::id())),
            entry: GameEntry::Classic,
        };
        let viewport = size(px(760.), px(540.));
        let window = cx.open_window(viewport, |_, cx| {
            let mut view = GameView::new(&launch, cx);
            view.hint(cx);
            let play = view.book.edit();
            let puzzle = play.board.puzzle();
            let before = play.board.snapshot();
            let last = (0..play.board.cells().len())
                .find(|&index| puzzle.filled(index) && play.board.cells()[index] == Cell::Blank)
                .unwrap();
            for index in 0..play.board.cells().len() {
                if puzzle.filled(index) && index != last {
                    play.board.paint(index, Cell::Filled);
                }
            }
            play.cursor = last;
            view.commit(before, cx);
            view
        });
        cx.run_until_parked();
        window
            .update(cx, |view, window, cx| view.board_focus.focus(window, cx))
            .unwrap();
        cx.simulate_keystrokes(window.into(), "space");
        cx.run_until_parked();
        window
            .read_with(cx, |view, _| {
                assert!(view.book.play().solved);
                assert!(view.reveal_at.is_some());
                assert!(view.book.play().used_hint);
                assert_eq!(view.book.play().moves, 3);
            })
            .unwrap();
        let mut ui = VisualTestContext::from_window(window.into(), cx);
        let artwork = ui.debug_bounds("picture-finished-print").unwrap();
        let next = ui.debug_bounds("picture-next").unwrap();
        assert!(next.left() > artwork.right());
        assert!(next.right() < viewport.width);
        assert!(next.bottom() < viewport.height);
        ui.simulate_keystrokes("enter");
        ui.run_until_parked();
        window
            .read_with(&ui, |view, _| {
                assert_eq!(view.book.active, 1);
                assert!(view.book.plays[0].solved);
                assert!(view.reveal_at.is_none());
            })
            .unwrap();
    }

    #[gpui::test]
    fn advancing_to_a_lower_row_scrolls_the_compact_grid(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId("picture-logic-test-scrolling".to_owned()),
            entry: GameEntry::Classic,
        };
        let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
            let mut view = GameView::new(&launch, cx);
            view.book.active = view
                .book
                .plays
                .iter()
                .position(|play| play.board.puzzle().width() == 15)
                .unwrap();
            view
        });
        cx.run_until_parked();
        window
            .update(cx, |view, window, cx| view.board_focus.focus(window, cx))
            .unwrap();
        cx.simulate_keystrokes(window.into(), &["down"; 14].join(" "));
        window
            .read_with(cx, |view, _| {
                let plot = view.plot.get().unwrap();
                let viewport = view.grid_scroll.bounds();
                assert_eq!(view.book.play().cursor, 14 * 15);
                assert!(view.grid_scroll.offset().y < px(0.));
                assert!(plot.bottom() <= viewport.bottom());
            })
            .unwrap();
    }

    #[gpui::test]
    fn right_drag_marks_empty_with_the_fill_tool_selected(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId("picture-logic-test-right-drag".to_owned()),
            entry: GameEntry::Classic,
        };
        let window = cx.add_window(|_, cx| GameView::new(&launch, cx));
        cx.run_until_parked();
        let plot = window
            .read_with(cx, |view, _| {
                view.plot.get().expect("grid bounds are painted")
            })
            .unwrap();
        let unit = plot.size.width / 5.;
        let square = |col: f32| plot.origin + point(unit * (col + 0.5), unit * 0.5);
        cx.update_window(window.into(), |_, window, cx| {
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Right,
                    position: square(0.),
                    modifiers: Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: square(4.),
                    pressed_button: Some(MouseButton::Right),
                    modifiers: Modifiers::default(),
                }),
                cx,
            );
        })
        .unwrap();
        cx.run_until_parked();
        window
            .read_with(cx, |view, _| {
                assert!(view.tool == Tool::Fill);
                assert_eq!(&view.book.play().board.cells()[..5], &[Cell::Marked; 5]);
            })
            .unwrap();
    }

    #[test]
    fn fast_drag_paints_intermediate_tiles_in_both_directions() {
        let mut path = Vec::new();
        trace_cells(24, 20, 5, |index| path.push(index));
        assert_eq!(path, [24, 23, 22, 21, 20]);
        path.clear();
        trace_cells(0, 24, 5, |index| path.push(index));
        assert_eq!(path, [0, 6, 12, 18, 24]);
        path.clear();
        trace_cells(7, 7, 5, |index| path.push(index));
        assert_eq!(path, [7]);
    }

    #[test]
    fn paint_tool_can_toggle_and_replace_other_marks() {
        assert_eq!(Tool::Fill.paint(Cell::Marked), Cell::Filled);
        assert_eq!(Tool::Fill.paint(Cell::Filled), Cell::Blank);
        assert_eq!(Tool::Mark.paint(Cell::Filled), Cell::Marked);
        assert_eq!(Tool::Mark.paint(Cell::Marked), Cell::Blank);
        assert_eq!(Tool::Erase.paint(Cell::Filled), Cell::Blank);
    }
}
