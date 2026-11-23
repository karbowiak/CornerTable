use corner_table_mahjong::{Board, DAILY_SEED, Face, MoveResult, Tile};
use gpui::{
    Context, Div, FontWeight, IntoElement, Render, Stateful, Window, div, prelude::*, px, rgb,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{GameEntry, GameLaunch};
use crate::theme;

const INSTRUCTION: &str = "Match identical tiles with no tile above and at least one open side.";

pub struct GameView {
    board: Board,
    daily: bool,
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
            MoveResult::Matched if self.board.hint().is_none() => {
                "No free matches remain. Undo a move or restart the deal."
            }
            MoveResult::Matched => "Pair matched.",
        };
        cx.notify();
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
            .left(px(f32::from(tile.x) * (width * 0.5) + 10. + depth))
            .top(px(f32::from(tile.y) * (height * 0.5) + 14. - depth))
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
    active: bool,
    action: impl Fn(&mut GameView, &mut Context<GameView>) + 'static,
    cx: &Context<GameView>,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(theme::BORDER))
        .bg(rgb(theme::SURFACE))
        .text_color(rgb(if active { theme::INK } else { 0x7c817c }))
        .when(active, |button| {
            button
                .tab_index(0)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme::SAGE)))
                .focus(|style| style.border_color(rgb(theme::MUTED)))
                .on_click(cx.listener(move |view, _, _, cx| {
                    action(view, cx);
                    cx.notify();
                }))
        })
        .child(label)
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = f32::from(window.viewport_size().width);
        let chrome = if viewport < 1080. { 218. } else { 288. };
        let width = ((viewport - chrome - 62.) / 14.).clamp(22., 49.);
        let height = width * 1.15;
        let mut controls = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(control(
                "mahjong-hint",
                "Hint",
                self.board.remaining() > 0,
                |view, _| {
                    view.highlighted = view.board.hint();
                    view.status = if view.highlighted.is_some() {
                        "The highlighted tiles form a free pair."
                    } else {
                        "No free matches remain. Undo a move or restart the deal."
                    };
                },
                cx,
            ))
            .child(control(
                "mahjong-undo",
                "Undo",
                self.board.can_undo(),
                |view, _| {
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
                |view, _| {
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
                |view, _| {
                    view.board = Board::new(new_seed());
                    view.highlighted = None;
                    view.status = INSTRUCTION;
                },
                cx,
            ));
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(theme::display_type(
                if self.daily {
                    "Mahjong Daily Challenge"
                } else {
                    "Mahjong"
                },
                40.,
            ))
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
                            .child(format!("{} tiles left", self.board.remaining())),
                    )
                    .child(controls),
            )
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
                            .w(px(width * 14. + 22.))
                            .h(px(height * 8. + 28.))
                            .children(
                                self.board
                                    .tiles()
                                    .map(|(id, tile)| self.tile(id, tile, width, height, cx)),
                            ),
                    ),
            )
            .child(div().text_color(rgb(theme::MUTED)).child(self.status))
    }
}
