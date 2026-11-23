mod table;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use corner_table_mahjong::{Board, DAILY_SEED, Layout, MoveResult, Tile};
use gpui::{
    Context, Div, FocusHandle, FontWeight, KeyDownEvent, Render, Role, Stateful, Toggled, Window,
    div, prelude::*, px, rgb,
};
use serde::{Deserialize, Serialize};

use super::{GameEntry, GameLaunch, themed_control};
use crate::{
    audio::{self, Effect},
    storage::{AttemptId, Outcome, ResultMetric, Session},
    theme,
};
use table::{TableColors, TilePosition, face_name};

const INSTRUCTION: &str = "Match two free tiles with the same face.";
const NO_MATCHES: &str = "No free pairs remain. Undo a move or restart.";
const LAYOUTS: [(Layout, &str, &str); 4] = [
    (Layout::Terrace, "terrace", "Terrace"),
    (Layout::Atrium, "atrium", "Atrium"),
    (Layout::Crown, "crown", "Crown"),
    (Layout::Lantern, "lantern", "Lantern"),
];

#[derive(Deserialize, Serialize)]
struct SavedRound {
    layout: String,
    seed: u64,
    #[serde(default)]
    attempt: AttemptId,
    matches: Vec<[usize; 2]>,
    assisted: bool,
    clean_clear: bool,
}

#[derive(Deserialize)]
struct DailyAttempt {
    #[serde(default)]
    attempt: Option<AttemptId>,
}

struct Departure {
    tiles: [Tile; 2],
    sequence: usize,
}

pub struct GameView {
    board: Board,
    session: Session,
    is_daily: bool,
    seed: u64,
    attempt: AttemptId,
    clean_clear: bool,
    highlighted: Option<[usize; 2]>,
    departure: Option<Departure>,
    sequence: usize,
    status: String,
    focus: FocusHandle,
    focus_on_open: bool,
    cursor: Option<usize>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let seed = match &launch.entry {
            GameEntry::Challenge(id) if id == "fixture-mahjong-001" => DAILY_SEED,
            GameEntry::Challenge(id) => id.bytes().fold(DAILY_SEED, |seed, byte| {
                seed.wrapping_mul(1099511628211) ^ u64::from(byte)
            }),
            _ => new_seed(),
        };
        let mut view = Self {
            board: Board::new(seed),
            session: Session::new(launch),
            is_daily: matches!(launch.entry, GameEntry::Challenge(_)),
            seed,
            attempt: AttemptId::default(),
            clean_clear: false,
            highlighted: None,
            departure: None,
            sequence: 0,
            status: INSTRUCTION.into(),
            focus: cx.focus_handle().tab_stop(true),
            focus_on_open: true,
            cursor: None,
        };
        let restored = view
            .session
            .load::<SavedRound>()
            .is_some_and(|saved| view.resume(saved));
        if view.daily() && !restored {
            if let Some(attempt) = view
                .session
                .load::<DailyAttempt>()
                .and_then(|saved| saved.attempt)
            {
                view.attempt = attempt;
            }
            view.save();
        }
        view
    }

    fn resume(&mut self, saved: SavedRound) -> bool {
        let Some(&(layout, _, _)) = LAYOUTS.iter().find(|(_, key, _)| *key == saved.layout) else {
            return false;
        };
        if self.daily() && (saved.seed != self.seed || layout != self.board.layout()) {
            return false;
        }
        let Some(board) = Board::resume(layout, saved.seed, &saved.matches, saved.assisted) else {
            return false;
        };
        self.board = board;
        self.seed = saved.seed;
        self.attempt = saved.attempt;
        self.clean_clear = saved.clean_clear;
        self.status = self.idle_status("Deal resumed.").into();
        true
    }

    fn save(&self) {
        let (_, key, _) = LAYOUTS
            .into_iter()
            .find(|(layout, _, _)| *layout == self.board.layout())
            .expect("every layout has a save key");
        self.session.save(&SavedRound {
            layout: key.into(),
            seed: self.seed,
            attempt: self.attempt,
            matches: self.board.matched_pairs().to_vec(),
            assisted: self.board.used_assistance(),
            clean_clear: self.clean_clear,
        });
    }

    fn daily(&self) -> bool {
        self.is_daily
    }

    fn cleared_title(&self) -> &'static str {
        if self.clean_clear && !self.board.used_assistance() {
            "Clean Clear complete!"
        } else {
            "Board cleared!"
        }
    }

    fn idle_status(&self, otherwise: &'static str) -> &'static str {
        if self.board.remaining() == 0 {
            self.cleared_title()
        } else if self.board.hint().is_none() {
            NO_MATCHES
        } else {
            otherwise
        }
    }

    fn choose(&mut self, id: usize, cx: &mut Context<Self>) {
        let face = face_name(self.board.tile(id).expect("tile is from this deal").face);
        if self.board.is_free(id) {
            self.cursor = Some(id);
            if self.highlighted.is_some_and(|pair| !pair.contains(&id)) {
                self.highlighted = None;
            }
        }
        let (status, effect) = match self.board.select(id) {
            MoveResult::Blocked => (
                "A free tile needs an open side and a clear top.".into(),
                Some(Effect::InvalidMove),
            ),
            MoveResult::Selected => (format!("{face} selected. Choose its match."), None),
            MoveResult::Deselected => (self.idle_status(INSTRUCTION).into(), None),
            MoveResult::DifferentFace => (
                format!("Faces differ. {face} is selected."),
                Some(Effect::InvalidMove),
            ),
            MoveResult::Matched => {
                self.highlighted = None;
                let pair = *self
                    .board
                    .matched_pairs()
                    .last()
                    .expect("just matched a pair");
                self.sequence = self.sequence.wrapping_add(1);
                self.departure = Some(Departure {
                    tiles: pair.map(|id| *self.board.tile(id).expect("matched tile exists")),
                    sequence: self.sequence,
                });
                self.cursor = self.nearest_tile(id, None);
                let effect = if self.board.remaining() == 0 {
                    let mode = if self.daily() {
                        "Daily"
                    } else {
                        let (_, _, layout) = LAYOUTS
                            .into_iter()
                            .find(|(layout, _, _)| *layout == self.board.layout())
                            .expect("every layout has a title");
                        layout
                    };
                    self.session.record_result(
                        self.attempt,
                        mode,
                        Outcome::Won,
                        ResultMetric::Pairs(self.board.matched_pairs().len() as u64),
                    );
                    Effect::Complete
                } else {
                    Effect::Clear
                };
                self.save();
                (self.idle_status("Pair matched.").into(), Some(effect))
            }
        };
        self.status = status;
        if let Some(effect) = effect {
            audio::play(cx, effect);
        }
        cx.notify();
    }

    fn new_deal(&mut self, layout: Layout) {
        self.seed = new_seed();
        self.board = Board::with_layout(layout, self.seed);
        self.attempt = AttemptId::default();
        self.reset_feedback();
        self.save();
    }

    fn restart(&mut self) {
        self.board.restart();
        self.attempt = AttemptId::default();
        self.reset_feedback();
        self.save();
    }

    fn reset_feedback(&mut self) {
        self.cursor = None;
        self.highlighted = None;
        self.departure = None;
        self.status = INSTRUCTION.into();
    }

    fn hint(&mut self) {
        let had_assistance = self.board.used_assistance();
        self.highlighted = self.board.reveal_hint();
        if let Some([first, second]) = self.highlighted {
            self.cursor = Some(if self.board.selected() == Some(first) {
                second
            } else {
                first
            });
            self.status = "Highlighted tiles form a free pair.".into();
            if !had_assistance {
                self.save();
            }
        } else {
            self.status = self.idle_status(INSTRUCTION).into();
        }
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        if self.board.undo() {
            self.highlighted = None;
            self.departure = None;
            self.status = "Last pair restored.".into();
            self.save();
            audio::play(cx, Effect::Move);
        }
    }

    fn toggle_clean_clear(&mut self) {
        self.clean_clear = !self.clean_clear;
        self.status = self
            .idle_status(if !self.clean_clear {
                INSTRUCTION
            } else if self.board.used_assistance() {
                "Restart to begin a clean run."
            } else {
                "Clear the board without Hint or Undo."
            })
            .into();
        self.save();
    }

    fn focused_tile(&self) -> Option<usize> {
        self.cursor
            .filter(|&id| self.board.is_free(id))
            .or_else(|| self.board.selected())
            .or_else(|| {
                self.board
                    .tiles()
                    .find(|(id, _)| self.board.is_free(*id))
                    .map(|(id, _)| id)
            })
    }

    fn nearest_tile(&self, from: usize, direction: Option<(f32, f32)>) -> Option<usize> {
        let (x, y) = TilePosition::relative(self.board.tile(from)?);
        self.board
            .tiles()
            .filter(|&(id, _)| id != from && self.board.is_free(id))
            .filter_map(|(id, tile)| {
                let (tx, ty) = TilePosition::relative(tile);
                let (dx, dy) = (tx - x, ty - y);
                let score = if let Some((vx, vy)) = direction {
                    let ahead = dx * vx + dy * vy;
                    if ahead <= 0.15 {
                        return None;
                    }
                    let across = dx * vy - dy * vx;
                    ahead.powi(2) + 4. * across.powi(2)
                } else {
                    dx.powi(2) + dy.powi(2)
                };
                Some((id, score))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(id, _)| id)
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        if stroke.modifiers.control || stroke.modifiers.platform || stroke.modifiers.alt {
            return;
        }
        let key = stroke.key.as_str();
        if !self.focus.is_focused(window) {
            return;
        }
        if key == "escape" {
            self.board.clear_selection();
            self.highlighted = None;
            self.status = self.idle_status(INSTRUCTION).into();
        } else {
            let direction = match key {
                "left" => Some((-1., 0.)),
                "right" => Some((1., 0.)),
                "up" => Some((0., -1.)),
                "down" => Some((0., 1.)),
                _ => None,
            };
            if let Some(direction) = direction {
                if let Some(id) = self.focused_tile() {
                    self.cursor = self.nearest_tile(id, Some(direction)).or(Some(id));
                }
            } else {
                match key {
                    "enter" | "space" if !event.is_held => {
                        if let Some(id) = self.focused_tile() {
                            self.choose(id, cx);
                        }
                    }
                    "h" if !event.is_held && self.board.remaining() > 0 => self.hint(),
                    "u" if !event.is_held => self.undo(cx),
                    _ => return,
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn deal_options(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        div()
            .flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .mr_2()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(palette.muted))
                            .child("Layout"),
                    )
                    .children(LAYOUTS.map(|(layout, key, label)| {
                        toggle(
                            key,
                            label,
                            self.board.layout() == layout,
                            move |view| {
                                if view.board.layout() != layout {
                                    view.new_deal(layout);
                                }
                            },
                            cx,
                        )
                    })),
            )
            .child(
                toggle(
                    "mahjong-clean-clear",
                    "Clean Clear",
                    self.clean_clear,
                    Self::toggle_clean_clear,
                    cx,
                )
                .aria_description("Clear all tiles without using Hint or Undo."),
            )
    }

    fn toolbar(&self, cx: &Context<Self>) -> Div {
        let remaining = self.board.remaining();
        let count = if self.daily() {
            format!("Daily · {remaining} tiles left")
        } else {
            format!("{remaining} tiles left")
        };
        div()
            .flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(count),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        control(
                            "mahjong-hint",
                            "Hint",
                            remaining > 0,
                            |view, _| view.hint(),
                            cx,
                        )
                        .aria_keyshortcuts("H"),
                    )
                    .child(
                        control(
                            "mahjong-undo",
                            "Undo",
                            self.board.can_undo(),
                            Self::undo,
                            cx,
                        )
                        .aria_keyshortcuts("U"),
                    )
                    .child(control(
                        "mahjong-restart",
                        "Restart",
                        true,
                        |view, _| view.restart(),
                        cx,
                    ))
                    .when(!self.daily(), |row| {
                        row.child(control(
                            "mahjong-new",
                            "New deal",
                            true,
                            |view, _| view.new_deal(view.board.layout()),
                            cx,
                        ))
                    }),
            )
    }

    fn feedback(&self, cx: &Context<Self>) -> Div {
        let palette = theme::colors(cx);
        let assistance = if self.board.used_assistance() {
            "Assisted run"
        } else {
            "Clean run"
        };
        let status = &self.status;
        div()
            .flex()
            .justify_between()
            .items_start()
            .gap_2()
            .text_size(px(13.))
            .line_height(px(19.))
            .text_color(rgb(palette.muted))
            .child(
                div()
                    .id("mahjong-status")
                    .role(Role::Status)
                    .aria_label(status.clone())
                    .when(self.clean_clear, |status| {
                        status.aria_description(assistance)
                    })
                    .child(status.clone()),
            )
            .when(self.clean_clear, |row| {
                row.child(
                    div()
                        .whitespace_nowrap()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(if self.board.used_assistance() {
                            palette.gold_text
                        } else {
                            palette.success
                        }))
                        .child(assistance),
                )
            })
    }
}

fn new_seed() -> u64 {
    static DEAL_COUNT: AtomicU64 = AtomicU64::new(0);
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);
    time.wrapping_add(DEAL_COUNT.fetch_add(1, Ordering::Relaxed))
}

fn control(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    action: impl Fn(&mut GameView, &mut Context<GameView>) + 'static,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    themed_control(cx, id, label, enabled)
        .min_h(px(33.))
        .px(px(10.))
        .whitespace_nowrap()
        .when(enabled, |button| {
            button.on_click(cx.listener(move |view, _, _, cx| {
                action(view, cx);
                cx.notify();
            }))
        })
}

fn toggle(
    id: &'static str,
    label: &'static str,
    selected: bool,
    action: impl Fn(&mut GameView) + 'static,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    let palette = theme::colors(cx);
    control(id, label, true, move |view, _| action(view), cx)
        .aria_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
        .when(selected, |button| {
            button
                .bg(rgb(palette.hover))
                .border_color(rgb(palette.muted))
        })
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_on_open {
            self.focus_on_open = false;
            self.focus.focus(window, cx);
        }
        let palette = theme::colors(cx);
        div()
            .id("mahjong-game")
            .on_key_down(cx.listener(Self::key_down))
            .flex()
            .flex_col()
            .gap_2()
            .when(!self.daily(), |root| root.child(self.deal_options(cx)))
            .child(self.toolbar(cx))
            .child(self.table(TableColors::from(palette), window, cx))
            .when(self.board.remaining() > 0, |root| {
                root.child(self.feedback(cx))
            })
    }
}
