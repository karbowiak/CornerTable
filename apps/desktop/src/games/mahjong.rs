use corner_table_mahjong::{Board, DAILY_SEED, Face, Layout, MoveResult, Tile};
use gpui::{
    Context, Div, FontWeight, IntoElement, Render, Stateful, Window, div, prelude::*, px, rgb,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{GameEntry, GameLaunch};
use crate::theme;

const INSTRUCTION: &str = "Match identical tiles with no tile above and at least one open side.";
const NO_MATCHES: &str = "No free matches remain. Undo a move or restart the deal.";
const MAT_INSET: f32 = 15.;
const TILE_RATIO: f32 = 1.15;

pub struct GameView {
    board: Board,
    daily: bool,
    clean_clear: bool,
    highlighted: Option<[usize; 2]>,
    status: &'static str,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _cx: &mut Context<Self>) -> Self {
        let seed = match &launch.entry {
            GameEntry::Challenge(id) if id == "fixture-mahjong-001" => DAILY_SEED,
            GameEntry::Challenge(id) => id.bytes().fold(DAILY_SEED, |seed, byte| {
                seed.wrapping_mul(1099511628211) ^ u64::from(byte)
            }),
            _ => new_seed(),
        };
        Self {
            board: Board::new(seed),
            daily: matches!(&launch.entry, GameEntry::Challenge(_)),
            clean_clear: false,
            highlighted: None,
            status: INSTRUCTION,
        }
    }

    fn choose(&mut self, id: usize, cx: &mut Context<Self>) {
        self.highlighted = None;
        self.status = match self.board.select(id) {
            MoveResult::Blocked => "That tile is covered or has tiles on both sides.",
            MoveResult::Selected => "Choose another free tile with the same face.",
            MoveResult::Deselected => INSTRUCTION,
            MoveResult::DifferentFace => "Those tiles differ. Your last choice is selected.",
            MoveResult::Matched if self.board.remaining() == 0 => "Board cleared!",
            MoveResult::Matched if self.board.hint().is_none() => NO_MATCHES,
            MoveResult::Matched => "Pair matched.",
        };
        cx.notify();
    }

    fn new_deal(&mut self, layout: Layout) {
        self.board = Board::with_layout(layout, new_seed());
        self.highlighted = None;
        self.status = INSTRUCTION;
    }

    fn deal_options(&self, cx: &Context<Self>) -> Div {
        let layout = self.board.layout();
        div()
            .flex()
            .flex_wrap()
            .justify_between()
            .gap_3()
            .child(
                option_group("Layout").children(
                    [
                        ("mahjong-terrace", "Terrace", Layout::Terrace),
                        ("mahjong-atrium", "Atrium", Layout::Atrium),
                        ("mahjong-crown", "Crown", Layout::Crown),
                        ("mahjong-lantern", "Lantern", Layout::Lantern),
                    ]
                    .map(|(id, label, option)| {
                        choice(
                            id,
                            label,
                            layout == option,
                            move |view| {
                                if view.board.layout() != option {
                                    view.new_deal(option);
                                }
                            },
                            cx,
                        )
                    }),
                ),
            )
            .child(
                option_group("Objective")
                    .child(choice(
                        "mahjong-classic",
                        "Classic",
                        !self.clean_clear,
                        |view| view.clean_clear = false,
                        cx,
                    ))
                    .child(choice(
                        "mahjong-clean-clear",
                        "Clean Clear",
                        self.clean_clear,
                        |view| view.clean_clear = true,
                        cx,
                    )),
            )
    }

    fn tile(
        &self,
        id: usize,
        tile: &Tile,
        width: f32,
        height: f32,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let free = self.board.is_free(id);
        let selected = self.board.selected() == Some(id);
        let hinted = self.highlighted.is_some_and(|pair| pair.contains(&id));
        let emphasis = selected || hinted;
        let background = if emphasis {
            0xf5d78f
        } else if free {
            0xfff8e8
        } else {
            0xe2dcc8
        };
        let depth = f32::from(tile.level) * 2.8;
        let (label, mark, color) = match tile.face {
            Face::Dots(rank) => (rank.to_string(), "●", 0x254f83),
            Face::Bamboo(rank) => (rank.to_string(), "Ⅲ", 0x286345),
            Face::Characters(rank) => (rank.to_string(), "◆", 0x9a4439),
            Face::Wind(direction) => (direction.to_string(), "✦", 0x335662),
            Face::Flower => ("F".into(), "✿", 0x9a4439),
            Face::Season => ("S".into(), "◈", 0x7c4e26),
            Face::Dragon(color) => (
                color.to_string(),
                "♜",
                match color {
                    'R' => 0x9a4439,
                    'G' => 0x286345,
                    _ => 0x5e5368,
                },
            ),
        };

        div()
            .id(("mahjong-tile", id))
            .absolute()
            .left(px(f32::from(tile.x) * (width * 0.5) + MAT_INSET + depth))
            .top(px(f32::from(tile.y) * (height * 0.5) + MAT_INSET - depth))
            .w(px(width - 1.))
            .h(px(height - 1.))
            .border_1()
            .border_b_4()
            .border_color(rgb(if emphasis { 0xc68f37 } else { 0xa99b7e }))
            .rounded_md()
            .bg(rgb(background))
            .text_color(rgb(color))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .line_height(px(height * 0.36))
            .when(free, |div| {
                div.tab_index(0)
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0xffe9aa)))
                    .focus(|style| style.border_color(rgb(0xb37e24)))
            })
            .on_click(cx.listener(move |view, _, _, cx| view.choose(id, cx)))
            .child(
                div()
                    .text_size(px(height * 0.37))
                    .font_weight(FontWeight::BOLD)
                    .child(label),
            )
            .child(div().text_size(px(height * 0.28)).child(mark))
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
    action: impl Fn(&mut GameView) + 'static,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    super::control(id, label, enabled).when(enabled, |button| {
        button.on_click(cx.listener(move |view, _, _, cx| {
            action(view);
            cx.notify();
        }))
    })
}

fn choice(
    id: &'static str,
    label: &'static str,
    selected: bool,
    action: impl Fn(&mut GameView) + 'static,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    control(id, label, true, action, cx).when(selected, |button| {
        button.bg(rgb(theme::SAGE)).border_color(rgb(theme::MUTED))
    })
}

fn option_group(label: &'static str) -> Div {
    div().flex().flex_wrap().items_center().gap_2().child(
        div()
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme::MUTED))
            .child(label),
    )
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (extent_x, extent_y) = self.board.dimensions();
        let (columns, rows) = (f32::from(extent_x) * 0.5, f32::from(extent_y) * 0.5);
        let padding = MAT_INSET * 2.;
        let fit_width = (super::board_space(window) - padding - 2.) / columns;
        // Reserve space for the game page header, controls, and objective text.
        let fit_height =
            (f32::from(window.viewport_size().height) - 350. - padding) / (rows * TILE_RATIO);
        let width = fit_width.min(fit_height.max(30.)).clamp(22., 49.).floor();
        let height = width * TILE_RATIO;
        let remaining = self.board.remaining();
        let mut controls = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(control(
                "mahjong-hint",
                "Hint",
                remaining > 0,
                |view| {
                    view.highlighted = view.board.reveal_hint();
                    view.status = if view.highlighted.is_some() {
                        "The highlighted tiles form a free pair."
                    } else {
                        NO_MATCHES
                    };
                },
                cx,
            ))
            .child(control(
                "mahjong-undo",
                "Undo",
                self.board.can_undo(),
                |view| {
                    view.board.undo();
                    view.highlighted = None;
                    view.status = "Last pair restored.";
                },
                cx,
            ))
            .child(control(
                "mahjong-restart",
                "Restart",
                true,
                |view| {
                    view.board.restart();
                    view.highlighted = None;
                    view.status = INSTRUCTION;
                },
                cx,
            ));

        if !self.daily {
            controls = controls.child(control(
                "mahjong-new",
                "New deal",
                true,
                |view| view.new_deal(view.board.layout()),
                cx,
            ));
        }

        let objective = if self.board.used_assistance() {
            "Assisted run. Restart to try for a Clean Clear."
        } else {
            "Clear every tile without using Hint or Undo."
        };
        let status = if self.clean_clear && remaining == 0 && !self.board.used_assistance() {
            "Clean Clear complete!"
        } else {
            self.status
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .when(!self.daily, |root| root.child(self.deal_options(cx)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{remaining} tiles left")),
                    )
                    .child(controls),
            )
            .child(div().text_color(rgb(theme::MUTED)).child(status).when(
                self.clean_clear && (remaining > 0 || self.board.used_assistance()),
                |group| group.child(div().font_weight(FontWeight::SEMIBOLD).child(objective)),
            ))
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .overflow_hidden()
                    .rounded_xl()
                    .bg(rgb(0x204b3d))
                    .border_1()
                    .border_color(rgb(0x285246))
                    .child(
                        div()
                            .relative()
                            .w(px(width * columns + padding))
                            .h(px(height * rows + padding))
                            .children(
                                self.board
                                    .tiles()
                                    .map(|(id, tile)| self.tile(id, tile, width, height, cx)),
                            ),
                    ),
            )
    }
}
