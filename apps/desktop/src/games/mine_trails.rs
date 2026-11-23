mod art;
mod board;
mod chart;
mod record;

use corner_table_mine_trails::{Expedition, Position, Room, RoomStatus, Tile};
use gpui::{
    Context, Div, ElementId, FocusHandle, FontWeight, KeyDownEvent, Render, Role, SharedString,
    Stateful, Window, div, prelude::*, px, rgb,
};

use crate::{
    games::{GameEntry, GameLaunch},
    storage::Session,
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

#[derive(Default)]
struct Strike {
    generation: usize,
    opened: u64,
    hit: Option<Position>,
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
    strike: Strike,
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
            strike: Strike::default(),
            show_final_board: false,
            pending_reset: None,
        }
    }

    fn apply(&mut self, action: Move, cx: &mut Context<Self>) {
        let excavation = matches!(action, Move::Dig(..) | Move::Sweep(..));
        let before = excavation.then(|| self.opened_tiles());
        if !action.apply(&mut self.expedition) {
            return;
        }

        if let Some(before) = before {
            self.strike = Strike {
                generation: self.strike.generation.wrapping_add(1),
                opened: self.opened_tiles() & !before,
                hit: self.positions().find(|&position| {
                    self.expedition.tile(position) == Some(Tile::Mine { hit: true })
                }),
            };
        }
        if matches!(action, Move::Travel(_) | Move::Retry) {
            self.reset_tools();
        }

        self.pending_reset = None;
        self.record.remember(action, &self.expedition);
        self.session.save(&self.record);
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
        self.strike.opened = 0;
        self.strike.hit = None;
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
            "enter" => self.activate_tile(self.cursor, cx),
            "space" => self.apply(Move::Flag(self.cursor.0 as u8, self.cursor.1 as u8), cx),
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

    fn objective(&self, room: Room) -> &'static str {
        match self.expedition.status(room) {
            RoomStatus::Exploring => match room {
                Room::Entrance => "Dig from the lit edge to reach both diamonds.",
                Room::AmberDrift | Room::SlateDrift => {
                    "Recover both diamonds to open the way to the Floodgate."
                }
                Room::Floodgate => "Recover three diamonds to open the Deep Vault.",
                Room::DeepVault => "Recover three diamonds to finish the expedition.",
            },
            RoomStatus::Surveyed => match room {
                Room::Entrance => {
                    "Amber has fewer mines. Slate has tighter passages. Both lead to the Floodgate."
                }
                Room::AmberDrift | Room::SlateDrift => {
                    "The Floodgate is open. The other drift is an optional detour."
                }
                Room::Floodgate => "The route to the Deep Vault is open.",
                Room::DeepVault => "The final seam is charted.",
            },
            RoomStatus::Collapsed => "A mine caved in the chamber. Retry to reopen it.",
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
                    .child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .text_color(rgb(colors.muted))
                            .child(self.objective(room)),
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
            RoomStatus::Collapsed => bar.child(
                button("mine-retry", "Retry chamber", true, colors).on_click(cx.listener(
                    |view, _, window, cx| {
                        view.apply(Move::Retry, cx);
                        view.board_focus.focus(window, cx);
                    },
                )),
            ),
            RoomStatus::Surveyed if room == Room::DeepVault => bar.child(
                button("mine-show-summary", "Show expedition log", true, colors).on_click(
                    cx.listener(|view, _, _, cx| {
                        view.show_final_board = false;
                        cx.notify();
                    }),
                ),
            ),
            RoomStatus::Surveyed => {
                let exits: &[Room] = match room {
                    Room::Entrance => &[Room::AmberDrift, Room::SlateDrift],
                    Room::AmberDrift | Room::SlateDrift => &[Room::Floodgate],
                    Room::Floodgate => &[Room::DeepVault],
                    Room::DeepVault => &[],
                };
                bar.children(exits.iter().map(|&destination| {
                    button(
                        ("mine-next", destination as usize),
                        format!("Enter {}", destination.name()),
                        true,
                        colors,
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.travel(destination, window, cx);
                    }))
                }))
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
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(height.clamp(207., 365.)))
                    .bg(rgb(0x212d30))
                    .child(art::painting(art::VAULT)),
            )
            .child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(t::display_type("Expedition complete", 29.))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(art::symbol(Mark::Diamond, 16., colors.gold_text))
                            .child(format!(
                                "{rooms} rooms surveyed  ·  {diamonds} diamonds  ·  {}",
                                cave_in_count(self.expedition.cave_ins()),
                            )),
                    )
                    .child(
                        div()
                            .text_color(rgb(colors.muted))
                            .text_size(px(13.))
                            .child(optional.map_or_else(
                                || {
                                    "Every chamber is charted. The expedition log is complete."
                                        .to_owned()
                                },
                                |room| {
                                    format!("You can still survey {} from the map.", room.name())
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .when(!self.challenge, |actions| {
                                actions.child(
                                    button("mine-new", "New expedition", true, colors).on_click(
                                        cx.listener(|view, _, window, cx| {
                                            view.reset(Reset::New, window, cx);
                                        }),
                                    ),
                                )
                            })
                            .child(
                                button("mine-final-board", "View final excavation", false, colors)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.show_final_board = true;
                                        view.board_focus.focus(window, cx);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }

    fn expedition_actions(&self, showing_summary: bool, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let row = div().flex().flex_wrap().items_center().gap_2();
        if let Some(reset) = self.pending_reset {
            let (message, label) = match reset {
                Reset::Restart => ("Restart this route from the Entrance?", "Restart"),
                Reset::New => ("Leave this route and start a new layout?", "Start new"),
            };
            return row
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
                );
        }
        row.when(self.record.has_progress(), |row| {
            row.child(
                button("mine-restart", "Restart route", false, colors).on_click(cx.listener(
                    |view, _, _, cx| {
                        view.pending_reset = Some(Reset::Restart);
                        cx.notify();
                    },
                )),
            )
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
        let finished =
            room == Room::DeepVault && self.expedition.is_complete() && !self.show_final_board;

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
    let label = label.into();
    div()
        .id(id)
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
