use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, IntoElement, Role, SharedString, div, ease_out_quint,
    prelude::*, px, rgb,
};

use crate::{motion, storage::Outcome, theme, widgets};

pub fn round_result(
    cx: &App,
    id: &'static str,
    outcome: Outcome,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    actions: impl IntoElement,
) -> AnyElement {
    let palette = theme::colors(cx);
    let accent = match outcome {
        Outcome::Won => palette.success,
        Outcome::Lost => palette.danger,
        Outcome::Draw | Outcome::Finished => palette.gold_text,
    };
    let title = title.into();
    let detail = detail.into();
    let card = theme::panel(cx)
        .id(id)
        .role(Role::Group)
        .aria_label(title.clone())
        .w_full()
        .max_w(px(522.))
        .border_l_4()
        .border_color(rgb(accent))
        .px_5()
        .py_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .when(outcome == Outcome::Won, |line| {
                    line.child(widgets::symbol("trophy", accent, 24.))
                })
                .child(
                    theme::display_type(title.clone(), 27.)
                        .id("round-title")
                        .role(Role::Heading)
                        .aria_level(2)
                        .aria_label(title.clone())
                        .text_color(rgb(accent)),
                ),
        )
        .when(!detail.is_empty(), |card| {
            card.child(
                div()
                    .id("round-details")
                    .role(Role::Status)
                    .aria_label(format!("{title}. {detail}"))
                    .text_color(rgb(palette.muted))
                    .child(detail),
            )
        })
        .child(actions);
    if motion::enabled(cx) {
        card.with_animation(
            SharedString::from(format!("{id}-entrance")),
            Animation::new(Duration::from_millis(260)).with_easing(ease_out_quint()),
            |card, value| card.opacity(0.55 + value * 0.45),
        )
        .into_any_element()
    } else {
        card.into_any_element()
    }
}
