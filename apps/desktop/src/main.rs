mod app;
mod assets;
mod games;
mod pages;
mod theme;
mod widgets;

use corner_table_api::FixtureApi;
use gpui::{
    App, AppContext as _, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds,
    WindowOptions, px, size,
};

fn main() -> anyhow::Result<()> {
    let fixtures = FixtureApi::load().map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            if let Err(error) = assets::register_fonts(cx) {
                eprintln!("Could not load bundled fonts: {error:#}");
            }
            cx.bind_keys([
                KeyBinding::new("tab", app::FocusNext, None),
                KeyBinding::new("shift-tab", app::FocusPrevious, None),
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
                    let desktop = app::Desktop::new(fixtures, cx);
                    desktop.focus.focus(window);
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
