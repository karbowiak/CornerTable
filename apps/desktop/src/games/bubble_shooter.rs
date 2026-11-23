use std::cell::Cell;
use std::rc::Rc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use corner_table_bubble_shooter::{
    Aim, Color, DANGER_Y, FIELD_HEIGHT, FIELD_WIDTH, Point, RADIUS, Round, SHOOTER, Shot, Status,
};
use gpui::{
    Bounds, Context, Div, FontWeight, MouseButton, PathBuilder, Pixels, Render, Stateful, Window,
    canvas, div, fill, hsla, point, prelude::*, px, relative, rgb, size,
};

use crate::{
    games::{GameEntry, GameLaunch},
    theme as t,
};

pub struct GameView {
    round: Round,
    seed: u64,
    fixed_seed: bool,
    flight: Option<Flight>,
    last_shot: Option<Shot>,
}

impl GameView {
    pub fn new(launch: &GameLaunch, _cx: &mut Context<Self>) -> Self {
        let (seed, fixed_seed) = match &launch.entry {
            GameEntry::Challenge(id) => (stable_seed(id), true),
            _ => (fresh_seed(), false),
        };
        Self {
            round: Round::new(seed),
            seed,
            fixed_seed,
            flight: None,
            last_shot: None,
        }
    }

    fn reset(&mut self, new_board: bool, cx: &mut Context<Self>) {
        if new_board {
            self.seed = self.seed.wrapping_add(1);
        }
        self.round = Round::new(self.seed);
        self.flight = None;
        self.last_shot = None;
        cx.notify();
    }

    fn fire(&mut self, target: Point, cx: &mut Context<Self>) {
        if self.flight.is_none()
            && let Some(aim) = self.round.aim(target)
        {
            self.flight = Some(Flight::new(aim, target));
            cx.notify();
        }
    }

    fn board(&self, height: f32, cx: &Context<Self>) -> Stateful<Div> {
        let field = Rc::new(Cell::new(None::<Playfield>));
        let hit_field = Rc::clone(&field);
        let view = cx.entity();
        let playing = self.round.status() == Status::Playing;
        div()
            .w(px(height * (FIELD_WIDTH / FIELD_HEIGHT) as f32))
            .max_w(relative(1.0))
            .h(px(height))
            .flex_shrink_0()
            .when(playing, |board| {
                board
                    .cursor_crosshair()
                    .on_mouse_move(cx.listener(|_, _, _, cx| cx.notify()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &gpui::MouseDownEvent, _, cx| {
                            if let Some(target) = hit_field
                                .get()
                                .and_then(|field| field.target(event.position))
                            {
                                view.fire(target, cx);
                            }
                        }),
                    )
            })
            .id("bubble-shooter-board")
            .on_hover(cx.listener(|_, _: &bool, _, cx| cx.notify()))
            .child(
                canvas(
                    move |bounds, _, _| {
                        let fitted = Playfield::fit(bounds);
                        field.set(Some(fitted));
                        fitted
                    },
                    move |_, field, window, cx| view.read(cx).paint_board(field, window),
                )
                .size_full(),
            )
    }

    fn paint_board(&self, field: Playfield, window: &mut Window) {
        let background = field.rect(0., 0., FIELD_WIDTH, FIELD_HEIGHT);
        window.paint_quad(
            fill(background, rgb(t::RAIL))
                .corner_radii(px(10.))
                .border_color(rgb(0x527963))
                .border_widths(px(1.)),
        );

        for step in 0..40 {
            let x = 0.08 + step as f64 * 0.25;
            window.paint_quad(fill(
                field.rect(x, DANGER_Y, 0.12, 0.038),
                rgb(if self.round.status() == Status::HitDangerLine {
                    0xf59482
                } else {
                    0x9e7564
                }),
            ));
        }

        let launcher = field.screen(SHOOTER);
        let radius = field.scale * RADIUS as f32;
        window.paint_quad(
            fill(disc(launcher, radius * 1.22), rgb(t::RAIL_ACTIVE))
                .corner_radii(radius * 1.22)
                .border_color(rgb(0x64927a))
                .border_widths(px(1.)),
        );

        for (cell, color) in self.round.bubbles() {
            paint_bubble(window, field.screen(cell.center()), radius, color);
        }

        if let Some(flight) = &self.flight {
            if let Some(position) = flight.position() {
                paint_bubble(window, field.screen(position), radius, self.round.current());
            }
        } else if self.round.status() == Status::Playing {
            if let Some(aim) = field
                .target(window.mouse_position())
                .and_then(|target| self.round.aim(target))
            {
                paint_aim(window, field, &aim, self.round.current());
            }
            paint_bubble(window, launcher, radius, self.round.current());
        }
    }

    fn status(&self) -> (&'static str, u32) {
        match self.round.status() {
            Status::Playing => ("Aim with your mouse and click to shoot.", t::INK),
            Status::Won => ("Board cleared!", 0x286248),
            Status::HitDangerLine => ("The bubbles reached the danger line.", 0x963f33),
            Status::OutOfShots => ("You're out of shots.", 0x963f33),
        }
    }

    fn round_info(&self) -> Div {
        let color = self.round.next();
        div()
            .w(px(146.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_3()
            .when(self.round.status() == Status::Playing, |info| {
                info.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, (), window, _| {
                                    paint_bubble(window, bounds.center(), px(14.), color);
                                },
                            )
                            .size(px(28.)),
                        )
                        .child("Next"),
                )
            })
            .child(stat("Shots left", self.round.shots_left()))
            .child(stat("Bubbles left", self.round.bubbles().count()))
            .when_some(self.last_shot, |info, shot| {
                let message = match (shot.popped, shot.dropped) {
                    (0, _) => "No match".to_owned(),
                    (popped, 0) => format!("{popped} popped"),
                    (popped, dropped) => format!("{popped} popped · {dropped} dropped"),
                };
                info.child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .text_color(rgb(t::MUTED))
                        .child(message),
                )
            })
    }
}

impl Render for GameView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.flight.is_some() {
            cx.on_next_frame(window, |view, _, cx| {
                if let Some(flight) = &view.flight {
                    if flight.position().is_none() {
                        view.last_shot = view.round.fire(flight.target);
                        view.flight = None;
                    }
                    cx.notify();
                }
            });
        }

        let viewport = window.viewport_size();
        let shell_width = if viewport.width < px(1080.) {
            218.
        } else {
            288.
        };
        let board_width = f32::from(viewport.width) - shell_width - 196.;
        let height = (f32::from(viewport.height) - 286.)
            .min(board_width * (FIELD_HEIGHT / FIELD_WIDTH) as f32)
            .clamp(292., 612.);
        let (status, status_color) = self.status();
        let playing = self.round.status() == Status::Playing;
        t::surface()
            .w_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(status_color))
                                    .child(status),
                            )
                            .child(div().text_size(px(13.)).text_color(rgb(t::MUTED)).child(
                                if playing {
                                    "Match 3 or more. Use either wall to bank a shot."
                                } else if self.fixed_seed {
                                    "Restart to replay this board."
                                } else {
                                    "Restart to replay or try a new board."
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                control("bubble-shooter-restart", "Restart", self.fixed_seed)
                                    .on_click(cx.listener(|view, _, _, cx| view.reset(false, cx))),
                            )
                            .when(!self.fixed_seed, |controls| {
                                controls.child(
                                    control("bubble-shooter-new", "New board", true).on_click(
                                        cx.listener(|view, _, _, cx| view.reset(true, cx)),
                                    ),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .gap_4()
                    .child(self.board(height, cx))
                    .child(self.round_info()),
            )
    }
}

fn control(id: &'static str, label: &'static str, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .tab_index(0)
        .cursor_pointer()
        .flex()
        .items_center()
        .h(px(36.))
        .px_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(if primary { t::GOLD } else { t::BORDER }))
        .bg(rgb(if primary { t::GOLD } else { t::SURFACE }))
        .hover(|style| style.bg(rgb(if primary { t::GOLD_HOVER } else { t::SAGE })))
        .focus(|style| style.border_color(rgb(t::MUTED)))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}

fn stat(label: &'static str, count: usize) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .border_t_1()
        .border_color(rgb(t::BORDER))
        .pt_2()
        .text_size(px(13.))
        .text_color(rgb(t::MUTED))
        .child(label)
        .child(t::display_type(count.to_string(), 21.).text_color(rgb(t::INK)))
}

#[derive(Clone, Copy)]
struct Playfield {
    bounds: Bounds<Pixels>,
    scale: Pixels,
}

impl Playfield {
    fn fit(bounds: Bounds<Pixels>) -> Self {
        let scale =
            (bounds.size.width / FIELD_WIDTH as f32).min(bounds.size.height / FIELD_HEIGHT as f32);
        let field_size = size(scale * FIELD_WIDTH as f32, scale * FIELD_HEIGHT as f32);
        let origin = point(
            bounds.origin.x + (bounds.size.width - field_size.width) / 2.,
            bounds.origin.y + (bounds.size.height - field_size.height) / 2.,
        );
        Self {
            bounds: Bounds::new(origin, field_size),
            scale,
        }
    }

    fn screen(self, at: Point) -> gpui::Point<Pixels> {
        self.bounds.origin + point(self.scale * at.x as f32, self.scale * at.y as f32)
    }

    fn target(self, at: gpui::Point<Pixels>) -> Option<Point> {
        if !self.bounds.contains(&at) || self.scale <= px(0.) {
            return None;
        }
        let offset = at - self.bounds.origin;
        Some(Point {
            x: f64::from(offset.x / self.scale),
            y: f64::from(offset.y / self.scale),
        })
    }

    fn rect(self, x: f64, y: f64, width: f64, height: f64) -> Bounds<Pixels> {
        Bounds::new(
            self.screen(Point { x, y }),
            size(self.scale * width as f32, self.scale * height as f32),
        )
    }
}

struct Flight {
    path: Vec<Point>,
    target: Point,
    length: f64,
    duration: f64,
    started: Instant,
}

impl Flight {
    fn new(aim: Aim, target: Point) -> Self {
        let length: f64 = aim.path.windows(2).map(|p| distance(p[0], p[1])).sum();
        Self {
            path: aim.path,
            target,
            length,
            duration: (length / 42.0).clamp(0.10, 0.45),
            started: Instant::now(),
        }
    }

    fn position(&self) -> Option<Point> {
        let mut remaining = self.length * self.started.elapsed().as_secs_f64() / self.duration;
        for segment in self.path.windows(2) {
            let length = distance(segment[0], segment[1]);
            if remaining < length {
                return Some(interpolate(segment[0], segment[1], remaining / length));
            }
            remaining -= length;
        }
        None
    }
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

fn paint_aim(window: &mut Window, field: Playfield, aim: &Aim, color: Color) {
    let dot = (field.scale * 0.044).max(px(1.2));
    let mut offset = 0.76;
    for segment in aim.path.windows(2) {
        let length = distance(segment[0], segment[1]);
        while offset < length {
            let center = field.screen(interpolate(segment[0], segment[1], offset / length));
            window.paint_quad(fill(disc(center, dot), rgb(0xd5e2cb)).corner_radii(dot));
            offset += 0.30;
        }
        offset -= length;
    }

    let radius = field.scale * 0.42;
    window.paint_quad(
        fill(
            disc(field.screen(aim.landing.center()), radius),
            hsla(0., 0., 0., 0.),
        )
        .corner_radii(radius)
        .border_color(rgb(palette(color).0))
        .border_widths(px(2.)),
    );
}

fn disc(center: gpui::Point<Pixels>, radius: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        center - point(radius, radius),
        size(radius * 2., radius * 2.),
    )
}

fn palette(color: Color) -> (u32, u32, u32) {
    match color {
        Color::Coral => (0xf39180, 0xffbeac, 0x72372c),
        Color::Gold => (0xf1c45f, 0xffe099, 0x5e481b),
        Color::Jade => (0x69c6a4, 0xa4e5cb, 0x1c5242),
        Color::Violet => (0xaf9ce5, 0xd7cbfa, 0x4d3c78),
    }
}

fn paint_bubble(window: &mut Window, center: gpui::Point<Pixels>, radius: Pixels, color: Color) {
    let (body, rim, mark) = palette(color);
    let radius = radius * 0.93;
    window.paint_quad(
        fill(disc(center, radius), rgb(body))
            .corner_radii(radius)
            .border_color(rgb(rim))
            .border_widths(px(1.)),
    );

    // Shape cues keep adjoining colors distinguishable independently of hue.
    let inset = radius * 0.28;
    let at = |x: f32, y: f32| center + point(inset * x, inset * y);
    let vertices: &[(f32, f32)] = match color {
        Color::Coral => {
            window.paint_quad(fill(disc(center, inset * 0.88), rgb(mark)).corner_radii(inset));
            return;
        }
        Color::Gold => &[(0., -1.15), (1.05, 0.86), (-1.05, 0.86)],
        Color::Jade => &[(-0.8, -0.8), (0.8, -0.8), (0.8, 0.8), (-0.8, 0.8)],
        Color::Violet => &[(0., -1.15), (1., 0.), (0., 1.15), (-1., 0.)],
    };
    let mut shape = PathBuilder::fill();
    shape.move_to(at(vertices[0].0, vertices[0].1));
    for &(x, y) in &vertices[1..] {
        shape.line_to(at(x, y));
    }
    shape.close();
    if let Ok(path) = shape.build() {
        window.paint_path(path, rgb(mark));
    }
}

fn stable_seed(id: &str) -> u64 {
    id.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64)
}
