use std::path::PathBuf;

use gpui::{Context, Div, PathPromptOptions, Role, div, prelude::*};

use super::{
    GALLERY, GameView, Painting, PictureSource, PuzzleKey, action_button, photos, segment,
    segments, small_text,
};

const PAGE_SIZE: usize = 5;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Tab {
    #[default]
    Collection,
    Photos,
}

impl Tab {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Collection => "Collection",
            Self::Photos => "My photos",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Collection => 0,
            Self::Photos => 1,
        }
    }
}

#[derive(Default)]
enum ImportStatus {
    #[default]
    Idle,
    Choosing,
    Importing,
    Added,
    Error(String),
}

impl ImportStatus {
    fn message(&self) -> Option<(&str, Role)> {
        match self {
            Self::Idle | Self::Choosing => None,
            Self::Importing => Some(("Preparing your photo…", Role::Status)),
            Self::Added => Some(("Photo added to your gallery.", Role::Status)),
            Self::Error(message) => Some((message, Role::Alert)),
        }
    }

    fn pending(&self) -> bool {
        matches!(self, Self::Choosing | Self::Importing)
    }
}

#[derive(Default)]
pub(super) struct Gallery {
    pub tab: Tab,
    photos: Vec<Painting>,
    directory: Option<PathBuf>,
    pages: [usize; 2],
    import: ImportStatus,
}

impl Gallery {
    pub(super) fn load(directory: Option<PathBuf>) -> Self {
        let mut gallery = Self {
            directory,
            ..Self::default()
        };
        if let Some(directory) = gallery.directory.as_deref() {
            match photos::stored(directory) {
                Ok(photos) => {
                    for photo in photos {
                        gallery.add(photo);
                    }
                }
                Err(_) => {
                    gallery.import =
                        ImportStatus::Error("Couldn’t open your local photo gallery.".into());
                }
            }
        }
        gallery
    }

    pub(super) fn painting(&self, index: usize) -> &Painting {
        GALLERY
            .get(index)
            .unwrap_or_else(|| &self.photos[index - GALLERY.len()])
    }

    pub(super) fn photos(&self) -> &[Painting] {
        &self.photos
    }

    pub(super) fn reveal(&mut self, index: usize) {
        let (tab, local_index) = if let Some(photo) = index.checked_sub(GALLERY.len()) {
            (Tab::Photos, photo)
        } else {
            (Tab::Collection, index)
        };
        self.tab = tab;
        self.set_page(local_index / PAGE_SIZE);
    }

    pub(super) fn visible(&self) -> impl ExactSizeIterator<Item = (usize, &Painting)> {
        let (offset, pictures) = match self.tab {
            Tab::Collection => (0, GALLERY),
            Tab::Photos => (GALLERY.len(), self.photos.as_slice()),
        };
        let start = (self.page() * PAGE_SIZE).min(pictures.len());
        let end = (start + PAGE_SIZE).min(pictures.len());
        pictures[start..end]
            .iter()
            .enumerate()
            .map(move |(position, painting)| (offset + start + position, painting))
    }

    fn add(&mut self, directory: PathBuf) -> usize {
        let index = GALLERY.len() + self.photos.len();
        self.photos.push(Painting {
            id: directory
                .file_name()
                .and_then(|name| name.to_str())
                .expect("photo directory has a generated UTF-8 name")
                .to_owned()
                .into(),
            title: format!("Photo {}", self.photos.len() + 1).into(),
            source: PictureSource::Local(directory.join(photos::PICTURE).into()),
            thumbnail: Some(directory.join(photos::THUMBNAIL).into()),
            aspect: photos::ASPECT,
        });
        index
    }

    fn pages(&self) -> usize {
        match self.tab {
            Tab::Collection => GALLERY.len(),
            Tab::Photos => self.photos.len(),
        }
        .div_ceil(PAGE_SIZE)
    }

    fn page(&self) -> usize {
        self.pages[self.tab.index()]
    }

    fn set_page(&mut self, index: usize) {
        self.pages[self.tab.index()] = index;
    }
}

impl GameView {
    fn add_photo(&mut self, cx: &mut Context<Self>) {
        if self.gallery.import.pending() {
            return;
        }
        let Some(directory) = self.gallery.directory.clone() else {
            self.gallery.import =
                ImportStatus::Error("Couldn’t find a local folder for your photos.".into());
            cx.notify();
            return;
        };
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Add Photo".into()),
        });
        let executor = cx.background_executor().clone();
        self.gallery.import = ImportStatus::Choosing;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = match picker.await {
                Ok(Ok(paths)) => match paths.and_then(|paths| paths.into_iter().next()) {
                    Some(source) => {
                        if view
                            .update(cx, |view, cx| {
                                view.gallery.import = ImportStatus::Importing;
                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        }
                        Some(
                            executor
                                .spawn(async move { photos::import(&source, &directory) })
                                .await,
                        )
                    }
                    None => None,
                },
                _ => Some(Err(anyhow::anyhow!("Couldn’t open the file picker."))),
            };
            view.update(cx, |view, cx| {
                view.gallery.import = match result {
                    Some(Ok(photo)) => {
                        let painting = view.gallery.add(photo);
                        view.gallery.reveal(painting);
                        view.choose(PuzzleKey {
                            painting,
                            ..view.key
                        });
                        ImportStatus::Added
                    }
                    Some(Err(error)) => ImportStatus::Error(error.to_string()),
                    None => ImportStatus::Idle,
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn gallery_panel(&self, available: f32, cx: &Context<Self>) -> Div {
        let gallery = &self.gallery;
        let pending = gallery.import.pending();
        let show_photos = gallery.tab == Tab::Photos;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        segments(cx).children(
                            [
                                (Tab::Collection, "jigsaw-collection"),
                                (Tab::Photos, "jigsaw-my-photos"),
                            ]
                            .into_iter()
                            .map(|(tab, id)| {
                                segment(id, tab.label(), gallery.tab == tab, cx).on_click(
                                    cx.listener(move |view, _, _, cx| {
                                        view.gallery.tab = tab;
                                        cx.notify();
                                    }),
                                )
                            }),
                        ),
                    )
                    .when(gallery.pages() > 1, |header| {
                        header.child(self.gallery_pages(cx))
                    })
                    .child(
                        action_button("jigsaw-add-photo", "Add photo…", true, cx)
                            .when(pending, |button| {
                                button
                                    .opacity(0.55)
                                    .cursor_default()
                                    .aria_description("A photo import is in progress.")
                            })
                            .on_click(cx.listener(|view, _, _, cx| view.add_photo(cx))),
                    ),
            )
            .when_some(gallery.import.message(), |panel, (message, role)| {
                panel.child(
                    small_text(message.to_owned(), cx)
                        .id("jigsaw-import-status")
                        .role(role)
                        .aria_label(message.to_owned()),
                )
            })
            .when(show_photos && gallery.photos.is_empty(), |panel| {
                let message = "Choose a JPEG, PNG, or WebP to save a puzzle on this device.";
                panel.child(
                    small_text(message, cx)
                        .id("jigsaw-photo-empty")
                        .role(Role::Status)
                        .aria_label(message),
                )
            })
            .when(!show_photos || !gallery.photos.is_empty(), |panel| {
                panel.child(self.gallery_row(available, cx))
            })
    }

    fn gallery_pages(&self, cx: &Context<Self>) -> Div {
        let gallery = &self.gallery;
        let page = gallery.page();
        let pages = gallery.pages();
        let step = |id, label, target: Option<usize>| {
            action_button(id, label, target.is_some(), cx).when_some(target, |button, target| {
                button.on_click(cx.listener(move |view, _, _, cx| {
                    view.gallery.set_page(target);
                    cx.notify();
                }))
            })
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(step(
                "jigsaw-gallery-previous",
                "Previous",
                page.checked_sub(1),
            ))
            .child(
                small_text(format!("{} / {pages}", page + 1), cx)
                    .id("jigsaw-gallery-page")
                    .role(Role::Status)
                    .aria_label(format!("Gallery page {} of {pages}", page + 1)),
            )
            .child(step(
                "jigsaw-gallery-next",
                "Next",
                (page + 1 < pages).then_some(page + 1),
            ))
    }
}
