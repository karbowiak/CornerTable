use std::path::PathBuf;

use gpui::{
    ClipboardEntry, ClipboardItem, Context, Div, ExternalPaths, PathPromptOptions, Role, div,
    prelude::*, rgb,
};

use super::{
    AFTER_DARK_START, GALLERY, GameView, Painting, PictureSource, PuzzleKey, action_button, photos,
    segment, segments, small_text,
};

const PAGE_SIZE: usize = 5;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Tab {
    #[default]
    Collection,
    AfterDark,
    Photos,
}

impl Tab {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Collection => "Collection",
            Self::AfterDark => "After Dark",
            Self::Photos => "My photos",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Collection => 0,
            Self::AfterDark => 1,
            Self::Photos => 2,
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
    PickerUnavailable,
    Error(String),
}

impl ImportStatus {
    fn message(&self) -> Option<(&str, Role)> {
        match self {
            Self::Idle | Self::Choosing => None,
            Self::Importing => Some(("Preparing your photo…", Role::Status)),
            Self::Added => Some(("Photo added to your gallery.", Role::Status)),
            Self::PickerUnavailable => Some((
                "The file picker is unavailable. Drop an image here or import a copied file path.",
                Role::Alert,
            )),
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
    pages: [usize; 3],
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
        } else if let Some(painting) = index.checked_sub(AFTER_DARK_START) {
            (Tab::AfterDark, painting)
        } else {
            (Tab::Collection, index)
        };
        self.tab = tab;
        self.set_page(local_index / PAGE_SIZE);
    }

    pub(super) fn visible(&self) -> impl ExactSizeIterator<Item = (usize, &Painting)> {
        let (offset, pictures) = self.entries();
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
        self.entries().1.len().div_ceil(PAGE_SIZE)
    }

    fn entries(&self) -> (usize, &[Painting]) {
        match self.tab {
            Tab::Collection => (0, &GALLERY[..AFTER_DARK_START]),
            Tab::AfterDark => (AFTER_DARK_START, &GALLERY[AFTER_DARK_START..]),
            Tab::Photos => (GALLERY.len(), self.photos.as_slice()),
        }
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
        if self.gallery.directory.is_none() {
            self.gallery.import =
                ImportStatus::Error("Couldn’t find a local folder for your photos.".into());
            cx.notify();
            return;
        }
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Add Photo".into()),
        });
        self.gallery.import = ImportStatus::Choosing;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let selection = picker.await;
            view.update(cx, |view, cx| {
                match selection {
                    Ok(Ok(paths)) => {
                        view.gallery.import = ImportStatus::Idle;
                        if let Some(path) = paths.and_then(|paths| paths.into_iter().next()) {
                            view.import_photo(path, cx);
                        }
                    }
                    Ok(Err(_)) | Err(_) => {
                        view.gallery.tab = Tab::Photos;
                        view.gallery.import = ImportStatus::PickerUnavailable;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn import_photo(&mut self, source: PathBuf, cx: &mut Context<Self>) {
        if self.gallery.import.pending() {
            return;
        }
        let Some(directory) = self.gallery.directory.clone() else {
            self.gallery.import =
                ImportStatus::Error("Couldn’t find a local folder for your photos.".into());
            cx.notify();
            return;
        };
        self.gallery.import = ImportStatus::Importing;
        cx.notify();
        let import = cx
            .background_executor()
            .spawn(async move { photos::import(&source, &directory) });
        cx.spawn(async move |view, cx| {
            let result = import.await;
            view.update(cx, |view, cx| {
                view.gallery.import = match result {
                    Ok(photo) => {
                        let painting = view.gallery.add(photo);
                        view.gallery.reveal(painting);
                        view.choose(PuzzleKey {
                            painting,
                            ..view.key
                        });
                        ImportStatus::Added
                    }
                    Err(error) => ImportStatus::Error(error.to_string()),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn import_copied_photo(&mut self, cx: &mut Context<Self>) {
        if self.gallery.import.pending() {
            return;
        }
        if let Some(path) = cx.read_from_clipboard().as_ref().and_then(copied_path) {
            self.import_photo(path, cx);
        } else {
            self.gallery.import =
                ImportStatus::Error("Copy one image file or its full path, then try again.".into());
            cx.notify();
        }
    }

    pub(super) fn gallery_panel(&self, available: f32, cx: &Context<Self>) -> Div {
        let gallery = &self.gallery;
        let pending = gallery.import.pending();
        let show_photos = gallery.tab == Tab::Photos;
        let palette = crate::theme::colors(cx);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .when(!pending, |panel| {
                panel
                    .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(rgb(palette.hover)))
                    .on_drop(cx.listener(|view, paths: &ExternalPaths, _, cx| {
                        if let [path] = paths.paths() {
                            view.import_photo(path.clone(), cx);
                        } else {
                            view.gallery.import =
                                ImportStatus::Error("Drop one image at a time.".into());
                            cx.notify();
                        }
                    }))
            })
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
                                (Tab::AfterDark, "jigsaw-after-dark"),
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
                            .when(cfg!(test), |button| {
                                button.debug_selector(|| "jigsaw-add-photo".into())
                            })
                            .on_click(cx.listener(|view, _, _, cx| view.add_photo(cx))),
                    )
                    .when(show_photos, |header| {
                        header.child(
                            action_button("jigsaw-paste-photo", "Import copied path", !pending, cx)
                                .when(cfg!(test), |button| {
                                    button.debug_selector(|| "jigsaw-paste-photo".into())
                                })
                                .when(!pending, |button| {
                                    button.on_click(cx.listener(|view, _, _, cx| {
                                        view.import_copied_photo(cx);
                                    }))
                                }),
                        )
                    }),
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
                let message = "Drop a JPEG, PNG, or WebP here, or choose Add photo.";
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

fn copied_path(item: &ClipboardItem) -> Option<PathBuf> {
    if let Some(paths) = item.entries().iter().find_map(|entry| match entry {
        ClipboardEntry::ExternalPaths(paths) => Some(paths.paths()),
        _ => None,
    }) {
        let [path] = paths else { return None };
        return Some(path.clone());
    }
    let text = item.text()?;
    let text = text.trim();
    if text.lines().count() != 1 {
        return None;
    }
    let path = PathBuf::from(text.trim_matches('"'));
    path.is_absolute().then_some(path)
}
