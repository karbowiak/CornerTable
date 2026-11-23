use corner_table_mine_trails::{Room, RoomStatus};
use gpui::{
    Context, Div, FontWeight, PathBuilder, Role, Stateful, canvas, div, point, prelude::*, px,
    relative, rgb,
};

use crate::theme as t;

use super::{GameView, art};
use art::Mark;

const NODE_HEIGHT: f32 = 49.;
const MAP_HEIGHT: f32 = 304.;
// X is a fraction of the chart width. Y is in logical pixels.
const PINS: [(f32, f32); Room::ALL.len()] = [
    (0.50, 39.),
    (0.25, 114.),
    (0.75, 114.),
    (0.50, 189.),
    (0.50, 264.),
];
const TUNNELS: [(Room, Room); 5] = [
    (Room::Entrance, Room::AmberDrift),
    (Room::Entrance, Room::SlateDrift),
    (Room::AmberDrift, Room::Floodgate),
    (Room::SlateDrift, Room::Floodgate),
    (Room::Floodgate, Room::DeepVault),
];

#[derive(Clone, Copy)]
enum Tunnel {
    Sealed,
    Open,
    Charted,
    Nearby,
}

impl Tunnel {
    fn ink(self) -> u32 {
        match self {
            Self::Sealed => 0x83766a,
            Self::Open => 0xadb6a9,
            Self::Charted => 0x81b6ae,
            Self::Nearby => 0xf4c37e,
        }
    }
}

impl GameView {
    pub(super) fn tunnel_map(&self, width: f32, cx: &Context<Self>) -> Div {
        let palette = t::colors(cx);
        let current = self.expedition.current_room();
        let is_surveyed = |room| self.expedition.status(room) == RoomStatus::Surveyed;
        let tunnels = TUNNELS.map(|(from, to)| {
            let state = if !self.expedition.is_unlocked(to) {
                Tunnel::Sealed
            } else if from == current || to == current {
                Tunnel::Nearby
            } else if is_surveyed(from) && is_surveyed(to) {
                Tunnel::Charted
            } else {
                Tunnel::Open
            };
            (from, to, state)
        });
        let pin_width = (width / 2. - 19.).clamp(105., 142.);

        t::panel(cx)
            .w_full()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .child(t::display_type("Tunnel map", 19.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(palette.muted))
                            .child(format!(
                                "{} / {} surveyed",
                                self.expedition.surveyed_count(),
                                Room::ALL.len()
                            )),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(MAP_HEIGHT))
                    .bg(rgb(0x292b2c))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .child(art::painting(art::TUNNELS)),
                    )
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                for (from, to, tunnel) in tunnels {
                                    let anchor = |room: Room| {
                                        let (x, y) = PINS[room as usize];
                                        bounds.origin + point(bounds.size.width * x, px(y))
                                    };
                                    let a = anchor(from);
                                    let b = anchor(to);
                                    let mid_y = (a.y + b.y) * 0.5;
                                    let draw = |width, dashed| {
                                        let mut path = PathBuilder::stroke(px(width));
                                        if dashed {
                                            path = path.dash_array(&[px(2.5), px(4.5)]);
                                        }
                                        path.move_to(a);
                                        path.cubic_bezier_to(
                                            b,
                                            point(a.x, mid_y),
                                            point(b.x, mid_y),
                                        );
                                        path.build().ok()
                                    };
                                    if let Some(bed) = draw(6., false) {
                                        window.paint_path(bed, rgb(0x201c1a));
                                    }
                                    if let Some(track) = draw(
                                        if matches!(tunnel, Tunnel::Nearby) {
                                            2.5
                                        } else {
                                            1.5
                                        },
                                        matches!(tunnel, Tunnel::Sealed),
                                    ) {
                                        window.paint_path(track, rgb(tunnel.ink()));
                                    }
                                }
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .children(
                        Room::ALL
                            .into_iter()
                            .map(|room| self.map_pin(room, pin_width, cx)),
                    ),
            )
    }

    fn map_pin(&self, room: Room, width: f32, cx: &Context<Self>) -> Stateful<Div> {
        let current = self.expedition.current_room() == room;
        let passable = self.expedition.can_travel(room);
        let unlocked = self.expedition.is_unlocked(room);
        let status = self.expedition.status(room);
        let (icon, label, accent, surface, border) = if current {
            (Mark::Here, "You are here", 0xffd597, 0x473222, 0xeeaf68)
        } else if !unlocked {
            (Mark::Lock, "Sealed tunnel", 0xbbb1a5, 0x2a2828, 0x655e58)
        } else {
            match status {
                RoomStatus::Exploring => (Mark::Ring, "Unsurveyed", 0xd4d6c8, 0x303538, 0x7f8c8a),
                RoomStatus::Surveyed => (Mark::Check, "Surveyed", 0xb0ded5, 0x263938, 0x78aaa2),
                RoomStatus::Collapsed => (Mark::Mine, "Cave-in", 0xf8b8a3, 0x42302c, 0xa96956),
            }
        };
        let (x, y) = PINS[room as usize];
        div()
            .id(("mine-trails-pin", room as usize))
            .role(if passable { Role::Button } else { Role::Label })
            .aria_label(format!("{}. {label}.", room.name()))
            .absolute()
            .left(relative(x))
            .ml(px(-width / 2.))
            .top(px(y - NODE_HEIGHT / 2.))
            .w(px(width))
            .h(px(NODE_HEIGHT))
            .rounded_md()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(surface))
            .px_2()
            .gap_1p5()
            .flex()
            .items_center()
            .child(art::symbol(icon, 13., accent))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .line_height(px(15.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(if unlocked { 0xf9ebd8 } else { 0xdfd5c9 }))
                            .child(room.name()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(accent))
                            .child(label),
                    ),
            )
            .when(passable, |pin| {
                pin.tab_index(0)
                    .cursor_pointer()
                    .hover(|node| node.bg(rgb(0x4b4335)).border_color(rgb(0xf4c37e)))
                    .focus_visible(|node| node.border_color(rgb(0xffe1ad)))
                    .active(|node| node.bg(rgb(0x5c4834)))
                    .on_click(cx.listener(move |view, _, window, cx| view.travel(room, window, cx)))
            })
    }
}
