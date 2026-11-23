mod art;
mod board;
mod chart;
mod record;

use std::time::{Duration, Instant};

use corner_table_mine_trails::{Expedition, Position, Room, RoomStatus, Tile};
use gpui::{
    Context, Div, ElementId, FocusHandle, FontWeight, KeyDownEvent, Render, Role, SharedString,
    Stateful, Window, div, prelude::*, px, rgb,
};

use crate::{
    audio::{self, Effect},
    games::{GameEntry, GameLaunch, round_result},
    storage::{Outcome, ResultMetric, Session},
    theme as t,
};

use art::Mark;
use record::{Move, Record};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Tool {
    #[default]
    Dig,
    Flag,
}

#[derive(Clone, Copy)]
enum Reset {
    Restart,
    New,
}

impl Reset {
    fn seed(self, current: u64) -> u64 {
        match self {
            Self::Restart => current,
            Self::New => current.wrapping_add(1),
        }
    }
}

struct Strike {
    started: Instant,
    opened: u64,
    hit: Option<Position>,
}

impl Strike {
    fn progress(&self) -> f32 {
        let duration = Duration::from_millis(if self.hit.is_some() { 430 } else { 320 });
        self.started.elapsed().as_secs_f32() / duration.as_secs_f32()
    }
}

pub struct GameView {
    expedition: Expedition,
    session: Session,
    record: Record,
    challenge: bool,
    tool: Tool,
    board_focus: FocusHandle,
    cursor: Position,
    keyboard_cursor: bool,
    hovered_clue: Option<Position>,
    pinned_clue: Option<Position>,
    strike: Option<Strike>,
    new_best: bool,
    show_final_board: bool,
    pending_reset: Option<Reset>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let fixed_seed = match &launch.entry {
            GameEntry::Challenge(id) => {
                Some(id.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
                }))
            }
            _ => None,
        };
        let session = Session::new(launch);
        let (record, expedition) = session
            .load::<Record>()
            .and_then(|record| {
                let expedition = record.replay(fixed_seed)?;
                Some((record, expedition))
            })
            .unwrap_or_else(|| {
                let seed = fixed_seed.unwrap_or(0);
                (Record::new(seed), Expedition::new(seed))
            });
        Self {
            cursor: expedition.entry(),
            expedition,
            session,
            record,
            challenge: fixed_seed.is_some(),
            tool: Tool::Dig,
            board_focus: cx.focus_handle(),
            keyboard_cursor: true,
            hovered_clue: None,
            pinned_clue: None,
            strike: None,
            new_best: false,
            show_final_board: false,
            pending_reset: None,
        }
    }

    fn apply(&mut self, action: Move, cx: &mut Context<Self>) {
        let excavation = matches!(action, Move::Dig(..) | Move::Sweep(..));
        let before = (excavation && crate::motion::enabled(cx)).then(|| self.opened_tiles());
        let room = self.expedition.current_room();
        let was_complete = self.expedition.is_complete();
        if !action.apply(&mut self.expedition) {
            if let Move::Dig(x, y) | Move::Flag(x, y) = action
                && self.expedition.status(room) == RoomStatus::Exploring
                && matches!(
                    self.expedition.tile((usize::from(x), usize::from(y))),
                    Some(Tile::Covered { .. } | Tile::Flagged)
                )
            {
                audio::play(cx, Effect::InvalidMove);
            }
            return;
        }

        if let Some(before) = before {
            let opened = self.opened_tiles() & !before;
            let hit = if self.expedition.status(room) == RoomStatus::Collapsed {
                self.positions().find(|&position| {
                    self.expedition.tile(position) == Some(Tile::Mine { hit: true })
                })
            } else {
                None
            };
            if opened != 0 || hit.is_some() {
                self.strike = Some(Strike {
                    started: Instant::now(),
                    opened,
                    hit,
                });
            }
        }
        if matches!(action, Move::Travel(_) | Move::Retry) {
            self.reset_tools();
        }

        self.pending_reset = None;
        let previous_best = self.record.best;
        self.record.remember(action, &self.expedition);
        self.new_best |= self.record.best != previous_best;
        self.session.save(&self.record);
        let effect = if !was_complete && self.expedition.is_complete() {
            self.session.record_result(
                self.record.attempt,
                if self.challenge {
                    "Challenge"
                } else {
                    "Expedition"
                },
                Outcome::Won,
                ResultMetric::Moves(self.record.move_count()),
            );
            Effect::Complete
        } else if excavation && self.expedition.status(room) == RoomStatus::Surveyed {
            Effect::Clear
        } else {
            Effect::Move
        };
        audio::play(cx, effect);
        cx.notify();
    }

    fn travel(&mut self, room: Room, window: &mut Window, cx: &mut Context<Self>) {
        self.apply(Move::Travel(room as u8), cx);
        if room != Room::DeepVault || !self.expedition.is_complete() {
            self.board_focus.focus(window, cx);
        }
    }

    fn reset(&mut self, reset: Reset, window: &mut Window, cx: &mut Context<Self>) {
        self.expedition = self.record.restart(reset.seed(self.record.seed));
        self.reset_tools();
        self.new_best = false;
        self.pending_reset = None;
        self.session.save(&self.record);
        self.board_focus.focus(window, cx);
        cx.notify();
    }

    fn reset_tools(&mut self) {
        self.cursor = self.expedition.entry();
        self.keyboard_cursor = true;
        self.tool = Tool::Dig;
        self.hovered_clue = None;
        self.pinned_clue = None;
        self.strike = None;
        self.show_final_board = false;
    }

    fn positions(&self) -> impl Iterator<Item = Position> + use<> {
        let side = self.expedition.side();
        (0..side).flat_map(move |y| (0..side).map(move |x| (x, y)))
    }

    fn opened_tiles(&self) -> u64 {
        self.positions()
            .enumerate()
            .filter(|&(_, position)| {
                matches!(self.expedition.tile(position), Some(Tile::Open { .. }))
            })
            .fold(0, |mask, (i, _)| mask | (1 << i))
    }

    fn activate_tile(&mut self, at: Position, cx: &mut Context<Self>) {
        let (x, y) = (at.0 as u8, at.1 as u8);
        if matches!(self.expedition.tile(at), Some(Tile::Open { .. })) {
            self.pinned_clue = if self.pinned_clue == Some(at) {
                None
            } else {
                Some(at)
            };
            cx.notify();
        } else {
            self.apply(
                match self.tool {
                    Tool::Dig => Move::Dig(x, y),
                    Tool::Flag => Move::Flag(x, y),
                },
                cx,
            );
        }
    }

    fn keyboard(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keys = &event.keystroke;
        if keys.modifiers.platform || keys.modifiers.control || keys.modifiers.alt {
            return;
        }
        self.keyboard_cursor = true;
        self.hovered_clue = None;
        match keys.key.as_str() {
            "left" => self.move_cursor(-1, 0),
            "right" => self.move_cursor(1, 0),
            "up" => self.move_cursor(0, -1),
            "down" => self.move_cursor(0, 1),
            "enter" if !event.is_held => self.activate_tile(self.cursor, cx),
            "space" if !event.is_held => {
                self.apply(Move::Flag(self.cursor.0 as u8, self.cursor.1 as u8), cx);
            }
            "d" => self.tool = Tool::Dig,
            "f" => self.tool = Tool::Flag,
            "escape" => self.pinned_clue = None,
            _ => return,
        }
        cx.notify();
        cx.stop_propagation();
    }

    fn move_cursor(&mut self, dx: isize, dy: isize) {
        let (mut x, mut y) = self.cursor;
        loop {
            let Some(next) = x.checked_add_signed(dx).zip(y.checked_add_signed(dy)) else {
                return;
            };
            (x, y) = next;
            match self.expedition.tile(next) {
                Some(Tile::Bedrock) => continue,
                Some(_) => self.cursor = next,
                None => (),
            }
            return;
        }
    }

    fn objective(room: Room) -> &'static str {
        match room {
            Room::Entrance => "Dig from the lit edge to reach both diamonds.",
            Room::AmberDrift | Room::SlateDrift => {
                "Recover both diamonds to open the way to the Floodgate."
            }
            Room::Floodgate => "Recover three diamonds to open the Deep Vault.",
            Room::DeepVault => "Recover three diamonds to finish the expedition.",
        }
    }

    fn room_header(&self, room: Room, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let (found, total) = self.expedition.diamonds(room);
        div()
            .flex()
            .items_start()
            .justify_between()
            .gap_4()
            .px_4()
            .pt_4()
            .pb_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(t::display_type(room.name(), 28.))
                    .when(
                        self.strike.is_some()
                            || self.expedition.status(room) == RoomStatus::Exploring,
                        |heading| {
                            heading.child(
                                div()
                                    .text_size(px(13.))
                                    .line_height(px(18.))
                                    .text_color(rgb(colors.muted))
                                    .child(Self::objective(room)),
                            )
                        },
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .bg(rgb(colors.paper))
                    .px_2()
                    .py_1()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(art::symbol(Mark::Diamond, 15., colors.gold_text))
                    .child(format!("{found} / {total} diamonds")),
            )
    }

    fn tools(&self, room: Room, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let bar = div()
            .min_h(px(51.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .px_4()
            .pb_3();
        match self.expedition.status(room) {
            RoomStatus::Exploring => {
                let tools = [
                    (Tool::Dig, "mine-dig", "Dig"),
                    (Tool::Flag, "mine-flag", "Flag"),
                ];
                bar.children(tools.map(|(tool, id, label)| {
                    button(id, label, self.tool == tool, colors)
                        .aria_toggled((self.tool == tool).into())
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.tool = tool;
                            view.board_focus.focus(window, cx);
                            cx.notify();
                        }))
                }))
                .child(
                    div()
                        .ml_auto()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_color(rgb(colors.muted))
                        .text_size(px(12.))
                        .child(art::symbol(Mark::Flag, 13., colors.gold_text))
                        .child(format!(
                            "{} / {} flagged",
                            self.expedition.flag_count(),
                            self.expedition.mine_count()
                        )),
                )
            }
            _ if self.strike.is_some() => bar,
            RoomStatus::Collapsed => {
                let (found, total) = self.expedition.diamonds(room);
                bar.child(round_result(
                    cx,
                    "mine-trails-room-result",
                    Outcome::Lost,
                    "Chamber collapsed",
                    format!("{found}/{total} diamonds found here · {} on this route. Retry this chamber to reopen it.",
                        cave_in_count(self.expedition.cave_ins())),
                    button("mine-retry", "Retry chamber", true, colors).on_click(cx.listener(
                        |view, _, window, cx| {
                            view.apply(Move::Retry, cx);
                            view.board_focus.focus(window, cx);
                        },
                    )),
                ))
            }
            RoomStatus::Surveyed if room == Room::DeepVault => bar.child(
                button("mine-show-summary", "Show expedition log", true, colors).on_click(
                    cx.listener(|view, _, _, cx| {
                        view.show_final_board = false;
                        cx.notify();
                    }),
                ),
            ),
            RoomStatus::Surveyed => {
                let (exits, route_hint): (&[Room], _) = match room {
                    Room::Entrance => (
                        &[Room::AmberDrift, Room::SlateDrift],
                        "Amber has fewer mines. Slate has tighter passages. Both lead to the Floodgate.",
                    ),
                    Room::AmberDrift | Room::SlateDrift => (
                        &[Room::Floodgate],
                        "The Floodgate is open. The other drift is an optional detour.",
                    ),
                    Room::Floodgate => (&[Room::DeepVault], "The route to the Deep Vault is open."),
                    Room::DeepVault => (&[], "The final seam is charted."),
                };
                let actions =
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .children(exits.iter().map(|&destination| {
                            button(
                                ("mine-next", destination as usize),
                                format!("Enter {}", destination.name()),
                                true,
                                colors,
                            )
                            .on_click(cx.listener(
                                move |view, _, window, cx| {
                                    view.travel(destination, window, cx);
                                },
                            ))
                        }));
                let (found, _) = self.expedition.diamonds(room);
                bar.child(round_result(
                    cx,
                    "mine-trails-room-result",
                    Outcome::Won,
                    format!("{} surveyed", room.name()),
                    format!("{found} diamonds recovered. {route_hint}"),
                    actions,
                ))
            }
        }
    }

    fn field_notes(&self, room: Room, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let notes = match room {
            Room::Entrance => {
                "Lit tiles are within reach. A number counts mines in all eight neighboring tiles, including diagonals."
            }
            Room::AmberDrift => {
                "Amber is the gentler route. Dig around the outcrops to reach the diamonds at opposite ends."
            }
            Room::SlateDrift => {
                "A central pillar splits the route. Follow both sides and use clues to mark the denser mine seams."
            }
            Room::Floodgate => {
                "Two openings touch different banks. Pick up the excavation from either one and work around the flooded rock."
            }
            Room::DeepVault => {
                "Subtract nearby flags before comparing clues. If one set of unknown tiles contains the other, equal mine counts make the extra tiles safe. One extra mine and one extra tile means that tile holds the mine."
            }
        };
        t::panel(cx)
            .w_full()
            .overflow_hidden()
            .when(room == Room::Entrance, |panel| {
                panel.child(div().h(px(120.)).w_full().child(art::painting(art::ENTRANCE)))
            })
            .child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .line_height(px(19.))
                    .text_size(px(13.))
                    .child(t::display_type("Field notes", 18.))
                    .child(notes)
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_2()
                            .text_color(rgb(colors.muted))
                            .child(div().pt_1().child(art::symbol(Mark::Diamond, 13., colors.gold_text)))
                            .child(div().flex_1().min_w_0().child("Diamonds are safe to dig.")),
                    )
                    .child(
                        div()
                            .border_t_1()
                            .border_color(rgb(colors.border))
                            .pt_2()
                            .text_size(px(12.))
                            .text_color(rgb(colors.muted))
                            .child("Right-click a tile to place or remove a flag.")
                            .child(div().mt_1().child("On the board, use the arrows to move, Enter to act, Space to flag, and D or F to switch tools.")),
                    ),
            )
    }

    fn summary(&self, height: f32, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let rooms = self.expedition.surveyed_count();
        let diamonds: usize = Room::ALL
            .into_iter()
            .map(|room| self.expedition.diamonds(room).0)
            .sum();
        let optional = [Room::AmberDrift, Room::SlateDrift]
            .into_iter()
            .find(|&room| self.expedition.status(room) != RoomStatus::Surveyed);
        let mut detail = format!(
            "{rooms} rooms surveyed · {diamonds} diamonds · {}",
            cave_in_count(self.expedition.cave_ins()),
        );
        if self.new_best
            && self.record.best.is_some_and(|best| {
                best.rooms == rooms && best.cave_ins == self.expedition.cave_ins()
            })
        {
            detail.push_str(" · New expedition best");
        }
        if let Some(room) = optional {
            detail.push_str(&format!(
                ". You can still survey {} from the map.",
                room.name()
            ));
        } else {
            detail.push_str(". Every chamber is charted.");
        }
        let actions = if let Some(reset) = self.pending_reset {
            self.reset_confirmation(reset, cx)
        } else {
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .when(!self.challenge, |actions| {
                    actions.child(button("mine-new", "New expedition", true, colors).on_click(
                        cx.listener(|view, _, window, cx| {
                            view.reset(Reset::New, window, cx);
                        }),
                    ))
                })
                .child(self.restart_route(self.challenge, cx))
                .child(
                    button("mine-final-board", "View final excavation", false, colors).on_click(
                        cx.listener(|view, _, window, cx| {
                            view.show_final_board = true;
                            view.board_focus.focus(window, cx);
                            cx.notify();
                        }),
                    ),
                )
        };
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(height.clamp(207., 365.)))
                    .bg(rgb(0x212d30))
                    .child(art::painting(art::VAULT)),
            )
            .child(div().p_4().child(round_result(
                cx,
                "mine-trails-expedition-result",
                Outcome::Won,
                "Expedition complete",
                detail,
                actions,
            )))
    }

    fn restart_route(&self, primary: bool, cx: &Context<Self>) -> Stateful<Div> {
        button("mine-restart", "Restart route", primary, t::colors(cx)).on_click(cx.listener(
            |view, _, _, cx| {
                view.pending_reset = Some(Reset::Restart);
                cx.notify();
            },
        ))
    }

    fn reset_confirmation(&self, reset: Reset, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let (message, label) = match reset {
            Reset::Restart => ("Restart this route from the Entrance?", "Restart"),
            Reset::New => ("Leave this route and start a new layout?", "Start new"),
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(colors.muted))
                    .text_size(px(12.))
                    .child(message),
            )
            .child(
                button("mine-restart-confirm", label, true, colors).on_click(
                    cx.listener(move |view, _, window, cx| view.reset(reset, window, cx)),
                ),
            )
            .child(
                button("mine-restart-cancel", "Cancel", false, colors).on_click(cx.listener(
                    |view, _, _, cx| {
                        view.pending_reset = None;
                        cx.notify();
                    },
                )),
            )
    }

    fn expedition_actions(&self, showing_summary: bool, cx: &Context<Self>) -> Div {
        if let Some(reset) = self.pending_reset
            && !showing_summary
        {
            return self.reset_confirmation(reset, cx);
        }
        let colors = t::colors(cx);
        let row = div().flex().flex_wrap().items_center().gap_2();
        row.when(!showing_summary && self.record.has_progress(), |row| {
            row.child(self.restart_route(false, cx))
        })
        .when(!showing_summary && !self.challenge, |row| {
            row.child(
                button("mine-new", "New expedition", false, colors).on_click(cx.listener(
                    |view, _, window, cx| {
                        if view.record.has_progress() && !view.expedition.is_complete() {
                            view.pending_reset = Some(Reset::New);
                            cx.notify();
                        } else {
                            view.reset(Reset::New, window, cx);
                        }
                    },
                )),
            )
        })
        .when_some(self.record.best, |row, best| {
            row.child(
                div()
                    .text_color(rgb(colors.muted))
                    .text_size(px(12.))
                    .child(format!(
                        "Local best · {} rooms · {}",
                        best.rooms,
                        cave_in_count(best.cave_ins),
                    )),
            )
        })
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(strike) = &self.strike {
            if crate::motion::enabled(cx) && strike.progress() < 1. {
                window.request_animation_frame();
            } else {
                self.strike = None;
            }
        }
        let room = self.expedition.current_room();
        let available = super::board_space(window);
        let wide = available >= 610.;
        let map_width = if wide {
            (available * 0.33).clamp(248., 334.)
        } else {
            available
        };
        let board_width = if wide {
            available - map_width - 16.
        } else {
            available
        };
        let height = f32::from(window.viewport_size().height);
        let side = self.expedition.side() as f32;
        let board_size = (board_width - 32.).min((height - 294.).max(310.));
        let tile_size = ((board_size - (side - 1.) * board::TILE_GAP) / side)
            .clamp(38., 78.)
            .floor();
        let finished = room == Room::DeepVault
            && self.expedition.is_complete()
            && self.strike.is_none()
            && !self.show_final_board;

        div()
            .flex()
            .w_full()
            .items_start()
            .gap_4()
            .when(!wide, |layout| layout.flex_col())
            .child(
                t::panel(cx)
                    .w_full()
                    .min_w_0()
                    .when(wide, |panel| panel.flex_1())
                    .overflow_hidden()
                    .child(self.room_header(room, cx))
                    .when(finished, |panel| {
                        panel.child(self.summary(height - 430., cx))
                    })
                    .when(!finished, |panel| {
                        panel
                            .child(self.tools(room, cx))
                            .child(self.board(room, tile_size, window, cx))
                            .child(self.board_note(room, cx))
                    }),
            )
            .child(
                div()
                    .w_full()
                    .when(wide, |side| side.w(px(map_width)).flex_none())
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(self.tunnel_map(map_width, cx))
                    .when(!finished, |side| side.child(self.field_notes(room, cx)))
                    .child(self.expedition_actions(finished, cx)),
            )
    }
}

fn cave_in_count(count: u32) -> String {
    format!(
        "{count} {}",
        if count == 1 { "cave-in" } else { "cave-ins" }
    )
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    colors: t::Palette,
) -> Stateful<Div> {
    let id = id.into();
    let label = label.into();
    div()
        .id(id.clone())
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tab_index(0)
        .cursor_pointer()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if primary { 0x9c6237 } else { colors.border }))
        .bg(rgb(if primary { 0x754526 } else { colors.surface }))
        .text_color(rgb(if primary { 0xffebcf } else { colors.ink }))
        .hover(move |style| style.bg(rgb(if primary { 0x8b4f29 } else { colors.hover })))
        .active(move |style| style.bg(rgb(if primary { 0x623a22 } else { colors.paper })))
        .focus_visible(move |style| {
            style.border_color(rgb(if primary { 0xffd597 } else { colors.gold_text }))
        })
        .px_3()
        .min_h(px(34.))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use corner_table_api::GameId;
    use gpui::{Modifiers, TestAppContext, VisualTestContext, size};

    use super::*;

    fn survey(view: &mut GameView, cx: &mut Context<GameView>) {
        let room = view.expedition.current_room();
        while view.expedition.status(room) == RoomStatus::Exploring {
            let (x, y) = view
                .positions()
                .find(|&at| {
                    let mut probe = view.expedition.clone();
                    probe.dig(at) && probe.status(room) != RoomStatus::Collapsed
                })
                .expect("a safe dig remains");
            view.apply(Move::Dig(x as u8, y as u8), cx);
        }
    }

    #[gpui::test]
    fn chamber_retry_and_vault_log_follow_excavation_effects(cx: &mut TestAppContext) {
        let launch = GameLaunch {
            game: GameId("mine-trails-result-test".into()),
            entry: GameEntry::Classic,
        };
        let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
            cx.set_reduce_motion(true);
            let mut view = GameView::new(&launch, cx);
            view.expedition = Expedition::new(42);
            view.record = Record::new(42);
            survey(&mut view, cx);
            view
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual.run_until_parked();
        let amber = visual.debug_bounds("mine-next-1").unwrap();
        visual.simulate_click(amber.center(), Modifiers::none());
        let attempt = window
            .update(&mut visual.cx, |view, _, cx| {
                assert_eq!(view.expedition.current_room(), Room::AmberDrift);
                view.cursor = view
                    .positions()
                    .find(|&at| {
                        let mut probe = view.expedition.clone();
                        probe.dig(at) && probe.status(Room::AmberDrift) == RoomStatus::Collapsed
                    })
                    .expect("a mine borders the entry");
                cx.set_reduce_motion(false);
                cx.notify();
                view.record.attempt
            })
            .unwrap();
        visual.simulate_keystrokes("enter");
        assert!(visual.debug_bounds("mine-retry").is_none());
        window
            .update(&mut visual.cx, |view, _, cx| {
                assert_eq!(
                    view.expedition.status(Room::AmberDrift),
                    RoomStatus::Collapsed
                );
                view.strike.as_mut().expect("cave-in strike").started -= Duration::from_secs(1);
                cx.notify();
            })
            .unwrap();
        visual.run_until_parked();
        let retry = visual.debug_bounds("mine-retry").unwrap();
        visual.simulate_click(retry.center(), Modifiers::none());
        window
            .update(&mut visual.cx, |view, _, cx| {
                assert!(view.record.attempt == attempt);
                assert_eq!(view.expedition.cave_ins(), 1);
                assert_eq!(view.expedition.status(Room::Entrance), RoomStatus::Surveyed);
                assert_eq!(
                    view.expedition.status(Room::AmberDrift),
                    RoomStatus::Exploring
                );
                survey(view, cx);
                for room in [Room::Floodgate, Room::DeepVault] {
                    view.apply(Move::Travel(room as u8), cx);
                    survey(view, cx);
                }
                assert!(view.expedition.is_complete());
                assert_eq!(view.expedition.surveyed_count(), 4);
                assert!(view.strike.is_some());
            })
            .unwrap();
        visual.run_until_parked();
        assert!(visual.debug_bounds("mine-final-board").is_none());
        window
            .update(&mut visual.cx, |view, _, cx| {
                view.strike.as_mut().unwrap().started -= Duration::from_secs(1);
                cx.notify();
            })
            .unwrap();
        visual.run_until_parked();
        let restart = visual.debug_bounds("mine-restart").unwrap();
        visual.simulate_click(restart.center(), Modifiers::none());
        let cancel = visual.debug_bounds("mine-restart-cancel").unwrap();
        visual.simulate_click(cancel.center(), Modifiers::none());
        let final_board = visual.debug_bounds("mine-final-board").unwrap();
        visual.simulate_click(final_board.center(), Modifiers::none());
        visual.run_until_parked();
        assert!(visual.debug_bounds("mine-show-summary").is_some());
    }
}
