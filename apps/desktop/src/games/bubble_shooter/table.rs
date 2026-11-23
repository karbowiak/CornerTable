use std::time::Instant;

use corner_table_bubble_shooter::{
    Aim, Angle, Cell, Color, DANGER_Y, FIELD_HEIGHT, FIELD_WIDTH, Point, RADIUS, ROW_SPACING, ROWS,
    Round, SHOOTER, Shot, Status,
};
use gpui::{
    Background, Bounds, ContentMask, Hsla, PathBuilder, Pixels, Window, fill, linear_color_stop,
    linear_gradient, point, px, rgb, size,
};

const SIDE_RAIL: f64 = 0.36;
const TOP_RAIL: f64 = 0.31;
pub(super) const TABLE_WIDTH: f64 = FIELD_WIDTH + 2. * SIDE_RAIL;
pub(super) const TABLE_HEIGHT: f64 = FIELD_HEIGHT + TOP_RAIL + 0.25;
const IVORY: u32 = 0xfff4d2;

#[derive(Clone, Copy)]
pub(super) struct Playfield {
    frame: Bounds<Pixels>,
    bounds: Bounds<Pixels>,
    scale: Pixels,
}

impl Playfield {
    pub fn fit(bounds: Bounds<Pixels>) -> Self {
        let scale =
            (bounds.size.width / TABLE_WIDTH as f32).min(bounds.size.height / TABLE_HEIGHT as f32);
        let frame_size = size(scale * TABLE_WIDTH as f32, scale * TABLE_HEIGHT as f32);
        let origin = bounds.center() - point(frame_size.width / 2., frame_size.height / 2.);
        let well_origin = origin + point(scale * SIDE_RAIL as f32, scale * TOP_RAIL as f32);
        Self {
            frame: Bounds::new(origin, frame_size),
            bounds: Bounds::new(
                well_origin,
                size(scale * FIELD_WIDTH as f32, scale * FIELD_HEIGHT as f32),
            ),
            scale,
        }
    }

    pub(super) fn screen(self, at: Point) -> gpui::Point<Pixels> {
        self.bounds.origin + point(self.scale * at.x as f32, self.scale * at.y as f32)
    }

    pub fn angle_at(self, at: gpui::Point<Pixels>) -> Option<Angle> {
        if !self.frame.contains(&at) || self.scale <= px(0.) {
            return None;
        }
        let offset = at - self.bounds.origin;
        Angle::toward(Point {
            x: f64::from(offset.x / self.scale).clamp(0., FIELD_WIDTH),
            y: f64::from(offset.y / self.scale).max(0.),
        })
    }

    fn rect(self, x: f64, y: f64, width: f64, height: f64) -> Bounds<Pixels> {
        Bounds::new(
            self.screen(Point { x, y }),
            size(self.scale * width as f32, self.scale * height as f32),
        )
    }

    pub fn paint(
        self,
        round: &Round,
        angle: Angle,
        aim: Option<&Aim>,
        flight: Option<&Flight>,
        impact: Option<(&Impact, &Shot)>,
        window: &mut Window,
    ) {
        let crowding_line = round.bubbles().any(|(cell, _)| cell.row() >= ROWS - 3);
        self.paint_frame(
            window,
            crowding_line,
            round.status() == Status::HitDangerLine,
        );

        if let Some(aim) = aim {
            self.paint_aim(window, aim, round.current());
        }

        let radius = self.scale * RADIUS as f32;
        for (cell, color) in round.bubbles() {
            paint_bubble(window, self.screen(cell.center()), radius, color, 1.);
        }
        if let Some((impact, shot)) = impact {
            self.paint_impact(window, impact, shot);
        }

        let launcher = self.screen(SHOOTER);
        let direction = angle.direction();
        let muzzle = self.screen(Point {
            x: SHOOTER.x + direction.x * 0.73,
            y: SHOOTER.y + direction.y * 0.73,
        });
        let ready = flight.is_none() && round.status() == Status::Playing;
        if ready {
            paint_line(window, launcher, muzzle, radius * 0.18, tint(IVORY, 0.62));
            circle(window, muzzle, radius * 0.15, tint(IVORY, 0.9));
        }
        window.paint_quad(
            fill(
                disc(launcher, radius * 1.25),
                glaze(140., tint(0x61827b, 1.), tint(0x193b3b, 1.)),
            )
            .corner_radii(radius * 1.25)
            .border_color(tint(0xaac3a7, 0.78))
            .border_widths(px(1.)),
        );
        ring(window, launcher, radius * 1.10, tint(0x0c2626, 0.9), px(1.));
        if let Some(flight) = flight {
            if let Some(trail) = flight.position(0.042) {
                circle(window, self.screen(trail), radius * 0.54, tint(IVORY, 0.15));
            }
            if let Some(at) = flight.position(0.) {
                paint_bubble(window, self.screen(at), radius, round.current(), 1.);
            }
        } else if ready {
            paint_bubble(window, launcher, radius, round.current(), 1.);
        }
    }

    fn paint_frame(self, window: &mut Window, crowding_line: bool, lost: bool) {
        window.paint_quad(
            fill(
                self.frame,
                glaze(130., tint(0x49665d, 1.), tint(0x1b3935, 1.)),
            )
            .corner_radii(px(12.))
            .border_color(tint(0x80917b, 0.7))
            .border_widths(px(1.)),
        );
        window.paint_quad(
            fill(
                self.rect(-0.09, -0.12, FIELD_WIDTH + 0.18, FIELD_HEIGHT + 0.22),
                glaze(165., tint(0x2b5551, 1.), tint(0x102d2e, 1.)),
            )
            .corner_radii(px(6.))
            .border_color(tint(0x071f1d, 0.85))
            .border_widths(px(1.)),
        );
        for side in [-0.15, FIELD_WIDTH + 0.13] {
            let seam = self.rect(side, 0., 0.022, FIELD_HEIGHT);
            window.paint_quad(fill(seam, tint(0xb3c1a2, 0.34)));
        }
        for row in 0..ROWS {
            let y = f64::from(row as u32) * ROW_SPACING + RADIUS;
            for x in [-0.25, FIELD_WIDTH + 0.17] {
                window.paint_quad(fill(self.rect(x, y, 0.08, 0.024), tint(0xd6d6ac, 0.44)));
            }
        }
        let top = self.rect(-0.08, -0.086, FIELD_WIDTH + 0.16, 0.023);
        window.paint_quad(fill(top, tint(0xe1c894, 0.55)));

        let warning = if lost {
            tint(0xffae95, 1.)
        } else if crowding_line {
            tint(0xffb38d, 0.89)
        } else {
            tint(0xe3b18c, 0.55)
        };
        for step in 0..40 {
            let x = 0.062 + f64::from(step) * 0.25;
            let dash = self.rect(x, DANGER_Y, 0.126, 0.032);
            window.paint_quad(fill(dash, warning).corner_radii(px(1.)));
        }
        for x in [-0.19, FIELD_WIDTH + 0.19] {
            circle(
                window,
                self.screen(Point { x, y: DANGER_Y }),
                self.scale * 0.063,
                warning,
            );
        }
    }

    fn paint_aim(self, window: &mut Window, aim: &Aim, color: Color) {
        let dot = (self.scale * 0.037).max(px(1.35));
        let mut offset = 0.85;
        for segment in aim.path.windows(2) {
            let length = distance(segment[0], segment[1]);
            while offset < length {
                let center = self.screen(interpolate(segment[0], segment[1], offset / length));
                circle(window, center, dot * 1.56, tint(0x0a2220, 0.53));
                circle(window, center, dot, tint(IVORY, 0.91));
                offset += 0.25;
            }
            offset -= length;
        }
        for &bounce in &aim.path[1..aim.path.len() - 1] {
            let center = self.screen(bounce);
            let radius = (self.scale * 0.092).max(px(2.6));
            circle(window, center, radius, tint(0x143331, 1.));
            ring(window, center, radius, tint(IVORY, 0.92), px(1.));
        }

        let surface = Glass::of(color);
        let center = self.screen(aim.landing.center());
        let radius = self.scale * 0.43;
        let match_ready = aim.matching >= 3;
        circle(
            window,
            center,
            radius,
            tint(surface.body, if match_ready { 0.36 } else { 0.13 }),
        );
        ring(
            window,
            center,
            radius,
            tint(surface.rim, 0.96),
            if match_ready { px(2.25) } else { px(1.5) },
        );
        if match_ready {
            ring(
                window,
                center,
                self.scale * 0.488,
                tint(IVORY, 0.64),
                px(1.),
            );
        }
        paint_mark(
            window,
            center,
            radius * 0.32,
            color,
            tint(surface.light, 0.91),
        );
    }

    fn paint_impact(self, window: &mut Window, impact: &Impact, shot: &Shot) {
        let seconds = impact.started.elapsed().as_secs_f64();
        let progress = (seconds / 0.26).min(1.) as f32;
        let radius = self.scale * RADIUS as f32;
        if progress < 1. {
            if shot.popped.is_empty() {
                ring(
                    window,
                    self.screen(impact.landing.center()),
                    radius * (0.95 + progress * 0.26),
                    tint(IVORY, (1. - progress) * 0.75),
                    px(1.5),
                );
            }
            for &(cell, color) in &shot.popped {
                let center = self.screen(cell.center());
                let surface = Glass::of(color);
                ring(
                    window,
                    center,
                    radius * (0.62 + progress * 0.47),
                    tint(surface.rim, 1. - progress),
                    px(1.8),
                );
                for ray in 0..4 {
                    let angle = (ray as f32 + 0.25) * std::f32::consts::FRAC_PI_2;
                    let reach = radius * (0.22 + progress * 0.53);
                    let spark = center + point(reach * angle.cos(), reach * angle.sin());
                    circle(
                        window,
                        spark,
                        radius * (1. - progress) * 0.105,
                        tint(surface.light, 1. - progress),
                    );
                }
            }
        }
        if !shot.dropped.is_empty() {
            window.with_content_mask(
                Some(ContentMask {
                    bounds: self.bounds,
                }),
                |window| {
                    let fade = (1. - (seconds / 0.50).powi(2)).clamp(0., 1.) as f32;
                    for &(cell, color) in &shot.dropped {
                        let at = cell.center();
                        let drift = (at.x - FIELD_WIDTH / 2.) * seconds * 0.28;
                        let fall = 24. * seconds * seconds + seconds * 1.2;
                        paint_bubble(
                            window,
                            self.screen(Point {
                                x: at.x + drift,
                                y: at.y + fall,
                            }),
                            radius * (0.72 + fade * 0.28),
                            color,
                            fade,
                        );
                    }
                },
            );
        }
    }
}

pub(super) struct Flight {
    pub aim: Aim,
    pub angle: Angle,
    length: f64,
    duration: f64,
    started: Instant,
}

impl Flight {
    pub fn new(aim: Aim, angle: Angle) -> Self {
        let length: f64 = aim.path.windows(2).map(|p| distance(p[0], p[1])).sum();
        Self {
            aim,
            angle,
            length,
            duration: (length / 56.).clamp(0.085, 0.46),
            started: Instant::now(),
        }
    }

    pub fn position(&self, behind: f64) -> Option<Point> {
        let elapsed = self.started.elapsed().as_secs_f64() - behind;
        if elapsed < 0. || elapsed >= self.duration {
            return None;
        }
        let mut remaining = self.length * elapsed / self.duration;
        for segment in self.aim.path.windows(2) {
            let span = distance(segment[0], segment[1]);
            if remaining < span {
                return Some(interpolate(segment[0], segment[1], remaining / span));
            }
            remaining -= span;
        }
        None
    }
}

pub(super) struct Impact {
    landing: Cell,
    started: Instant,
    duration: f64,
}

impl Impact {
    pub fn new(landing: Cell, shot: &Shot) -> Self {
        let duration = if shot.dropped.is_empty() { 0.26 } else { 0.50 };
        Self {
            landing,
            started: Instant::now(),
            duration,
        }
    }

    pub fn finished(&self) -> bool {
        self.started.elapsed().as_secs_f64() >= self.duration
    }
}

pub(super) fn color_name(color: Color) -> &'static str {
    match color {
        Color::Coral => "Coral",
        Color::Gold => "Gold",
        Color::Jade => "Jade",
        Color::Violet => "Violet",
    }
}

struct Glass {
    light: u32,
    body: u32,
    deep: u32,
    rim: u32,
    mark: u32,
}

impl Glass {
    fn of(color: Color) -> Self {
        let (light, body, deep, rim, mark) = match color {
            Color::Coral => (0xffc1ae, 0xea8479, 0xba5150, 0xffb4a3, 0x642323),
            Color::Gold => (0xffe29c, 0xedbb5c, 0xa97830, 0xffda81, 0x553812),
            Color::Jade => (0xb4e6ce, 0x70c5a6, 0x3b8274, 0xa5e8cc, 0x144734),
            Color::Violet => (0xe5d3fb, 0xbb9ddf, 0x7656a7, 0xdac2f2, 0x482963),
        };
        Self {
            light,
            body,
            deep,
            rim,
            mark,
        }
    }
}

pub(super) fn paint_bubble(
    window: &mut Window,
    center: gpui::Point<Pixels>,
    radius: Pixels,
    color: Color,
    opacity: f32,
) {
    let g = Glass::of(color);
    let r = radius * 0.951;
    let alpha = |hex, a: f32| tint(hex, a * opacity);
    circle(
        window,
        center + point(r * 0.047, r * 0.08),
        r * 0.99,
        alpha(0x071816, 0.43),
    );
    window.paint_quad(
        fill(
            disc(center, r),
            glaze(142., alpha(g.light, 0.97), alpha(g.deep, 0.86)),
        )
        .corner_radii(r)
        .border_color(alpha(g.rim, 0.96))
        .border_widths(px(1.)),
    );
    let lens = r * 0.85;
    window.paint_quad(
        fill(
            disc(center - point(r * 0.055, r * 0.075), lens),
            glaze(151., alpha(g.light, 0.31), alpha(g.body, 0.62)),
        )
        .corner_radii(lens),
    );
    let glint = Bounds::new(
        center - point(r * 0.55, r * 0.66),
        size(r * 0.39, r * 0.173),
    );
    window.paint_quad(fill(glint, alpha(0xfffbeb, 0.82)).corner_radii(r * 0.10));
    circle(
        window,
        center + point(r * 0.47, r * 0.49),
        r * 0.09,
        alpha(g.rim, 0.60),
    );
    paint_mark(
        window,
        center + point(px(0.), r * 0.04),
        r * 0.33,
        color,
        alpha(g.mark, 0.96),
    );
}

fn paint_mark(
    window: &mut Window,
    center: gpui::Point<Pixels>,
    r: Pixels,
    color: Color,
    fill: Hsla,
) {
    let vertices: &[(f32, f32)] = match color {
        Color::Coral => {
            circle(window, center, r * 0.83, fill);
            return;
        }
        Color::Gold => &[(0., -1.1), (1.03, 0.80), (-1.03, 0.80)],
        Color::Jade => &[(-0.76, -0.76), (0.76, -0.76), (0.76, 0.76), (-0.76, 0.76)],
        Color::Violet => &[(0., -1.12), (0.95, 0.), (0., 1.12), (-0.95, 0.)],
    };
    let mut shape = PathBuilder::fill();
    shape.move_to(center + point(r * vertices[0].0, r * vertices[0].1));
    for &(x, y) in &vertices[1..] {
        shape.line_to(center + point(r * x, r * y));
    }
    shape.close();
    if let Ok(path) = shape.build() {
        window.paint_path(path, fill);
    }
}

pub(super) fn paint_star(window: &mut Window, center: gpui::Point<Pixels>, r: Pixels, color: u32) {
    let mut shape = PathBuilder::fill();
    for tip in 0..10 {
        let angle = tip as f32 * std::f32::consts::TAU / 10. - std::f32::consts::FRAC_PI_2;
        let reach = if tip % 2 == 0 { r } else { r * 0.47 };
        let at = center + point(reach * angle.cos(), reach * angle.sin());
        if tip == 0 {
            shape.move_to(at);
        } else {
            shape.line_to(at);
        }
    }
    shape.close();
    if let Ok(path) = shape.build() {
        window.paint_path(path, rgb(color));
    }
}

fn paint_line(
    window: &mut Window,
    from: gpui::Point<Pixels>,
    to: gpui::Point<Pixels>,
    width: Pixels,
    color: Hsla,
) {
    let mut line = PathBuilder::stroke(width);
    line.move_to(from);
    line.line_to(to);
    if let Ok(path) = line.build() {
        window.paint_path(path, color);
    }
}

fn circle(window: &mut Window, center: gpui::Point<Pixels>, radius: Pixels, color: Hsla) {
    window.paint_quad(fill(disc(center, radius), color).corner_radii(radius));
}

fn ring(
    window: &mut Window,
    center: gpui::Point<Pixels>,
    radius: Pixels,
    color: Hsla,
    width: Pixels,
) {
    window.paint_quad(
        fill(disc(center, radius), tint(0, 0.))
            .corner_radii(radius)
            .border_color(color)
            .border_widths(width),
    );
}

fn disc(center: gpui::Point<Pixels>, radius: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        center - point(radius, radius),
        size(radius * 2., radius * 2.),
    )
}

fn tint(hex: u32, opacity: f32) -> Hsla {
    Hsla::from(rgb(hex)).opacity(opacity)
}

fn glaze(angle: f32, first: Hsla, last: Hsla) -> Background {
    linear_gradient(
        angle,
        linear_color_stop(first, 0.),
        linear_color_stop(last, 1.),
    )
}

fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

fn interpolate(a: Point, b: Point, ratio: f64) -> Point {
    Point {
        x: a.x + (b.x - a.x) * ratio,
        y: a.y + (b.y - a.y) * ratio,
    }
}
