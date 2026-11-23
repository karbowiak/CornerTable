use corner_table_mine_trails::{Expedition, Room, RoomStatus, SIDE, Tile};
use gpui::{
    Context, Div, FontWeight, MouseButton, ObjectFit, Render, Stateful, Window, div, img,
    prelude::*, px, rgb,
};

use crate::{
    games::{GameEntry, GameLaunch},
    theme as t,
};

const ENTRANCE_ART: &str = "games/mine-trails/mine-entrance.jpg";
const TILE_SIZE: f32 = 51.;
const MAP_NODE_WIDTH: f32 = 122.;
const MAP_GAP: f32 = 12.;
const MAP_WIDTH: f32 = 2. * MAP_NODE_WIDTH + MAP_GAP;
const CACHE: &str = "◆";
const FLAG: &str = "⚑";
const MINE: &str = "●";

pub struct GameView {
    expedition: Expedition,
    seed: u64,
    challenge: bool,
    flag_mode: bool,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _cx: &mut Context<Self>) -> Self {
        let (seed, challenge) = match &launch.entry {
            GameEntry::Challenge(id) => (
                id.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
                }),
                true,
            ),
            _ => (0, false),
        };
        Self {
            expedition: Expedition::new(seed),
            seed,
            challenge,
            flag_mode: false,
        }
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        self.expedition = Expedition::new(self.seed);
        self.flag_mode = false;
        cx.notify();
    }

    fn cell(&self, x: usize, y: usize, exploring: bool, cx: &Context<Self>) -> Stateful<Div> {
        let tile = self.expedition.tile(x, y).expect("cell inside room");
        let diggable = self.expedition.can_dig(x, y);
        let flaggable = exploring && matches!(tile, Tile::Covered { cache: false } | Tile::Flagged);
        let (background, border, ink) = match tile {
            Tile::Open { clue: 0, .. } => (0xe0e7d4, 0xc3ceb6, t::MUTED),
            Tile::Open { clue, .. } => (t::PAPER, t::BORDER, clue_color(clue)),
            Tile::Covered { .. } if diggable => (0x3d6453, 0x70967d, t::PAPER),
            Tile::Covered { .. } | Tile::Flagged => (0x263f39, 0x456157, t::GOLD_HOVER),
            Tile::Mine => (0xa34335, 0xd28b6f, t::SURFACE),
        };
        let mut cell = div()
            .id(("mine-trails-cell", y * SIDE + x))
            .relative()
            .size(px(TILE_SIZE))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(background))
            .text_color(rgb(ink))
            .text_size(px(23.))
            .font_weight(FontWeight::BOLD);
        cell = match tile {
            Tile::Open { clue, cache } => cell.child(clue.to_string()).when(cache, |cell| {
                cell.child(
                    div()
                        .absolute()
                        .top(px(4.))
                        .right(px(5.))
                        .line_height(px(12.))
                        .text_size(px(12.))
                        .text_color(rgb(0x80601b))
                        .child(CACHE),
                )
            }),
            Tile::Covered { cache: true } => cell.text_color(rgb(t::GOLD_HOVER)).child(CACHE),
            Tile::Covered { cache: false } => cell,
            Tile::Flagged => cell.child(FLAG),
            Tile::Mine => cell.child(MINE),
        };

        let actionable = if self.flag_mode { flaggable } else { diggable };
        if actionable {
            cell = cell
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.border_color(rgb(t::GOLD_HOVER)))
                .focus(|style| style.border_color(rgb(t::GOLD)))
                .on_click(cx.listener(move |view, _, _, cx| {
                    let changed = if view.flag_mode {
                        view.expedition.toggle_flag(x, y)
                    } else {
                        view.expedition.dig(x, y)
                    };
                    if changed {
                        cx.notify();
                    }
                }));
        }
        if flaggable {
            cell = cell.on_mouse_down(
                MouseButton::Right,
                cx.listener(move |view, _, _, cx| {
                    if view.expedition.toggle_flag(x, y) {
                        cx.notify();
                    }
                }),
            );
        }
        cell
    }

    fn board(&self, room: Room, cx: &Context<Self>) -> Div {
        let status = self.expedition.status(room);
        let exploring = status == RoomStatus::Exploring;
        let (found, total) = self.expedition.cache_progress(room);
        let mut controls = div().flex().flex_wrap().items_center().gap_2();
        if exploring {
            for (id, label, flag_mode) in [
                ("mine-trails-dig", "Dig", false),
                ("mine-trails-flag", "Flag", true),
            ] {
                controls =
                    controls.child(control(id, label, self.flag_mode == flag_mode).on_click(
                        cx.listener(move |view, _, _, cx| {
                            view.flag_mode = flag_mode;
                            cx.notify();
                        }),
                    ));
            }
            controls = controls.child(div().text_color(rgb(t::MUTED)).text_size(px(13.)).child(
                format!(
                    "{} / {} flags",
                    self.expedition.flag_count(),
                    self.expedition.mine_count()
                ),
            ));
        } else if status == RoomStatus::Collapsed {
            controls = controls.child(
                control("mine-trails-retry-room", "Retry room", true).on_click(cx.listener(
                    |view, _, _, cx| {
                        if view.expedition.retry_room() {
                            view.flag_mode = false;
                            cx.notify();
                        }
                    },
                )),
            );
        }

        div()
            .flex()
            .flex_col()
            .items_start()
            .gap_3()
            .child(
                div()
                    .rounded_md()
                    .bg(rgb(t::SAGE))
                    .px_2()
                    .py_1()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{CACHE}  Caches recovered  {found} / {total}")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .rounded_lg()
                    .bg(rgb(t::RAIL))
                    .children((0..SIDE).map(|y| {
                        div()
                            .flex()
                            .gap_1()
                            .children((0..SIDE).map(|x| self.cell(x, y, exploring, cx)))
                    })),
            )
            .child(controls)
            .when(exploring, |board| {
                board.child(
                    div()
                        .max_w(px(344.))
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(rgb(t::MUTED))
                        .child(if self.flag_mode {
                            "Click to flag or unflag a suspected mine. Switch to Dig to open tiles."
                        } else {
                            "Dig beside an open tile. Each clue counts mines in the eight surrounding tiles. Right-click to flag."
                        }),
                )
            })
    }

    fn room_node(&self, room: Room, cx: &Context<Self>) -> Stateful<Div> {
        let current = room == self.expedition.current_room();
        let can_travel = self.expedition.can_travel(room);
        let detail = if current {
            "Current room"
        } else {
            match self.expedition.status(room) {
                RoomStatus::Surveyed => "Surveyed",
                RoomStatus::Collapsed => "Mine hit",
                RoomStatus::Exploring if can_travel => "Open tunnel",
                RoomStatus::Exploring if self.expedition.is_unlocked(room) => "Via another room",
                RoomStatus::Exploring => "Locked",
            }
        };
        let node = div()
            .id(("mine-trails-room", room as usize))
            .w(px(MAP_NODE_WIDTH))
            .h(px(46.))
            .rounded_md()
            .border_1()
            .border_color(rgb(if current || can_travel {
                t::MUTED
            } else {
                t::BORDER
            }))
            .bg(rgb(if current { t::RAIL_ACTIVE } else { t::SURFACE }))
            .text_color(rgb(if current { t::SURFACE } else { t::INK }))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_size(px(13.))
            .line_height(px(17.))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(room.name()))
            .child(
                div()
                    .text_color(rgb(if current { 0xe0e7d4 } else { t::MUTED }))
                    .text_size(px(11.))
                    .child(detail),
            );

        if can_travel {
            node.tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(t::SAGE)))
                .focus(|style| style.border_color(rgb(t::GOLD)))
                .on_click(cx.listener(move |view, _, _, cx| {
                    if view.expedition.travel(room) {
                        view.flag_mode = false;
                        cx.notify();
                    }
                }))
        } else {
            node
        }
    }

    fn tunnel_map(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(t::display_type("Tunnel map", 20.))
            .child(
                div()
                    .w(px(MAP_WIDTH))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(self.room_node(Room::Entrance, cx))
                    .child(map_connector(false))
                    .child(
                        div()
                            .flex()
                            .gap(px(MAP_GAP))
                            .child(self.room_node(Room::AmberDrift, cx))
                            .child(self.room_node(Room::SlateDrift, cx)),
                    )
                    .child(map_connector(true))
                    .child(self.room_node(Room::DeepVault, cx)),
            )
    }

    fn objective(&self, room: Room) -> (&'static str, u32) {
        match self.expedition.status(room) {
            RoomStatus::Collapsed => ("Mine hit. Retry this room to start it again.", 0x963f33),
            RoomStatus::Surveyed => {
                let message = if self.expedition.is_complete() {
                    "Expedition complete. All four rooms surveyed."
                } else if self.expedition.is_unlocked(Room::DeepVault) {
                    "Both drifts surveyed. The Deep Vault is open."
                } else if room == Room::Entrance {
                    "Entrance surveyed. Choose a drift on the tunnel map."
                } else {
                    "Drift surveyed. Return to the Entrance to reach the other drift."
                };
                (message, 0x286248)
            }
            RoomStatus::Exploring => {
                let message = match room {
                    Room::Entrance => "Recover both diamonds to open the drift tunnels.",
                    Room::AmberDrift | Room::SlateDrift => {
                        "Recover both diamonds to survey the drift."
                    }
                    Room::DeepVault => "Recover both diamonds to complete the expedition.",
                };
                (message, t::MUTED)
            }
        }
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let room = self.expedition.current_room();
        let (objective, color) = self.objective(room);
        let wide = window.viewport_size().width >= px(950.);
        let mut actions = div().flex().flex_wrap().gap_2().child(
            control("mine-trails-restart", "Restart expedition", false)
                .on_click(cx.listener(|view, _, _, cx| view.restart(cx))),
        );
        if !self.challenge {
            actions = actions.child(
                control(
                    "mine-trails-new",
                    "New expedition",
                    self.expedition.is_complete(),
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.seed = view.seed.wrapping_add(1);
                    view.restart(cx);
                })),
            );
        }

        t::surface()
            .w_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(t::display_type(room.name(), 28.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(color))
                                    .child(objective),
                            ),
                    )
                    .child(actions),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_5()
                    .when(!wide, |layout| layout.flex_col())
                    .child(self.board(room, cx))
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .when(wide, |panel| panel.flex_col())
                            .child(self.tunnel_map(cx))
                            .when(room == Room::Entrance, |panel| {
                                panel.child(
                                    div()
                                        .w(px(if wide { MAP_WIDTH } else { 190. }))
                                        .h(px(if wide { 112. } else { 126. }))
                                        .rounded_md()
                                        .overflow_hidden()
                                        .child(
                                            img(ENTRANCE_ART)
                                                .size_full()
                                                .object_fit(ObjectFit::Cover),
                                        ),
                                )
                            }),
                    ),
            )
    }
}

fn map_connector(merge: bool) -> Div {
    let center = MAP_WIDTH * 0.5;
    let left_node_center = MAP_NODE_WIDTH * 0.5;
    let color = rgb(t::MUTED);
    let line = |left, top, width, height| {
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(width))
            .h(px(height))
            .bg(color)
    };
    let (stem_top, branches_top) = if merge { (6., 0.) } else { (0., 6.) };
    div()
        .relative()
        .w(px(MAP_WIDTH))
        .h(px(13.))
        .child(line(left_node_center, 6., MAP_NODE_WIDTH + MAP_GAP, 1.))
        .child(line(center, stem_top, 1., 7.))
        .child(line(left_node_center, branches_top, 1., 7.))
        .child(line(MAP_WIDTH - left_node_center, branches_top, 1., 7.))
}

fn clue_color(clue: u8) -> u32 {
    match clue {
        1 => 0x22546e,
        2 => 0x275b36,
        3 => 0x963f33,
        _ => 0x604a83,
    }
}

fn control(id: &'static str, label: &'static str, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .tab_index(0)
        .cursor_pointer()
        .flex()
        .items_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if selected { t::GOLD } else { t::BORDER }))
        .bg(rgb(if selected { t::GOLD } else { t::SURFACE }))
        .hover(|style| style.bg(rgb(if selected { t::GOLD_HOVER } else { t::SAGE })))
        .focus(|style| style.border_color(rgb(t::MUTED)))
        .px_3()
        .h(px(34.))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}
