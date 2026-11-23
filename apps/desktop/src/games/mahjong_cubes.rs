mod scene;

use std::{
    collections::BTreeMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use corner_table_mahjong_cubes::{Board, Layout, MoveResult};
use gpui::{
    Context, CursorStyle, DispatchPhase, Div, FocusHandle, FontWeight, HitboxBehavior,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Role,
    Stateful, Task, Window, canvas, div, linear_color_stop, linear_gradient, point, prelude::*, px,
    rgb,
};
use serde::{Deserialize, Serialize};

use super::{GameEntry, GameLaunch};
use crate::{storage::Session, theme};
use scene::{FadingPair, Orientation, Scene, TileStates};

const INSTRUCTION: &str = "Match identical cubes with a clear top and an open side.";
const HINT: &str = "Gold edges mark a matching pair.";
const HINT_TURN: &str = "Gold edges mark the pair. Turn to see the other cube.";
const HINT_CHOOSE: &str = "Choose the other gold-edged cube.";
const SAVE_VERSION: u8 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    Layout(Layout),
    Daily(u64),
    Challenge(u64),
}

impl Table {
    fn key(self) -> &'static str {
        match self {
            Self::Layout(layout) => layout.id(),
            Self::Daily(_) => "daily",
            Self::Challenge(_) => "challenge",
        }
    }

    fn fixed_seed(self) -> Option<u64> {
        match self {
            Self::Layout(_) => None,
            Self::Daily(day) => Some(0x4441_494c_5943_5542 ^ day),
            Self::Challenge(seed) => Some(seed),
        }
    }

    fn deal(self, seed: u64) -> Board {
        match self {
            Self::Layout(layout) => Board::with_layout(layout, seed),
            Self::Daily(day) => {
                let layout = Layout::ALL[(day % Layout::ALL.len() as u64) as usize];
                Board::with_layout(layout, seed)
            }
            Self::Challenge(_) => Board::new(seed),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct Save {
    version: u8,
    active: String,
    rounds: BTreeMap<String, RoundProgress>,
}

impl Default for Save {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            active: Layout::Terrace.id().to_owned(),
            rounds: BTreeMap::new(),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct RoundProgress {
    seed: u64,
    moves: Vec<[usize; 2]>,
    cleared: bool,
}

fn load_round(table: Table, saved: Option<&RoundProgress>) -> (Board, u64, bool) {
    let fixed_seed = table.fixed_seed();
    if let Some(saved) = saved
        && fixed_seed.is_none_or(|seed| seed == saved.seed)
    {
        let mut board = table.deal(saved.seed);
        if saved.moves.iter().all(|&pair| board.remove_pair(pair)) {
            return (board, saved.seed, saved.cleared);
        }
    }
    let seed = fixed_seed.unwrap_or_else(fresh_seed);
    (table.deal(seed), seed, false)
}

pub struct GameView {
    session: Session,
    saved: Save,
    table: Table,
    seed: u64,
    board: Board,
    cleared: bool,
    orientation: Orientation,
    press: Option<Press>,
    board_focus: FocusHandle,
    keyed: Option<usize>,
    hinted: Option<[usize; 2]>,
    hint_timer: Option<Task<()>>,
    fading: Option<FadingPair>,
    feedback: Option<&'static str>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let session = Session::new(launch);
        let saved = session
            .load::<Save>()
            .filter(|saved| saved.version == SAVE_VERSION)
            .unwrap_or_default();
        let table = match &launch.entry {
            GameEntry::Challenge(id) => {
                Table::Challenge(id.bytes().fold(0x434f_524e_4355_4245_u64, |seed, byte| {
                    seed.wrapping_mul(1099511628211) ^ u64::from(byte)
                }))
            }
            _ if saved.active == "daily" => daily(),
            _ => Table::Layout(Layout::from_id(&saved.active).unwrap_or(Layout::Terrace)),
        };
        let (board, seed, cleared) = load_round(table, saved.rounds.get(table.key()));
        let mut view = Self {
            session,
            saved,
            table,
            seed,
            board,
            cleared,
            orientation: Orientation::default(),
            press: None,
            board_focus: cx.focus_handle(),
            keyed: None,
            hinted: None,
            hint_timer: None,
            fading: None,
            feedback: None,
        };
        view.save();
        view
    }

    fn save(&mut self) {
        self.saved.active = self.table.key().to_owned();
        self.saved.rounds.insert(
            self.saved.active.clone(),
            RoundProgress {
                seed: self.seed,
                moves: self.board.moves().to_vec(),
                cleared: self.cleared,
            },
        );
        self.session.save(&self.saved);
    }

    fn switch_table(&mut self, table: Table) {
        if self.table == table {
            return;
        }
        self.table = table;
        (self.board, self.seed, self.cleared) =
            load_round(table, self.saved.rounds.get(table.key()));
        self.reset_view();
        self.save();
    }

    fn redeal(&mut self) {
        self.seed = fresh_seed();
        self.board = self.table.deal(self.seed);
        self.cleared = false;
        self.reset_view();
        self.save();
    }

    fn restart(&mut self) {
        self.board.restart();
        self.reset_view();
        self.save();
    }

    fn reset_view(&mut self) {
        self.orientation = Orientation::default();
        self.press = None;
        self.keyed = None;
        self.fading = None;
        self.feedback = None;
        self.clear_hint();
    }

    fn clear_hint(&mut self) {
        self.hinted = None;
        self.hint_timer = None;
        if matches!(self.feedback, Some(HINT | HINT_TURN | HINT_CHOOSE)) {
            self.feedback = None;
        }
    }

    fn choose(&mut self, id: usize) {
        let result = self.board.select(id);
        if result == MoveResult::Matched || self.hinted.is_some_and(|pair| !pair.contains(&id)) {
            self.clear_hint();
        }
        self.feedback = match result {
            MoveResult::Blocked => Some("That cube needs a clear top and an open side."),
            MoveResult::Selected if self.hinted.is_some() => Some(HINT_CHOOSE),
            MoveResult::Selected => Some("Choose a free cube with the same marks."),
            MoveResult::Deselected => None,
            MoveResult::DifferentFace => Some("Different marks. Your last cube is selected."),
            MoveResult::Matched => {
                let ids = *self.board.moves().last().expect("match just recorded");
                self.fading = Some(FadingPair {
                    ids,
                    at: Instant::now(),
                });
                self.cleared |= self.board.remaining() == 0;
                self.keyed = None;
                self.save();
                None
            }
        };
    }

    fn hint(&mut self, keyboard: bool, cx: &Context<Self>) {
        let Some(hint) = Scene::hint(&self.board, self.orientation) else {
            return;
        };
        if self
            .board
            .selected()
            .is_some_and(|id| !hint.pair.contains(&id))
        {
            self.board.deselect();
        }
        self.orientation = hint.orientation;
        self.keyed = keyboard
            .then(|| hint.target(self.board.selected()))
            .flatten();
        self.feedback = Some(if hint.visible == [true; 2] {
            HINT
        } else {
            HINT_TURN
        });
        self.hinted = Some(hint.pair);

        let timer = cx.background_executor().timer(Duration::from_secs(5));
        self.hint_timer = Some(cx.spawn(async move |view, cx| {
            timer.await;
            let _ = view.update(cx, |view, cx| {
                view.hinted = None;
                if matches!(view.feedback, Some(HINT | HINT_TURN | HINT_CHOOSE)) {
                    view.feedback = None;
                }
                cx.notify();
            });
        }));
    }

    fn undo(&mut self) {
        if self.board.undo() {
            self.clear_hint();
            self.fading = None;
            self.keyed = None;
            self.feedback = Some("Last pair returned.");
            self.save();
        }
    }

    fn turn(&mut self, quarters: i32) {
        self.orientation = self.orientation.turn(quarters);
        self.keyed = None;
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.modifiers.alt || key.modifiers.platform || key.modifiers.control {
            return;
        }
        match key.key.as_str() {
            "left" | "right" | "up" | "down" => {
                let step = match key.key.as_str() {
                    "left" => point(-1., 0.),
                    "right" => point(1., 0.),
                    "up" => point(0., -1.),
                    _ => point(0., 1.),
                };
                self.keyed =
                    Scene::keyboard_target(&self.board, self.orientation, self.keyed, step);
                self.feedback = None;
            }
            "enter" | "space" if !event.is_held => {
                if let Some(id) = self.keyed {
                    self.choose(id);
                }
            }
            "q" if !event.is_held => self.turn(1),
            "e" if !event.is_held => self.turn(-1),
            "h" if !event.is_held => self.hint(true, cx),
            "u" if !event.is_held => self.undo(),
            "escape" => {
                self.board.deselect();
                self.clear_hint();
                self.feedback = None;
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn selection_status(&self, keyboard: bool) -> String {
        if let Some(feedback) = self.feedback {
            return feedback.to_owned();
        }
        if keyboard
            && let Some(keyed) = self.keyed
            && let Some((_, cube)) = self.board.cubes().find(|(id, _)| *id == keyed)
        {
            return format!(
                "{} {} · Enter to select",
                cube.face.rank,
                scene::suit_name(cube.face.suit)
            );
        }
        if self.board.selected().is_some() {
            return "Choose a free cube with the same marks.".to_owned();
        }
        INSTRUCTION.to_owned()
    }

    fn layouts(&self, narrow: bool, cx: &Context<Self>) -> Div {
        let strip = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(if narrow { 5. } else { 8. }))
            .children(Layout::ALL.map(|layout| {
                button(
                    layout.id(),
                    layout.name(),
                    true,
                    Some(self.table == Table::Layout(layout)),
                    move |view, _| view.switch_table(Table::Layout(layout)),
                    cx,
                )
                .when(narrow, |button| button.px_2())
            }))
            .child(
                button(
                    "cubes-daily",
                    "Daily",
                    true,
                    Some(matches!(self.table, Table::Daily(_))),
                    |view, _| view.switch_table(daily()),
                    cx,
                )
                .when(narrow, |button| button.px_2()),
            );
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .child(strip)
            .when(matches!(self.table, Table::Layout(_)), |row| {
                row.child(
                    button(
                        "cubes-new-deal",
                        "New deal",
                        true,
                        None,
                        |view, _| view.redeal(),
                        cx,
                    )
                    .when(narrow, |button| button.px_2()),
                )
            })
    }

    fn progress(&self, compact: bool, cx: &Context<Self>) -> Div {
        let total = self.board.total_pairs();
        let matched = self.board.moves().len();
        let ready = self.board.available_pairs().len();
        let palette = theme::colors(cx);
        let title = match self.table {
            Table::Daily(_) => Some(format!("{} · Daily stack", self.board.shape().name())),
            Table::Challenge(_) => Some(format!("{} · Challenge", self.board.shape().name())),
            Table::Layout(_) => None,
        };
        let mut ready_label = match ready {
            0 if matched == total => "All pairs matched".to_owned(),
            1 => "1 match ready".to_owned(),
            _ => format!("{ready} matches ready"),
        };
        if matches!(self.table, Table::Daily(_)) && self.cleared {
            ready_label = if matched == total {
                "Daily cleared".to_owned()
            } else {
                format!("{ready_label} · Daily cleared")
            };
        }
        let ready = if title.is_some() {
            div()
                .text_size(px(12.))
                .line_height(px(17.))
                .text_color(rgb(palette.muted))
                .child(ready_label.clone())
        } else {
            theme::display_type(ready_label.clone(), 19.)
        }
        .id("cubes-ready")
        .role(Role::Label)
        .aria_label(ready_label);
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .px_3()
            .min_h(px(if compact { 32. } else { 42. }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .when_some(title, |column, title| {
                        column.child(theme::display_type(title, 19.))
                    })
                    .child(ready),
            )
            .child(
                div()
                    .id("cubes-progress")
                    .role(Role::ProgressIndicator)
                    .aria_label("Pairs cleared")
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(total as f64)
                    .aria_numeric_value(matched as f64)
                    .aria_value(format!("{matched} of {total}"))
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{matched} / {total} pairs cleared")),
                    )
                    .child(div().flex().gap(px(2.)).children((0..total).map(|pair| {
                        div()
                            .w(px(4.))
                            .h(px(5.))
                            .rounded_sm()
                            .bg(rgb(if pair < matched {
                                0xc9903e
                            } else {
                                palette.border
                            }))
                    }))),
            )
    }

    fn playfield(&self, height: Pixels, cx: &Context<Self>) -> Stateful<Div> {
        let entity = cx.entity();
        let weak = entity.downgrade();
        let cleared = self.board.remaining() == 0;
        div()
            .id("cube-table")
            .role(Role::Group)
            .aria_label("Cube board")
            .aria_description("Arrow keys move between visible free cubes. Enter selects. Q and E turn the board. H hints. U undoes.")
            .track_focus(&self.board_focus)
            .tab_index(0)
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .h(height)
            .relative()
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x425844))
            .focus_visible(|style| style.border_color(rgb(theme::GOLD)))
            .overflow_hidden()
            .bg(linear_gradient(145., linear_color_stop(rgb(0x253e32), 0.), linear_color_stop(rgb(0x0e2922), 1.)))
            .child(canvas(
                move |bounds, window, cx| {
                    let view = entity.read(cx);
                    let scene = Scene::new(&view.board, bounds, view.orientation);
                    let state = TileStates {
                        selected: view.board.selected(),
                        hinted: view.hinted,
                        keyed: view.keyed.filter(|_| view.board_focus.is_focused(window)),
                        fading: view.fading,
                        pressed: view.press.filter(|press| !press.moved).and_then(|press| press.cube),
                    };
                    let turning = view.press.is_some_and(|press| press.moved);
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    (scene, state, hitbox, turning)
                },
                move |_, (scene, state, hitbox, turning), window, _| {
                    let hovered = hitbox.is_hovered(window)
                        .then(|| scene.hit(window.mouse_position()))
                        .flatten()
                        .filter(|cube| cube.free)
                        .map(|cube| cube.id);
                    scene.paint(state, hovered.filter(|_| !turning), window);
                    if !cleared {
                        window.set_cursor_style(
                            if turning { CursorStyle::ClosedHand }
                            else if hovered.is_some() { CursorStyle::PointingHand }
                            else { CursorStyle::OpenHand },
                            &hitbox,
                        );
                    }

                    let scene = Rc::new(scene);
                    let down_scene = scene.clone();
                    let down_hitbox = hitbox.clone();
                    let down_entity = weak.clone();
                    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble
                            && event.button == MouseButton::Left
                            && down_hitbox.is_hovered(window)
                            && !cleared
                        {
                            let id = down_scene.hit(event.position).map(|cube| cube.id);
                            let _ = down_entity.update(cx, |view, cx| {
                                view.board_focus.focus(window, cx);
                                view.keyed = None;
                                view.press = Some(Press::new(event.position, id));
                                cx.notify();
                            });
                        }
                    });

                    let move_scene = scene.clone();
                    let move_hitbox = hitbox.clone();
                    let move_entity = weak.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        if event.dragging() || turning || state.pressed.is_some() {
                            let _ = move_entity.update(cx, |view, cx| {
                                if let Some(press) = &mut view.press {
                                    if event.dragging() {
                                        let moved_before = press.moved;
                                        let turns = press.advance(event.position);
                                        if turns != 0 || moved_before != press.moved {
                                            view.turn(turns);
                                            cx.notify();
                                        }
                                    } else {
                                        view.press = None;
                                        cx.notify();
                                    }
                                }
                            });
                        }
                        let next = move_scene.hit(event.position)
                            .filter(|cube| cube.free && move_hitbox.is_hovered(window))
                            .map(|cube| cube.id);
                        if next != hovered {
                            window.refresh();
                        }
                    });

                    let up_entity = weak.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                            return;
                        }
                        let _ = up_entity.update(cx, |view, cx| {
                            if let Some(mut press) = view.press.take() {
                                let turns = press.advance(event.position);
                                if press.moved {
                                    view.turn(turns);
                                } else if let Some(id) = press.cube
                                    && hitbox.is_hovered(window)
                                    && scene.hit(event.position).is_some_and(|cube| cube.id == id)
                                {
                                    view.choose(id);
                                }
                                cx.notify();
                            }
                        });
                    });
                },
            ).size_full())
            .when(cleared, |board| {
                board.child(
                    div().absolute().inset_0().flex().items_center().justify_center().child(
                        div().px_6().py_4().bg(rgb(0x15352b)).text_color(rgb(0xffe7b9))
                            .border_1().border_color(rgb(0x9a7c50)).rounded_md()
                            .child(theme::display_type("Stack cleared", 29.)),
                    ),
                )
            })
    }

    fn tools(&self, cx: &Context<Self>) -> Div {
        let colors = theme::colors(cx);
        let cleared = self.board.remaining() == 0;
        let started = !self.board.moves().is_empty() || self.board.selected().is_some();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .px_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(button(
                        "cubes-turn-left",
                        "Turn left",
                        !cleared,
                        None,
                        |view, _| view.turn(1),
                        cx,
                    ))
                    .child(
                        div()
                            .id("cubes-viewpoint")
                            .role(Role::Label)
                            .aria_label(format!("{} view", self.orientation.label()))
                            .w(px(26.))
                            .text_center()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(colors.muted))
                            .child(self.orientation.label()),
                    )
                    .child(button(
                        "cubes-turn-right",
                        "Turn right",
                        !cleared,
                        None,
                        |view, _| view.turn(-1),
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(button(
                        "cubes-hint",
                        "Hint",
                        !cleared,
                        None,
                        |view, cx| view.hint(false, cx),
                        cx,
                    ))
                    .child(button(
                        "cubes-undo",
                        "Undo",
                        !self.board.moves().is_empty(),
                        None,
                        |view, _| view.undo(),
                        cx,
                    ))
                    .child(button(
                        "cubes-replay",
                        if cleared { "Replay" } else { "Restart" },
                        started,
                        None,
                        |view, _| view.restart(),
                        cx,
                    )),
            )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::colors(cx);
        let narrow = window.viewport_size().width < px(850.);
        let controls_height = match (narrow, self.table) {
            (true, Table::Challenge(_)) => 218.,
            (true, Table::Daily(_)) => 264.,
            (true, Table::Layout(_)) => 256.,
            (false, Table::Challenge(_)) => 240.,
            _ => 286.,
        };
        let height = (window.viewport_size().height - px(controls_height)).max(px(248.));
        let keyboard = self.board_focus.is_focused(window) && window.last_input_was_keyboard();
        let cleared = self.board.remaining() == 0;
        let status = match (cleared, self.table) {
            (true, Table::Challenge(_)) => "Replay this challenge.".to_owned(),
            (true, _) => "Replay this deal or choose another stack.".to_owned(),
            _ => self.selection_status(keyboard),
        };
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .when(!matches!(self.table, Table::Challenge(_)), |view| {
                view.child(self.layouts(narrow, cx))
            })
            .child(
                theme::panel(cx)
                    .w_full()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .when(narrow, |panel| panel.p_2().gap_2())
                    .child(self.progress(narrow, cx))
                    .child(self.playfield(height, cx))
                    .child(self.tools(cx))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .justify_between()
                            .gap_x_3()
                            .px_1()
                            .line_height(px(18.))
                            .text_size(px(12.))
                            .text_color(rgb(palette.muted))
                            .child(
                                div()
                                    .id("cubes-status")
                                    .role(Role::Status)
                                    .aria_label(status.clone())
                                    .child(status),
                            )
                            .when(!cleared, |row| {
                                row.child(div().child(match (keyboard, narrow) {
                                    (true, _) => "Arrows select · Enter chooses · Q / E turns",
                                    (false, true) => "Drag sideways to turn",
                                    (false, false) => "Drag sideways to turn the stack",
                                }))
                            }),
                    ),
            )
    }
}

fn button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    active: Option<bool>,
    action: impl Fn(&mut GameView, &Context<GameView>) + 'static,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    let colors = theme::colors(cx);
    let filled = active == Some(true);
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .when_some(active, |button, active| button.aria_toggled(active.into()))
        .when(!enabled, |button| {
            button
                .role(Role::Label)
                .aria_label(format!("{label} unavailable"))
        })
        .h(px(34.))
        .px_3()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if filled { 0xc39448 } else { colors.border }))
        .bg(rgb(if filled {
            0xf0cf8a
        } else if enabled {
            colors.surface
        } else {
            colors.paper
        }))
        .text_color(rgb(if filled {
            theme::INK
        } else if enabled {
            colors.ink
        } else {
            colors.muted
        }))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |button| {
            button
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(if filled { 0xf6d99d } else { colors.hover })))
                .active(|style| style.bg(rgb(if filled { 0xe3ba71 } else { colors.paper })))
                .focus_visible(|style| style.border_color(rgb(colors.gold_text)))
                .on_click(cx.listener(move |view, _, _, cx| {
                    action(view, cx);
                    cx.notify();
                }))
        })
        .child(label)
}

fn daily() -> Table {
    let day = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400;
    Table::Daily(day)
}

fn fresh_seed() -> u64 {
    static DEAL: AtomicU64 = AtomicU64::new(0);
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    time.wrapping_add(DEAL.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Copy)]
struct Press {
    origin: Point<Pixels>,
    anchor_x: Pixels,
    cube: Option<usize>,
    moved: bool,
}

impl Press {
    fn new(origin: Point<Pixels>, cube: Option<usize>) -> Self {
        Self {
            origin,
            anchor_x: origin.x,
            cube,
            moved: false,
        }
    }

    fn advance(&mut self, pointer: Point<Pixels>) -> i32 {
        const TURN_DISTANCE: f32 = 62.;
        let delta = pointer - self.origin;
        self.moved |= delta.x.abs().max(delta.y.abs()) >= px(6.);
        let steps = (f32::from(pointer.x - self.anchor_x) / TURN_DISTANCE) as i32;
        self.anchor_x += px(TURN_DISTANCE * steps as f32);
        -steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_jitter_can_click_but_dragging_never_clicks_on_release() {
        let start = point(px(90.), px(90.));
        let mut press = Press::new(start, Some(4));
        assert_eq!(press.advance(point(px(94.), px(92.))), 0);
        assert!(!press.moved);
        assert_eq!(press.advance(point(px(155.), px(92.))), -1);
        assert_eq!(press.advance(point(px(230.), px(92.))), -1);
        assert_eq!(press.advance(start), 2);
        assert!(press.moved);

        let mut press = Press::new(start, Some(4));
        assert_eq!(press.advance(point(px(90.), px(117.))), 0);
        assert!(press.moved);
    }

    #[test]
    fn saved_moves_replay_and_previous_daily_does_not_carry_over() {
        let yesterday = Table::Daily(1234);
        let (mut board, seed, _) = load_round(yesterday, None);
        let pair = board.hint().unwrap();
        assert!(board.remove_pair(pair));
        let save = RoundProgress {
            seed,
            moves: vec![pair],
            cleared: true,
        };
        let (restored, resumed_seed, cleared) = load_round(yesterday, Some(&save));
        assert_eq!(resumed_seed, seed);
        assert!(cleared);
        assert!(restored.cubes().eq(board.cubes()));

        let (today, todays_seed, cleared) = load_round(Table::Daily(1235), Some(&save));
        assert_ne!(todays_seed, seed);
        assert_ne!(today.shape(), board.shape());
        assert!(!cleared);
        assert!(today.moves().is_empty());
    }

    #[test]
    fn corrupt_replay_cannot_partially_remove_cubes() {
        let table = Table::Daily(1234);
        let (board, seed, _) = load_round(table, None);
        let pair = board.hint().unwrap();
        let corrupt = RoundProgress {
            seed,
            moves: vec![pair, pair],
            cleared: true,
        };
        let (restored, _, cleared) = load_round(table, Some(&corrupt));
        assert_eq!(restored.remaining(), board.remaining());
        assert!(restored.moves().is_empty());
        assert!(!cleared);
    }
}
