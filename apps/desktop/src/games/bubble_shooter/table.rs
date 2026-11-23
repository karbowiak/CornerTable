use std::{f32::consts::TAU, time::Instant};

use corner_table_bubble_shooter::{
    Aim, Angle, Cell, Color, DANGER_Y, FIELD_HEIGHT, FIELD_WIDTH, Point, RADIUS, ROW_SPACING, ROWS,
    Round, SHOOTER, Shot, Status,
};
use gpui::{
    Background, Bounds, BoxShadow, ContentMask, Hsla, LineCap, PathBuilder, PathStyle, Pixels,
    StrokeOptions, Window, fill, linear_color_stop, linear_gradient, point, px, rgb, size,
};

const SIDE_RAIL: f64 = 0.36;
const TOP_RAIL: f64 = 0.31;
pub(super) const TABLE_WIDTH: f64 = FIELD_WIDTH + 2. * SIDE_RAIL;
pub(super) const TABLE_HEIGHT: f64 = FIELD_HEIGHT + TOP_RAIL + 0.25;
const IVORY: u32 = 0xfff4d2;
const BRASS: u32 = 0xd6b37b;
const WELL_SHADOW: u32 = 0x0c211f;
const POP_SECONDS: f64 = 0.26;
const DROP_SECONDS: f64 = 0.50;

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
            paint_line(window, launcher, muzzle, radius * 0.16, tint(BRASS, 0.85));
            circle(window, muzzle, radius * 0.12, tint(IVORY, 0.98));
        }
        let socket_radius = radius * 1.23;
        let socket = disc(launcher, socket_radius);
        cast_shadow(window, socket, socket_radius, radius * 0.31, 0.9);
        window.paint_quad(
            fill(
                socket,
                glaze(150., rgb(0xb18c57).into(), rgb(0x493a26).into()),
            )
            .corner_radii(socket_radius)
            .border_color(tint(BRASS, 0.82))
            .border_widths(px(1.)),
        );
        let seat = radius * 1.02;
        window.paint_quad(
            fill(
                disc(launcher, seat),
                glaze(156., rgb(0x132421).into(), rgb(0x31504a).into()),
            )
            .corner_radii(seat)
            .border_color(tint(WELL_SHADOW, 0.85))
            .border_widths(px(1.)),
        );
        if let Some(flight) = flight {
            self.paint_bank_contacts(flight, radius, window);
            for (behind, scale, opacity) in [(0.035, 0.26, 0.16), (0.018, 0.52, 0.26)] {
                if let Some(trail) = flight.position(behind) {
                    circle(
                        window,
                        self.screen(trail),
                        radius * scale,
                        tint(Glass::of(round.current()).light, opacity),
                    );
                }
            }
            if let Some(at) = flight.position(0.) {
                paint_bubble(window, self.screen(at), radius, round.current(), 1.);
            }
        } else if ready {
            paint_bubble(window, launcher, radius, round.current(), 1.);
        }
    }

    fn paint_bank_contacts(self, flight: &Flight, radius: Pixels, window: &mut Window) {
        let elapsed = flight.started.elapsed().as_secs_f64();
        let bounces = flight.aim.bounces();
        if bounces == 0 {
            return;
        }
        let mut traveled = 0.;
        window.with_content_mask(
            Some(ContentMask {
                bounds: self.bounds,
            }),
            |window| {
                for segment in flight.aim.path.windows(2).take(bounces) {
                    traveled += distance(segment[0], segment[1]);
                    let contact = traveled * flight.duration / flight.length;
                    let progress = ((elapsed - contact) / 0.14) as f32;
                    if (0. ..1.).contains(&progress) {
                        ring(
                            window,
                            self.screen(segment[1]),
                            radius * (0.58 + 0.86 * progress),
                            tint(0xffdc87, (1. - progress).powi(2) * 0.9),
                            px(1.8),
                        );
                    }
                }
            },
        );
    }

    fn paint_frame(self, window: &mut Window, crowding_line: bool, lost: bool) {
        window.paint_quad(
            fill(
                self.frame,
                glaze(135., rgb(0x516055).into(), rgb(0x222f29).into()),
            )
            .corner_radii(px(12.))
            .border_color(tint(BRASS, 0.54))
            .border_widths(px(1.)),
        );
        let well = self.rect(-0.11, -0.13, FIELD_WIDTH + 0.22, FIELD_HEIGHT + 0.23);
        let well_corners = px(7.);
        window.paint_quad(
            fill(
                well,
                glaze(162., rgb(0x294d45).into(), rgb(0x142d2b).into()),
            )
            .corner_radii(well_corners)
            .border_color(tint(BRASS, 0.34))
            .border_widths(px(1.)),
        );
        window.paint_inset_shadows(
            well,
            well_corners.into(),
            &[BoxShadow::new(px(0.), px(2.), tint(WELL_SHADOW, 0.8))
                .blur_radius(px(11.))
                .spread_radius(px(2.))
                .inset()],
        );
        // Scribe the row marks into the rails at the same height as the cell centers.
        for row in 0..ROWS {
            let y = f64::from(row as u32) * ROW_SPACING + RADIUS;
            let width = if row % 3 == 0 { 0.12 } else { 0.07 };
            for x in [-0.16 - width, FIELD_WIDTH + 0.16] {
                window.paint_quad(fill(self.rect(x, y, width, 0.026), tint(BRASS, 0.51)));
            }
        }
        let top = self.rect(0.12, -0.218, FIELD_WIDTH - 0.24, 0.024);
        window.paint_quad(fill(top, tint(BRASS, 0.57)).corner_radii(px(1.)));
        for x in [-0.19, FIELD_WIDTH + 0.19] {
            for y in [-0.18, FIELD_HEIGHT + 0.10] {
                let pin = self.screen(Point { x, y });
                circle(window, pin, px(2.1), tint(WELL_SHADOW, 0.9));
                circle(
                    window,
                    pin - point(px(0.2), px(0.3)),
                    px(1.2),
                    tint(BRASS, 0.88),
                );
            }
        }

        let warning = if lost {
            tint(0xffae95, 1.)
        } else if crowding_line {
            tint(0xffb38d, 0.89)
        } else {
            tint(0xe3b18c, 0.55)
        };
        for step in 0..40 {
            let x = 0.062 + f64::from(step) * 0.25;
            let dash = self.rect(x, DANGER_Y - 0.02, 0.126, 0.04);
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
        let mut offset = 0.95;
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
            let radius = (self.scale * 0.112).max(px(3.2));
            circle(window, center, radius, rgb(WELL_SHADOW).into());
            ring(window, center, radius, tint(IVORY, 0.94), px(1.));
            circle(window, center, px(1.), tint(IVORY, 0.94));
        }

        let surface = Glass::of(color);
        let center = self.screen(aim.landing.center());
        let radius = self.scale * 0.43;
        let match_ready = aim.matching >= 3;
        circle(window, center, radius, tint(WELL_SHADOW, 0.88));
        circle(
            window,
            center,
            radius,
            tint(surface.body, if match_ready { 0.28 } else { 0.12 }),
        );
        ring(
            window,
            center,
            radius,
            tint(surface.rim, 0.96),
            if match_ready { px(2.) } else { px(1.25) },
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
        let radius = self.scale * RADIUS as f32;
        let landing = impact.landing.center();
        window.with_content_mask(
            Some(ContentMask {
                bounds: self.bounds,
            }),
            |window| {
                if seconds < POP_SECONDS {
                    if shot.popped.is_empty() {
                        let progress = (seconds / POP_SECONDS) as f32;
                        ring(
                            window,
                            self.screen(landing),
                            radius * (0.98 + progress * 0.32),
                            tint(IVORY, (1. - progress).powi(2) * 0.85),
                            px(1.4),
                        );
                    }
                    for &(cell, color) in &shot.popped {
                        let center = self.screen(cell.center());
                        let delay = (distance(landing, cell.center()) * 0.012).min(0.045);
                        if seconds < delay {
                            paint_bubble(window, center, radius, color, 1.);
                        } else {
                            let progress = ((seconds - delay) / (POP_SECONDS - delay)) as f32;
                            paint_pop(window, center, radius, cell, color, progress);
                        }
                    }
                }

                let fade = (1. - (seconds / DROP_SECONDS).powi(2)).clamp(0., 1.) as f32;
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
        let duration = if shot.dropped.is_empty() {
            POP_SECONDS
        } else {
            DROP_SECONDS
        };
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
            Color::Coral => (0xffc6b5, 0xeb8073, 0x9f3b43, 0xffa98e, 0x511e25),
            Color::Gold => (0xffe4a0, 0xeaba54, 0x946025, 0xffd980, 0x4c310e),
            Color::Jade => (0xb7f1d8, 0x62c6a3, 0x276c60, 0x99e7c8, 0x153c32),
            Color::Violet => (0xe9d5ff, 0xb799e2, 0x66488f, 0xd4b8f7, 0x392457),
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
    let glass = Glass::of(color);
    let r = radius * 0.944;
    let alpha = |hex, a: f32| tint(hex, a * opacity);
    let bounds = disc(center, r);
    cast_shadow(window, bounds, r, r * 0.13, opacity);
    window.paint_quad(
        fill(
            bounds,
            glaze(145., alpha(glass.body, 1.), alpha(glass.deep, 1.)),
        )
        .corner_radii(r)
        .border_color(alpha(glass.rim, 0.68))
        .border_widths(px(1.)),
    );
    let lens = r * 0.91;
    window.paint_quad(
        fill(
            disc(center - point(r * 0.025, r * 0.046), lens),
            glaze(158., alpha(glass.light, 0.92), alpha(glass.body, 0.22)),
        )
        .corner_radii(lens),
    );
    window.paint_inset_shadows(
        bounds,
        r.into(),
        &[
            BoxShadow::new(-r * 0.075, -r * 0.12, alpha(glass.deep, 0.56))
                .blur_radius(r * 0.31)
                .inset(),
        ],
    );

    // The reflection follows the upper left of the sphere. The dimmer lower edge is
    // light transmitted through the glass, leaving the center clear for the symbol.
    paint_arc(
        window,
        center,
        r * 0.68,
        3.55,
        4.70,
        (r * 0.12).max(px(1.)),
        alpha(0xfff7e4, 0.86),
    );
    paint_arc(
        window,
        center,
        r * 0.80,
        0.50,
        1.38,
        (r * 0.085).max(px(0.7)),
        alpha(glass.light, 0.51),
    );
    circle(
        window,
        center + point(r * 0.16, -r * 0.67),
        (r * 0.057).max(px(0.65)),
        alpha(0xfff7e4, 0.79),
    );
    paint_mark(
        window,
        center + point(px(0.), r * 0.04),
        r * 0.36,
        color,
        alpha(glass.mark, 0.98),
    );
}

fn paint_pop(
    window: &mut Window,
    center: gpui::Point<Pixels>,
    radius: Pixels,
    cell: Cell,
    color: Color,
    progress: f32,
) {
    let glass = Glass::of(color);
    let fade = (1. - progress).powi(2);
    let expansion = 1. - (1. - progress).powi(3);
    let reach = radius * (0.92 + 0.46 * expansion);
    let tilt = (cell.column() + cell.row() * 2) as f32 * 0.29;

    circle(
        window,
        center,
        radius * (0.78 - 0.35 * progress),
        tint(glass.body, 0.38 * fade),
    );
    for (from, length, strength) in [(0.12, 1.52, 0.84), (2.35, 0.75, 0.58), (3.85, 1.12, 0.93)] {
        let from = tilt + from + 0.26 * progress;
        let to = from + length * (1. - 0.57 * expansion);
        paint_arc(
            window,
            center,
            reach,
            from,
            to,
            (radius * 0.10).max(px(1.1)),
            tint(glass.rim, strength * fade),
        );
    }

    let mut shards = PathBuilder::fill();
    for (index, offset) in [0.1, 0.9, 1.9, 3.15, 4.2, 5.4].into_iter().enumerate() {
        let angle = tilt + offset;
        let (sine, cosine) = angle.sin_cos();
        let travel = radius * (0.34 + (0.65 + 0.12 * (index % 3) as f32) * expansion);
        let at = center
            + point(
                travel * cosine,
                travel * sine + radius * progress.powi(2) * 0.30,
            );
        let length = radius * (0.06 + 0.11 * fade);
        let radial = point(length * cosine, length * sine);
        let edge = point(-length * sine, length * cosine) * 0.52;
        shards.move_to(at + radial);
        shards.line_to(at + edge);
        shards.line_to(at - radial * 0.7);
        shards.line_to(at - edge);
        shards.close();
    }
    if let Ok(path) = shards.build() {
        window.paint_path(path, tint(glass.light, 0.94 * (1. - progress)));
    }
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
        let angle = tip as f32 * TAU / 10. - std::f32::consts::FRAC_PI_2;
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

fn cast_shadow(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    r: Pixels,
    offset: Pixels,
    opacity: f32,
) {
    window.paint_drop_shadows(
        bounds,
        r.into(),
        &[
            BoxShadow::new(r * 0.055, offset, tint(0x081913, 0.72 * opacity))
                .blur_radius((r * 0.19).max(px(1.))),
        ],
    );
}

fn paint_arc(
    window: &mut Window,
    center: gpui::Point<Pixels>,
    radius: Pixels,
    from: f32,
    to: f32,
    width: Pixels,
    color: Hsla,
) {
    let at = |angle: f32| center + point(radius * angle.cos(), radius * angle.sin());
    let mut arc = PathBuilder::default().with_style(PathStyle::Stroke(StrokeOptions {
        line_width: f32::from(width),
        line_cap: LineCap::Round,
        ..Default::default()
    }));
    arc.move_to(at(from));
    arc.arc_to(point(radius, radius), px(0.), false, true, at(to));
    if let Ok(path) = arc.build() {
        window.paint_path(path, color);
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
