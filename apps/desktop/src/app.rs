use std::collections::HashMap;
use std::time::Duration;

use corner_table_api::{FixtureApi, GameId};
use gpui::{
    Context, Entity, FocusHandle, MouseButton, NavigationDirection, Pixels, Point, ScrollHandle,
    Task, Window, div, point, prelude::*, px, rgb,
};

use crate::games::{GameEntry, GameHost, GameLaunch};
use crate::library_search::{QueryChanged, SearchInput};
use crate::{audio, pages, storage, theme, widgets};

gpui::actions!(
    corner_table,
    [
        FocusNext,
        FocusPrevious,
        NavigateBack,
        NavigateForward,
        OpenLibrary,
        FindGame,
        OpenSearchResult,
        ToggleGuide,
        ToggleFullScreen,
        Quit,
    ]
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Page {
    Library,
    Solitaire,
    DailyChallenge,
    Records,
    Game(GameLaunch),
}

#[derive(Clone)]
pub enum Action {
    Navigate(Page),
    History(NavigationDirection),
    ReturnToDailyCalendar,
    OpenGame(GameId),
    SurpriseGame,
    Record(pages::RecordAction),
    ToggleFavorite(GameId),
    FilterLibrary(pages::LibraryFilter),
    SetAppearance(theme::Appearance),
    SetSound(bool),
    OpenGuide,
    CloseGuide,
}

struct Visit {
    page: Page,
    scroll: Point<Pixels>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidebarLayout {
    Compact,
    Medium,
    Wide,
}

pub(crate) struct ShellLayout {
    pub sidebar: SidebarLayout,
    viewport_width: f32,
}

impl ShellLayout {
    const MAX_PAGE_WIDTH: f32 = 1440.;

    pub fn new(viewport_width: Pixels) -> Self {
        let viewport_width = f32::from(viewport_width);
        let sidebar = if viewport_width < 960. {
            SidebarLayout::Compact
        } else if viewport_width < 1080. {
            SidebarLayout::Medium
        } else {
            SidebarLayout::Wide
        };
        Self {
            sidebar,
            viewport_width,
        }
    }

    pub fn page_width(&self) -> f32 {
        (self.viewport_width - self.sidebar_width()).clamp(0., Self::MAX_PAGE_WIDTH)
    }

    pub fn content_width(&self) -> f32 {
        self.page_width() - 2. * self.page_gutter()
    }

    pub fn sidebar_width(&self) -> f32 {
        match self.sidebar {
            SidebarLayout::Compact => 64.,
            SidebarLayout::Medium => 174.,
            SidebarLayout::Wide => 220.,
        }
    }

    fn page_gutter(&self) -> f32 {
        match self.sidebar {
            SidebarLayout::Compact => 16.,
            SidebarLayout::Medium => 22.,
            SidebarLayout::Wide => 34.,
        }
    }
}

pub struct Desktop {
    pub api: FixtureApi,
    pub page: Page,
    pub records: pages::Records,
    pub daily_calendar: pages::DailyCalendar,
    pub search: Entity<SearchInput>,
    pub recent: Option<GameLaunch>,
    pub favorites: Vec<GameId>,
    pub library_filter: pages::LibraryFilter,
    pub appearance: theme::Appearance,
    pub sound_enabled: bool,
    pub guide: pages::GuideOverlay,
    scroll: ScrollHandle,
    back: Vec<Visit>,
    forward: Vec<Visit>,
    games: HashMap<GameLaunch, Entity<GameHost>>,
    current_size: storage::WindowSize,
    remember_size: Option<Task<()>>,
    reload_records: Option<Task<()>>,
    pub focus: FocusHandle,
}

impl Desktop {
    pub fn new(
        api: FixtureApi,
        appearance: theme::Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let recent = storage::recent_game().filter(|launch| api.game(&launch.game).is_some());
        let mut favorites = storage::favorites();
        favorites.retain(|id| api.game(id).is_some());
        let sound_enabled = audio::is_supported() && storage::sound_enabled();
        audio::set_enabled(cx, sound_enabled);
        theme::apply(appearance, window, cx);
        cx.observe_window_appearance(window, |desktop, window, cx| {
            theme::apply(desktop.appearance, window, cx);
        })
        .detach();
        cx.observe_window_bounds(window, Self::remember_window_size)
            .detach();
        cx.observe_window_activation(window, |_, window, cx| {
            if window.is_window_active() {
                crate::motion::sync_from_system(cx);
            }
        })
        .detach();
        let focus = cx.focus_handle();
        let search = cx.new(|cx| SearchInput::new(focus.clone(), window, cx));
        cx.subscribe(&search, |_, _, _: &QueryChanged, cx| cx.notify())
            .detach();
        Self {
            api,
            page: Page::Library,
            records: pages::Records::default(),
            daily_calendar: pages::DailyCalendar::new(cx),
            search,
            recent,
            favorites,
            library_filter: pages::LibraryFilter::All,
            appearance,
            sound_enabled,
            guide: pages::GuideOverlay::new(cx),
            scroll: ScrollHandle::new(),
            back: Vec::new(),
            forward: Vec::new(),
            games: HashMap::new(),
            current_size: Self::window_size(window),
            remember_size: None,
            reload_records: None,
            focus,
        }
    }

    fn window_size(window: &Window) -> storage::WindowSize {
        let size = window.viewport_size();
        storage::WindowSize {
            width: size.width.into(),
            height: size.height.into(),
        }
    }

    fn remember_window_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(window.window_bounds(), gpui::WindowBounds::Windowed(_))
            || window.is_fullscreen()
        {
            return;
        }
        let size = Self::window_size(window);
        if size == self.current_size {
            return;
        }
        self.current_size = size;
        let executor = cx.background_executor().clone();
        self.remember_size = Some(cx.spawn(async move |_, _| {
            executor.timer(Duration::from_millis(250)).await;
            storage::save_window_size(size);
        }));
    }

    pub fn dispatch(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.page.clone();
        match action {
            Action::Navigate(page) => self.navigate(page),
            Action::History(direction) => self.step_history(direction),
            Action::ReturnToDailyCalendar => {
                if self
                    .back
                    .last()
                    .is_some_and(|visit| visit.page == Page::DailyChallenge)
                {
                    self.step_history(NavigationDirection::Back);
                } else {
                    self.navigate(Page::DailyChallenge);
                }
                cx.on_next_frame(window, |desktop, window, cx| {
                    if desktop.page == Page::DailyChallenge {
                        desktop.daily_calendar.focus(window, cx);
                    }
                });
            }
            Action::OpenGame(id) if id.0 == "solitaire" => self.navigate(Page::Solitaire),
            Action::OpenGame(game) => self.navigate(Page::Game(GameLaunch {
                game,
                entry: GameEntry::Classic,
            })),
            Action::SurpriseGame => {
                if let Some(launch) = self.surprise_game() {
                    self.navigate(Page::Game(launch));
                }
            }
            Action::Record(action) => self.records.apply(action),
            Action::ToggleFavorite(game) => {
                if let Some(index) = self.favorites.iter().position(|id| id == &game) {
                    self.favorites.remove(index);
                } else {
                    self.favorites.push(game);
                }
                storage::save_favorites(&self.favorites);
            }
            Action::FilterLibrary(filter) => {
                self.library_filter = filter;
                self.scroll.set_offset(point(px(0.), px(0.)));
            }
            Action::SetAppearance(appearance) => {
                if self.appearance != appearance {
                    self.appearance = appearance;
                    storage::set_appearance(appearance);
                    cx.set_window_appearance(appearance.native());
                    theme::apply(appearance, window, cx);
                }
            }
            Action::SetSound(enabled) => {
                self.sound_enabled = audio::is_supported() && enabled;
                storage::set_sound_enabled(self.sound_enabled);
                audio::set_enabled(cx, self.sound_enabled);
                if self.sound_enabled {
                    audio::play(cx, audio::Effect::Move);
                }
            }
            Action::OpenGuide => {
                if let Page::Game(launch) = &self.page {
                    self.guide.open(launch, &self.focus, window, cx);
                }
            }
            Action::CloseGuide => self.guide.close(window, cx),
        }
        if self.page != previous {
            self.guide.dismiss();
            self.focus.focus(window, cx);
            if let Page::Game(launch) = &self.page
                && let Some(host) = self.games.get(launch)
            {
                host.update(cx, |host, cx| host.restore_focus(&self.focus, cx));
            }
            if matches!(self.page, Page::Records | Page::DailyChallenge) {
                self.refresh_records(cx);
            }
        }
        cx.notify();
    }

    fn navigate(&mut self, page: Page) {
        if page == self.page {
            return;
        }
        let previous = self.current_visit();
        self.back.push(previous);
        self.forward.clear();
        self.show_visit(Visit {
            page,
            scroll: point(px(0.), px(0.)),
        });
    }

    fn surprise_game(&self) -> Option<GameLaunch> {
        let library = &self.api.library;
        let last_game = self.recent.as_ref().map(|launch| &launch.game);
        let choices: Vec<_> = library
            .games
            .iter()
            .filter(|game| Some(&game.id) != last_game)
            .collect();
        let game = fastrand::choice(choices).or_else(|| library.games.first())?;
        let entry = if game.id.0 == "solitaire" {
            fastrand::choice(&library.solitaire_modes)
                .map_or(GameEntry::Classic, |mode| GameEntry::Mode(mode.id.clone()))
        } else {
            GameEntry::Classic
        };
        Some(GameLaunch {
            game: game.id.clone(),
            entry,
        })
    }

    pub fn can_step_history(&self, direction: NavigationDirection) -> bool {
        match direction {
            NavigationDirection::Back => !self.back.is_empty(),
            NavigationDirection::Forward => !self.forward.is_empty(),
        }
    }

    fn step_history(&mut self, direction: NavigationDirection) {
        let current = self.current_visit();
        let (source, destination) = match direction {
            NavigationDirection::Back => (&mut self.back, &mut self.forward),
            NavigationDirection::Forward => (&mut self.forward, &mut self.back),
        };
        if let Some(visit) = source.pop() {
            destination.push(current);
            self.show_visit(visit);
        }
    }

    fn current_visit(&self) -> Visit {
        Visit {
            page: self.page.clone(),
            scroll: self.scroll.offset(),
        }
    }

    fn show_visit(&mut self, visit: Visit) {
        self.page = visit.page;
        if let Page::Game(launch) = &self.page
            && self.recent.as_ref() != Some(launch)
        {
            storage::remember_game(launch);
            self.recent = Some(launch.clone());
        }
        self.records.close_picker();
        self.scroll.set_offset(visit.scroll);
    }

    fn refresh_records(&mut self, cx: &mut Context<Self>) {
        self.reload_records = Some(cx.spawn(async |desktop, cx| {
            let history = cx
                .background_executor()
                .spawn(async { storage::recent_results(usize::MAX) })
                .await;
            let _ = desktop.update(cx, |desktop, cx| {
                desktop.records.history = Some(history);
                cx.notify();
            });
        }));
    }
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let layout = ShellLayout::new(viewport.width);
        let narrow = layout.sidebar != SidebarLayout::Wide;
        let short = viewport.height < px(630.);
        let playing = matches!(self.page, Page::Game(_));
        let compact_daily = viewport.width < px(900.) || short;
        let page_padding = if playing || (self.page == Page::DailyChallenge && compact_daily) {
            16.
        } else {
            27.
        };
        let palette = theme::colors(cx);
        let content = match &self.page {
            Page::Library => pages::library(self, layout.content_width(), cx),
            Page::Solitaire => pages::solitaire(self, narrow, cx),
            Page::DailyChallenge => pages::daily_challenge(self, compact_daily, cx),
            Page::Records => pages::records(self, narrow, cx),
            Page::Game(launch) => {
                if !self.games.contains_key(launch)
                    && let Some(view) = crate::games::open(launch, cx)
                {
                    let host = cx.new(|cx| GameHost::new(view, window, cx));
                    self.games.insert(launch.clone(), host);
                }
                let compact_header = viewport.width < px(900.);
                pages::game(
                    self,
                    launch,
                    self.games.get(launch).cloned().map(Into::into),
                    compact_header,
                    cx,
                )
            }
        };
        let guide = match &self.page {
            Page::Game(launch) => self.guide.render(launch, window, cx),
            _ => None,
        };

        div()
            .id("corner-table")
            .track_focus(&self.focus)
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
            .on_action(|_: &ToggleFullScreen, window, _| window.toggle_fullscreen())
            .on_action(cx.listener(|desktop, _: &NavigateBack, window, cx| {
                desktop.dispatch(Action::History(NavigationDirection::Back), window, cx);
            }))
            .on_action(cx.listener(|desktop, _: &NavigateForward, window, cx| {
                desktop.dispatch(Action::History(NavigationDirection::Forward), window, cx);
            }))
            .on_action(cx.listener(|desktop, _: &OpenLibrary, window, cx| {
                desktop.dispatch(Action::Navigate(Page::Library), window, cx);
            }))
            .on_action(cx.listener(|desktop, _: &OpenSearchResult, window, cx| {
                if desktop.page == Page::Library
                    && let Some(query) = desktop.search.read(cx).submitted_query()
                    && let Some(action) = pages::open_search_result(desktop, query)
                {
                    desktop.dispatch(action, window, cx);
                }
            }))
            .on_action(cx.listener(|desktop, _: &ToggleGuide, window, cx| {
                let action = if desktop.guide.is_open() {
                    Action::CloseGuide
                } else {
                    Action::OpenGuide
                };
                desktop.dispatch(action, window, cx);
            }))
            .on_action(cx.listener(|desktop, _: &FindGame, window, cx| {
                desktop.dispatch(Action::Navigate(Page::Library), window, cx);
                desktop.library_filter = pages::LibraryFilter::All;
                desktop.scroll.set_offset(point(px(0.), px(0.)));
                cx.on_next_frame(window, |desktop, window, cx| {
                    if desktop.page == Page::Library {
                        desktop
                            .search
                            .update(cx, |search, cx| search.find(window, cx));
                    }
                });
            }))
            .capture_any_mouse_down(cx.listener(
                |desktop, event: &gpui::MouseDownEvent, window, cx| {
                    if let MouseButton::Navigate(direction) = event.button {
                        desktop.dispatch(Action::History(direction), window, cx);
                        cx.stop_propagation();
                    }
                },
            ))
            .size_full()
            .flex()
            .tab_group()
            .font_family(theme::BODY)
            .text_size(px(15.))
            .line_height(px(23.))
            .text_color(rgb(palette.ink))
            .bg(rgb(palette.paper))
            .child(widgets::sidebar(self, &layout, short, cx))
            .child(
                div()
                    .id("page-scroll")
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        div()
                            .w(px(layout.page_width()))
                            .mx_auto()
                            .px(px(layout.page_gutter()))
                            .py(px(page_padding))
                            .child(content),
                    ),
            )
            .children(guide)
    }
}

#[cfg(test)]
mod tests {
    use gpui::{
        KeyUpEvent, Keystroke, Modifiers, MouseDownEvent, PlatformInput, TestAppContext,
        VisualTestContext, size,
    };

    use super::*;

    #[test]
    fn shell_widths_account_for_sidebar_gutters_and_page_cap() {
        for (viewport, sidebar, content) in [
            (760., 64., 664.),
            (900., 64., 804.),
            (960., 174., 742.),
            (1080., 220., 792.),
            (1800., 220., 1372.),
        ] {
            let layout = ShellLayout::new(px(viewport));
            assert_eq!(layout.sidebar_width(), sidebar, "{viewport}pt sidebar");
            assert_eq!(layout.content_width(), content, "{viewport}pt content");
        }
    }

    #[gpui::test]
    fn library_star_toggles_the_favorite_without_opening_the_game(cx: &mut TestAppContext) {
        let window = cx.open_window(size(px(900.), px(540.)), |window, cx| {
            Desktop::new(
                FixtureApi::load().unwrap(),
                theme::Appearance::Light,
                window,
                cx,
            )
        });
        cx.run_until_parked();

        let mut visual = VisualTestContext::from_window(window.into(), cx);
        let star = visual
            .debug_bounds("library-favorite-solitaire")
            .unwrap()
            .center();
        let game = GameId("solitaire".into());
        for expected in [true, false] {
            visual.simulate_mouse_down(star, MouseButton::Left, Modifiers::none());
            visual.simulate_mouse_up(star, MouseButton::Left, Modifiers::none());
            window
                .read_with(cx, |desktop, _| {
                    assert_eq!(desktop.page, Page::Library);
                    assert_eq!(desktop.favorites.contains(&game), expected);
                })
                .unwrap();
        }
    }

    #[gpui::test]
    fn surprise_game_opens_a_different_game_and_can_play_a_solitaire_variant(
        cx: &mut TestAppContext,
    ) {
        let window = cx.open_window(size(px(900.), px(540.)), |window, cx| {
            Desktop::new(
                FixtureApi::load().unwrap(),
                theme::Appearance::Light,
                window,
                cx,
            )
        });
        window
            .update(cx, |desktop, window, cx| {
                let previous = GameLaunch {
                    game: GameId("solitaire".into()),
                    entry: GameEntry::Mode("klondike".into()),
                };
                desktop.recent = Some(previous.clone());
                desktop.dispatch(Action::SurpriseGame, window, cx);
                let Page::Game(launch) = &desktop.page else {
                    panic!("expected a playable game");
                };
                assert_ne!(launch.game, previous.game);

                desktop
                    .api
                    .library
                    .games
                    .retain(|game| game.id == previous.game);
                desktop.dispatch(Action::SurpriseGame, window, cx);
                let Page::Game(GameLaunch {
                    game,
                    entry: GameEntry::Mode(id),
                }) = &desktop.page
                else {
                    panic!("expected a Solitaire variant");
                };
                assert_eq!(game, &previous.game);
                assert!(
                    desktop
                        .api
                        .library
                        .solitaire_modes
                        .iter()
                        .any(|mode| &mode.id == id)
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn compact_sidebar_supports_tab_activation_and_preserves_focus_on_resize(
        cx: &mut TestAppContext,
    ) {
        cx.update(crate::install_shortcuts);
        let compact = size(px(760.), px(540.));
        let window = cx.open_window(compact, |window, cx| {
            let desktop = Desktop::new(
                FixtureApi::load().unwrap(),
                theme::Appearance::Dark,
                window,
                cx,
            );
            desktop.focus.focus(window, cx);
            desktop
        });
        cx.run_until_parked();

        let focused = |cx: &mut TestAppContext| {
            cx.update_window(window.into(), |_, window, cx| window.focused(cx))
                .unwrap()
                .unwrap()
        };
        let root = focused(cx);
        cx.simulate_keystrokes(window.into(), "tab");
        let library = focused(cx);
        assert_ne!(root, library);
        cx.simulate_window_resize(window.into(), size(px(1100.), px(700.)));
        cx.run_until_parked();
        assert_eq!(focused(cx), library);
        cx.simulate_window_resize(window.into(), compact);
        cx.run_until_parked();
        assert_eq!(focused(cx), library);

        cx.simulate_keystrokes(window.into(), "tab shift-tab");
        assert_eq!(focused(cx), library);

        for (tabs, key, page) in [
            ("tab", "enter", Page::Solitaire),
            ("tab", "space", Page::Library),
            ("tab", "enter", Page::Solitaire),
            ("tab tab tab tab", "enter", Page::DailyChallenge),
            ("tab tab tab tab tab", "enter", Page::Records),
            ("tab tab", "enter", Page::Library),
        ] {
            cx.simulate_keystrokes(window.into(), tabs);
            let keystroke = Keystroke::parse(key).unwrap();
            cx.dispatch_keystroke(window.into(), keystroke.clone());
            // GPUI buttons activate on key release. The test helper sends key down only.
            cx.update_window(window.into(), |_, window, cx| {
                window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
            })
            .unwrap();
            cx.run_until_parked();
            window
                .read_with(cx, |desktop, _| {
                    assert_eq!(desktop.page, page, "{tabs} {key}")
                })
                .unwrap();
        }
    }

    #[gpui::test]
    fn navigation_buttons_restore_history_and_new_routes_drop_forward(cx: &mut TestAppContext) {
        cx.update(crate::install_shortcuts);
        let window = cx.open_window(size(px(900.), px(540.)), |window, cx| {
            let desktop = Desktop::new(
                FixtureApi::load().unwrap(),
                theme::Appearance::Light,
                window,
                cx,
            );
            desktop.focus.focus(window, cx);
            desktop
        });
        window
            .update(cx, |desktop, window, cx| {
                desktop.dispatch(Action::Navigate(Page::Solitaire), window, cx);
                desktop.dispatch(Action::Navigate(Page::DailyChallenge), window, cx);
            })
            .unwrap();
        cx.run_until_parked();

        let mouse = |direction| {
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Navigate(direction),
                position: point(px(400.), px(150.)),
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            })
        };
        cx.update_window(window.into(), |_, window, cx| {
            window.dispatch_event(mouse(NavigationDirection::Back), cx)
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(
            window
                .read_with(cx, |desktop, _| desktop.page.clone())
                .unwrap(),
            Page::Solitaire
        );

        cx.update_window(window.into(), |_, window, cx| {
            window.dispatch_event(mouse(NavigationDirection::Forward), cx)
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(
            window
                .read_with(cx, |desktop, _| desktop.page.clone())
                .unwrap(),
            Page::DailyChallenge
        );

        cx.simulate_keystrokes(window.into(), "alt-left");
        assert_eq!(
            window
                .read_with(cx, |desktop, _| desktop.page.clone())
                .unwrap(),
            Page::Solitaire
        );
        window
            .update(cx, |desktop, window, cx| {
                desktop.dispatch(Action::Navigate(Page::Records), window, cx);
                assert!(!desktop.can_step_history(NavigationDirection::Forward));
            })
            .unwrap();
    }
}
