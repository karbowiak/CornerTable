use std::time::Duration;

use corner_table_mahjong::{Board, Tile};
use gpui::{
    Animation, AnimationExt as _, BoxShadow, Context, Div, MouseButton, ObjectFit, Role, Stateful,
    Window, div, img, linear_color_stop, linear_gradient, prelude::*, px, rgb,
};

use super::{
    GameView,
    art::{Finish, Stamp, glaze},
};
use crate::{games::board_space, theme};

const TILE_RATIO: f32 = 1.14;
const LAYER_OFFSET: f32 = 0.089;
const FELT_INSET: f32 = 14.;
const TABLE_FRAME: f32 = 10.;
const TABLE_INSETS: f32 = 2. * (TABLE_FRAME + FELT_INSET + 1.);
const MIN_CONTROLS_WIDTH: f32 = 480.;

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
}

#[derive(Clone, Copy)]
pub(super) struct TableGeometry {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    tile_size: f32,
}

impl TableGeometry {
    pub(super) fn for_window(board: &Board, daily: bool, window: &Window) -> Self {
        // Reserve the shell title, toolbar and feedback, plus the classic
        // layout picker. The complete footprint stays fixed during the deal.
        let chrome = if daily { 148. } else { 196. };
        Self::fit(
            board,
            board_space(window),
            f32::from(window.viewport_size().height) - chrome,
        )
    }

    fn fit(board: &Board, available_width: f32, available_height: f32) -> Self {
        let [left, top, right, bottom] = board.all_tiles().iter().map(TilePosition::relative).fold(
            [
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ],
            |[left, top, right, bottom], (x, y)| {
                [left.min(x), top.min(y), right.max(x), bottom.max(y)]
            },
        );
        let width = right - left + 1.;
        let height = bottom - top + TILE_RATIO;
        let fit_width = (available_width - TABLE_INSETS) / width;
        let fit_height = (available_height - TABLE_INSETS) / height;
        Self {
            left,
            top,
            width,
            height,
            tile_size: fit_width.min(fit_height).clamp(1., 72.).floor(),
        }
    }

    pub(super) fn game_width(self) -> f32 {
        (self.width * self.tile_size + TABLE_INSETS).max(MIN_CONTROLS_WIDTH)
    }

    fn position(self, tile: &Tile) -> TilePosition {
        let (x, y) = TilePosition::relative(tile);
        TilePosition {
            x: (x - self.left) * self.tile_size + FELT_INSET,
            y: (y - self.top) * self.tile_size + FELT_INSET,
        }
    }
}

fn gradient([top, bottom]: [u32; 2]) -> gpui::Background {
    linear_gradient(
        180.,
        linear_color_stop(rgb(top), 0.),
        linear_color_stop(rgb(bottom), 1.),
    )
}

fn ceramic_tile(tile: &Tile, stamp: &Stamp, geometry: TableGeometry, finish: Finish) -> Div {
    let position = geometry.position(tile);
    let size = geometry.tile_size;
    let clearance = (size * 0.035).max(1.);
    let contact = BoxShadow::new(
        px(-size * 0.014),
        px(size * 0.023),
        rgb(0x102019).opacity(0.53).into(),
    )
    .blur_radius(px(size * 0.038));
    let mut shadows = vec![contact];
    if tile.level > 0 {
        // The stack steps up and right, so the lifted piece shades the part
        // of its neighbor that remains visible to the left and below it.
        shadows.push(
            BoxShadow::new(
                px(-size * 0.048),
                px(size * 0.078),
                rgb(0x202619).opacity(0.43).into(),
            )
            .blur_radius(px(size * 0.104)),
        );
    }

    div()
        .absolute()
        .left(px(position.x))
        .top(px(position.y))
        .w(px(size - clearance))
        .h(px(size * TILE_RATIO - clearance))
        .rounded(px(size * 0.11))
        .shadow(shadows)
        .child(glaze(size, stamp, finish))
}

impl GameView {
    fn interactive_tile(
        &self,
        id: usize,
        tile: &Tile,
        geometry: TableGeometry,
        cursor: Option<usize>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let free = self.board.is_free(id);
        let selected = self.board.selected() == Some(id);
        let hinted = self.highlighted.is_some_and(|pair| pair.contains(&id));
        let finish = if selected {
            Finish::Selected
        } else if hinted {
            Finish::Hinted
        } else if free {
            Finish::Free
        } else {
            Finish::Blocked
        };
        let stamp = Stamp::from(tile.face);
        let size = geometry.tile_size;
        ceramic_tile(tile, &stamp, geometry, finish)
            .id(("mahjong-tile", id))
            .block_mouse_except_scroll()
            .when(free, |tile| {
                tile.role(Role::Button)
                    .aria_label(stamp.name)
                    .aria_selected(selected)
                    .when(hinted, |tile| tile.aria_description("Hinted pair"))
                    .cursor_pointer()
                    .group("mahjong-ceramic")
                    .child(
                        div()
                            .id("mahjong-ceramic-rim")
                            .absolute()
                            .top(px(size * 0.018))
                            .left(px(size * 0.068))
                            .right(px(size * 0.023))
                            .bottom(px(size * 0.131))
                            .rounded(px(size * 0.084))
                            .border_1()
                            .border_color(rgb(0xfffdf1))
                            .opacity(0.)
                            .group_hover("mahjong-ceramic", |style| style.opacity(1.))
                            .group_active("mahjong-ceramic", |style| {
                                style.opacity(1.).border_color(rgb(0x705c39))
                            }),
                    )
            })
            .when(cursor == Some(id), |tile| {
                tile.aria_active_descendant().child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(px(size * 0.11))
                        .border_2()
                        .border_color(rgb(0x224856))
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .rounded(px(size * 0.082))
                                .border_1()
                                .border_color(rgb(0xfff1ba)),
                        ),
                )
            })
            .on_click(cx.listener(move |view, _, window, cx| {
                view.focus.focus(window, cx);
                view.choose(id, cx);
            }))
    }

    pub(super) fn table(
        &self,
        geometry: TableGeometry,
        colors: TableColors,
        window: &Window,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let size = geometry.tile_size;
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
                    .w(px(size * geometry.width + 2. * FELT_INSET))
                    .h(px(size * geometry.height + 2. * FELT_INSET))
                    .when(cfg!(test), |board| {
                        board.debug_selector(|| "mahjong-footprint".into())
                    })
                    .children(
                        self.board.tiles().map(|(id, tile)| {
                            self.interactive_tile(id, tile, geometry, cursor, cx)
                        }),
                    )
                    .when_some(self.departure.as_ref(), |board, departure| {
                        board.children(departure.tiles.iter().enumerate().map(|(index, tile)| {
                            let top = geometry.position(tile).y;
                            ceramic_tile(tile, &Stamp::from(tile.face), geometry, Finish::Matched)
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

#[cfg(test)]
mod tests {
    use corner_table_mahjong::{DAILY_SEED, Layout};

    use super::*;

    #[test]
    fn the_full_boards_fit_at_narrow_and_wide_sizes() {
        for (width, height) in [(506., 344.), (1056., 704.)] {
            for layout in Layout::ALL {
                let board = Board::with_layout(layout, DAILY_SEED);
                let geometry = TableGeometry::fit(&board, width, height);
                assert!(geometry.tile_size >= 25., "{layout:?}");
                assert!(geometry.game_width() <= width, "{layout:?}");
                assert!(
                    geometry.height * geometry.tile_size + TABLE_INSETS <= height,
                    "{layout:?}"
                );
                for tile in board.all_tiles() {
                    let position = geometry.position(tile);
                    assert!(position.x >= FELT_INSET && position.y >= FELT_INSET);
                    assert!(
                        position.x + geometry.tile_size
                            <= geometry.width * geometry.tile_size + FELT_INSET + 0.01
                    );
                    assert!(
                        position.y + geometry.tile_size * TILE_RATIO
                            <= geometry.height * geometry.tile_size + FELT_INSET + 0.01
                    );
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    #[gpui::test]
    fn click_hits_the_visible_tile_where_koi_layers_overlap(cx: &mut gpui::TestAppContext) {
        use corner_table_api::GameId;
        use gpui::{Modifiers, VisualTestContext, point, size};

        let launch = crate::games::GameLaunch {
            game: GameId("mahjong-picking-test".into()),
            entry: crate::games::GameEntry::Classic,
        };
        let window = cx.open_window(size(px(760.), px(540.)), |_, cx| {
            let mut view = GameView::new(&launch, cx);
            view.board = Board::with_layout(Layout::Koi, DAILY_SEED);
            view
        });
        cx.run_until_parked();
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        let footprint = visual.debug_bounds("mahjong-footprint").unwrap();
        let (top, overlap, exposed) = window
            .update(cx, |view, window, _| {
                let geometry = TableGeometry::for_window(&view.board, false, window);
                let id_at = |x, y, level| {
                    view.board
                        .tiles()
                        .find(|(_, tile)| (tile.x, tile.y, tile.level) == (x, y, level))
                        .unwrap()
                        .0
                };
                let top = id_at(22, 7, 3);
                let covered = id_at(22, 6, 2);
                let click = |id, x, y| {
                    let pos = geometry.position(view.board.tile(id).unwrap());
                    footprint.origin
                        + point(
                            px(pos.x + x * geometry.tile_size),
                            px(pos.y + y * geometry.tile_size),
                        )
                };
                (top, click(top, 0.45, 0.25), click(covered, 0.5, 0.2))
            })
            .unwrap();
        visual.simulate_click(overlap, Modifiers::none());
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.board.selected(), Some(top));
                assert!(view.status.contains("selected"));
            })
            .unwrap();
        visual.simulate_click(exposed, Modifiers::none());
        window
            .read_with(cx, |view, _| {
                assert_eq!(view.board.selected(), Some(top));
                assert_eq!(view.status, "Clear the tiles above this one first.");
            })
            .unwrap();
    }
}
