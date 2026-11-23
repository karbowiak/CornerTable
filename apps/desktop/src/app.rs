use std::collections::HashMap;

use corner_table_api::{FixtureApi, GameId, Period};
use gpui::{
    AnyView, Context, FocusHandle, MouseButton, NavigationDirection, Pixels, Point, ScrollHandle,
    Window, div, point, prelude::*, px, rgb,
};

use crate::games::{GameEntry, GameLaunch};
use crate::{pages, storage, theme, widgets};

gpui::actions!(
    corner_table,
    [
        FocusNext,
        FocusPrevious,
        NavigateBack,
        NavigateForward,
        OpenLibrary,
        FindGame,
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
    ClearSearch,
    SetAppearance(theme::Appearance),
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
    pub search: String,
    pub search_focus: FocusHandle,
    pub search_select_all: bool,
    pub recent: Option<GameLaunch>,
    pub appearance: theme::Appearance,
    scroll: ScrollHandle,
    back: Vec<Visit>,
    forward: Vec<Visit>,
    games: HashMap<GameLaunch, AnyView>,
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
        theme::apply(appearance, window, cx);
        cx.observe_window_appearance(window, |desktop, window, cx| {
            theme::apply(desktop.appearance, window, cx);
        })
        .detach();
        Self {
            api,
            page: Page::Library,
            board_game,
            board_period: Period::Today,
            game_picker_open: false,
            search: String::new(),
            search_focus: cx.focus_handle().tab_stop(true),
            search_select_all: false,
            recent,
            appearance,
            scroll: ScrollHandle::new(),
            back: Vec::new(),
            forward: Vec::new(),
            games: HashMap::new(),
            focus: cx.focus_handle(),
        }
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
            Action::ClearSearch => {
                self.search.clear();
                self.search_select_all = false;
            }
            Action::SetAppearance(appearance) => {
                if self.appearance != appearance {
                    self.appearance = appearance;
                    storage::set_appearance(appearance);
                    cx.set_window_appearance(appearance.native());
                    theme::apply(appearance, window, cx);
                }
            }
        }
        if self.page != previous {
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
        self.search_select_all = false;
        self.scroll.set_offset(visit.scroll);
    }
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(1080.);
        let playing = matches!(self.page, Page::Game(_));
        let palette = theme::colors(cx);
        let content = match &self.page {
            Page::Library => pages::library(self, narrow, cx),
            Page::Solitaire => pages::solitaire(self, narrow, cx),
            Page::DailyChallenge => pages::daily_challenge(cx),
            Page::Leaderboard => pages::leaderboard(self, narrow, cx),
            Page::Game(launch) => {
                if !self.games.contains_key(launch)
                    && let Some(view) = crate::games::open(launch, cx)
                {
                    self.games.insert(launch.clone(), view);
                }
                pages::game(self, launch, self.games.get(launch).cloned(), cx)
            }
        };

        div()
            .id("corner-table")
            .track_focus(&self.focus)
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
            .on_action(cx.listener(|desktop, _: &NavigateBack, window, cx| {
                desktop.dispatch(Action::History(NavigationDirection::Back), window, cx);
            }))
            .on_action(cx.listener(|desktop, _: &NavigateForward, window, cx| {
                desktop.dispatch(Action::History(NavigationDirection::Forward), window, cx);
            }))
            .on_action(cx.listener(|desktop, _: &OpenLibrary, window, cx| {
                desktop.dispatch(Action::Navigate(Page::Library), window, cx);
            }))
            .on_action(cx.listener(|desktop, _: &FindGame, window, cx| {
                desktop.dispatch(Action::Navigate(Page::Library), window, cx);
                desktop.search_select_all = true;
                cx.on_next_frame(window, |desktop, window, cx| {
                    desktop.search_focus.focus(window, cx);
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
            .child(widgets::sidebar(self, narrow, cx))
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
    }
}
