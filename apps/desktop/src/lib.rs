mod app;
mod assets;
pub mod audio;
mod daily;
mod games;
mod library_search;
mod motion;
mod pages;
mod storage;
mod theme;
mod widgets;

use corner_table_api::FixtureApi;
use gpui::{
    App, AppContext as _, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, px, size,
};

pub fn run() -> anyhow::Result<()> {
    let fixtures = FixtureApi::load().map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let application = gpui_platform::application().with_assets(assets::Assets);
    application.on_reopen(|cx| {
        if cx.windows().is_empty() {
            match FixtureApi::load() {
                Ok(fixtures) => open_main_window(fixtures, cx),
                Err(error) => eprintln!("Unable to reopen Corner Table: {error:#}"),
            }
        }
    });
    application.run(move |cx| {
        if let Err(error) = assets::register_fonts(cx) {
            eprintln!("Could not load bundled fonts: {error:#}");
        }
        motion::sync_from_system(cx);
        install_shortcuts(cx);
        open_main_window(fixtures, cx);
    });
    Ok(())
}

fn install_shortcuts(cx: &mut App) {
    cx.on_action(|_: &app::Quit, cx| cx.quit());
    cx.bind_keys([
        KeyBinding::new("tab", app::FocusNext, None),
        KeyBinding::new("shift-tab", app::FocusPrevious, None),
        KeyBinding::new("alt-left", app::NavigateBack, None),
        KeyBinding::new("alt-right", app::NavigateForward, None),
        KeyBinding::new("f1", app::ToggleGuide, None),
        KeyBinding::new("f11", app::ToggleFullScreen, None),
    ]);
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-[", app::NavigateBack, None),
        KeyBinding::new("cmd-]", app::NavigateForward, None),
        KeyBinding::new("cmd-1", app::OpenLibrary, None),
        KeyBinding::new("cmd-f", app::FindGame, None),
        KeyBinding::new("cmd-q", app::Quit, None),
        KeyBinding::new("ctrl-cmd-f", app::ToggleFullScreen, None),
    ]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-1", app::OpenLibrary, None),
        KeyBinding::new("ctrl-f", app::FindGame, None),
        KeyBinding::new("ctrl-q", app::Quit, None),
    ]);
    // Search needs Option+Arrow for word movement while its field has focus.
    library_search::install_shortcuts(cx);
    cx.set_menus([
        Menu::new("Corner Table").items([MenuItem::action("Quit Corner Table", app::Quit)]),
        library_search::edit_menu(),
        Menu::new("Navigate").items([
            MenuItem::action("Back", app::NavigateBack),
            MenuItem::action("Forward", app::NavigateForward),
            MenuItem::separator(),
            MenuItem::action("Game Library", app::OpenLibrary),
            MenuItem::action("Find a Game", app::FindGame),
        ]),
        Menu::new("View").items([MenuItem::action(
            "Toggle Full Screen",
            app::ToggleFullScreen,
        )]),
        Menu::new("Help").items([MenuItem::action("How to Play", app::ToggleGuide)]),
    ]);
}

fn open_main_window(fixtures: FixtureApi, cx: &mut App) {
    let appearance = storage::appearance();
    cx.set_window_appearance(appearance.native());
    let preferred = storage::window_size().unwrap_or(storage::WindowSize {
        width: 1380.,
        height: 900.,
    });
    let mut width = preferred.width.max(760.);
    let mut height = preferred.height.max(540.);
    if let Some(display) = cx.primary_display() {
        let area = display.bounds();
        width = width.min((f32::from(area.size.width) - 32.).max(760.));
        height = height.min((f32::from(area.size.height) - 72.).max(540.));
    }
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
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
}
