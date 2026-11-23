use std::collections::HashMap;
use std::time::Duration;

use corner_table_api::{FixtureApi, GameId, Period};
use gpui::{
    AnyView, Context, Entity, FocusHandle, MouseButton, NavigationDirection, Pixels, Point,
    ScrollHandle, Task, Window, div, point, prelude::*, px, rgb,
};

use crate::games::{GameEntry, GameLaunch};
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
        ToggleFullScreen,
        Quit,
    ]
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Page {
    Library,
    Solitaire,
    DailyChallenge,
    Leaderboard,
    Game(GameLaunch),
}

#[derive(Clone)]
pub enum Action {
    Navigate(Page),
    History(NavigationDirection),
    OpenGame(GameId),
    ToggleGamePicker,
    SelectGame(GameId),
    SelectPeriod(Period),
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

pub struct Desktop {
    pub api: FixtureApi,
    pub page: Page,
    pub board_game: GameId,
    pub board_period: Period,
    pub game_picker_open: bool,
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
    games: HashMap<GameLaunch, AnyView>,
    current_size: storage::WindowSize,
    remember_size: Option<Task<()>>,
    pub focus: FocusHandle,
}

impl Desktop {
    pub fn new(
        api: FixtureApi,
        appearance: theme::Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let board_game = api.home.challenge.game_id.clone();
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
        let focus = cx.focus_handle();
        let search = cx.new(|cx| SearchInput::new(focus.clone(), window, cx));
        cx.subscribe(&search, |_, _, _: &QueryChanged, cx| cx.notify())
            .detach();
        Self {
            api,
            page: Page::Library,
            board_game,
            board_period: Period::Today,
            game_picker_open: false,
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
            Action::OpenGame(id) if id.0 == "solitaire" => self.navigate(Page::Solitaire),
            Action::OpenGame(game) => self.navigate(Page::Game(GameLaunch {
                game,
                entry: GameEntry::Classic,
            })),
            Action::ToggleGamePicker => self.game_picker_open = !self.game_picker_open,
            Action::SelectGame(game) => {
                self.board_game = game;
                self.game_picker_open = false;
            }
            Action::SelectPeriod(period) => self.board_period = period,
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
        self.game_picker_open = false;
        self.scroll.set_offset(visit.scroll);
    }

    fn search_result(&self, cx: &Context<Self>) -> Option<GameId> {
        let query = self.search.read(cx).submitted_query()?;
        let mut matches = pages::matching_games(self, query);
        let first = matches.next()?;
        let other = matches.next();
        let exact = |game: &corner_table_api::Game| game.title.eq_ignore_ascii_case(query);
        if other.is_none() || exact(first) {
            return Some(first.id.clone());
        }
        other
            .into_iter()
            .chain(matches)
            .find(|game| exact(game))
            .map(|game| game.id.clone())
    }
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(1080.);
        let playing = matches!(self.page, Page::Game(_));
        let palette = theme::colors(cx);
        let content = match &self.page {
            Page::Library => pages::library(self, window.viewport_size().width.into(), cx),
            Page::Solitaire => pages::solitaire(self, narrow, cx),
            Page::DailyChallenge => pages::daily_challenge(cx),
            Page::Leaderboard => pages::leaderboard(self, narrow, cx),
            Page::Game(launch) => {
                if !self.games.contains_key(launch)
                    && let Some(view) = crate::games::open(launch, cx)
                {
                    self.games.insert(launch.clone(), view);
                }
                let compact_header = window.viewport_size().width < px(900.);
                pages::game(
                    self,
                    launch,
                    self.games.get(launch).cloned(),
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
                    && let Some(game) = desktop.search_result(cx)
                {
                    desktop.dispatch(Action::OpenGame(game), window, cx);
                }
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
            .child(widgets::sidebar(
                self,
                narrow,
                window.viewport_size().height < px(630.),
                cx,
            ))
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
                            .w_full()
                            .max_w(px(1440.))
                            .mx_auto()
                            .px(px(if narrow { 22. } else { 34. }))
                            .py(px(if playing { 16. } else { 27. }))
                            .child(content),
                    ),
            )
            .children(guide)
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Modifiers, MouseDownEvent, PlatformInput, TestAppContext};

    use super::*;

    #[gpui::test]
    fn navigation_buttons_restore_history_and_new_routes_drop_forward(cx: &mut TestAppContext) {
        cx.update(crate::install_shortcuts);
        let window = cx.add_window(|window, cx| {
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
                desktop.dispatch(Action::Navigate(Page::Leaderboard), window, cx);
                assert!(!desktop.can_step_history(NavigationDirection::Forward));
            })
            .unwrap();
    }
}
