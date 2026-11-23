use corner_table_peg_solitaire::{Hole, Jump, SIDE, Status};
use gpui::{
    Bounds, BoxShadow, Context, Div, FontWeight, Role, Stateful, Window, canvas, div, fill,
    linear_color_stop, linear_gradient, point, prelude::*, px, rgb, rgba, size,
};

use super::GameView;

pub(super) const FRAME: f32 = 22.;
const FOCUS: u32 = 0xfff2d4;
const SELECTED: u32 = 0xffc869;
const HINT: u32 = 0x9ef3dc;

impl GameView {
    pub(super) fn board(
        &self,
        cell: f32,
        flight: Option<(Jump, f32)>,
        window: &Window,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let width = cell * f32::from(SIDE) + FRAME * 2.;
        let focused = self.focus.is_focused(window);
        let playing = self.position().status() == Status::Playing;
        let landing = flight.map(|(jump, _)| jump.to);
        div()
            .id("peg-solitaire-board")
            .debug_selector(|| "peg-solitaire-board".into())
            .role(Role::Grid)
            .aria_label("33-hole Peg Solitaire board")
            .aria_description(
                "Arrow keys move. Enter selects a peg or landing hole. H shows a jump.",
            )
            .aria_row_count(usize::from(SIDE))
            .aria_column_count(usize::from(SIDE))
            .track_focus(&self.focus)
            .tab_index(0)
            .flex_none()
            .relative()
            .size(px(width))
            .rounded(px(20.))
            .bg(linear_gradient(
                148.,
                linear_color_stop(rgb(0x4a777a), 0.),
                linear_color_stop(rgb(0x152d38), 1.),
            ))
            .shadow(vec![
                BoxShadow::new(px(0.), px(6.), rgba(0x11192168).into()).blur_radius(px(12.)),
                BoxShadow::new(px(0.), px(2.), rgba(0xc8ecdd88).into()).inset(),
                BoxShadow::new(px(0.), px(-4.), rgba(0x101a23c2).into()).inset(),
            ])
            .child(
                div()
                    .absolute()
                    .inset(px(5.))
                    .rounded(px(15.))
                    .border_1()
                    .border_color(rgba(0x99ccc957))
                    .overflow_hidden()
                    .child(enamel_flecks()),
            )
            .child(
                div()
                    .absolute()
                    .left(px(FRAME))
                    .top(px(6.))
                    .flex()
                    .children((b'A'..b'A' + SIDE).map(|col| axis(cell, 15., char::from(col)))),
            )
            .child(
                div()
                    .absolute()
                    .left(px(6.))
                    .top(px(FRAME))
                    .flex()
                    .flex_col()
                    .children((b'1'..b'1' + SIDE).map(|row| axis(14., cell, char::from(row)))),
            )
            .child(
                div()
                    .absolute()
                    .top(px(FRAME))
                    .left(px(FRAME))
                    .flex()
                    .flex_col()
                    .children((0..SIDE).map(|row| {
                        div()
                            .id(("peg-solitaire-row", usize::from(row)))
                            .role(Role::Row)
                            .aria_row_index(usize::from(row) + 1)
                            .flex()
                            .children((0..SIDE).map(|column| {
                                match Hole::new(row, column) {
                                    Some(hole) => self
                                        .hole(hole, cell, focused, playing, landing, cx)
                                        .into_any_element(),
                                    None => div().size(px(cell)).flex_none().into_any_element(),
                                }
                            }))
                    })),
            )
            .children(flight.map(|(jump, progress)| {
                let interpolate = |from: u8, to: u8| {
                    FRAME + (f32::from(from) * (1. - progress) + f32::from(to) * progress) * cell
                };
                let rise = cell * 1.65 * progress * (1. - progress);
                div()
                    .debug_selector(|| "peg-solitaire-flight".into())
                    .absolute()
                    .left(px(interpolate(jump.from.column(), jump.to.column())))
                    .top(px(interpolate(jump.from.row(), jump.to.row()) - rise))
                    .size(px(cell))
                    .child(peg(cell, false))
            }))
    }

    fn hole(
        &self,
        hole: Hole,
        cell: f32,
        focused: bool,
        playing: bool,
        flying_to: Option<Hole>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let position = self.position();
        let occupied = position.occupied(hole);
        let center = hole == Hole::CENTER;
        let cursor = focused && self.cursor == hole;
        let selected = self.selected == Some(hole);
        let hinted = self
            .hint
            .is_some_and(|jump| jump.from == hole || jump.to == hole);
        let landing = self
            .selected
            .is_some_and(|from| position.jumps_from(from).any(|jump| jump.to == hole));
        let location = if center {
            format!("{hole}, brass center")
        } else {
            hole.to_string()
        };
        let contents = match (occupied, selected, hinted, landing) {
            (true, true, _, _) => "selected peg",
            (true, false, true, _) => "suggested peg",
            (true, _, _, _) => "peg",
            (false, _, true, _) => "suggested landing hole",
            (false, _, _, true) => "legal landing hole",
            (false, _, _, _) => "empty hole",
        };
        let label = format!("{location}, {contents}");
        let marker = if hinted {
            Some(HINT)
        } else if selected || landing {
            Some(SELECTED)
        } else {
            None
        };

        div()
            .id(("peg-solitaire-hole", usize::from(hole.index())))
            .debug_selector(move || format!("peg-solitaire-hole-{hole}"))
            .role(Role::GridCell)
            .aria_column_index(usize::from(hole.column()) + 1)
            .aria_label(label)
            .aria_selected(selected)
            .when(cursor, |square| square.aria_active_descendant())
            .relative()
            .size(px(cell))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .when(playing, |square| {
                square
                    .cursor_pointer()
                    .hover(|style| style.bg(rgba(0xe3ffef21)))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.focus.focus(window, cx);
                        view.activate(hole, cx);
                    }))
            })
            .when(cursor, |square| {
                square.child(
                    div()
                        .absolute()
                        .inset(px(1.))
                        .rounded(px(8.))
                        .border_2()
                        .border_color(rgb(FOCUS)),
                )
            })
            .child(socket(cell, center, marker))
            .when(occupied && flying_to != Some(hole), |square| {
                square.child(peg(cell, selected))
            })
            .when(!occupied && (landing || hinted), |square| {
                square.child(
                    div()
                        .absolute()
                        .top(px(cell * 0.43))
                        .left(px(cell * 0.43))
                        .size(px(cell * 0.14))
                        .rounded_full()
                        .bg(rgb(marker.unwrap_or(SELECTED))),
                )
            })
    }
}

fn axis(width: f32, height: f32, label: char) -> Div {
    div()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .flex()
        .justify_center()
        .items_center()
        .text_size(px(11.))
        .line_height(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(0xcfebe1))
        .child(label.to_string())
}

fn socket(cell: f32, center: bool, marker: Option<u32>) -> Div {
    let (rim_dark, rim_light) = if center {
        (0x97642c, 0xf5ca75)
    } else {
        (0x102631, 0x72949a)
    };
    div()
        .size(px(cell * 0.81))
        .relative()
        .rounded_full()
        .bg(linear_gradient(
            150.,
            linear_color_stop(rgb(rim_dark), 0.),
            linear_color_stop(rgb(rim_light), 1.),
        ))
        .child(
            div()
                .absolute()
                .inset(px(cell * if center { 0.083 } else { 0.064 }))
                .rounded_full()
                .bg(linear_gradient(
                    148.,
                    linear_color_stop(rgb(0x081d27), 0.),
                    linear_color_stop(rgb(0x284b55), 1.),
                ))
                .shadow(vec![
                    BoxShadow::new(px(0.), px(2.), rgba(0x050e16bc).into())
                        .blur_radius(px(4.))
                        .inset(),
                ]),
        )
        .when_some(marker, |rim, color| {
            rim.child(
                div()
                    .absolute()
                    .inset(px(-1.))
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(color)),
            )
        })
}

fn peg(cell: f32, raised: bool) -> Div {
    div()
        .absolute()
        .inset(px(cell * 0.18))
        .top(px(cell * if raised { 0.09 } else { 0.135 }))
        .bottom(px(cell * if raised { 0.27 } else { 0.225 }))
        .rounded_full()
        .bg(rgb(0x6a321d))
        .shadow(vec![
            BoxShadow::new(
                px(1.),
                px(if raised { 6. } else { 3. }),
                rgba(0x08151ab8).into(),
            )
            .blur_radius(px(if raised { 5. } else { 3. })),
        ])
        .child(
            div()
                .absolute()
                .inset_0()
                .bottom(px(cell * 0.067))
                .rounded_full()
                .bg(linear_gradient(
                    144.,
                    linear_color_stop(rgb(0xffd99d), 0.),
                    linear_color_stop(rgb(0xa24e28), 1.),
                ))
                .child(
                    div()
                        .absolute()
                        .inset(px(cell * 0.065))
                        .rounded_full()
                        .bg(linear_gradient(
                            144.,
                            linear_color_stop(rgb(0xffc57e), 0.),
                            linear_color_stop(rgb(0xc06b36), 1.),
                        ))
                        .shadow(vec![
                            BoxShadow::new(px(0.), px(1.), rgba(0xfff0bc70).into()).inset(),
                            BoxShadow::new(px(0.), px(-1.), rgba(0x6e351c86).into()).inset(),
                        ])
                        .child(
                            div()
                                .absolute()
                                .top(px(cell * 0.082))
                                .left(px(cell * 0.103))
                                .w(px(cell * 0.15))
                                .h(px(cell * 0.047))
                                .rounded_full()
                                .bg(rgba(0xffedbead)),
                        ),
                ),
        )
}

fn enamel_flecks() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            let mut grain = 49_u32;
            for i in 0..560 {
                grain = grain.wrapping_mul(1664525).wrapping_add(1013904223);
                let x = (grain >> 16) as f32 / 65536.;
                let y = (grain & 0xffff) as f32 / 65536.;
                let radius = px(if i % 7 == 0 { 0.85 } else { 0.45 });
                let origin = bounds.origin + point(bounds.size.width * x, bounds.size.height * y);
                let tint = if i % 3 == 0 { 0xc7e2ca36 } else { 0x081b2768 };
                window.paint_quad(
                    fill(Bounds::new(origin, size(2. * radius, radius)), rgba(tint))
                        .corner_radii(radius),
                );
            }
        },
    )
    .size_full()
}
