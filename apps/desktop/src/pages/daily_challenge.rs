use std::collections::HashSet;

use chrono::{Datelike as _, NaiveDate, TimeDelta};
use gpui::{
    Context, Div, FocusHandle, FontWeight, KeyDownEvent, Role, SharedString, Stateful, Window, div,
    prelude::*, px, relative, rgb,
};

use crate::app::{Action, Desktop, Page};
use crate::daily;
use crate::{theme as t, widgets};

pub struct DailyCalendar {
    selected: Option<NaiveDate>,
    focus: FocusHandle,
}

impl DailyCalendar {
    pub fn new(cx: &Context<Desktop>) -> Self {
        Self {
            selected: None,
            focus: cx.focus_handle().tab_index(0).tab_stop(true),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Desktop>) {
        self.focus.focus(window, cx);
    }

    fn select(&mut self, date: NaiveDate, cx: &mut Context<Desktop>) {
        self.selected = Some(date);
        cx.notify();
    }
}

pub fn daily_challenge(app: &Desktop, compact: bool, cx: &Context<Desktop>) -> Div {
    let today = daily::today();
    let palette = t::colors(cx);
    let completed = daily::completed_days(app.records.history.as_deref().unwrap_or_default());
    let streak = daily::current_streak(today.date, &completed);
    let won_today = completed.contains(&today.date);
    div()
        .flex()
        .flex_col()
        .gap(px(if compact { 10. } else { 16. }))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    t::display_type("Daily Challenge", if compact { 30. } else { 36. })
                        .id("daily-heading")
                        .role(Role::Heading)
                        .aria_level(1)
                        .aria_label("Daily Challenge"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .when(compact, |details| details.text_size(px(14.)))
                        .when(streak != 0, |details| {
                            let label = format!("{streak}-day streak");
                            details.child(
                                div()
                                    .id("daily-streak")
                                    .role(Role::Label)
                                    .aria_label(label.clone())
                                    .text_color(rgb(palette.success))
                                    .child(label),
                            )
                        })
                        .child(
                            div()
                                .text_color(rgb(palette.muted))
                                .child(today.date.format("%A, %B %-d").to_string()),
                        ),
                ),
        )
        .child(
            widgets::hero(
                today.title,
                widgets::challenge_art(today.game),
                widgets::button(
                    "launch-daily",
                    if won_today {
                        "Open completed puzzle"
                    } else {
                        "Play today’s puzzle"
                    },
                    Action::Navigate(Page::Game(today.launch())),
                    true,
                    cx,
                )
                .when(compact, |button| button.px_2().text_size(px(13.))),
            )
            .id("daily-hero")
            .role(Role::Group)
            .aria_label(format!("Today’s puzzle · {}", today.title)),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .when(compact, |status| {
                    status.text_size(px(14.)).line_height(px(20.))
                })
                .child(div().text_color(rgb(palette.muted)).child(today.objective))
                .when(won_today, |status| {
                    status.child(
                        div()
                            .id("daily-completed")
                            .role(Role::Label)
                            .aria_label("Completed")
                            .text_color(rgb(palette.success))
                            .child("Completed"),
                    )
                }),
        )
        .child(archive(app, today.date, &completed, compact, cx))
}

fn archive(
    app: &Desktop,
    today: NaiveDate,
    completed: &HashSet<NaiveDate>,
    compact: bool,
    cx: &Context<Desktop>,
) -> Div {
    let palette = t::colors(cx);
    let selected = app.daily_calendar.selected.unwrap_or(today).min(today);
    let month = selected.format("%B %Y").to_string();
    let weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let previous = daily::previous_month(selected);
    let next = daily::next_month(selected, today);

    t::panel(cx)
        .w_full()
        .flex()
        .gap(px(if compact { 14. } else { 22. }))
        .p(px(if compact { 12. } else { 16. }))
        .child(
            div()
                .id("daily-calendar")
                .role(Role::Group)
                .aria_label("Puzzle archive calendar")
                .aria_description(
                    "Use the arrow keys to select a day, then Tab to open its puzzle.",
                )
                .w(relative(0.55))
                .max_w(px(640.))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(month_button(
                            "daily-previous-month",
                            "Previous",
                            previous,
                            false,
                            cx,
                        ))
                        .child(
                            t::display_type(month.clone(), if compact { 18. } else { 22. })
                                .id("daily-month")
                                .role(Role::Heading)
                                .aria_level(2)
                                .aria_label(month)
                                .whitespace_nowrap(),
                        )
                        .child(month_button(
                            "daily-next-month",
                            "Next",
                            next,
                            next == Some(today),
                            cx,
                        )),
                )
                .child(div().flex().gap_1().children(weekdays.map(|label| {
                    div()
                        .flex_1()
                        .text_center()
                        .text_size(px(12.))
                        .line_height(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(palette.muted))
                        .child(label)
                })))
                .children(daily::month_weeks(selected).map(|week| {
                    div().flex().gap_1().children(week.map(|date| {
                        div().flex_1().min_w_0().children(date.map(|date| {
                            day_button(
                                date,
                                today,
                                selected,
                                completed.contains(&date),
                                compact,
                                &app.daily_calendar,
                                cx,
                            )
                        }))
                    }))
                })),
        )
        .child(selected_puzzle(
            selected,
            today,
            completed.contains(&selected),
            compact,
            cx,
        ))
}

fn month_button(
    id: &'static str,
    label: &'static str,
    date: Option<NaiveDate>,
    focus_day: bool,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let palette = t::colors(cx);
    div()
        .id(id)
        .h(px(30.))
        .w(px(72.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .when_some(date, |button, date| {
            button
                .role(Role::Button)
                .aria_label(format!("{label} month"))
                .tab_index(0)
                .cursor_pointer()
                .border_color(rgb(palette.border))
                .hover(|style| style.bg(rgb(palette.hover)))
                .focus_visible(|style| style.border_color(rgb(palette.ink)))
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.daily_calendar.select(date, cx);
                    if focus_day {
                        app.daily_calendar.focus(window, cx);
                    }
                }))
        })
        .when(date.is_none(), |button| {
            button
                .role(Role::Label)
                .aria_label(format!("{label} month unavailable"))
                .border_color(rgb(palette.surface))
                .text_color(rgb(palette.muted))
                .opacity(0.6)
        })
        .child(label)
}

fn day_button(
    date: NaiveDate,
    today: NaiveDate,
    selected: NaiveDate,
    completed: bool,
    compact: bool,
    calendar: &DailyCalendar,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let palette = t::colors(cx);
    let current = date == today;
    let active = date == selected;
    let available = date <= today;
    let full_date = date.format("%A, %B %-d, %Y");
    let label = if available {
        let puzzle = daily::puzzle_on(date);
        let today = if current { ", today" } else { "" };
        let completed = if completed { ", completed" } else { "" };
        format!("{full_date}{today}, {}{completed}", puzzle.title)
    } else {
        format!("{full_date}, unavailable")
    };
    let background = if active { t::GOLD } else { palette.surface };
    let color = if active { t::INK } else { palette.ink };
    let dot = if active {
        t::LIGHT.success
    } else {
        palette.success
    };

    div()
        .id(SharedString::from(format!("daily-date-{date}")))
        .aria_label(label)
        .h(px(if compact { 28. } else { 44. }))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if current && !active {
            palette.gold_text
        } else {
            background
        }))
        .bg(rgb(background))
        .text_color(rgb(color))
        .text_size(px(13.))
        .line_height(px(17.))
        .when(available, |button| {
            button
                .role(Role::Button)
                .aria_selected(active)
                .tab_index(0)
                .tab_stop(active)
                .when(active, |button| button.track_focus(&calendar.focus))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(if active { t::GOLD_HOVER } else { palette.hover })))
                .focus_visible(|style| style.border_2().border_color(rgb(palette.ink)))
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.daily_calendar.select(date, cx);
                    app.daily_calendar.focus(window, cx);
                }))
                .on_key_down(cx.listener(move |app, event: &KeyDownEvent, window, cx| {
                    let stroke = &event.keystroke;
                    if stroke.modifiers.modified() {
                        return;
                    }
                    let offset = match stroke.key.as_str() {
                        "left" => -1,
                        "right" => 1,
                        "up" => -7,
                        "down" => 7,
                        _ => return,
                    };
                    if let Some(day) = date.checked_add_signed(TimeDelta::days(offset))
                        && day.year() >= 0
                        && day <= today
                    {
                        app.daily_calendar.select(day, cx);
                        app.daily_calendar.focus(window, cx);
                    }
                    cx.stop_propagation();
                }))
        })
        .when(!available, |cell| {
            cell.role(Role::Label)
                .text_color(rgb(palette.muted))
                .opacity(0.55)
        })
        .child(
            div()
                .relative()
                .when(active || current, |number| {
                    number.font_weight(FontWeight::BOLD)
                })
                .when(current, |number| number.underline())
                .child(date.day().to_string())
                .when(completed && available, |number| {
                    number.child(completion_dot(dot).absolute().left_full().top_0())
                }),
        )
}

fn completion_dot(color: u32) -> Div {
    div()
        .size(px(7.))
        .rounded_full()
        .bg(rgb(color))
        .flex_shrink_0()
}

fn selected_puzzle(
    date: NaiveDate,
    today: NaiveDate,
    completed: bool,
    compact: bool,
    cx: &Context<Desktop>,
) -> Div {
    let palette = t::colors(cx);
    let puzzle = daily::puzzle_on(date);
    let short_date = date.format("%a, %b %-d");
    let date_label = if date == today {
        format!("Today · {short_date}")
    } else {
        short_date.to_string()
    };
    let button_label = if completed {
        "Open completed puzzle"
    } else {
        "Play puzzle"
    };
    let game_label = format!(
        "{button_label} · {} · {}",
        puzzle.title,
        date.format("%B %-d, %Y")
    );

    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .justify_between()
        .gap_2()
        .border_l_1()
        .border_color(rgb(palette.border))
        .pl(px(if compact { 13. } else { 21. }))
        .child(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap_1()
                .when(!compact, |details| {
                    details.child(
                        div()
                            .w_full()
                            .h(px(150.))
                            .mb_2()
                            .rounded_lg()
                            .overflow_hidden()
                            .child(widgets::artwork(widgets::challenge_art(puzzle.game))),
                    )
                })
                .child(
                    div()
                        .text_color(rgb(palette.muted))
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .child(date_label),
                )
                .child(t::display_type(
                    puzzle.title,
                    if compact { 19. } else { 23. },
                ))
                .when(completed, |details| {
                    details.child(
                        div()
                            .text_color(rgb(palette.success))
                            .text_size(px(13.))
                            .line_height(px(17.))
                            .child("Completed"),
                    )
                })
                .child(
                    div()
                        .id("daily-selected-objective")
                        .role(Role::Label)
                        .aria_label(puzzle.objective)
                        .text_color(rgb(palette.muted))
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .child(puzzle.objective),
                )
                .child(
                    widgets::button(
                        "launch-archive-puzzle",
                        button_label,
                        Action::Navigate(Page::Game(puzzle.launch())),
                        false,
                        cx,
                    )
                    .aria_label(game_label)
                    .mt_2()
                    .min_h(px(34.))
                    .px_2()
                    .text_size(px(13.)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_color(rgb(palette.muted))
                .text_size(px(12.))
                .line_height(px(18.))
                .child(completion_dot(palette.success))
                .child("Completed days"),
        )
}

pub fn game_calendar_button(
    date: NaiveDate,
    compact: bool,
    cx: &Context<Desktop>,
) -> Stateful<Div> {
    let full_date = date.format("%b %-d, %Y");
    let label = if compact {
        date.format("%b %-d").to_string()
    } else {
        format!("Calendar · {full_date}")
    };
    widgets::button(
        "game-daily-date",
        label,
        Action::ReturnToDailyCalendar,
        false,
        cx,
    )
    .aria_label(format!("Back to calendar · Daily puzzle · {full_date}"))
    .min_h(px(34.))
    .px_3()
    .text_size(px(13.))
    .when(compact, |button| {
        button
            .px_2()
            .text_size(px(12.))
            .gap_1()
            .child(widgets::symbol("calendar", t::colors(cx).ink, 14.))
    })
}
