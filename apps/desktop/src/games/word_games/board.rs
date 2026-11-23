use gpui::{
    Bounds, BoxShadow, Canvas, Div, FontWeight, Hsla, PathBuilder, Pixels, Point, Stateful, Window,
    canvas, div, fill, linear_color_stop, linear_gradient, point, prelude::*, px, rgb, size,
};

use crate::theme as t;

pub(super) const PAPER: u32 = 0xfff6e1;
pub(super) const SOLVED_PAPER: u32 = 0xd9e6c8;
pub(super) const PRESS_INK: u32 = 0x3c5044;
pub(super) const HINT_INK: u32 = 0x315c66;
pub(super) const CHECK_INK: u32 = 0x943a2e;
const WOOD_INK: u32 = 0x422922;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TileState {
    Resting,
    Picked,
    Hint,
}

pub(super) fn letter_bed(bed: Stateful<Div>, dark: bool) -> Stateful<Div> {
    let (edge, face, shadow) = if dark {
        (0x72613c, 0x284237, 0x172c25)
    } else {
        (0x886e46, 0x425c46, 0x283e33)
    };
    bed.relative()
        .p_3()
        .rounded_lg()
        .border_2()
        .border_color(rgb(edge))
        .focus_visible(|style| style.border_color(rgb(t::GOLD)))
        .bg(linear_gradient(
            180.,
            linear_color_stop(rgb(face), 0.),
            linear_color_stop(rgb(shadow), 1.),
        ))
        .child(grain(Fibre::Cloth, 0))
        .child(
            div()
                .absolute()
                .top_1()
                .right_1()
                .bottom_1()
                .left_1()
                .rounded_md()
                .border_1()
                .border_color(rgb(0xe6cc8e).opacity(0.32)),
        )
}

pub(super) fn wood_tile(
    index: usize,
    edge_length: f32,
    letter: char,
    state: TileState,
    order: Option<usize>,
) -> Stateful<Div> {
    let small = edge_length < 52.;
    let (light, dark, edge, ink) = match state {
        TileState::Resting => (
            [0xf9e6b6, 0xf5dca5, 0xf7e2b2][index % 3],
            0xe3bb7f,
            0xa47545,
            WOOD_INK,
        ),
        TileState::Picked => (0xffdb77, 0xe9a647, 0xb57230, WOOD_INK),
        TileState::Hint => (0xe0ece4, 0xaccece, 0x54878b, 0x193f46),
    };
    div()
        .id(("letter-tile", index))
        .relative()
        .size(px(edge_length))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_b(px(4.))
        .border_color(rgb(edge))
        .bg(linear_gradient(
            180.,
            linear_color_stop(rgb(light), 0.),
            linear_color_stop(rgb(dark), 1.),
        ))
        .text_color(rgb(ink))
        .font_family(t::DISPLAY)
        .font_weight(FontWeight::BOLD)
        .child(grain(Fibre::Wood, index as u32))
        .child(
            div()
                .absolute()
                .top(px(1.))
                .left(px(5.))
                .right(px(5.))
                .h(px(1.))
                .bg(rgb(0xfff7da).opacity(0.65)),
        )
        .child(
            div()
                .relative()
                .top(px(if small { 4. } else { 0. }))
                .child(letter.to_string()),
        )
        .children(order.map(|order| {
            let stamp = if state == TileState::Picked {
                0x654219
            } else {
                HINT_INK
            };
            let stamp_size = px(if small { 12. } else { 14. });
            div()
                .absolute()
                .top(px(3.))
                .right(px(3.))
                .min_w(stamp_size)
                .h(stamp_size)
                .px(px(2.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .bg(rgb(stamp))
                .font_family(t::BODY)
                .font_weight(FontWeight::BOLD)
                .text_size(px(10.))
                .line_height(stamp_size)
                .text_color(rgb(0xfff6df))
                .child(order.to_string())
        }))
}

pub(super) fn letter_trails(
    order: &[usize],
    columns: usize,
    chosen: &[usize],
    hinted: &[usize],
    step: f32,
    tile: f32,
) -> Canvas<()> {
    let centers = |path: &[usize]| {
        path.iter()
            .filter_map(|index| order.iter().position(|tile| tile == index))
            .map(|position| {
                point(
                    px((position % columns) as f32 * step + tile / 2.),
                    px((position / columns) as f32 * step + tile / 2.),
                )
            })
            .collect::<Vec<_>>()
    };
    let chosen = centers(chosen);
    let hinted = centers(hinted);
    canvas(
        |_, _, _| {},
        move |area, (), window, _| {
            for pair in hinted.windows(2) {
                for fraction in [0.46, 0.54] {
                    let center = area.origin + pair[0] + (pair[1] - pair[0]) * fraction;
                    dot(center, px(3.), rgb(0xa5d2ce).into(), window);
                }
            }
            let points = chosen
                .iter()
                .map(|center| area.origin + *center)
                .collect::<Vec<_>>();
            if points.len() > 1 {
                ribbon(&points, 10., rgb(0x6b431f).into(), window);
                ribbon(&points, 6., rgb(0xfdd48a).into(), window);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub(super) fn folio(sheet: Stateful<Div>) -> Stateful<Div> {
    sheet
        .relative()
        .p_3()
        .rounded_sm()
        .border_2()
        .border_color(rgb(0x927c5b))
        .focus_visible(|style| style.border_color(rgb(0xa16b1e)))
        .shadow(vec![
            BoxShadow::new(px(0.), px(2.), rgb(0x38291d).opacity(0.22).into()).blur_radius(px(7.)),
        ])
        .bg(rgb(0xeadebf))
        .child(grain(Fibre::Paper, 11))
        .child(press_marks())
}

fn press_marks() -> Canvas<()> {
    canvas(
        |_, _, _| {},
        |area, (), window, _| {
            let ink = rgb(PRESS_INK).opacity(0.56);
            let mut marks = PathBuilder::stroke(px(0.75));
            for (x, horizontal) in [(area.left() + px(7.), 1.), (area.right() - px(7.), -1.)] {
                for (y, vertical) in [(area.top() + px(7.), 1.), (area.bottom() - px(7.), -1.)] {
                    marks.move_to(point(x + px(5. * horizontal), y));
                    marks.line_to(point(x, y));
                    marks.line_to(point(x, y + px(5. * vertical)));
                }
            }
            if let Ok(path) = marks.build() {
                window.paint_path(path, ink);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub(super) fn crossword_cell(index: usize, face: Hsla) -> Stateful<Div> {
    div()
        .id(("crossword-square", index))
        .relative()
        .size(px(super::CROSSWORD_TILE))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .border_2()
        .border_color(gpui::transparent_black())
        .bg(face)
        .text_color(rgb(t::INK))
        .font_family(t::DISPLAY)
        .font_weight(FontWeight::BOLD)
        .child(grain(Fibre::Paper, index as u32))
}

#[derive(Clone, Copy)]
pub(super) enum Fibre {
    Wood,
    Paper,
    DarkPaper,
    Cloth,
}

/// The fibre pattern is stable across gestures and kept below the lettering.
pub(super) fn grain(stock: Fibre, seed: u32) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |area, (), window, _| {
            let (mut seed, width, height) = (
                seed.wrapping_add(451),
                f32::from(area.size.width),
                f32::from(area.size.height),
            );
            if width <= 10. || height <= 10. {
                return;
            }
            let (strokes, ink, length, slope) = match stock {
                Fibre::Wood => (12, rgb(0x765230).opacity(0.13), width * 0.78, 2.),
                Fibre::Paper | Fibre::DarkPaper => (
                    (width * height / 120.).clamp(10., 360.) as usize,
                    if matches!(stock, Fibre::DarkPaper) {
                        rgb(0xd2d6a2).opacity(0.13)
                    } else {
                        rgb(0x7b5b36).opacity(0.15)
                    },
                    3.,
                    0.8,
                ),
                Fibre::Cloth => (
                    (width * height / 440.).clamp(46., 240.) as usize,
                    rgb(0xe4d197).opacity(0.15),
                    3.,
                    1.,
                ),
            };
            let mut path = PathBuilder::stroke(px(0.65));
            for _ in 0..strokes {
                let x = 4. + noise(&mut seed) * (width - 8.);
                let y = 4. + noise(&mut seed) * (height - 8.);
                let span = ((0.35 + 0.65 * noise(&mut seed)) * length).min(width - x - 3.);
                let drift = slope * (2. * noise(&mut seed) - 1.);
                let end = area.origin + point(px(x + span), px(y + drift));
                path.move_to(area.origin + point(px(x), px(y)));
                match stock {
                    Fibre::Wood => path.cubic_bezier_to(
                        end,
                        area.origin + point(px(x + span / 3.), px(y + drift + 1.)),
                        area.origin + point(px(x + span * 2. / 3.), px(y + drift - 1.)),
                    ),
                    _ => path.line_to(end),
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, ink);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn noise(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 16) as f32 / 65_535.
}

pub(super) fn ribbon(points: &[Point<Pixels>], width: f32, ink: Hsla, window: &mut Window) {
    let width = px(width);
    if let [start, rest @ ..] = points {
        let mut path = PathBuilder::stroke(width);
        path.move_to(*start);
        for end in rest {
            path.line_to(*end);
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, ink);
        }
        for &center in points {
            dot(center, width, ink, window);
        }
    }
}

fn dot(center: Point<Pixels>, width: Pixels, ink: Hsla, window: &mut Window) {
    let radius = width / 2.;
    window.paint_quad(
        fill(
            Bounds::new(center - point(radius, radius), size(width, width)),
            ink,
        )
        .corner_radii(radius),
    );
}
