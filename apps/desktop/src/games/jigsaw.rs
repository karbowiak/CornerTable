mod artwork;
mod assemble;
mod swap;

use std::collections::HashMap;

use corner_table_jigsaw::{Assembly, Puzzle, Size, TrayFilter};
use gpui::{
    Context, Div, FocusHandle, FontWeight, ObjectFit, Render, Role, SharedString, Stateful, Window,
    div, img, prelude::*, px, rgb,
};

use crate::theme as t;

use super::{GameLaunch, board_space, control};
use artwork::PieceArt;
use assemble::TrayLayout;

const MAT: u32 = 0x25463e;
const FRAME_PADDING: f32 = 10.;
const WORKSPACE_GAP: f32 = 18.;
const MAX_TABLE_WIDTH: f32 = 1092.;

struct Painting {
    title: &'static str,
    asset: &'static str,
    aspect: f32,
}

const GALLERY: &[Painting] = &[
    Painting {
        title: "Lighthouse at Dusk",
        asset: "games/jigsaw/lighthouse.jpg",
        aspect: 4. / 3.,
    },
    Painting {
        title: "Harbor Morning",
        asset: "games/jigsaw/harbor.jpg",
        aspect: 3. / 2.,
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PuzzleKey {
    painting: usize,
    size: Size,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Assemble,
    Swap,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reference {
    Photo,
    Ghost,
    Off,
}

enum Artwork {
    Cutting,
    Ready(Vec<PieceArt>),
    Failed,
}

#[derive(Default)]
struct Tray {
    filter: TrayFilter,
    page: usize,
    selected: Option<usize>,
    hint: Option<usize>,
    hints_used: usize,
    missed: Option<usize>,
    cursor: usize,
}

struct Session {
    assembly: Assembly,
    swap: Puzzle,
    assembly_deal: u64,
    swap_deal: u64,
    tray: Tray,
}

impl Session {
    fn new(size: Size, seed: u64) -> Self {
        Self {
            assembly: Assembly::new(size, seed),
            swap: Puzzle::with_size(size, seed),
            assembly_deal: seed,
            swap_deal: seed,
            tray: Tray::default(),
        }
    }
}

pub struct GameView {
    key: PuzzleKey,
    sessions: HashMap<PuzzleKey, Session>,
    artwork: HashMap<PuzzleKey, Artwork>,
    seed: u64,
    mode: Mode,
    reference: Reference,
    confirm_restart: bool,
    board_focus: FocusHandle,
    piece_focus: Vec<FocusHandle>,
    all_pieces_focus: FocusHandle,
}

impl GameView {
    pub fn new(launch: &GameLaunch, cx: &mut Context<Self>) -> Self {
        let key = PuzzleKey {
            painting: 0,
            size: Size::Twelve,
        };
        let seed = launch.seed();
        Self {
            key,
            sessions: HashMap::from([(key, Session::new(key.size, seed))]),
            artwork: HashMap::new(),
            seed,
            mode: Mode::Assemble,
            reference: Reference::Photo,
            confirm_restart: false,
            board_focus: cx.focus_handle(),
            piece_focus: (0..Size::FortyEight.piece_count())
                .map(|_| cx.focus_handle())
                .collect(),
            all_pieces_focus: cx.focus_handle(),
        }
    }

    fn session(&self) -> &Session {
        &self.sessions[&self.key]
    }

    fn session_mut(&mut self) -> &mut Session {
        self.sessions
            .get_mut(&self.key)
            .expect("active puzzle exists")
    }

    fn painting(&self) -> &'static Painting {
        &GALLERY[self.key.painting]
    }

    fn choose(&mut self, key: PuzzleKey) {
        self.key = key;
        self.confirm_restart = false;
        self.sessions
            .entry(key)
            .or_insert_with(|| Session::new(key.size, self.seed));
    }

    fn prepare_artwork(&mut self, cx: &mut Context<Self>) {
        let key = self.key;
        let asset = self.painting().asset;
        self.artwork.insert(key, Artwork::Cutting);
        let task = cx
            .background_executor()
            .spawn(async move { artwork::cut_painting(asset, key.size) });
        cx.spawn(async move |view, cx| {
            let state = match task.await {
                Ok(pieces) => Artwork::Ready(pieces),
                Err(error) => {
                    eprintln!("Could not prepare {asset}: {error:#}");
                    Artwork::Failed
                }
            };
            view.update(cx, |view, cx| {
                view.artwork.insert(key, state);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn restart(&mut self) {
        self.confirm_restart = false;
        self.seed = self.seed.wrapping_add(1);
        let (size, seed) = (self.key.size, self.seed);
        let mode = self.mode;
        let session = self.session_mut();
        match mode {
            Mode::Assemble => {
                session.assembly = Assembly::new(size, seed);
                session.assembly_deal = seed;
                session.tray = Tray::default();
            }
            Mode::Swap => {
                session.swap = Puzzle::with_size(size, seed);
                session.swap_deal = seed;
            }
        }
    }

    fn placed_count(&self) -> usize {
        match self.mode {
            Mode::Assemble => self.session().assembly.placed_count(),
            Mode::Swap => self.session().swap.placed_count(),
        }
    }

    fn is_complete(&self) -> bool {
        self.placed_count() == self.key.size.piece_count()
    }

    fn reference(&self) -> Reference {
        match (self.mode, self.reference) {
            (Mode::Swap, Reference::Ghost) => Reference::Photo,
            (_, reference) => reference,
        }
    }

    fn toolbar(&self, tray_layout: TrayLayout, cx: &Context<Self>) -> Div {
        let can_undo = self.mode == Mode::Assemble && self.placed_count() != 0;
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(
                        segments().children(
                            [
                                (Mode::Assemble, "jigsaw-assemble", "Assemble"),
                                (Mode::Swap, "jigsaw-swap", "Swap"),
                            ]
                            .into_iter()
                            .map(|(mode, id, name)| {
                                segment(id, name, mode == self.mode).on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        view.mode = mode;
                                        view.confirm_restart = false;
                                        cx.notify();
                                    },
                                ))
                            }),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                segments().children(
                                    Size::ALL
                                        .into_iter()
                                        .zip(["jigsaw-12", "jigsaw-24", "jigsaw-48"])
                                        .map(|(size, id)| {
                                            segment(
                                                id,
                                                size.piece_count().to_string(),
                                                self.key.size == size,
                                            )
                                            .aria_label(format!("{} pieces", size.piece_count()))
                                            .on_click(
                                                cx.listener(move |view, _, _, cx| {
                                                    view.choose(PuzzleKey { size, ..view.key });
                                                    cx.notify();
                                                }),
                                            )
                                        }),
                                ),
                            )
                            .child(small_text("pieces")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(self.confirm_restart, |row| {
                        row.child(small_text("Restart this puzzle?")).child(
                            action_button("jigsaw-cancel-restart", "Cancel", true).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.confirm_restart = false;
                                    cx.notify();
                                }),
                            ),
                        )
                    })
                    .when(
                        self.mode == Mode::Assemble && !self.confirm_restart,
                        |row| {
                            row.child(action_button("jigsaw-undo", "Undo", can_undo).when(
                                can_undo,
                                |button| {
                                    button.on_click(cx.listener(move |view, _, _, cx| {
                                        view.undo_placement(tray_layout.capacity());
                                        cx.notify();
                                    }))
                                },
                            ))
                        },
                    )
                    .when(!self.is_complete(), |row| {
                        row.child(
                            action_button("jigsaw-restart", "Restart", true)
                                .when(self.confirm_restart, |button| button.bg(rgb(t::GOLD)))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    let progressed = match view.mode {
                                        Mode::Assemble => view.placed_count() != 0,
                                        Mode::Swap => view.session().swap.moves() != 0,
                                    };
                                    if progressed && !view.is_complete() && !view.confirm_restart {
                                        view.confirm_restart = true;
                                    } else {
                                        view.restart();
                                    }
                                    cx.notify();
                                })),
                        )
                    }),
            )
    }

    fn gallery(&self, cx: &Context<Self>) -> Stateful<Div> {
        div()
            .id("jigsaw-gallery")
            .role(Role::Group)
            .aria_label("Paintings")
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(small_text("Paintings"))
            .children(GALLERY.iter().enumerate().map(|(index, painting)| {
                let current = index == self.key.painting;
                div()
                    .id(("jigsaw-painting", index))
                    .role(Role::Button)
                    .aria_label(painting.title)
                    .aria_selected(current)
                    .tab_index(0)
                    .w(px(61.))
                    .h(px(40.))
                    .p(px(2.))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(if current { t::RAIL_ACTIVE } else { t::BORDER }))
                    .bg(rgb(if current { t::RAIL_ACTIVE } else { t::PAPER }))
                    .cursor_pointer()
                    .hover(|style| style.border_color(rgb(t::RAIL_ACTIVE)))
                    .focus_visible(|style| style.bg(rgb(t::GOLD)).border_color(rgb(t::INK)))
                    .child(
                        img(painting.asset)
                            .size_full()
                            .rounded_sm()
                            .object_fit(ObjectFit::Cover),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.choose(PuzzleKey {
                            painting: index,
                            ..view.key
                        });
                        cx.notify();
                    }))
            }))
    }

    fn reference_panel(&self, width: f32, viewport_height: f32, cx: &Context<Self>) -> Div {
        let reference = self.reference();
        div()
            .w(px(width))
            .flex_none()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_x_2()
                    .gap_y_1()
                    .child(t::display_type("Reference", 17.))
                    .child(
                        segments().children(
                            [
                                (
                                    Reference::Photo,
                                    "jigsaw-photo",
                                    "Photo",
                                    "Show the reference photo",
                                ),
                                (
                                    Reference::Ghost,
                                    "jigsaw-ghost",
                                    "Ghost",
                                    "Show a faint reference on the board",
                                ),
                                (
                                    Reference::Off,
                                    "jigsaw-hide-reference",
                                    "Off",
                                    "Hide the reference",
                                ),
                            ]
                            .into_iter()
                            .filter(|(value, _, _, _)| {
                                *value != Reference::Ghost || self.mode == Mode::Assemble
                            })
                            .map(|(value, id, label, description)| {
                                segment(id, label, value == reference)
                                    .aria_label(description)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.reference = value;
                                        cx.notify();
                                    }))
                            }),
                        ),
                    ),
            )
            .when(reference == Reference::Photo, |panel| {
                let limit = if viewport_height < 660. { 120. } else { 210. };
                let height = (width / self.painting().aspect).min(limit);
                panel.child(
                    div().h(px(height)).flex().justify_center().child(
                        img(self.painting().asset)
                            .w(px(height * self.painting().aspect))
                            .h_full()
                            .object_fit(ObjectFit::Fill)
                            .rounded_md(),
                    ),
                )
            })
    }

    fn board_footer(&self, window: &Window) -> Stateful<Div> {
        let session = self.session();
        let keyboard = self.board_focus.is_focused(window);
        let message = match self.mode {
            Mode::Swap if session.swap.selected().is_some() => "Choose another tile to swap.",
            Mode::Swap => "Drag tiles to swap them, or select two in turn. Correct tiles lock.",
            Mode::Assemble if session.tray.missed.is_some() => {
                "That spot doesn’t fit. Try another."
            }
            Mode::Assemble if session.tray.hint.is_some() && keyboard => {
                "The highlighted spot fits. Press Enter to place the piece."
            }
            Mode::Assemble if session.tray.hint.is_some() => {
                "The highlighted spot fits the selected piece."
            }
            Mode::Assemble if session.tray.selected.is_some() && keyboard => {
                "Use the arrow keys to move, Enter to place, or Esc to cancel."
            }
            Mode::Assemble if session.tray.selected.is_some() => "Choose a spot on the board.",
            Mode::Assemble => "Drag a piece onto the board, or select it and choose a spot.",
        };
        div()
            .id("jigsaw-board-status")
            .role(Role::Status)
            .aria_label(message)
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_x_3()
            .gap_y_1()
            .child(small_text(message))
            .when(self.mode == Mode::Swap, |footer| {
                footer.child(small_text(self.moves_text()))
            })
    }

    fn moves_text(&self) -> String {
        match self.session().swap.moves() {
            1 => "1 move".into(),
            moves => format!("{moves} moves"),
        }
    }

    fn progress(&self) -> Div {
        let placed = self.placed_count();
        let total = self.key.size.piece_count();
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .h(px(4.))
                    .flex_1()
                    .rounded_full()
                    .overflow_hidden()
                    .bg(rgb(t::SAGE))
                    .child(
                        div()
                            .h_full()
                            .w(gpui::relative(placed as f32 / total as f32))
                            .bg(rgb(t::RAIL_ACTIVE)),
                    ),
            )
            .child(
                small_text(format!("{placed} / {total} placed")).font_weight(FontWeight::SEMIBOLD),
            )
    }

    fn completion(&self, width: f32, cx: &Context<Self>) -> Div {
        let summary = match self.mode {
            Mode::Assemble => match self.session().tray.hints_used {
                0 => "No hints used".into(),
                1 => "1 hint used".into(),
                hints => format!("{hints} hints used"),
            },
            Mode::Swap => self.moves_text(),
        };
        div()
            .w(px(width))
            .flex_none()
            .flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap_3()
            .p_3()
            .rounded_md()
            .bg(rgb(t::SAGE))
            .child(
                div()
                    .id("jigsaw-complete")
                    .role(Role::Status)
                    .aria_label(format!(
                        "Picture complete. {} pieces. {summary}.",
                        self.key.size.piece_count()
                    ))
                    .min_w_0()
                    .child(t::display_type("Picture complete", 20.))
                    .child(small_text(format!(
                        "{} pieces · {summary}",
                        self.key.size.piece_count()
                    ))),
            )
            .child(
                action_button("jigsaw-play-again", "Play again", true).on_click(cx.listener(
                    |view, _, _, cx| {
                        view.restart();
                        cx.notify();
                    },
                )),
            )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.mode == Mode::Assemble && !self.artwork.contains_key(&self.key) {
            self.prepare_artwork(cx);
        }

        let available = board_space(window).min(MAX_TABLE_WIDTH - 34.);
        let wide = available >= 710.;
        let side_width = if wide {
            (available * 0.28).clamp(235., 288.)
        } else {
            176.
        };
        let height = f32::from(window.viewport_size().height);
        let max_image_height = (height - 385.).clamp(240., 516.);
        let max_board_width = if wide {
            available - side_width - WORKSPACE_GAP
        } else {
            available
        };
        let board_width = (max_board_width - 2. * (FRAME_PADDING + 1.))
            .min(max_image_height * self.painting().aspect);
        let board_height = board_width / self.painting().aspect;
        let frame_width = board_width + 2. * (FRAME_PADDING + 1.);
        let tray_width = if wide {
            side_width
        } else {
            available - side_width - WORKSPACE_GAP
        };
        let tray_layout = TrayLayout::new(tray_width, wide, height);
        let complete = self.is_complete();
        let art = match self.artwork.get(&self.key) {
            Some(Artwork::Ready(pieces)) => Some(pieces.as_slice()),
            _ => None,
        };
        let board = match self.mode {
            Mode::Assemble => {
                self.assembly_board((board_width, board_height), art, tray_layout, window, cx)
            }
            Mode::Swap => self.swap_board(board_width, cx),
        };
        let tray = match (self.mode, complete, art) {
            (Mode::Assemble, false, Some(art)) => {
                Some(self.piece_tray(tray_layout, (board_width, board_height), art, cx))
            }
            _ => None,
        };
        let reference = (!complete).then(|| self.reference_panel(side_width, height, cx));
        let side = if wide {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .children(reference)
                .children(tray)
                .when(complete, |side| side.child(self.completion(side_width, cx)))
        } else {
            div()
                .flex()
                .items_start()
                .gap(px(WORKSPACE_GAP))
                .children(tray)
                .children(reference)
        };

        t::surface()
            .w_full()
            .max_w(px(MAX_TABLE_WIDTH))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.toolbar(tray_layout, cx))
            .child(div().h(px(1.)).bg(rgb(t::BORDER)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(t::display_type(self.painting().title, 23.))
                    .child(self.gallery(cx)),
            )
            .child(
                div()
                    .flex()
                    .when(!wide, |layout| layout.flex_col())
                    .items_start()
                    .gap(px(WORKSPACE_GAP))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(px(frame_width))
                            .gap_2()
                            .when(complete && !wide, |column| {
                                column.child(self.completion(frame_width, cx))
                            })
                            .child(board)
                            .when(!complete, |column| {
                                column
                                    .child(self.progress())
                                    .child(self.board_footer(window))
                            }),
                    )
                    .when(wide || !complete, |layout| layout.child(side)),
            )
    }
}

fn framed_board(board: Div) -> Div {
    div()
        .p(px(FRAME_PADDING))
        .border_1()
        .border_color(rgb(0x39584e))
        .rounded_lg()
        .bg(rgb(MAT))
        .child(board.rounded_sm().overflow_hidden())
}

fn small_text(label: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(rgb(t::MUTED))
        .child(label.into())
}

fn segments() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(1.))
        .p(px(2.))
        .rounded_md()
        .border_1()
        .border_color(rgb(t::BORDER))
        .bg(rgb(t::PAPER))
}

fn segment(id: &'static str, label: impl Into<SharedString>, current: bool) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_selected(current)
        .tab_index(0)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_h(px(28.))
        .px_2()
        .rounded_sm()
        .cursor_pointer()
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .when(current, |button| {
            button.bg(rgb(t::RAIL_ACTIVE)).text_color(rgb(t::SURFACE))
        })
        .when(!current, |button| {
            button.hover(|style| style.bg(rgb(t::SAGE)))
        })
        .focus_visible(|style| style.bg(rgb(t::GOLD)).text_color(rgb(t::INK)))
        .child(label)
}

fn action_button(id: &'static str, name: &'static str, enabled: bool) -> Stateful<Div> {
    control(id, name, enabled)
        .role(if enabled { Role::Button } else { Role::Label })
        .aria_label(name)
        .min_h(px(34.))
}
