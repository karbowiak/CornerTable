use corner_table_mine_trails::{Position, Room, RoomStatus, Tile};
use gpui::{Context, Div, FontWeight, Role, Stateful, Window, div, prelude::*, px, rgb, rgba};

use crate::theme as t;

use super::{GameView, Strike, Tool, art, button, record::Move};
use art::{Chip, Mark};

pub(super) const TILE_GAP: f32 = 4.;

impl GameView {
    pub(super) fn board(
        &self,
        room: Room,
        size: f32,
        window: &Window,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let focused = self.board_focus.is_focused(window) && self.keyboard_cursor;
        let side = self.expedition.side();
        let strike = self
            .strike
            .as_ref()
            .map(|strike| (strike, strike.progress()));
        div()
            .id("mine-trails-board")
            .role(Role::Grid)
            .aria_label(format!("{} excavation", room.name()))
            .aria_row_count(side)
            .aria_column_count(side)
            .aria_description(
                "Arrow keys move. Enter acts on the selected tile. Space toggles a flag.",
            )
            .track_focus(&self.board_focus)
            .tab_index(0)
            .on_key_down(cx.listener(Self::keyboard))
            .focus_visible(|style| style.border_color(rgb(0xe2bb82)))
            .relative()
            .overflow_hidden()
            .w_full()
            .bg(rgb(0x272b30))
            .border_y_1()
            .border_color(rgb(0x595247))
            .child(div().absolute().inset_0().child(art::painting(art::SLATE)))
            .child(
                div()
                    .relative()
                    .w_full()
                    .py_4()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(TILE_GAP))
                    .children((0..side).map(|y| {
                        div()
                            .id(("mine-trails-row", y))
                            .role(Role::Row)
                            .aria_row_index(y)
                            .flex()
                            .gap(px(TILE_GAP))
                            .children(
                                (0..side)
                                    .map(|x| self.cell((x, y), size, room, focused, strike, cx)),
                            )
                    })),
            )
    }

    fn cell(
        &self,
        at: Position,
        size: f32,
        room: Room,
        focused: bool,
        strike: Option<(&Strike, f32)>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let tile = self.expedition.tile(at).expect("position inside chamber");
        let digging = self.expedition.can_dig(at);
        let exploring = self.expedition.status(room) == RoomStatus::Exploring;
        let covered = matches!(tile, Tile::Covered { .. } | Tile::Flagged);
        let flaggable =
            exploring && matches!(tile, Tile::Covered { diamond: false } | Tile::Flagged);
        let clue = matches!(tile, Tile::Open { .. });
        let (fill, rim) = match tile {
            Tile::Bedrock if room == Room::Floodgate => (0x12363a92, 0x12363a00),
            Tile::Bedrock => (0x16171961, 0x16171900),
            Tile::Open { diamond: true, .. } => (0x284447ed, 0x8ab9b9a0),
            Tile::Open { .. } => (0x292d2de9, 0x90958762),
            Tile::Covered { .. } if digging => (0x825e3dae, 0xe1b17cae),
            Tile::Flagged => (0x413222df, 0xc09758a0),
            Tile::Covered { .. } => (0x25272d83, 0x9290846a),
            Tile::Mine { hit: true } => (0x913e31fb, 0xffad89ee),
            Tile::Mine { hit: false } => (0x502e2bed, 0xc28573aa),
            Tile::FalseFlag => (0x3c3032ed, 0xb48376ae),
        };
        let pinned = near(at, self.pinned_clue);
        let traced = near(at, self.hovered_clue);
        let rim = if focused && self.cursor == at {
            0xffe9b8ff
        } else if tile == Tile::Bedrock {
            rim
        } else if pinned && traced && self.pinned_clue != self.hovered_clue {
            0xf2da9de5
        } else if pinned {
            0xe7b776dc
        } else if traced {
            0x9ac9cadd
        } else {
            rim
        };
        let index = at.1 * self.expedition.side() + at.0;
        let mut cell = div()
            .id(("mine-trails-cell", index))
            .role(Role::GridCell)
            .aria_column_index(at.0)
            .aria_label(tile_label(tile, digging))
            .aria_selected(focused && self.cursor == at)
            .relative()
            .flex_none()
            .size(px(size))
            .rounded(px(5.))
            .border_1()
            .when(self.pinned_clue == Some(at), |cell| cell.border_2())
            .border_color(rgba(rim))
            .bg(rgba(fill))
            .flex()
            .items_center()
            .justify_center();

        if covered {
            let chip = if tile == Tile::Flagged {
                Chip::Flagged
            } else if digging {
                Chip::Reachable
            } else {
                Chip::OutOfReach
            };
            cell = cell.child(art::stone_chip(chip, size));
        }
        cell = match tile {
            Tile::Open { clue, diamond } => cell
                .child(art::excavated_floor(diamond))
                .when(clue > 0, |tile| {
                    tile.font_weight(FontWeight::BOLD)
                        .text_size(px((size * 0.41).clamp(17., 29.)))
                        .text_color(rgb(clue_ink(clue)))
                        .child(clue.to_string())
                })
                .when(diamond, |tile| {
                    tile.child(div().absolute().top_1().right_1().child(art::symbol(
                        Mark::Diamond,
                        size * 0.23,
                        0xa7e1e7,
                    )))
                }),
            Tile::Covered { diamond: true } => cell.child(art::symbol(
                Mark::Diamond,
                size * 0.39,
                if digging { 0xaee6ef } else { 0x7daeb9 },
            )),
            Tile::Flagged => cell.child(art::symbol(Mark::Flag, size * 0.38, 0xf7c37f)),
            Tile::FalseFlag => cell.child(art::symbol(Mark::WrongFlag, size * 0.40, 0xe0a762)),
            Tile::Mine { hit } => cell.child(art::symbol(
                Mark::Mine,
                size * 0.36,
                if hit { 0xffe0bc } else { 0xdc9e8b },
            )),
            Tile::Covered { diamond: false } | Tile::Bedrock => cell,
        };

        if let Some((strike, progress)) = strike
            && (strike.hit == Some(at) || strike.opened & (1 << index) != 0)
        {
            cell = cell.child(art::strike(
                progress,
                strike.hit == Some(at),
                matches!(tile, Tile::Open { diamond: true, .. }),
            ));
        }
        let tool_applies = match self.tool {
            Tool::Dig => digging,
            Tool::Flag => flaggable,
        };
        cell.when(tool_applies || clue, |cell| {
            cell.cursor_pointer()
                .hover(|style| style.border_color(rgb(0xffd594)))
                .active(|style| style.border_color(rgb(0xffefd1)).bg(rgb(0x493426)))
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.cursor = at;
                    view.keyboard_cursor = false;
                    view.board_focus.focus(window, cx);
                    view.activate_tile(at, cx);
                }))
        })
        .when(clue, |cell| {
            cell.on_hover(cx.listener(move |view, hovered: &bool, _, cx| {
                if *hovered {
                    view.hovered_clue = Some(at);
                    cx.notify();
                } else if view.hovered_clue == Some(at) {
                    view.hovered_clue = None;
                    cx.notify();
                }
            }))
        })
        .when(flaggable, |cell| {
            cell.on_aux_click(
                cx.listener(move |view, event: &gpui::ClickEvent, window, cx| {
                    if event.is_secondary() {
                        view.cursor = at;
                        view.keyboard_cursor = false;
                        view.board_focus.focus(window, cx);
                        view.apply(Move::Flag(at.0 as u8, at.1 as u8), cx);
                    }
                }),
            )
        })
    }

    pub(super) fn board_note(&self, room: Room, cx: &Context<Self>) -> Div {
        let colors = t::colors(cx);
        let status = self.expedition.status(room);
        let inspected = self.pinned_clue.or(self.hovered_clue);
        let mut bar = div()
            .w_full()
            .min_h(px(53.))
            .px_4()
            .py_2()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .text_size(px(12.))
            .line_height(px(17.))
            .text_color(rgb(colors.muted));

        if status == RoomStatus::Collapsed {
            return bar.child("Red marks the mine you hit. A crossed flag was over safe ground.");
        }
        if let Some(position) = inspected
            && let Some(Tile::Open { clue, .. }) = self.expedition.tile(position)
        {
            let flags = self.expedition.adjacent_flags(position);
            let mine_label = if clue == 1 { "mine" } else { "mines" };
            let flag_label = if flags == 1 { "flag" } else { "flags" };
            bar = bar.child(format!(
                "{clue} adjacent {mine_label}  ·  {flags} {flag_label}"
            ));
            if self.pinned_clue == Some(position) && self.expedition.can_sweep(position) {
                return bar
                    .child(
                        button("mine-sweep", "Sweep unmarked tiles", false, colors).on_click(
                            cx.listener(move |view, _, window, cx| {
                                view.apply(Move::Sweep(position.0 as u8, position.1 as u8), cx);
                                view.board_focus.focus(window, cx);
                            }),
                        ),
                    )
                    .child("Sweep trusts your flags.");
            }
            return bar;
        }
        match status {
            RoomStatus::Exploring if room == Room::DeepVault => {
                bar.child("Pin one clue, then hover over another to compare their neighbors.")
            }
            RoomStatus::Exploring => {
                bar.child("Hover over a clue to trace its neighbors. Click to pin it.")
            }
            RoomStatus::Surveyed => bar.child("Chamber surveyed. You can revisit it from the map."),
            RoomStatus::Collapsed => unreachable!(),
        }
    }
}

fn near(at: Position, clue: Option<Position>) -> bool {
    clue.is_some_and(|(x, y)| x.abs_diff(at.0) <= 1 && y.abs_diff(at.1) <= 1)
}

fn clue_ink(clue: u8) -> u32 {
    match clue {
        1 => 0xa1d9e4,
        2 => 0xc7d6a7,
        3 => 0xf5bf86,
        4 => 0xf0ac9d,
        _ => 0xe3c6ea,
    }
}

fn tile_label(tile: Tile, diggable: bool) -> String {
    match tile {
        Tile::Bedrock => "Solid rock".into(),
        Tile::Covered { diamond: false } if diggable => "Covered, within reach".into(),
        Tile::Covered { diamond: false } => "Covered, out of reach".into(),
        Tile::Covered { diamond: true } if diggable => "Diamond, within reach".into(),
        Tile::Covered { diamond: true } => "Diamond, out of reach".into(),
        Tile::Open { clue, diamond } => {
            let label = if diamond {
                "Diamond recovered"
            } else {
                "Cleared"
            };
            let mines = if clue == 1 { "mine" } else { "mines" };
            format!("{label}, {clue} adjacent {mines}")
        }
        Tile::Flagged => "Flagged".into(),
        Tile::FalseFlag => "Flagged safe tile".into(),
        Tile::Mine { hit: true } => "Mine hit".into(),
        Tile::Mine { hit: false } => "Mine".into(),
    }
}
