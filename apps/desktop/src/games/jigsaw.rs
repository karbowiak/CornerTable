mod artwork;
mod assemble;
mod gallery;
mod photos;
mod progress;
mod swap;
#[cfg(test)]
mod tests;

use std::{borrow::Cow, collections::HashMap, path::Path, sync::Arc};

use corner_table_jigsaw::{Assembly, Puzzle, Size, TrayFilter};
use gpui::{
    AnyElement, App, Context, Div, FocusHandle, FontWeight, ObjectFit, Render, Role, SharedString,
    Stateful, Toggled, Window, div, img, prelude::*, px, rgb,
};
use serde::{Deserialize, Serialize};

use crate::storage::{AttemptId, Outcome, ResultMetric, Session};
use crate::theme as t;

use super::{GameLaunch, board_space, round_result, themed_control};
use artwork::PieceArt;
use assemble::{TableLayout, TrayLayout};
use gallery::Gallery;
use progress::SavedGallery;

const MAT: u32 = 0x3a2e23;
const FRAME_PADDING: f32 = 10.;
const WORKSPACE_GAP: f32 = 18.;
const RESULT_WIDTH: f32 = 276.;

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

const AFTER_DARK_START: usize = 9;

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
    Painting::bundled(
        "coastal-railway",
        "Coastal Railway",
        "games/jigsaw/coastal-railway.jpg",
        3. / 2.,
    ),
    Painting::bundled(
        "citrus-courtyard",
        "Citrus Courtyard",
        "games/jigsaw/citrus-courtyard.jpg",
        3. / 2.,
    ),
    Painting::bundled(
        "lantern-market",
        "Lantern Market",
        "games/jigsaw/after-dark/lantern-market.jpg",
        1280. / 853.,
    ),
    Painting::bundled(
        "hilltop-observatory",
        "Hilltop Observatory",
        "games/jigsaw/after-dark/hilltop-observatory.jpg",
        1280. / 853.,
    ),
    Painting::bundled(
        "moonlit-reef",
        "Moonlit Reef",
        "games/jigsaw/after-dark/moonlit-reef.jpg",
        1280. / 853.,
    ),
    Painting::bundled(
        "last-tram",
        "Last Tram",
        "games/jigsaw/after-dark/last-tram.jpg",
        1280. / 853.,
    ),
    Painting::bundled(
        "moths-at-the-window",
        "Moths at the Window",
        "games/jigsaw/after-dark/moths-at-the-window.jpg",
        1280. / 853.,
    ),
    Painting::bundled(
        "northern-lights",
        "Northern Lights",
        "games/jigsaw/after-dark/northern-lights.jpg",
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

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
struct LoosePiece {
    piece: usize,
    position: [f32; 2],
}

struct Round {
    assembly: Assembly,
    swap: Puzzle,
    assembly_deal: u64,
    swap_deal: u64,
    assembly_attempt: AttemptId,
    swap_attempt: AttemptId,
    assembly_moves: u64,
    loose: Vec<LoosePiece>,
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
            loose: Vec::new(),
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
    show_grid: bool,
    confirm_restart: bool,
    settling_piece: Option<usize>,
    board_focus: FocusHandle,
    piece_focus: Vec<FocusHandle>,
    all_pieces_focus: FocusHandle,
    mode_focus: FocusHandle,
    result_focus: FocusHandle,
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
            reference: Reference::Ghost,
            show_grid: true,
            confirm_restart: false,
            settling_piece: None,
            board_focus: cx.focus_handle(),
            piece_focus: (0..Size::FortyEight.piece_count())
                .map(|_| cx.focus_handle())
                .collect(),
            all_pieces_focus: cx.focus_handle(),
            mode_focus: cx.focus_handle().tab_stop(true),
            result_focus: cx.focus_handle().tab_stop(true),
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
            self.settling_piece = None;
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
        self.settling_piece = None;
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
                round.loose.clear();
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
                                segment(id, name, mode == self.mode, cx)
                                    .debug_selector(|| id.into())
                                    .when(mode == self.mode, |button| {
                                        button.track_focus(&self.mode_focus)
                                    })
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.confirm_restart = false;
                                        if view.mode != mode {
                                            view.mode = mode;
                                            view.settling_piece = None;
                                            view.save();
                                            view.mode_focus.focus(window, cx);
                                        }
                                        cx.notify();
                                    }))
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
                    .when(self.mode == Mode::Assemble && !self.is_complete(), |row| {
                        row.child(
                            segments(cx).child(
                                segment("jigsaw-grid", "Grid", self.show_grid, cx)
                                    .debug_selector(|| "jigsaw-grid".into())
                                    .aria_label("Assembly grid")
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.show_grid = !view.show_grid;
                                        view.save();
                                        cx.notify();
                                    })),
                            ),
                        )
                    })
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
                                    button.border_color(rgb(t::colors(cx).gold_text))
                                })
                                .on_click(cx.listener(|view, _, _, cx| {
                                    let progressed = match view.mode {
                                        Mode::Assemble => {
                                            view.placed_count() != 0
                                                || !view.round().loose.is_empty()
                                        }
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
                    .child(
                        t::display_type("Reference", 17.)
                            .when(self.mode == Mode::Assemble, |label| {
                                label.text_color(rgb(t::PAPER))
                            }),
                    )
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
            Mode::Assemble => "Move pieces around the table. They snap into place when they fit.",
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

    fn next_picture(&self) -> Option<usize> {
        let end = if self.key.painting < GALLERY.len() {
            GALLERY.len()
        } else {
            GALLERY.len() + self.gallery.photos().len()
        };
        let next = self.key.painting + 1;
        (next < end).then_some(next)
    }

    fn completion(&self, cx: &Context<Self>) -> AnyElement {
        let round = self.round();
        let (title, stat) = match self.mode {
            Mode::Assemble => {
                let hints = match round.tray.hints_used {
                    0 => "no hints".into(),
                    1 => "1 hint".into(),
                    count => format!("{count} hints"),
                };
                (
                    "Assembled",
                    format!("{} placements · {hints}", round.assembly_moves),
                )
            }
            Mode::Swap => ("Solved", self.moves_text()),
        };
        let actions = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                action_button("jigsaw-play-again", "Play again", true, cx)
                    .track_focus(&self.result_focus)
                    .border_color(rgb(t::colors(cx).gold_text))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.restart();
                        view.mode_focus.focus(window, cx);
                        cx.notify();
                    })),
            )
            .children(self.next_picture().map(|painting| {
                action_button("jigsaw-next-picture", "Next picture", true, cx).on_click(
                    cx.listener(move |view, _, window, cx| {
                        view.gallery.reveal(painting);
                        view.choose(PuzzleKey {
                            painting,
                            ..view.key
                        });
                        if view.is_complete() {
                            view.result_focus.focus(window, cx);
                        } else {
                            view.mode_focus.focus(window, cx);
                        }
                        cx.notify();
                    }),
                )
            }));
        round_result(
            cx,
            "jigsaw-complete",
            Outcome::Won,
            title,
            format!(
                "{} · {} pieces · {stat}",
                self.painting().title,
                self.key.size.piece_count(),
            ),
            actions,
        )
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let complete = self.is_complete();
        if self.mode == Mode::Assemble && !complete && !self.artwork.contains_key(&self.key) {
            self.prepare_artwork(cx);
        }

        let available = board_space(window);
        let height = crate::app::content_height(window);
        let table = TableLayout::new(available, height, self.painting().aspect);
        let panel = t::panel(cx)
            .w_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.gallery_panel(available, cx))
            .child(div().h(px(1.)).bg(rgb(t::colors(cx).border)))
            .child(self.toolbar(table.tray, cx));
        if self.mode == Mode::Assemble && !complete {
            let art = match self.artwork.get(&self.key) {
                Some(Artwork::Ready(pieces)) => Some(pieces.as_slice()),
                _ => None,
            };
            return panel
                .child(self.assembly_table(table, art, window, cx))
                .child(self.progress(cx))
                .child(self.board_footer(window, cx));
        }

        let wide = available >= 710.;
        let side_by_side = wide || (complete && available >= 600.);
        // Match the shared stage rail.
        let side_width = if wide {
            (available * 0.3).clamp(240., 360.).floor()
        } else {
            176.
        };
        let workspace_side_width = if complete { RESULT_WIDTH } else { side_width };
        let max_image_height = (height - 360.).clamp(240., 760.);
        let max_board_width = if side_by_side {
            available - workspace_side_width - WORKSPACE_GAP
        } else {
            available
        };
        let board_width = (max_board_width - 2. * (FRAME_PADDING + 1.))
            .min(max_image_height * self.painting().aspect);
        let board_height = board_width / self.painting().aspect;
        let frame_width = board_width + 2. * (FRAME_PADDING + 1.);
        let board = match self.mode {
            Mode::Assemble => framed_board(
                div().w(px(board_width)).h(px(board_height)).child(
                    self.painting()
                        .source
                        .image()
                        .size_full()
                        .object_fit(ObjectFit::Fill),
                ),
            ),
            Mode::Swap => self.swap_board(board_width, cx),
        };
        let reference = (!complete).then(|| self.reference_panel(side_width, height, cx));
        let side = if side_by_side {
            div()
                .w(px(workspace_side_width))
                .flex_none()
                .flex()
                .flex_col()
                .gap_4()
                .children(reference)
                .when(complete, |side| side.child(self.completion(cx)))
        } else {
            div()
                .flex()
                .items_start()
                .gap(px(WORKSPACE_GAP))
                .children(reference)
        };

        panel.child(
            div()
                .flex()
                .when(!side_by_side, |layout| layout.flex_col())
                .items_start()
                .gap(px(WORKSPACE_GAP))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .w(px(frame_width))
                        .gap_2()
                        .when(complete && !side_by_side, |column| {
                            column.child(self.completion(cx))
                        })
                        .child(board)
                        .when(!complete, |column| {
                            column
                                .child(self.progress(cx))
                                .child(self.board_footer(window, cx))
                        }),
                )
                .when(side_by_side || !complete, |layout| layout.child(side)),
        )
    }
}

fn framed_board(board: Div) -> Div {
    div()
        .p(px(FRAME_PADDING))
        .border_1()
        .border_color(rgb(0x5a4733))
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
            button
                .bg(rgb(t::GOLD))
                .text_color(rgb(t::INK))
                .hover(|style| style.bg(rgb(t::GOLD_HOVER)))
        })
        .when(!current, |button| {
            button.hover(move |style| style.bg(rgb(palette.hover)))
        })
        .focus_visible(|style| style.bg(rgb(t::GOLD_HOVER)).text_color(rgb(t::INK)))
        .child(label)
}

fn action_button(id: &'static str, name: &'static str, enabled: bool, cx: &App) -> Stateful<Div> {
    themed_control(cx, id, name, enabled)
        .debug_selector(|| id.into())
        .min_h(px(34.))
}
