mod artwork;
mod assemble;
mod gallery;
mod photos;
mod progress;
mod swap;

use std::{borrow::Cow, collections::HashMap, path::Path, sync::Arc};

use corner_table_jigsaw::{Assembly, Puzzle, Size, TrayFilter};
use gpui::{
    App, Context, Div, FocusHandle, FontWeight, ObjectFit, Render, Role, SharedString, Stateful,
    Toggled, Window, div, img, prelude::*, px, rgb,
};
use serde::{Deserialize, Serialize};

use crate::storage::{AttemptId, Outcome, ResultMetric, Session};
use crate::theme as t;

use super::{GameLaunch, board_space, themed_control};
use artwork::PieceArt;
use assemble::TrayLayout;
use gallery::Gallery;
use progress::SavedGallery;

const MAT: u32 = 0x25463e;
const FRAME_PADDING: f32 = 10.;
const WORKSPACE_GAP: f32 = 18.;
const MAX_TABLE_WIDTH: f32 = 1092.;

struct Painting {
    id: Cow<'static, str>,
    title: Cow<'static, str>,
    source: PictureSource,
    thumbnail: Option<Arc<Path>>,
    aspect: f32,
}

#[derive(Clone)]
enum PictureSource {
    Bundled(&'static str),
    Local(Arc<Path>),
}

impl PictureSource {
    fn image(&self) -> gpui::Img {
        match self {
            Self::Bundled(asset) => img(*asset),
            Self::Local(path) => img(path.clone()),
        }
    }
}

impl Painting {
    const fn bundled(
        id: &'static str,
        title: &'static str,
        asset: &'static str,
        aspect: f32,
    ) -> Self {
        Self {
            id: Cow::Borrowed(id),
            title: Cow::Borrowed(title),
            source: PictureSource::Bundled(asset),
            thumbnail: None,
            aspect,
        }
    }
}

const GALLERY: &[Painting] = &[
    Painting::bundled(
        "lighthouse",
        "Lighthouse at Dusk",
        "games/jigsaw/lighthouse.jpg",
        4. / 3.,
    ),
    Painting::bundled(
        "harbor",
        "Harbor Morning",
        "games/jigsaw/harbor.jpg",
        3. / 2.,
    ),
    Painting::bundled(
        "greenhouse-afternoon",
        "Greenhouse Afternoon",
        "games/jigsaw/greenhouse-afternoon.jpg",
        1280. / 853.,
    ),
    Painting::bundled(
        "snowbound-station",
        "Snowbound Station",
        "games/jigsaw/snowbound-station.jpg",
        1280. / 853.,
    ),
    Painting::bundled(
        "orchard-market",
        "Orchard Market",
        "games/jigsaw/orchard-market.jpg",
        1280. / 853.,
    ),
    Painting::bundled(
        "reef-at-noon",
        "Reef at Noon",
        "games/jigsaw/reef-at-noon.jpg",
        1280. / 853.,
    ),
    Painting::bundled(
        "rainy-bookshop",
        "Rainy Bookshop",
        "games/jigsaw/rainy-bookshop.jpg",
        1280. / 853.,
    ),
];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PuzzleKey {
    painting: usize,
    size: Size,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Assemble,
    Swap,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
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

struct Round {
    assembly: Assembly,
    swap: Puzzle,
    assembly_deal: u64,
    swap_deal: u64,
    assembly_attempt: AttemptId,
    swap_attempt: AttemptId,
    assembly_moves: u64,
    tray: Tray,
}

impl Round {
    fn new(size: Size, seed: u64) -> Self {
        Self {
            assembly: Assembly::new(size, seed),
            swap: Puzzle::with_size(size, seed),
            assembly_deal: seed,
            swap_deal: seed,
            assembly_attempt: AttemptId::default(),
            swap_attempt: AttemptId::default(),
            assembly_moves: 0,
            tray: Tray::default(),
        }
    }

    fn placed_count(&self, mode: Mode) -> usize {
        match mode {
            Mode::Assemble => self.assembly.placed_count(),
            Mode::Swap => self.swap.placed_count(),
        }
    }
}

pub struct GameView {
    key: PuzzleKey,
    rounds: HashMap<PuzzleKey, Round>,
    storage: Session,
    gallery: Gallery,
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
        let storage = Session::new(launch);
        let saved = storage.load::<SavedGallery>();
        let mut view = Self {
            key,
            rounds: HashMap::new(),
            storage,
            gallery: Gallery::load(photos::directory()),
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
        };
        if let Some(saved) = saved {
            view.restore(saved);
            view.gallery.reveal(view.key.painting);
        }
        view.rounds
            .entry(view.key)
            .or_insert_with(|| Round::new(view.key.size, view.seed));
        view
    }

    fn round(&self) -> &Round {
        &self.rounds[&self.key]
    }

    fn round_mut(&mut self) -> &mut Round {
        self.rounds
            .get_mut(&self.key)
            .expect("active puzzle exists")
    }

    fn painting(&self) -> &Painting {
        self.gallery.painting(self.key.painting)
    }

    fn choose(&mut self, key: PuzzleKey) {
        self.confirm_restart = false;
        if self.key != key {
            self.key = key;
            self.artwork
                .retain(|cached, _| cached.painting < GALLERY.len() || *cached == key);
            self.rounds
                .entry(key)
                .or_insert_with(|| Round::new(key.size, self.seed));
            self.save();
        }
    }

    fn prepare_artwork(&mut self, cx: &mut Context<Self>) {
        let key = self.key;
        let painting = self.painting();
        let source = painting.source.clone();
        let id = painting.id.clone();
        self.artwork.insert(key, Artwork::Cutting);
        let task = cx
            .background_executor()
            .spawn(async move { artwork::cut_painting(&source, key.size) });
        cx.spawn(async move |view, cx| {
            let state = match task.await {
                Ok(pieces) => Artwork::Ready(pieces),
                Err(error) => {
                    eprintln!("Could not prepare Jigsaw puzzle {id}: {error:#}");
                    Artwork::Failed
                }
            };
            view.update(cx, |view, cx| {
                if let Some(artwork) = view.artwork.get_mut(&key) {
                    *artwork = state;
                    cx.notify();
                }
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
        let round = self.round_mut();
        match mode {
            Mode::Assemble => {
                round.assembly = Assembly::new(size, seed);
                round.assembly_deal = seed;
                round.assembly_attempt = AttemptId::default();
                round.assembly_moves = 0;
                round.tray = Tray::default();
            }
            Mode::Swap => {
                round.swap = Puzzle::with_size(size, seed);
                round.swap_deal = seed;
                round.swap_attempt = AttemptId::default();
            }
        }
        self.save();
    }

    fn placed_count(&self) -> usize {
        self.round().placed_count(self.mode)
    }

    fn is_complete(&self) -> bool {
        self.placed_count() == self.key.size.piece_count()
    }

    fn record_win(&self) {
        let round = self.round();
        let (attempt, mode, moves) = match self.mode {
            Mode::Assemble => (round.assembly_attempt, "Assemble", round.assembly_moves),
            Mode::Swap => (round.swap_attempt, "Swap", round.swap.moves() as u64),
        };
        self.storage.record_result(
            attempt,
            format!(
                "{mode} · {} · {} pieces",
                self.painting().title,
                self.key.size.piece_count()
            ),
            Outcome::Won,
            ResultMetric::Moves(moves),
        );
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
                        segments(cx).children(
                            [
                                (Mode::Assemble, "jigsaw-assemble", "Assemble"),
                                (Mode::Swap, "jigsaw-swap", "Swap"),
                            ]
                            .into_iter()
                            .map(|(mode, id, name)| {
                                segment(id, name, mode == self.mode, cx).on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        view.confirm_restart = false;
                                        if view.mode != mode {
                                            view.mode = mode;
                                            view.save();
                                        }
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
                                segments(cx).children(
                                    Size::ALL
                                        .into_iter()
                                        .zip(["jigsaw-12", "jigsaw-24", "jigsaw-48"])
                                        .map(|(size, id)| {
                                            segment(
                                                id,
                                                size.piece_count().to_string(),
                                                self.key.size == size,
                                                cx,
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
                            .child(small_text("pieces", cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(self.confirm_restart, |row| {
                        row.child(small_text("Restart this puzzle?", cx)).child(
                            action_button("jigsaw-cancel-restart", "Cancel", true, cx).on_click(
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
                            row.child(action_button("jigsaw-undo", "Undo", can_undo, cx).when(
                                can_undo,
                                |button| {
                                    button.on_click(cx.listener(move |view, _, _, cx| {
                                        view.undo_placement(tray_layout.capacity(), cx);
                                        cx.notify();
                                    }))
                                },
                            ))
                        },
                    )
                    .when(!self.is_complete(), |row| {
                        row.child(
                            action_button("jigsaw-restart", "Restart", true, cx)
                                .when(self.confirm_restart, |button| {
                                    button
                                        .bg(rgb(t::GOLD))
                                        .text_color(rgb(t::INK))
                                        .hover(|style| style.bg(rgb(t::GOLD_HOVER)))
                                })
                                .on_click(cx.listener(|view, _, _, cx| {
                                    let progressed = match view.mode {
                                        Mode::Assemble => view.placed_count() != 0,
                                        Mode::Swap => view.round().swap.moves() != 0,
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

    fn gallery_row(&self, available: f32, cx: &Context<Self>) -> Stateful<Div> {
        const GAP: f32 = 7.;
        let pictures = self.gallery.visible();
        let count = pictures.len().max(1) as f32;
        let width = ((available - GAP * (count - 1.)) / count).min(138.);
        let palette = t::colors(cx);
        let total = self.key.size.piece_count();
        div()
            .id("jigsaw-gallery")
            .role(Role::Group)
            .aria_label(self.gallery.tab.label())
            .flex()
            .gap(px(GAP))
            .children(pictures.map(|(index, painting)| {
                let current = index == self.key.painting;
                let key = PuzzleKey {
                    painting: index,
                    ..self.key
                };
                let round = self.rounds.get(&key);
                let placed = round.map_or(0, |round| round.placed_count(self.mode));
                let started = placed != 0
                    || (self.mode == Mode::Swap
                        && round.is_some_and(|round| round.swap.moves() != 0));
                div()
                    .id(("jigsaw-painting", index))
                    .role(Role::Button)
                    .aria_label(painting.title.as_ref().to_owned())
                    .aria_toggled(if current {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .when(started, |tile| {
                        tile.aria_description(if placed == total {
                            "Picture complete".into()
                        } else {
                            format!("{placed} of {total} pieces placed")
                        })
                    })
                    .tab_index(0)
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(width))
                    .p(px(4.))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(if current {
                        palette.gold_text
                    } else {
                        palette.border
                    }))
                    .bg(rgb(if current {
                        palette.hover
                    } else {
                        palette.paper
                    }))
                    .cursor_pointer()
                    .hover(move |style| style.bg(rgb(palette.hover)))
                    .active(move |style| style.border_color(rgb(palette.ink)))
                    .focus_visible(move |style| {
                        style.border_color(rgb(palette.ink)).bg(rgb(palette.hover))
                    })
                    .child(
                        painting
                            .thumbnail
                            .as_ref()
                            .map_or_else(|| painting.source.image(), |path| img(path.clone()))
                            .w_full()
                            .h(px(if painting.thumbnail.is_some() {
                                (width - 10.) / painting.aspect
                            } else {
                                48.
                            }))
                            .rounded_sm()
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(
                        div().flex().items_center().min_h(px(30.)).px_1().child(
                            div()
                                .w_full()
                                .min_w_0()
                                .whitespace_normal()
                                .text_size(px(11.))
                                .line_height(px(13.))
                                .font_weight(if current {
                                    FontWeight::BOLD
                                } else {
                                    FontWeight::SEMIBOLD
                                })
                                .child(painting.title.as_ref().to_owned()),
                        ),
                    )
                    .child(
                        div()
                            .h(px(3.))
                            .w_full()
                            .rounded_full()
                            .overflow_hidden()
                            .when(started, |track| {
                                track.bg(rgb(palette.border)).child(
                                    div()
                                        .h_full()
                                        .min_w(px(4.))
                                        .w(gpui::relative(placed as f32 / total as f32))
                                        .bg(rgb(if placed == total {
                                            palette.success
                                        } else {
                                            palette.gold_text
                                        })),
                                )
                            }),
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
                        segments(cx).children(
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
                                segment(id, label, value == reference, cx)
                                    .aria_label(description)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.reference != value {
                                            view.reference = value;
                                            view.save();
                                            cx.notify();
                                        }
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
                        self.painting()
                            .source
                            .image()
                            .w(px(height * self.painting().aspect))
                            .h_full()
                            .object_fit(ObjectFit::Fill)
                            .rounded_md(),
                    ),
                )
            })
    }

    fn board_footer(&self, window: &Window, cx: &App) -> Stateful<Div> {
        let round = self.round();
        let keyboard = self.board_focus.is_focused(window);
        let message = match self.mode {
            Mode::Swap if round.swap.selected().is_some() => "Choose another tile to swap.",
            Mode::Swap => "Drag tiles to swap them, or select two in turn. Correct tiles lock.",
            Mode::Assemble if round.tray.missed.is_some() => "That spot doesn’t fit. Try another.",
            Mode::Assemble if round.tray.hint.is_some() && keyboard => {
                "The highlighted spot fits. Press Enter to place the piece."
            }
            Mode::Assemble if round.tray.hint.is_some() => {
                "The highlighted spot fits the selected piece."
            }
            Mode::Assemble if round.tray.selected.is_some() && keyboard => {
                "Use the arrow keys to move, Enter to place, or Esc to cancel."
            }
            Mode::Assemble if round.tray.selected.is_some() => "Choose a spot on the board.",
            Mode::Assemble => "Drag a piece onto the board, or select it and choose a spot.",
        };
        div()
            .id("jigsaw-board-status")
            .role(Role::Status)
            .aria_label(if self.mode == Mode::Swap {
                format!("{message} {}", self.moves_text())
            } else {
                message.into()
            })
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_x_3()
            .gap_y_1()
            .child(small_text(message, cx))
            .when(self.mode == Mode::Swap, |footer| {
                footer.child(small_text(self.moves_text(), cx))
            })
    }

    fn moves_text(&self) -> String {
        match self.round().swap.moves() {
            1 => "1 move".into(),
            moves => format!("{moves} moves"),
        }
    }

    fn progress(&self, cx: &App) -> Stateful<Div> {
        let placed = self.placed_count();
        let total = self.key.size.piece_count();
        let palette = t::colors(cx);
        div()
            .id("jigsaw-pieces-placed")
            .role(Role::Status)
            .aria_label(format!("{placed} of {total} pieces placed"))
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .h(px(4.))
                    .flex_1()
                    .rounded_full()
                    .overflow_hidden()
                    .bg(rgb(palette.border))
                    .child(
                        div()
                            .h_full()
                            .w(gpui::relative(placed as f32 / total as f32))
                            .bg(rgb(palette.success)),
                    ),
            )
            .child(
                small_text(format!("{placed} / {total} placed"), cx)
                    .font_weight(FontWeight::SEMIBOLD),
            )
    }

    fn completion(&self, width: f32, cx: &Context<Self>) -> Div {
        let summary = match self.mode {
            Mode::Assemble => match self.round().tray.hints_used {
                0 => "No hints used".into(),
                1 => "1 hint used".into(),
                hints => format!("{hints} hints used"),
            },
            Mode::Swap => self.moves_text(),
        };
        let palette = t::colors(cx);
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
            .bg(rgb(palette.hover))
            .border_1()
            .border_color(rgb(palette.border))
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
                    .child(small_text(
                        format!("{} pieces · {summary}", self.key.size.piece_count()),
                        cx,
                    )),
            )
            .child(
                action_button("jigsaw-play-again", "Play again", true, cx).on_click(cx.listener(
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
        let max_image_height = (height - 425.).clamp(240., 516.);
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

        t::panel(cx)
            .w_full()
            .max_w(px(MAX_TABLE_WIDTH))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.gallery_panel(available, cx))
            .child(div().h(px(1.)).bg(rgb(t::colors(cx).border)))
            .child(self.toolbar(tray_layout, cx))
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
                                    .child(self.progress(cx))
                                    .child(self.board_footer(window, cx))
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

fn small_text(label: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(rgb(t::colors(cx).muted))
        .child(label.into())
}

fn segments(cx: &App) -> Div {
    let palette = t::colors(cx);
    div()
        .flex()
        .items_center()
        .gap(px(1.))
        .p(px(2.))
        .rounded_md()
        .border_1()
        .border_color(rgb(palette.border))
        .bg(rgb(palette.paper))
}

fn segment(
    id: &'static str,
    label: impl Into<SharedString>,
    current: bool,
    cx: &App,
) -> Stateful<Div> {
    let label = label.into();
    let palette = t::colors(cx);
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_toggled(if current {
            Toggled::True
        } else {
            Toggled::False
        })
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
            button.bg(rgb(palette.ink)).text_color(rgb(palette.surface))
        })
        .when(!current, |button| {
            button.hover(move |style| style.bg(rgb(palette.hover)))
        })
        .focus_visible(|style| style.bg(rgb(t::GOLD)).text_color(rgb(t::INK)))
        .child(label)
}

fn action_button(id: &'static str, name: &'static str, enabled: bool, cx: &App) -> Stateful<Div> {
    themed_control(cx, id, name, enabled).min_h(px(34.))
}
