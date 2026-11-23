use gpui::{BoxShadow, Div, div, linear_color_stop, linear_gradient, prelude::*, px, rgb, rgba};

pub(super) const BONE: u32 = 0xfff3d8;
const PIP_INK: u32 = 0x211f23;

#[derive(Clone, Copy)]
pub(super) enum Pose {
    Horizontal,
    Vertical,
}

pub(super) fn ivory(faces: [u8; 2], unit: f32, pose: Pose, quiet: bool) -> Div {
    let portrait = matches!(pose, Pose::Vertical);
    let (width, height) = if portrait {
        (unit, unit * 2.)
    } else {
        (unit * 2., unit)
    };
    let (top, bottom, edge, glaze, ink) = if quiet {
        (0xe5dac4, 0xc8baa2, 0x947e66, 0xf1e7d2, 0x4a4035)
    } else {
        (0xfff9e9, 0xe8d4af, 0xa78962, 0xfffef1, PIP_INK)
    };
    let radius = px((unit * 0.145).max(3.));
    let depth = px((unit * 0.088).clamp(1.75, 3.5));
    let groove = div()
        .flex_none()
        .rounded(px(1.))
        .bg(linear_gradient(
            if portrait { 180. } else { 90. },
            linear_color_stop(rgba(0x735939b9), 0.28),
            linear_color_stop(rgb(glaze), 0.78),
        ))
        .when_else(
            portrait,
            |line| line.w(px(unit * 0.76)).h(px(2.)),
            |line| line.w(px(2.)).h(px(unit * 0.76)),
        );
    div()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .relative()
        .rounded(radius)
        .bg(rgb(edge))
        .shadow(vec![
            BoxShadow::new(px(0.), px(2.), rgba(0x100c0a7d).into()).blur_radius(px(3.)),
        ])
        .child(
            div()
                .absolute()
                .inset_0()
                .bottom(depth)
                .flex()
                .when(portrait, |tile| tile.flex_col())
                .items_center()
                .justify_center()
                .rounded(radius)
                .border_1()
                .border_color(rgb(glaze))
                .bg(linear_gradient(
                    152.,
                    linear_color_stop(rgb(top), 0.),
                    linear_color_stop(rgb(bottom), 1.),
                ))
                .child(face(faces[0], unit - 5., ink))
                .child(groove)
                .child(face(faces[1], unit - 5., ink)),
        )
}

pub(super) fn face(value: u8, unit: f32, ink: u32) -> Div {
    let dot = (unit * 0.175).max(2.8);
    let positions: &[(f32, f32)] = match value {
        0 => &[],
        1 => &[(0.5, 0.5)],
        2 => &[(0.26, 0.26), (0.74, 0.74)],
        3 => &[(0.26, 0.26), (0.5, 0.5), (0.74, 0.74)],
        4 => &[(0.26, 0.26), (0.74, 0.26), (0.26, 0.74), (0.74, 0.74)],
        5 => &[
            (0.26, 0.26),
            (0.74, 0.26),
            (0.5, 0.5),
            (0.26, 0.74),
            (0.74, 0.74),
        ],
        6 => &[
            (0.26, 0.24),
            (0.74, 0.24),
            (0.26, 0.5),
            (0.74, 0.5),
            (0.26, 0.76),
            (0.74, 0.76),
        ],
        _ => unreachable!(),
    };
    div()
        .relative()
        .size(px(unit))
        .flex_none()
        .children(positions.iter().map(|&(x, y)| {
            div()
                .absolute()
                .left(px(unit * x - dot * 0.5))
                .top(px(unit * y - dot * 0.5))
                .size(px(dot))
                .rounded_full()
                .bg(linear_gradient(
                    160.,
                    linear_color_stop(rgb(ink), 0.22),
                    linear_color_stop(rgba((ink << 8) | 0xc9), 1.),
                ))
                .shadow(vec![
                    BoxShadow::new(px(0.), px(0.75), rgba(0xfffbe3a3).into()).blur_radius(px(0.6)),
                    BoxShadow::new(px(0.), px(0.8), rgba(0x130f1488).into())
                        .blur_radius(px(0.8))
                        .inset(),
                ])
        }))
}

pub(super) fn back(width: f32, height: f32) -> Div {
    div()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .rounded(px(4.))
        .border_1()
        .border_b_2()
        .border_color(rgb(0xa78059))
        .bg(linear_gradient(
            145.,
            linear_color_stop(rgb(0x80583f), 0.),
            linear_color_stop(rgb(0x47312c), 1.),
        ))
        .shadow(vec![
            BoxShadow::new(px(0.), px(2.), rgba(0x17101480).into()).blur_radius(px(3.)),
            BoxShadow::new(px(0.), px(1.), rgba(0xf7c9976b).into()).inset(),
        ])
        .p(px((width * 0.22).max(2.)))
        .child(
            div()
                .size_full()
                .rounded(px(2.))
                .border_1()
                .border_color(rgb(0xab8465))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(width * 0.20))
                        .h(px(height * 0.17))
                        .rounded_full()
                        .bg(rgb(0xc29a6b)),
                ),
        )
}
