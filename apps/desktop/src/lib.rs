mod app;
mod assets;
mod daily;
mod games;
mod pages;
mod storage;
mod theme;
mod widgets;

use corner_table_api::FixtureApi;
use gpui::{
    App, AppContext as _, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, px,
    size,
};

pub fn run() -> anyhow::Result<()> {
    let fixtures = FixtureApi::load().map_err(|error| anyhow::anyhow!(error.to_string()))?;
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            let appearance = storage::appearance();
            cx.set_window_appearance(appearance.native());
            if let Err(error) = assets::register_fonts(cx) {
                eprintln!("Could not load bundled fonts: {error:#}");
            }
            cx.bind_keys([
                KeyBinding::new("tab", app::FocusNext, None),
                KeyBinding::new("shift-tab", app::FocusPrevious, None),
                KeyBinding::new("alt-left", app::NavigateBack, None),
                KeyBinding::new("alt-right", app::NavigateForward, None),
                #[cfg(target_os = "macos")]
                KeyBinding::new("cmd-[", app::NavigateBack, None),
                #[cfg(target_os = "macos")]
                KeyBinding::new("cmd-]", app::NavigateForward, None),
            ]);
            let bounds = Bounds::centered(None, size(px(1380.), px(900.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(760.), px(540.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Corner Table".into()),
                    ..Default::default()
                }),
                app_id: Some("dk.karbowiak.CornerTable".into()),
                ..Default::default()
            };
            match cx.open_window(options, move |window, cx| {
                cx.new(|cx| {
                    let desktop = app::Desktop::new(fixtures, appearance, window, cx);
                    desktop.focus.focus(window, cx);
                    desktop
                })
            }) {
                Ok(_) => cx.activate(true),
                Err(error) => {
                    eprintln!("Unable to open Corner Table: {error:#}");
                    cx.quit();
                }
            }
        });
    Ok(())
}
