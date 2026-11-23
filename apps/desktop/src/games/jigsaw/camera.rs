use std::{cell::Cell, rc::Rc};

use gpui::{
    Bounds, Context, Div, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, PinchEvent,
    Pixels, Point, Role, ScrollEvent, Stateful, TouchPhase, Window, div, point, prelude::*, px,
};
use serde::{Deserialize, Serialize};

use super::{GameView, action_button, segment, segments, small_text};

const ZOOM_STEP: f32 = 0.25;
const MAX_ZOOM: f32 = 4.;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub(super) struct Camera {
    zoom: f32,
    offset: [f32; 2],
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.,
            offset: [0., 0.],
        }
    }
}

impl Camera {
    pub(super) fn zoom(self) -> f32 {
        self.zoom
    }

    pub(super) fn offset(self) -> [f32; 2] {
        self.offset
    }

    pub(super) fn is_fit(self) -> bool {
        self.zoom == 1.
    }

    pub(super) fn is_max(self) -> bool {
        self.zoom == MAX_ZOOM
    }

    pub(super) fn restore(self) -> Self {
        if (1. ..=MAX_ZOOM).contains(&self.zoom)
            && self.offset.iter().all(|offset| offset.is_finite())
        {
            self.clamped()
        } else {
            Self::default()
        }
    }

    pub(super) fn zoom_at(self, zoom: f32, anchor: [f32; 2]) -> Self {
        if !zoom.is_finite() {
            return self;
        }
        let zoom = zoom.clamp(1., MAX_ZOOM);
        if zoom == self.zoom {
            return self;
        }
        Self {
            zoom,
            offset: std::array::from_fn(|axis| {
                anchor[axis] - (anchor[axis] - self.offset[axis]) * (zoom / self.zoom)
            }),
        }
        .clamped()
    }

    pub(super) fn pan(self, delta: [f32; 2]) -> Self {
        Self {
            offset: std::array::from_fn(|axis| self.offset[axis] + delta[axis]),
            ..self
        }
        .clamped()
    }

    pub(super) fn reveal(self, point: [f32; 2]) -> Self {
        self.pan(point.map(|axis| axis.clamp(0.15, 0.85) - axis))
    }

    fn clamped(self) -> Self {
        Self {
            offset: self.offset.map(|offset| offset.clamp(1. - self.zoom, 0.)),
            ..self
        }
    }
}

fn fraction(point: Point<Pixels>, bounds: Bounds<Pixels>) -> [f32; 2] {
    let offset = point - bounds.origin;
    [
        offset.x / bounds.size.width.max(px(1.)),
        offset.y / bounds.size.height.max(px(1.)),
    ]
}

#[derive(Default)]
pub(super) struct TableBounds {
    pub(super) viewport: Cell<Bounds<Pixels>>,
    pub(super) world: Cell<Bounds<Pixels>>,
    pub(super) picture: Cell<Bounds<Pixels>>,
    pub(super) pile: Cell<Bounds<Pixels>>,
}

impl GameView {
    pub(super) fn camera_controls(&self, cx: &Context<Self>) -> Stateful<Div> {
        let camera = self.round().camera;
        let zoomable = |id, label, accessible, enabled, delta| {
            action_button(id, label, enabled, cx)
                .aria_label(accessible)
                .when(enabled, |button| {
                    button.on_click(cx.listener(move |view, _, _, cx| {
                        let camera = view.round().camera;
                        let next = camera.zoom_at(camera.zoom() + delta, [0.5, 0.5]);
                        view.save_camera(next, cx);
                    }))
                })
        };
        div()
            .id("jigsaw-camera-controls")
            .role(Role::Group)
            .aria_label("Worktable view")
            .flex()
            .items_center()
            .gap_1()
            .child(zoomable(
                "jigsaw-zoom-out",
                "−",
                "Zoom out",
                !camera.is_fit(),
                -ZOOM_STEP,
            ))
            .child(
                small_text(format!("{:.0}%", camera.zoom() * 100.), cx)
                    .id("jigsaw-zoom-level")
                    .min_w(px(38.))
                    .text_center(),
            )
            .child(zoomable(
                "jigsaw-zoom-in",
                "+",
                "Zoom in",
                !camera.is_max(),
                ZOOM_STEP,
            ))
            .child(
                action_button("jigsaw-fit", "Fit", !camera.is_fit(), cx)
                    .aria_label("Fit worktable")
                    .when(!camera.is_fit(), |button| {
                        button.on_click(cx.listener(|view, _, _, cx| {
                            view.save_camera(Camera::default(), cx);
                        }))
                    }),
            )
            .child(if self.pan_mode {
                segments(cx).child(
                    segment("jigsaw-pan", "Pan", true, cx)
                        .debug_selector(|| "jigsaw-pan".into())
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.pan_mode = false;
                            cx.notify();
                        })),
                )
            } else {
                div().child(
                    action_button("jigsaw-pan", "Pan", !camera.is_fit(), cx)
                        .aria_toggled(false.into())
                        .when(!camera.is_fit(), |button| {
                            button.on_click(cx.listener(|view, _, window, cx| {
                                view.pan_mode = true;
                                view.view_focus.focus(window, cx);
                                cx.notify();
                            }))
                        }),
                )
            })
    }

    pub(super) fn camera_interactions(
        &self,
        table: Stateful<Div>,
        bounds: &Rc<TableBounds>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let pan_bounds = bounds.clone();
        let start_bounds = bounds.clone();
        let wheel_bounds = bounds.clone();
        let pinch_bounds = bounds.clone();
        table
            .track_focus(&self.view_focus)
            .when(!self.round().camera.is_fit(), |table| table.tab_index(0))
            .role(Role::Group)
            .aria_label("Jigsaw worktable")
            .aria_description(
                "When focused, use arrow keys to pan a zoomed table. Escape exits Pan mode.",
            )
            .when(self.pan_mode || self.pan_gesture.is_some(), |table| {
                if self.pan_gesture.is_some() {
                    table.cursor_grabbing()
                } else {
                    table.cursor_grab()
                }
            })
            .on_key_down(cx.listener(Self::camera_key))
            .capture_any_mouse_down(cx.listener(move |view, event, window, cx| {
                view.begin_pan(event, start_bounds.pile.get(), window, cx);
            }))
            .on_mouse_move(cx.listener(move |view, event, _, cx| {
                view.move_pan(event, pan_bounds.viewport.get(), cx);
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::release_pan))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::release_pan))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::release_pan))
            .on_mouse_up_out(MouseButton::Middle, cx.listener(Self::release_pan))
            .on_scroll(cx.listener(move |view, event, _, cx| {
                view.scroll_zoom(event, wheel_bounds.viewport.get(), cx);
            }))
            .on_pinch(cx.listener(move |view, event, _, cx| {
                view.pinch_zoom(event, pinch_bounds.viewport.get(), cx);
            }))
    }

    fn camera_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.view_focus.is_focused(window)
            || event.keystroke.modifiers.control
            || event.keystroke.modifiers.platform
            || event.keystroke.modifiers.alt
        {
            return;
        }
        let delta = match event.keystroke.key.as_str() {
            "left" => [0.12, 0.],
            "right" => [-0.12, 0.],
            "up" => [0., 0.12],
            "down" => [0., -0.12],
            "escape" if self.pan_mode => {
                self.pan_mode = false;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        let camera = self.round().camera;
        if !camera.is_fit() {
            self.save_camera(camera.pan(delta), cx);
            cx.stop_propagation();
        }
    }

    fn begin_pan(
        &mut self,
        event: &MouseDownEvent,
        pile: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let can_pan = event.button == MouseButton::Middle
            || (self.pan_mode
                && event.button == MouseButton::Left
                && !pile.contains(&event.position));
        if can_pan
            && !self.round().camera.is_fit()
            && !cx.has_active_drag()
            && self.pinch_start.is_none()
        {
            self.pan_gesture = Some((event.button, event.position));
            self.view_focus.focus(window, cx);
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn move_pan(
        &mut self,
        event: &MouseMoveEvent,
        viewport: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some((button, previous)) = self.pan_gesture else {
            return;
        };
        if event.pressed_button != Some(button) {
            self.end_pan(button, cx);
            return;
        }
        let from = fraction(previous, viewport);
        let to = fraction(event.position, viewport);
        let camera = self.round().camera;
        self.pan_gesture = Some((button, event.position));
        self.set_camera(camera.pan([to[0] - from[0], to[1] - from[1]]), cx);
        cx.stop_propagation();
    }

    fn release_pan(&mut self, event: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.end_pan(event.button, cx);
    }

    fn scroll_zoom(
        &mut self,
        event: &ScrollEvent,
        viewport: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if (event.control || event.platform)
            && !cx.has_active_drag()
            && self.pan_gesture.is_none()
            && self.pinch_start.is_none()
        {
            let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
            let camera = self.round().camera;
            let zoom = camera.zoom() * (delta * 0.009).clamp(-1., 1.).exp();
            self.save_camera(camera.zoom_at(zoom, fraction(event.position, viewport)), cx);
            cx.stop_propagation();
        }
    }

    fn pinch_zoom(&mut self, event: &PinchEvent, viewport: Bounds<Pixels>, cx: &mut Context<Self>) {
        if event.phase == TouchPhase::Cancelled {
            if let Some(camera) = self.pinch_start.take() {
                self.set_camera(camera, cx);
                cx.stop_propagation();
            }
            return;
        }
        if cx.has_active_drag() || self.pan_gesture.is_some() {
            return;
        }
        let camera = self.round().camera;
        self.pinch_start.get_or_insert(camera);
        let zoom = camera.zoom() * (1. + event.delta);
        self.set_camera(camera.zoom_at(zoom, fraction(event.position, viewport)), cx);
        if event.phase == TouchPhase::Ended {
            self.pinch_start = None;
            self.save();
        }
        cx.stop_propagation();
    }

    fn save_camera(&mut self, camera: Camera, cx: &mut Context<Self>) {
        if camera != self.round().camera {
            self.set_camera(camera, cx);
            self.save();
        }
    }

    fn set_camera(&mut self, camera: Camera, cx: &mut Context<Self>) {
        if self.round().camera != camera {
            self.round_mut().camera = camera;
            if camera.is_fit() {
                self.pan_mode = false;
                self.pan_gesture = None;
            }
            cx.notify();
        }
    }

    fn end_pan(&mut self, button: MouseButton, cx: &mut Context<Self>) {
        if self
            .pan_gesture
            .is_some_and(|(pressed, _)| pressed == button)
        {
            self.pan_gesture = None;
            self.save();
            cx.notify();
        }
    }

    pub(super) fn reveal_target(
        &mut self,
        target: usize,
        bounds: &TableBounds,
        cx: &mut Context<Self>,
    ) {
        let camera = self.round().camera;
        if camera.is_fit() {
            return;
        }
        let picture = bounds.picture.get();
        let (columns, rows) = self.key.size.dimensions();
        let center = picture.origin
            + point(
                picture.size.width * ((target % columns) as f32 + 0.5) / columns as f32,
                picture.size.height * ((target / columns) as f32 + 0.5) / rows as f32,
            );
        self.save_camera(camera.reveal(fraction(center, bounds.viewport.get())), cx);
    }
}
