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

use super::{GameEntry, GameLaunch, round_result};
use crate::{
    audio::{self, Effect},
    motion,
    storage::{AttemptId, Outcome, ResultMetric, Session},
    theme,
};
use scene::{FadingPair, Orbit, Scene, TileStates};

const INSTRUCTION: &str = "Match identical cubes with a clear top and an open side.";
const HINT: &str = "Gold edges mark a matching pair.";
const HINT_TURN: &str = "Gold edges mark the pair. Turn or tilt to see the other cube.";
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
    #[serde(default)]
    attempt: AttemptId,
    moves: Vec<[usize; 2]>,
    cleared: bool,
    #[serde(default)]
    orientation: Orbit,
}

fn load_round(table: Table, saved: Option<&RoundProgress>) -> (Board, u64, AttemptId, bool, Orbit) {
    let fixed_seed = table.fixed_seed();
    if let Some(saved) = saved
        && fixed_seed.is_none_or(|seed| seed == saved.seed)
    {
        let mut board = table.deal(saved.seed);
        if saved.moves.iter().all(|&pair| board.remove_pair(pair)) {
            return (
                board,
                saved.seed,
                saved.attempt,
                saved.cleared,
                saved.orientation.normalized(),
            );
        }
    }
    let seed = fixed_seed.unwrap_or_else(fresh_seed);
    (
        table.deal(seed),
        seed,
        AttemptId::default(),
        false,
        Orbit::default(),
    )
}

pub struct GameView {
    session: Session,
    saved: Save,
    table: Table,
    seed: u64,
    attempt: AttemptId,
    board: Board,
    cleared: bool,
    orientation: Orbit,
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
        let (board, seed, attempt, cleared, orientation) =
            load_round(table, saved.rounds.get(table.key()));
        let mut view = Self {
            session,
            saved,
            table,
            seed,
            attempt,
            board,
            cleared,
            orientation,
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
                attempt: self.attempt,
                moves: self.board.moves().to_vec(),
                cleared: self.cleared,
                orientation: self.orientation,
            },
        );
        self.session.save(&self.saved);
    }

    fn switch_table(&mut self, table: Table) {
        if self.table == table {
            return;
        }
        self.table = table;
        (
            self.board,
            self.seed,
            self.attempt,
            self.cleared,
            self.orientation,
        ) = load_round(table, self.saved.rounds.get(table.key()));
        self.reset_interaction();
        self.save();
    }

    fn redeal(&mut self) {
        self.seed = fresh_seed();
        self.attempt = AttemptId::default();
        self.board = self.table.deal(self.seed);
        self.cleared = false;
        self.reset_interaction();
        self.save();
    }

    fn restart(&mut self) {
        self.board.restart();
        self.attempt = AttemptId::default();
        self.reset_interaction();
        self.save();
    }

    fn reset_interaction(&mut self) {
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

    fn choose(&mut self, id: usize, cx: &Context<Self>) {
        let result = self.board.select(id);
        if result == MoveResult::Matched || self.hinted.is_some_and(|pair| !pair.contains(&id)) {
            self.clear_hint();
        }
        let (feedback, effect) = match result {
            MoveResult::Blocked => (
                Some("That cube needs a clear top and an open side."),
                Effect::InvalidMove,
            ),
            MoveResult::Selected if self.hinted.is_some() => (Some(HINT_CHOOSE), Effect::WoodTap),
            MoveResult::Selected => (
                Some("Choose a free cube with the same marks."),
                Effect::WoodTap,
            ),
            MoveResult::Deselected => (None, Effect::WoodTap),
            MoveResult::DifferentFace => (
                Some("Different marks. Your last cube is selected."),
                Effect::InvalidMove,
            ),
            MoveResult::Matched => {
                let ids = *self.board.moves().last().expect("match just recorded");
                let won = self.board.remaining() == 0;
                self.cleared |= won;
                self.keyed = None;
                self.save();
                if won {
                    let layout = self.board.shape().name();
                    let mode = match self.table {
                        Table::Layout(_) => layout.to_owned(),
                        Table::Daily(_) => format!("Daily · {layout}"),
                        Table::Challenge(_) => format!("Challenge · {layout}"),
                    };
                    self.session.record_result(
                        self.attempt,
                        mode,
                        Outcome::Won,
                        ResultMetric::Pairs(self.board.moves().len() as u64),
                    );
                }
                self.fading = motion::enabled(cx).then(|| FadingPair {
                    ids,
                    at: Instant::now(),
                });
                (
                    Some("Pair matched."),
                    if won {
                        Effect::Complete
                    } else {
                        Effect::WoodPair
                    },
                )
            }
        };
        self.feedback = feedback;
        audio::play(cx, effect);
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
        self.set_orbit(hint.orientation);
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

    fn undo(&mut self, cx: &Context<Self>) {
        if self.board.undo() {
            self.clear_hint();
            self.fading = None;
            self.keyed = None;
            self.feedback = Some("Last pair returned.");
            self.save();
            audio::play(cx, Effect::WoodPair);
        }
    }

    fn set_orbit(&mut self, orbit: Orbit) {
        if self.orientation != orbit {
            self.orientation = orbit;
            self.keyed = None;
            self.save();
        }
    }

    fn turn(&mut self, yaw: f32) {
        self.set_orbit(self.orientation.offset(yaw, 0.));
    }

    fn reset_camera(&mut self) {
        self.press = None;
        self.set_orbit(Orbit::default());
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.modifiers.alt || key.modifiers.platform || key.modifiers.control {
            return;
        }
        match key.key.as_str() {
            "up" if key.modifiers.shift => self.set_orbit(self.orientation.offset(0., 7.)),
            "down" if key.modifiers.shift => self.set_orbit(self.orientation.offset(0., -7.)),
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
                    self.choose(id, cx);
                }
            }
            "q" if !event.is_held => self.turn(-90.),
            "e" if !event.is_held => self.turn(90.),
            "r" if !event.is_held => self.reset_camera(),
            "h" if !event.is_held => self.hint(true, cx),
            "u" if !event.is_held => self.undo(cx),
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

    fn layouts(&self, narrow: bool, in_play: bool, cx: &Context<Self>) -> Div {
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
                    move |view, _, _| view.switch_table(Table::Layout(layout)),
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
                    |view, _, _| view.switch_table(daily()),
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
            .when(in_play && matches!(self.table, Table::Layout(_)), |row| {
                row.child(
                    button(
                        "cubes-new-deal",
                        "New deal",
                        true,
                        None,
                        |view, _, _| view.redeal(),
                        cx,
                    )
                    .when(narrow, |button| button.px_2()),
                )
            })
    }

    fn progress(&self, compact: bool, ready: usize, cx: &Context<Self>) -> Div {
        let total = self.board.total_pairs();
        let matched = self.board.moves().len();
        let palette = theme::colors(cx);
        let title = match self.table {
            Table::Daily(_) => Some(format!("{} · Daily stack", self.board.shape().name())),
            Table::Challenge(_) => Some(format!("{} · Challenge", self.board.shape().name())),
            Table::Layout(_) => None,
        };
        let mut ready_label = match ready {
            1 => "1 match ready".to_owned(),
            _ => format!("{ready} matches ready"),
        };
        if matches!(self.table, Table::Daily(_)) && self.cleared {
            ready_label.push_str(" · Daily cleared");
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
            .when(cfg!(test), |board| board.debug_selector(|| "cube-table".into()))
            .role(Role::Group)
            .aria_label("Cube board")
            .aria_description("Drag to orbit the board. Arrow keys browse visible free cubes. Enter selects. Q and E turn. Shift plus Up or Down tilts. R resets the view. H hints. U undoes.")
            .track_focus(&self.board_focus)
            .tab_index(0)
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .h(height)
            .relative()
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x4c5c47))
            .focus_visible(|style| style.border_color(rgb(theme::GOLD)))
            .overflow_hidden()
            .bg(linear_gradient(145., linear_color_stop(rgb(0x293e33), 0.), linear_color_stop(rgb(0x101f19), 1.)))
            .child(canvas(
                move |bounds, window, cx| {
                    let view = entity.read(cx);
                    let scene = Scene::new(&view.board, bounds, view.orientation);
                    let state = TileStates {
                        selected: view.board.selected(),
                        hinted: view.hinted,
                        keyed: view.keyed.filter(|_| view.board_focus.is_focused(window)),
                        fading: view.fading.filter(|_| motion::enabled(cx)),
                        pressed: view.press.filter(|press| !press.moved).and_then(|press| press.cube),
                    };
                    let press = view.press;
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    (scene, state, hitbox, press)
                },
                move |_, (scene, state, hitbox, press), window, _| {
                    let turning = press.is_some_and(|press| press.moved);
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
                        if event.dragging() || press.is_some() {
                            let _ = move_entity.update(cx, |view, cx| {
                                if let Some(press) = &mut view.press {
                                    if event.dragging() {
                                        let (yaw, pitch) = press.advance(event.position);
                                        if yaw != 0. || pitch != 0. {
                                            view.orientation = view.orientation.offset(yaw, pitch);
                                            cx.notify();
                                        }
                                    } else {
                                        if press.moved {
                                            view.save();
                                        }
                                        view.press = None;
                                        cx.notify();
                                    }
                                }
                            });
                        }
                        if turning {
                            return;
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
                                let (yaw, pitch) = press.advance(event.position);
                                if press.moved {
                                    view.orientation = view.orientation.offset(yaw, pitch);
                                    view.save();
                                } else if let Some(id) = press.cube
                                    && hitbox.is_hovered(window)
                                    && scene.hit(event.position).is_some_and(|cube| cube.id == id)
                                {
                                    view.choose(id, cx);
                                }
                                cx.notify();
                            }
                        });
                    });
                },
            ).size_full())
    }

    fn camera_tools(&self, narrow: bool, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(if narrow { 5. } else { 8. }))
            .child(
                button(
                    "cubes-turn-left",
                    "Turn left",
                    true,
                    None,
                    |view, _, _| view.turn(-90.),
                    cx,
                )
                .when(narrow, |button| button.px_2()),
            )
            .child(
                button(
                    "cubes-reset-view",
                    "Reset view",
                    self.orientation != Orbit::default(),
                    None,
                    |view, _, _| view.reset_camera(),
                    cx,
                )
                .when(narrow, |button| button.px_2()),
            )
            .child(
                button(
                    "cubes-turn-right",
                    "Turn right",
                    true,
                    None,
                    |view, _, _| view.turn(90.),
                    cx,
                )
                .when(narrow, |button| button.px_2()),
            )
    }

    fn tools(&self, narrow: bool, cx: &Context<Self>) -> Div {
        let can_undo = !self.board.moves().is_empty();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .px_1()
            .child(self.camera_tools(narrow, cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(button(
                        "cubes-hint",
                        "Hint",
                        true,
                        None,
                        |view, _, cx| view.hint(false, cx),
                        cx,
                    ))
                    .child(button(
                        "cubes-undo",
                        "Undo",
                        can_undo,
                        None,
                        |view, _, cx| view.undo(cx),
                        cx,
                    ))
                    .child(button(
                        "cubes-replay",
                        "Restart",
                        can_undo || self.board.selected().is_some(),
                        None,
                        |view, _, _| view.restart(),
                        cx,
                    )),
            )
    }

    fn round_outcome(&self, ready: usize) -> Option<Outcome> {
        if self.board.remaining() == 0 {
            Some(Outcome::Won)
        } else if ready == 0 {
            Some(Outcome::Lost)
        } else {
            None
        }
    }

    fn result(&self, outcome: Outcome, cx: &Context<Self>) -> gpui::AnyElement {
        let matched = self.board.moves().len();
        let can_undo = matched > 0;
        let layout = self.board.shape().name();
        let (title, detail) = if outcome == Outcome::Won {
            let title = match self.table {
                Table::Layout(_) => "Stack cleared",
                Table::Daily(_) => "Daily stack cleared",
                Table::Challenge(_) => "Challenge cleared",
            };
            (title, format!("Matched all {matched} pairs in {layout}."))
        } else {
            let total = self.board.total_pairs();
            let recovery = if can_undo {
                "Undo to try a different pair order."
            } else {
                "Replay the deal to try a different pair order."
            };
            (
                "No free pairs",
                format!("{matched} of {total} pairs matched in {layout}. {recovery}"),
            )
        };
        let undo = || {
            result_button(
                "cubes-result-undo",
                "Undo",
                outcome == Outcome::Lost,
                Self::undo,
                cx,
            )
            .aria_keyshortcuts("U")
        };
        let actions = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .when(outcome == Outcome::Lost && can_undo, |row| {
                row.child(undo())
            })
            .child(result_button(
                "cubes-result-replay",
                "Replay",
                outcome == Outcome::Won,
                |view, _| view.restart(),
                cx,
            ))
            .when(matches!(self.table, Table::Layout(_)), |row| {
                row.child(result_button(
                    "cubes-result-new",
                    "New deal",
                    false,
                    |view, _| view.redeal(),
                    cx,
                ))
            })
            .when(outcome == Outcome::Won && can_undo, |row| row.child(undo()))
            .when(
                outcome == Outcome::Won && self.orientation != Orbit::default(),
                |row| {
                    row.child(result_button(
                        "cubes-reset-view",
                        "Reset view",
                        false,
                        |view, _| view.reset_camera(),
                        cx,
                    ))
                },
            );
        round_result(cx, "cubes-result", outcome, title, detail, actions)
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::colors(cx);
        let narrow = window.viewport_size().width < px(850.);
        // Pairs can be free under the rules even when the camera hides one of them.
        let ready = self.board.available_pairs().len();
        let outcome = self.round_outcome(ready);
        let in_play = outcome.is_none();
        let controls_height = match (narrow, self.table) {
            (true, Table::Challenge(_)) => 218.,
            (true, Table::Daily(_)) => 264.,
            (true, Table::Layout(_)) => 256.,
            (false, Table::Challenge(_)) => 240.,
            _ => 286.,
        };
        let height = window.viewport_size().height - px(controls_height);
        let height = match outcome {
            Some(Outcome::Won) => (height - px(55.)).clamp(px(168.), px(290.)),
            Some(Outcome::Lost) => (height - px(96.)).max(px(168.)),
            _ => height.max(px(248.)),
        };
        let keyboard = self.board_focus.is_focused(window) && window.last_input_was_keyboard();
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .when(!matches!(self.table, Table::Challenge(_)), |view| {
                view.child(self.layouts(narrow, in_play, cx))
            })
            .child(
                theme::panel(cx)
                    .w_full()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .when(narrow, |panel| panel.p_2().gap_2())
                    .when(in_play, |panel| panel.child(self.progress(narrow, ready, cx)))
                    .child(self.playfield(height, cx))
                    .when(in_play, |panel| {
                        let status = self.selection_status(keyboard);
                        panel.child(self.tools(narrow, cx)).child(
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
                                .child(div().child(if keyboard {
                                    "Arrows browse · Enter selects · Q / E turn · Shift ↑ / ↓ tilts · R resets"
                                } else {
                                    "Drag to turn and tilt"
                                })),
                        )
                    }),
            )
            .when_some(outcome, |root, outcome| {
                root.child(
                    div().w_full().flex().justify_center().child(self.result(outcome, cx)),
                )
            })
            .when(outcome == Some(Outcome::Lost), |root| {
                root.child(div().flex().justify_center().child(self.camera_tools(narrow, cx)))
            })
    }
}

fn result_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    action: impl Fn(&mut GameView, &Context<GameView>) + 'static,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    button(
        id,
        label,
        true,
        None,
        move |view, window, cx| {
            action(view, cx);
            view.board_focus.focus(window, cx);
        },
        cx,
    )
    .when(primary, |button| {
        let palette = theme::colors(cx);
        button
            .border_color(rgb(palette.gold_text))
            .font_weight(FontWeight::BOLD)
            .focus_visible(|style| style.bg(rgb(palette.hover)).border_color(rgb(palette.ink)))
    })
}

fn button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    active: Option<bool>,
    action: impl Fn(&mut GameView, &mut Window, &mut Context<GameView>) + 'static,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    let colors = theme::colors(cx);
    let filled = active == Some(true);
    div()
        .id(id)
        .when(cfg!(test), |button| button.debug_selector(|| id.into()))
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
                .on_click(cx.listener(move |view, _, window, cx| {
                    action(view, window, cx);
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
    last: Point<Pixels>,
    cube: Option<usize>,
    moved: bool,
}

impl Press {
    fn new(origin: Point<Pixels>, cube: Option<usize>) -> Self {
        Self {
            origin,
            last: origin,
            cube,
            moved: false,
        }
    }

    fn advance(&mut self, pointer: Point<Pixels>) -> (f32, f32) {
        let travel = pointer - self.origin;
        self.moved |= f32::from(travel.x).hypot(f32::from(travel.y)) >= 6.;
        if !self.moved {
            return (0., 0.);
        }
        let delta = pointer - self.last;
        self.last = pointer;
        (f32::from(delta.x) * 0.58, f32::from(delta.y) * 0.42)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_jitter_can_click_but_dragging_never_clicks_on_release() {
        let start = point(px(90.), px(90.));
        let mut press = Press::new(start, Some(4));
        assert_eq!(press.advance(point(px(94.), px(92.))), (0., 0.));
        assert!(!press.moved);
        let (yaw, pitch) = press.advance(point(px(102.), px(98.)));
        assert!(yaw > 0. && yaw < 10.);
        assert!(pitch > 0. && pitch < 5.);
        assert_eq!(press.advance(point(px(102.), px(98.))), (0., 0.));
        assert_eq!(press.advance(start), (-yaw, -pitch));
        assert!(press.moved);

        let mut press = Press::new(start, Some(4));
        let (yaw, pitch) = press.advance(point(px(90.), px(117.)));
        assert_eq!(yaw, 0.);
        assert!(pitch > 10.);
        assert!(press.moved);
    }

    #[test]
    fn saved_moves_replay_and_previous_daily_does_not_carry_over() {
        let yesterday = Table::Daily(1234);
        let (mut board, seed, attempt, _, _) = load_round(yesterday, None);
        let pair = board.hint().unwrap();
        assert!(board.remove_pair(pair));
        let save = RoundProgress {
            seed,
            attempt,
            moves: vec![pair],
            cleared: true,
            orientation: Orbit::new(117., 59.),
        };
        let (restored, resumed_seed, resumed_attempt, cleared, orientation) =
            load_round(yesterday, Some(&save));
        assert_eq!(resumed_seed, seed);
        assert!(resumed_attempt == attempt);
        assert!(cleared);
        assert!(restored.cubes().eq(board.cubes()));
        assert_eq!(orientation, save.orientation);

        let (today, todays_seed, todays_attempt, cleared, orientation) =
            load_round(Table::Daily(1235), Some(&save));
        assert_ne!(todays_seed, seed);
        assert!(todays_attempt != attempt);
        assert_ne!(today.shape(), board.shape());
        assert!(!cleared);
        assert!(today.moves().is_empty());
        assert_eq!(orientation, Orbit::default());
    }

    #[test]
    fn legacy_round_replays_with_the_opening_angle() {
        let table = Table::Layout(Layout::Terrace);
        let seed = 42;
        let pair = table.deal(seed).hint().unwrap();
        let mut legacy = serde_json::json!({
            "version": 1,
            "active": "terrace",
            "rounds": { "terrace": { "seed": seed, "moves": [pair], "cleared": false } }
        });
        let save: Save = serde_json::from_value(legacy.clone()).unwrap();
        let (restored, _, _, _, orientation) = load_round(table, save.rounds.get(table.key()));
        assert_eq!(restored.moves(), &[pair]);
        assert_eq!(orientation, Orbit::default());

        legacy["rounds"]["terrace"]["orientation"] =
            serde_json::json!({ "yaw": 731., "pitch": 120. });
        let save: Save = serde_json::from_value(legacy).unwrap();
        let (restored, _, _, _, orientation) = load_round(table, save.rounds.get(table.key()));
        assert_eq!(restored.moves(), &[pair]);
        assert_eq!(orientation, Orbit::new(11., 82.));
    }

    #[test]
    fn corrupt_replay_cannot_partially_remove_cubes() {
        let table = Table::Daily(1234);
        let (board, seed, attempt, _, _) = load_round(table, None);
        let pair = board.hint().unwrap();
        let corrupt = RoundProgress {
            seed,
            attempt,
            moves: vec![pair, pair],
            cleared: true,
            orientation: Orbit::new(270., 60.),
        };
        let (restored, _, restored_attempt, cleared, orientation) =
            load_round(table, Some(&corrupt));
        assert!(restored_attempt != attempt);
        assert_eq!(restored.remaining(), board.remaining());
        assert!(restored.moves().is_empty());
        assert!(!cleared);
        assert_eq!(orientation, Orbit::default());
    }

    #[cfg(debug_assertions)]
    #[gpui::test]
    fn daily_clear_can_be_undone_and_replayed_from_the_keyboard(cx: &mut gpui::TestAppContext) {
        use gpui::{KeyUpEvent, Keystroke, VisualTestContext, size};

        cx.update(|cx| cx.set_reduce_motion(true));
        let launch = GameLaunch {
            game: corner_table_api::GameId("cubes-daily-result-test".into()),
            entry: GameEntry::Classic,
        };
        let table = daily();
        let (mut board, seed, attempt, _, _) = load_round(table, None);
        while board.remaining() > 2 {
            assert!(board.remove_pair(board.hint().unwrap()));
        }
        let total = board.total_pairs();
        let window = cx.open_window(size(px(760.), px(540.)), |window, cx| {
            let mut view = GameView::new(&launch, cx);
            view.table = table;
            view.seed = seed;
            view.attempt = attempt;
            view.board = board;
            view.cleared = false;
            view.orientation = Orbit::new(97., 72.);
            view.board_focus.focus(window, cx);
            view
        });
        cx.run_until_parked();
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        assert!(visual.debug_bounds("cubes-hint").is_some());
        visual.simulate_keystrokes("h enter h enter");
        let orbit = window
            .read_with(cx, |view, _| {
                assert_eq!(view.board.remaining(), 0);
                assert_eq!(view.board.moves().len(), total);
                assert!(view.cleared);
                assert!(view.fading.is_none());
                view.orientation
            })
            .unwrap();
        assert!(visual.debug_bounds("cubes-hint").is_none());
        assert!(visual.debug_bounds("cubes-result-undo").is_some());
        assert!(visual.debug_bounds("cubes-result-new").is_none());

        visual.simulate_keystrokes("u");
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.board.remaining(), 2);
                assert!(view.attempt == attempt);
                assert!(view.cleared);
            })
            .unwrap();
        assert!(visual.debug_bounds("cubes-result-replay").is_none());
        visual.simulate_keystrokes("h enter h enter");
        window
            .update(cx, |_, window, cx| window.focus_next(cx))
            .unwrap();
        visual.simulate_keystrokes("enter");
        visual.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
        });
        window
            .update(cx, |view, window, _| {
                assert!(view.board_focus.is_focused(window));
                assert_eq!(view.board.remaining(), total * 2);
                assert_eq!(view.table.fixed_seed(), Some(seed));
                assert_eq!(view.seed, seed);
                assert_eq!(view.orientation, orbit);
                assert!(view.attempt != attempt);
                assert!(view.cleared);
            })
            .unwrap();
        assert!(visual.debug_bounds("cubes-hint").is_some());
    }

    #[cfg(debug_assertions)]
    #[gpui::test]
    fn dragging_and_picking_work_in_wide_and_compact_windows(cx: &mut gpui::TestAppContext) {
        use gpui::{Modifiers, VisualTestContext, size};

        for (width, height, layout) in [
            (1080., 760., Layout::Gatehouse),
            (632., 540., Layout::Terrace),
        ] {
            let launch = GameLaunch {
                game: corner_table_api::GameId(format!("cubes-orbit-test-{width}")),
                entry: GameEntry::Classic,
            };
            let window = cx.open_window(size(px(width), px(height)), |_, cx| {
                let mut view = GameView::new(&launch, cx);
                view.table = Table::Layout(layout);
                view.seed = 42;
                view.board = view.table.deal(view.seed);
                view.orientation = Orbit::default();
                view
            });
            cx.run_until_parked();
            let mut visual = VisualTestContext::from_window(window.into(), cx);
            let bounds = visual.debug_bounds("cube-table").unwrap().inset(px(1.));
            let free_point = |view: &GameView| {
                let scene = Scene::new(&view.board, bounds, view.orientation);
                view.board
                    .cubes()
                    .filter_map(|(id, _)| view.board.is_free(id).then_some(id))
                    .find_map(|id| scene.visible_cube_point(id).map(|point| (id, point)))
                    .unwrap()
            };
            let (_, start) = window.read_with(cx, |view, _| free_point(view)).unwrap();
            let pointer = Modifiers::none();
            let offset = point(px(28.), px(31.));
            visual.simulate_mouse_move(start, None, pointer);
            visual.simulate_mouse_down(start, MouseButton::Left, pointer);
            visual.simulate_mouse_move(start + offset, MouseButton::Left, pointer);
            let orbit = Orbit::default().offset(28. * 0.58, 31. * 0.42);
            window
                .read_with(cx, |view, _| {
                    assert_eq!(view.orientation, orbit);
                    assert!(view.press.unwrap().moved);
                    assert!(view.board.selected().is_none());
                })
                .unwrap();
            visual.simulate_mouse_up(start + offset, MouseButton::Left, pointer);
            assert_eq!(
                Session::new(&launch).load::<Save>().unwrap().rounds[layout.id()].orientation,
                orbit
            );

            let (id, position) = window.read_with(cx, |view, _| free_point(view)).unwrap();
            visual.simulate_click(position, pointer);
            window
                .read_with(cx, |view, _| {
                    assert_eq!(view.board.selected(), Some(id));
                    assert!(view.press.is_none());
                })
                .unwrap();

            visual.simulate_keystrokes("escape q shift-up right");
            window
                .read_with(cx, |view, _| {
                    assert_eq!(view.orientation, orbit.offset(-90., 7.));
                    assert!(view.keyed.is_some_and(|id| view.board.is_free(id)));
                })
                .unwrap();
            visual.simulate_keystrokes("h enter h enter");
            window
                .read_with(cx, |view, _| {
                    assert_eq!(view.board.moves().len(), 1);
                })
                .unwrap();
            visual.simulate_keystrokes("u");
            window
                .read_with(cx, |view, _| assert!(view.board.moves().is_empty()))
                .unwrap();

            let reset = visual.debug_bounds("cubes-reset-view").unwrap().center();
            visual.simulate_click(reset, pointer);
            window
                .read_with(cx, |view, _| {
                    assert_eq!(view.orientation, Orbit::default());
                })
                .unwrap();
        }
    }
}
