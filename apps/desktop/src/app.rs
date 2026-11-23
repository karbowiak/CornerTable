use std::collections::HashMap;

use corner_table_api::{FixtureApi, GameId, Period};
use gpui::{AnyView, Context, FocusHandle, ScrollHandle, Window, div, point, prelude::*, px, rgb};

use crate::games::{GameEntry, GameLaunch};
use crate::{pages, theme, widgets};

gpui::actions!(corner_table, [FocusNext, FocusPrevious]);

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
    OpenGame(GameId),
    ChallengeLeaderboard,
    ToggleGamePicker,
    SelectGame(GameId),
    SelectPeriod(Period),
    ClearSearch,
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
    scroll: ScrollHandle,
    games: HashMap<GameLaunch, AnyView>,
    pub focus: FocusHandle,
}

impl Desktop {
    pub fn new(api: FixtureApi, cx: &mut Context<Self>) -> Self {
        let board_game = api.home.challenge.game_id.clone();
        Self {
            api,
            page: Page::Library,
            board_game,
            board_period: Period::Today,
            game_picker_open: false,
            search: String::new(),
            search_focus: cx.focus_handle().tab_stop(true),
            search_select_all: false,
            scroll: ScrollHandle::new(),
            games: HashMap::new(),
            focus: cx.focus_handle(),
        }
    }

    pub fn dispatch(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Navigate(page) => self.navigate(page),
            Action::OpenGame(id) if id.0 == "solitaire" => self.navigate(Page::Solitaire),
            Action::OpenGame(game) => self.navigate(Page::Game(GameLaunch {
                game,
                entry: GameEntry::Classic,
            })),
            Action::ChallengeLeaderboard => {
                self.board_game = self.api.home.challenge.game_id.clone();
                self.board_period = Period::Today;
                self.navigate(Page::Leaderboard);
            }
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
        }
        cx.notify();
    }

    fn navigate(&mut self, page: Page) {
        self.page = page;
        self.game_picker_open = false;
        self.search_select_all = false;
        self.scroll.set_offset(point(px(0.), px(0.)));
    }
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(1080.);
        let content = match &self.page {
            Page::Library => pages::library(self, narrow, cx),
            Page::Solitaire => pages::solitaire(self, narrow, cx),
            Page::DailyChallenge => pages::daily_challenge(self, cx),
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
            .size_full()
            .flex()
            .tab_group()
            .font_family(theme::BODY)
            .text_size(px(15.))
            .line_height(px(23.))
            .text_color(rgb(theme::INK))
            .bg(rgb(theme::PAPER))
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
                            .py(px(27.))
                            .child(content),
                    ),
            )
    }
}
