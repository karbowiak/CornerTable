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
    let (top, bottom, edge, ink) = if quiet {
        (0xe1d5bc, 0xc9bca5, 0x9e8f78, 0x514638)
    } else {
        (0xfff8e8, 0xf0dfbf, 0xa88c6a, PIP_INK)
    };
    let groove = div()
        .flex_none()
        .bg(rgb(if quiet { 0xaa9b82 } else { 0xb49c77 }))
        .when_else(
            portrait,
            |line| line.w(px(unit * 0.73)).h(px(1.)),
            |line| line.w(px(1.)).h(px(unit * 0.73)),
        );
    div()
        .w(px(width))
        .h(px(height))
        .flex()
        .flex_none()
        .when(portrait, |tile| tile.flex_col())
        .items_center()
        .justify_center()
        .rounded(px((unit * 0.14).max(3.)))
        .border_1()
        .border_b_2()
        .border_color(rgb(edge))
        .bg(linear_gradient(
            150.,
            linear_color_stop(rgb(top), 0.),
            linear_color_stop(rgb(bottom), 1.),
        ))
        .shadow(vec![
            BoxShadow::new(px(0.), px(2.), rgba(0x100c0a70).into()).blur_radius(px(4.)),
            BoxShadow::new(px(0.), px(1.), rgba(0xfffff5bf).into()).inset(),
        ])
        .child(face(faces[0], unit - 4., ink))
        .child(groove)
        .child(face(faces[1], unit - 4., ink))
}

pub(super) fn face(value: u8, unit: f32, ink: u32) -> Div {
    let dot = (unit * 0.153).max(2.);
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
                .bg(rgb(ink))
                .shadow(vec![BoxShadow::new(
                    px(0.),
                    px(1.),
                    rgba(0xffffff51).into(),
                )])
        }))
}

pub(super) fn back(width: f32, height: f32) -> Div {
    div()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .rounded(px(4.))
        .border_1()
        .border_color(rgb(0xbc9d76))
        .bg(linear_gradient(
            145.,
            linear_color_stop(rgb(0x704f41), 0.),
            linear_color_stop(rgb(0x3f2c2d), 1.),
        ))
        .shadow(vec![
            BoxShadow::new(px(0.), px(2.), rgba(0x17101480).into()).blur_radius(px(3.)),
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
