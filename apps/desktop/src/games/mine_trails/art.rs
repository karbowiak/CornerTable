use gpui::{
    AnyElement, ObjectFit, PathBuilder, Pixels, Point, Rgba, Window, canvas, div, img, point,
    prelude::*, px, rgb, rgba,
};

pub(super) const ENTRANCE: &str = "games/mine-trails/mine-entrance.jpg";
pub(super) const SLATE: &str = "games/mine-trails/slate-bed.jpg";
pub(super) const TUNNELS: &str = "games/mine-trails/tunnel-strata.jpg";
pub(super) const VAULT: &str = "games/mine-trails/vault-chamber.jpg";

pub(super) fn painting(asset: &'static str) -> impl IntoElement {
    div()
        .size_full()
        .overflow_hidden()
        .child(img(asset).size_full().object_fit(ObjectFit::Cover))
}

#[derive(Clone, Copy)]
pub(super) enum Mark {
    Diamond,
    Flag,
    WrongFlag,
    Mine,
    Here,
    Check,
    Ring,
    Lock,
}

pub(super) fn symbol(mark: Mark, size: f32, ink: u32) -> AnyElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let at = |x: f32, y: f32| {
                bounds.origin
                    + point(
                        bounds.size.width * (x / 20.),
                        bounds.size.height * (y / 20.),
                    )
            };
            let stroke = px((size / 11.).max(1.));
            let color = rgb(ink);
            match mark {
                Mark::Diamond => {
                    let outline = [at(10., 1.), at(18., 8.), at(10., 19.), at(2., 8.)];
                    poly(window, &outline, color, None, true);
                    poly(
                        window,
                        &[at(10., 1.), at(10., 8.), at(2., 8.)],
                        rgba(0xffffff88),
                        None,
                        true,
                    );
                    poly(
                        window,
                        &[at(10., 8.), at(18., 8.), at(10., 19.)],
                        rgba(0x102e4080),
                        None,
                        true,
                    );
                }
                Mark::Flag | Mark::WrongFlag => {
                    poly(
                        window,
                        &[at(5., 2.), at(5., 18.)],
                        color,
                        Some(stroke),
                        false,
                    );
                    poly(
                        window,
                        &[at(5., 2.), at(17., 6.), at(5., 11.)],
                        color,
                        None,
                        true,
                    );
                    if matches!(mark, Mark::WrongFlag) {
                        poly(
                            window,
                            &[at(2., 16.), at(19., 3.)],
                            rgb(0xffc6aa),
                            Some(stroke),
                            false,
                        );
                    }
                }
                Mark::Mine => {
                    poly(
                        window,
                        &[at(5., 5.), at(15., 5.), at(15., 15.), at(5., 15.)],
                        color,
                        None,
                        true,
                    );
                    for arm in [
                        [at(10., 1.), at(10., 19.)],
                        [at(1., 10.), at(19., 10.)],
                        [at(3.5, 3.5), at(16.5, 16.5)],
                        [at(3.5, 16.5), at(16.5, 3.5)],
                    ] {
                        poly(window, &arm, color, Some(stroke), false);
                    }
                }
                Mark::Here | Mark::Ring => {
                    let ring = [
                        at(10., 1.),
                        at(16., 4.),
                        at(19., 10.),
                        at(16., 16.),
                        at(10., 19.),
                        at(4., 16.),
                        at(1., 10.),
                        at(4., 4.),
                    ];
                    poly(window, &ring, color, Some(stroke), true);
                    if matches!(mark, Mark::Here) {
                        poly(
                            window,
                            &[at(10., 6.), at(14., 10.), at(10., 14.), at(6., 10.)],
                            color,
                            None,
                            true,
                        );
                    }
                }
                Mark::Check => poly(
                    window,
                    &[at(3., 10.), at(8., 15.), at(17., 5.)],
                    color,
                    Some(stroke),
                    false,
                ),
                Mark::Lock => {
                    poly(
                        window,
                        &[
                            at(6., 9.),
                            at(6., 4.),
                            at(8., 2.),
                            at(12., 2.),
                            at(14., 4.),
                            at(14., 9.),
                        ],
                        color,
                        Some(stroke),
                        false,
                    );
                    poly(
                        window,
                        &[at(4., 9.), at(16., 9.), at(16., 18.), at(4., 18.)],
                        color,
                        None,
                        true,
                    );
                }
            }
        },
    )
    .size(px(size))
    .flex_none()
    .into_any_element()
}

fn poly(
    window: &mut Window,
    points: &[Point<Pixels>],
    color: Rgba,
    width: Option<Pixels>,
    close: bool,
) {
    let mut path = width.map_or_else(PathBuilder::fill, PathBuilder::stroke);
    path.move_to(points[0]);
    for &point in &points[1..] {
        path.line_to(point);
    }
    if close {
        path.close();
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

pub(super) fn strike(progress: f32, hit: bool, diamond: bool) -> impl IntoElement {
    let color = rgba(if hit {
        0xff79519b
    } else if diamond {
        0x80e7ffb8
    } else {
        0xffdda37a
    });
    let strength = (1. - progress.clamp(0., 1.)).powi(3);
    div()
        .absolute()
        .inset_0()
        .rounded(px(5.))
        .border_2()
        .border_color(color)
        .bg(color.opacity(if diamond { 0.35 } else { 1. }))
        .opacity(strength)
}
