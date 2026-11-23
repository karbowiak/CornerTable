use std::time::Duration;

use corner_table_mahjong::{Face, Tile};
use gpui::{
    Animation, AnimationExt as _, BoxShadow, Context, Div, FontWeight, MouseButton, ObjectFit,
    Role, Stateful, Window, div, img, linear_color_stop, linear_gradient, prelude::*, px, rgb, svg,
};

use super::GameView;
use crate::{games::board_space, theme};

const TILE_RATIO: f32 = 1.14;
const LAYER_OFFSET: f32 = 0.089;
const DEPTH_ALLOWANCE: f32 = 4. * LAYER_OFFSET;
const FELT_INSET: f32 = 14.;
const TABLE_FRAME: f32 = 10.;

#[derive(Clone, Copy)]
pub(super) struct TableColors {
    rim: [u32; 2],
    felt: [u32; 2],
    trim: u32,
    edge: u32,
}

impl From<theme::Palette> for TableColors {
    fn from(palette: theme::Palette) -> Self {
        if palette.paper == theme::PAPER {
            Self {
                rim: [0x926b4c, 0x5d4332],
                felt: [0x3e6554, 0x284d40],
                trim: 0x897954,
                edge: 0x654b37,
            }
        } else {
            Self {
                rim: [0x654a38, 0x47382c],
                felt: [0x304f43, 0x203e33],
                trim: 0x6f684d,
                edge: 0x82664b,
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct TilePosition {
    x: f32,
    y: f32,
}

impl TilePosition {
    pub(super) fn relative(tile: &Tile) -> (f32, f32) {
        let offset = f32::from(tile.level) * LAYER_OFFSET;
        (
            f32::from(tile.x) * 0.5 + offset,
            f32::from(tile.y) * 0.5 * TILE_RATIO - offset,
        )
    }

    fn on_felt(tile: &Tile, size: f32) -> Self {
        let (x, y) = Self::relative(tile);
        Self {
            x: x * size + FELT_INSET,
            y: (y + DEPTH_ALLOWANCE) * size + FELT_INSET,
        }
    }
}

struct Emblem {
    label: String,
    name: String,
    art: &'static str,
    ink: u32,
}

impl From<Face> for Emblem {
    fn from(face: Face) -> Self {
        let (label, name, art, ink) = match face {
            Face::Dots(rank) => (rank.to_string(), format!("Dots {rank}"), "berry", 0x274f6d),
            Face::Bamboo(rank) => (
                rank.to_string(),
                format!("Bamboo {rank}"),
                "bamboo",
                0x26573d,
            ),
            Face::Characters(rank) => (
                rank.to_string(),
                format!("Characters {rank}"),
                "character",
                0x883a30,
            ),
            Face::Wind(letter) => (
                letter.to_string(),
                match letter {
                    'E' => "East Wind",
                    'S' => "South Wind",
                    'W' => "West Wind",
                    _ => "North Wind",
                }
                .into(),
                "wind",
                0x315565,
            ),
            Face::Dragon(letter) => {
                let (name, ink) = match letter {
                    'R' => ("Red Dragon", 0x883a30),
                    'G' => ("Green Dragon", 0x26573d),
                    _ => ("White Dragon", 0x55465c),
                };
                (letter.to_string(), name.into(), "dragon", ink)
            }
            Face::Flower => ("F".into(), "Flower".into(), "flower", 0x903954),
            Face::Season => ("S".into(), "Season".into(), "season", 0x734b26),
        };
        Self {
            label,
            name,
            art,
            ink,
        }
    }
}

pub(super) fn face_name(face: Face) -> String {
    Emblem::from(face).name
}

#[derive(Clone, Copy)]
struct Glaze {
    face: [u32; 2],
    rim: u32,
    edge: u32,
    bevel: u32,
}

const FREE: Glaze = Glaze {
    face: [0xfffcf0, 0xf3e8cd],
    rim: 0xb5a27b,
    edge: 0x827357,
    bevel: 0xfff9ea,
};

const BLOCKED: Glaze = Glaze {
    face: [0xd9d3bc, 0xc4bea6],
    rim: 0x989079,
    edge: 0x807961,
    bevel: 0xdcd5bb,
};

const SELECTED: Glaze = Glaze {
    face: [0xffe7a8, 0xf1c97c],
    rim: 0xba843c,
    edge: 0x9a5a20,
    bevel: 0xfff2cc,
};

const HINTED: Glaze = Glaze {
    face: [0xe4f8d7, 0xaad9b0],
    rim: 0x659b74,
    edge: 0x245d45,
    bevel: 0xe8ffc9,
};

fn gradient([top, bottom]: [u32; 2]) -> gpui::Background {
    linear_gradient(
        180.,
        linear_color_stop(rgb(top), 0.),
        linear_color_stop(rgb(bottom), 1.),
    )
}

fn ceramic_tile(tile: &Tile, mark: &Emblem, size: f32, glaze: Glaze) -> Div {
    let position = TilePosition::on_felt(tile, size);
    let clearance = (size * 0.035).max(1.);
    let thickness = (size * 0.075).max(2.);
    div()
        .absolute()
        .left(px(position.x))
        .top(px(position.y))
        .w(px(size - clearance))
        .h(px(size * TILE_RATIO - clearance))
        .rounded_sm()
        .border_1()
        .border_color(rgb(glaze.edge))
        .bg(rgb(glaze.rim))
        .when(tile.level > 0, |body| {
            body.shadow(vec![
                BoxShadow::new(
                    px(-1.),
                    px(thickness * 0.7),
                    rgb(0x17150f).opacity(0.47).into(),
                )
                .blur_radius(px(thickness + 1.)),
            ])
        })
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom(px(thickness))
                .rounded_sm()
                .border_1()
                .border_color(rgb(glaze.bevel))
                .bg(gradient(glaze.face))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_color(rgb(mark.ink))
                .child(
                    div()
                        .font_family(theme::DISPLAY)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(size * 0.39))
                        .line_height(px(size * 0.43))
                        .child(mark.label.clone()),
                )
                .child(
                    svg()
                        .path(format!("games/mahjong/{}.svg", mark.art))
                        .w(px(size * 0.61))
                        .h(px(size * 0.43))
                        .text_color(rgb(mark.ink)),
                ),
        )
}

impl GameView {
    fn interactive_tile(
        &self,
        id: usize,
        tile: &Tile,
        size: f32,
        cursor: Option<usize>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let free = self.board.is_free(id);
        let selected = self.board.selected() == Some(id);
        let hinted = self.highlighted.is_some_and(|pair| pair.contains(&id));
        let glaze = if selected {
            SELECTED
        } else if hinted {
            HINTED
        } else if free {
            FREE
        } else {
            BLOCKED
        };
        let mark = Emblem::from(tile.face);
        ceramic_tile(tile, &mark, size, glaze)
            .id(("mahjong-tile", id))
            .occlude()
            .when(free, |tile| {
                tile.role(Role::Button)
                    .aria_label(mark.name)
                    .aria_selected(selected)
                    .when(hinted, |tile| tile.aria_description("Hinted pair"))
                    .cursor_pointer()
                    .hover(|style| style.border_color(rgb(0x9b5d28)).bg(rgb(0xd6a66d)))
                    .active(|style| style.border_color(rgb(0x643c20)))
            })
            .when(cursor == Some(id), |tile| {
                tile.aria_active_descendant().child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded_sm()
                        .border_2()
                        .border_color(rgb(0x1b566b)),
                )
            })
            .on_click(cx.listener(move |view, _, window, cx| {
                view.focus.focus(window, cx);
                view.choose(id, cx);
            }))
    }

    pub(super) fn table(
        &self,
        colors: TableColors,
        window: &Window,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let (extent_x, extent_y) = self.board.dimensions();
        let columns = f32::from(extent_x) * 0.5 + DEPTH_ALLOWANCE;
        let rows = f32::from(extent_y) * 0.5 * TILE_RATIO + DEPTH_ALLOWANCE;
        let inset = 2. * (TABLE_FRAME + FELT_INSET);
        // Allow room for the shell header, toolbar and status even at 760 × 540.
        let chrome = if self.daily() { 144. } else { 185. };
        let fit_width = (board_space(window) - inset) / columns;
        let fit_height = (f32::from(window.viewport_size().height) - chrome - inset) / rows;
        let size = fit_width.min(fit_height).clamp(23., 68.).floor();
        let cursor = (self.focus.is_focused(window) && window.last_input_was_keyboard())
            .then(|| self.focused_tile())
            .flatten();
        let felt = div()
            .relative()
            .flex()
            .w_full()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded_lg()
            .border_1()
            .border_color(rgb(colors.trim))
            .bg(gradient(colors.felt))
            .shadow(vec![
                BoxShadow::new(px(0.), px(2.), rgb(0x10251a).opacity(0.42).into())
                    .blur_radius(px(8.))
                    .inset(),
            ])
            .child(
                img("games/mahjong/felt.png")
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .child(
                div()
                    .relative()
                    .flex_none()
                    .w(px(size * columns + 2. * FELT_INSET))
                    .h(px(size * rows + 2. * FELT_INSET))
                    .children(
                        self.board
                            .tiles()
                            .map(|(id, tile)| self.interactive_tile(id, tile, size, cursor, cx)),
                    )
                    .when_some(self.departure.as_ref(), |board, departure| {
                        board.children(departure.tiles.iter().enumerate().map(|(index, tile)| {
                            let top = TilePosition::on_felt(tile, size).y;
                            ceramic_tile(tile, &Emblem::from(tile.face), size, SELECTED)
                                .with_animation(
                                    (
                                        "mahjong-match",
                                        departure.sequence.wrapping_mul(2).wrapping_add(index),
                                    ),
                                    Animation::new(Duration::from_millis(230)),
                                    move |tile, progress| {
                                        let lift = 1. - (1. - progress).powi(3);
                                        tile.top(px(top - 8. * lift))
                                            .opacity((1. - progress).powi(2))
                                    },
                                )
                        }))
                    }),
            )
            .when(self.board.remaining() == 0, |felt| {
                felt.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id("mahjong-cleared")
                                .role(Role::Status)
                                .aria_label(self.cleared_title())
                                .px_6()
                                .py_3()
                                .rounded_lg()
                                .border_1()
                                .border_color(rgb(0x9f9871))
                                .bg(rgb(0x15392d))
                                .text_color(rgb(0xfff2d2))
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_1()
                                .child(theme::display_type(self.cleared_title(), 24.))
                                .when(self.clean_clear && self.board.used_assistance(), |result| {
                                    let detail =
                                        "Clean Clear requires a deal without Hint or Undo.";
                                    result
                                        .aria_description(detail)
                                        .child(div().text_size(px(13.)).child(detail))
                                }),
                        ),
                )
            });

        div()
            .id("mahjong-table")
            .role(Role::Group)
            .aria_label("Mahjong table")
            .aria_description(
                "Use the arrow keys to move between free tiles. Enter or Space selects a tile.",
            )
            .track_focus(&self.focus)
            .focus_visible(|style| style.border_color(rgb(theme::GOLD)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, cx| view.focus.focus(window, cx)),
            )
            .rounded_xl()
            .p(px(TABLE_FRAME - 1.))
            .border_1()
            .border_color(rgb(colors.edge))
            .bg(gradient(colors.rim))
            .child(felt)
    }
}
