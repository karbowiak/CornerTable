use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString};
use include_dir::{Dir, include_dir};

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets");

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ASSETS
            .get_file(path)
            .map(|file| Cow::Borrowed(file.contents())))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let directory = if path.is_empty() {
            Some(&ASSETS)
        } else {
            ASSETS.get_dir(path)
        };
        Ok(directory
            .into_iter()
            .flat_map(Dir::files)
            .map(|file| SharedString::from(file.path().to_string_lossy().into_owned()))
            .collect())
    }
}

pub fn register_fonts(cx: &App) -> Result<()> {
    let fonts = ASSETS
        .get_dir("fonts")
        .into_iter()
        .flat_map(Dir::files)
        .filter(|file| file.path().extension().is_some_and(|ext| ext == "ttf"))
        .map(|file| Cow::Borrowed(file.contents()))
        .collect();
    cx.text_system().add_fonts(fonts)
}
