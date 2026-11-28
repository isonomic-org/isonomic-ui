// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::f32::consts::{FRAC_PI_2, TAU};

use crate::{Icon, Sizable, Size};
use gpui::{
    Animation, AnimationExt as _, App, Bounds, ElementId, Hsla, InteractiveElement as _,
    IntoElement, ParentElement, PathBuilder, Pixels, Point, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Transformation, Window, canvas, div, ease_in_out,
    linear, percentage, point, prelude::FluentBuilder as _, px,
};
use instant::Duration;

/// How a [`Spinner`] is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpinnerStyle {
    /// A faint ring with a quarter arc running round it. The default.
    #[default]
    Ring,
    /// Eight short rays brightening in turn: the "thinking" indicator.
    Rays,
    /// A plain three-quarter arc, turning, with no ring behind it.
    Arc,
    /// Three dots in a row, each brightening in turn: a quieter "thinking"
    /// indicator for a line of text.
    Dots,
}

/// A cycling loading spinner.
///
/// By default a faint ring with a quarter arc running round it, drawn in the
/// icon ink and turning once every 800ms. With [`icon`](Self::icon) it turns
/// that icon instead.
#[derive(IntoElement)]
pub struct Spinner {
    size: Size,
    icon: Option<Icon>,
    speed: Duration,
    easing: Option<Box<dyn Fn(f32) -> f32>>,
    color: Option<Hsla>,
    style: SpinnerStyle,
    announce: Option<(ElementId, SharedString)>,
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Spinner {
    /// Create a new loading spinner.
    pub fn new() -> Self {
        Self {
            size: Size::Medium,
            speed: Duration::from_millis(800),
            easing: None,
            icon: None,
            color: None,
            style: SpinnerStyle::Ring,
            announce: None,
        }
    }

    /// Eight short rays instead of the ring, each brightening in turn so the
    /// light runs round clockwise: the "thinking" indicator.
    pub fn rays(self) -> Self {
        self.style(SpinnerStyle::Rays)
    }

    /// A plain turning arc with no ring behind it: shorthand for
    /// `style(SpinnerStyle::Arc)`.
    pub fn arc(self) -> Self {
        self.style(SpinnerStyle::Arc)
    }

    /// Three dots brightening in turn: shorthand for
    /// `style(SpinnerStyle::Dots)`.
    pub fn dots(self) -> Self {
        self.style(SpinnerStyle::Dots)
    }

    /// Announce what is loading, such as "Loading invoices", as a progress
    /// indicator. Without it the spinner is decoration and a screen reader
    /// skips it. The id must be unique among siblings.
    pub fn loading_label(
        mut self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
    ) -> Self {
        self.announce = Some((id.into(), label.into()));
        self
    }

    /// Set how the spinner is drawn. The default is [`SpinnerStyle::Ring`].
    pub fn style(mut self, style: SpinnerStyle) -> Self {
        self.style = style;
        self
    }

    /// Turn this icon instead of drawing the ring.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set the ink. The ring is the same ink, faint.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Set the easing of each turn. The ring turns linearly by default, a
    /// turned icon eases in and out.
    pub fn ease(mut self, easing: impl Fn(f32) -> f32 + 'static) -> Self {
        self.easing = Some(Box::new(easing));
        self
    }
}

impl Sizable for Spinner {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

/// The ring's diameter: 12, 14, 16 or 20px.
fn diameter(size: Size) -> Pixels {
    match size {
        Size::XSmall => px(12.),
        Size::Small => px(14.),
        Size::Large => px(20.),
        Size::Medium => px(16.),
        Size::Size(size) => size,
    }
}

fn on_circle(center: Point<Pixels>, radius: Pixels, angle: f32) -> Point<Pixels> {
    point(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
    )
}

/// A circle to stroke: its centre, radius and line width.
#[derive(Clone, Copy)]
struct Circle {
    center: Point<Pixels>,
    radius: Pixels,
    width: Pixels,
}

impl Circle {
    /// Strokes the arc from `start` through `sweep` radians, with round ends
    /// when asked.
    fn paint_arc(self, window: &mut Window, start: f32, sweep: f32, color: Hsla, round_ends: bool) {
        let Circle {
            center,
            radius,
            width,
        } = self;
        let steps = ((sweep.abs() / TAU) * 64.).ceil().max(4.) as usize;
        let mut line = PathBuilder::stroke(width);
        line.move_to(on_circle(center, radius, start));
        for step in 1..=steps {
            line.line_to(on_circle(
                center,
                radius,
                start + sweep * step as f32 / steps as f32,
            ));
        }
        if let Ok(path) = line.build() {
            window.paint_path(path, color);
        }
        if round_ends {
            for end in [start, start + sweep] {
                let cap = on_circle(center, radius, end);
                let points: Vec<_> = (0..12)
                    .map(|ix| on_circle(cap, width / 2., ix as f32 / 12. * TAU))
                    .collect();
                let mut dot = PathBuilder::fill();
                dot.add_polygon(&points, true);
                if let Ok(path) = dot.build() {
                    window.paint_path(path, color);
                }
            }
        }
    }
}

fn paint_ring(bounds: Bounds<Pixels>, turn: f32, color: Hsla, window: &mut Window) {
    let d = bounds.size.width.min(bounds.size.height);
    // Drawn on a 16-unit square: a radius of 6 and a 1.8 stroke.
    let circle = Circle {
        center: bounds.center(),
        radius: d * (6. / 16.),
        width: d * (1.8 / 16.),
    };
    circle.paint_arc(window, 0., TAU, color.alpha(color.a * 0.18), false);
    circle.paint_arc(window, -FRAC_PI_2 + turn * TAU, FRAC_PI_2, color, true);
}

/// Strokes a straight line from `from` to `to` with round ends.
fn paint_segment(
    window: &mut Window,
    from: Point<Pixels>,
    to: Point<Pixels>,
    width: Pixels,
    color: Hsla,
) {
    let mut line = PathBuilder::stroke(width);
    line.move_to(from);
    line.line_to(to);
    if let Ok(path) = line.build() {
        window.paint_path(path, color);
    }
    for end in [from, to] {
        let points: Vec<_> = (0..12)
            .map(|ix| on_circle(end, width / 2., ix as f32 / 12. * TAU))
            .collect();
        let mut dot = PathBuilder::fill();
        dot.add_polygon(&points, true);
        if let Ok(path) = dot.build() {
            window.paint_path(path, color);
        }
    }
}

/// The stroke of an arc spinner `d` wide: an eighth of it, never thinner
/// than one and a half pixels.
fn arc_stroke(d: Pixels) -> Pixels {
    (d / 8.).max(px(1.5))
}

fn paint_arc(bounds: Bounds<Pixels>, turn: f32, color: Hsla, window: &mut Window) {
    let d = bounds.size.width.min(bounds.size.height);
    let width = arc_stroke(d);
    let circle = Circle {
        center: bounds.center(),
        radius: (d - width) / 2.,
        width,
    };
    circle.paint_arc(window, -FRAC_PI_2 + turn * TAU, TAU * 0.75, color, false);
}

/// How strongly dot `ix` of three shows at `time` (0 to 1 over a cycle):
/// each brightens in turn, left to right, the others resting at a quarter.
pub fn dot_strength(ix: usize, time: f32) -> f32 {
    let center = (ix as f32 + 0.5) / 3.;
    let offset = (time.rem_euclid(1.) - center).abs();
    let distance = offset.min(1. - offset);
    0.25 + 0.75 * (1. - distance * 3.).max(0.)
}

fn paint_dots(bounds: Bounds<Pixels>, time: f32, color: Hsla, window: &mut Window) {
    let d = bounds.size.width.min(bounds.size.height);
    let center = bounds.center();
    // On a 16-unit square: dots 3.6 across, 5.2 apart.
    let (dot, pitch) = (d * (3.6 / 16.), d * (5.2 / 16.));
    for ix in 0..3 {
        let at = point(center.x + pitch * (ix as f32 - 1.), center.y);
        let points: Vec<_> = (0..16)
            .map(|step| on_circle(at, dot / 2., step as f32 / 16. * TAU))
            .collect();
        let mut path = PathBuilder::fill();
        path.add_polygon(&points, true);
        if let Ok(path) = path.build() {
            window.paint_path(path, color.alpha(color.a * dot_strength(ix, time)));
        }
    }
}

fn paint_rays(bounds: Bounds<Pixels>, time: f32, color: Hsla, window: &mut Window) {
    let d = bounds.size.width.min(bounds.size.height);
    let center = bounds.center();
    // On a 16-unit square each ray runs from 3.25 to 6.25 out from the
    // centre, 1.1 wide.
    let (inner, outer, width) = (d * (3.25 / 16.), d * (6.25 / 16.), d * (1.1 / 16.));
    for ray in 0..8 {
        let angle = -FRAC_PI_2 + ray as f32 * TAU / 8.;
        // Each ray fades from full to a fifth over a turn, an eighth of a turn
        // behind the ray before it.
        let phase = (time - ray as f32 / 8.).rem_euclid(1.);
        let alpha = 1. - 0.8 * phase;
        paint_segment(
            window,
            on_circle(center, inner, angle),
            on_circle(center, outer, angle),
            width,
            color.alpha(color.a * alpha),
        );
    }
}

impl RenderOnce for Spinner {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self.announce.take() {
            Some((id, label)) => div()
                .id(id)
                .flex_none()
                .role(Role::ProgressIndicator)
                .aria_label(label)
                .child(self.render_mark(window, cx))
                .into_any_element(),
            None => self.render_mark(window, cx),
        }
    }
}

impl Spinner {
    fn render_mark(self, _window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        if let Some(icon) = self.icon {
            let easing = self.easing.unwrap_or_else(|| Box::new(ease_in_out));
            return div()
                .child(
                    icon.with_size(self.size)
                        .when_some(self.color, |this, color| this.text_color(color))
                        .with_animation(
                            "circle",
                            Animation::new(self.speed).repeat().with_easing(easing),
                            |this, delta| this.transform(Transformation::rotate(percentage(delta))),
                        ),
                )
                .into_any_element();
        }

        let color = self.color.unwrap_or_else(|| crate::look::ink::icon(cx));
        let size = diameter(self.size);
        let easing = self.easing.unwrap_or_else(|| Box::new(linear));
        let style = self.style;
        // Dots read as a slow wave; a full cycle at the ring's pace flickers.
        let speed = if style == SpinnerStyle::Dots && self.speed == Duration::from_millis(800) {
            Duration::from_millis(1200)
        } else {
            self.speed
        };
        div()
            .flex_none()
            .size(size)
            .with_animation(
                "ring",
                Animation::new(speed).repeat().with_easing(easing),
                move |this, turn| {
                    this.child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| match style {
                                SpinnerStyle::Ring => paint_ring(bounds, turn, color, window),
                                SpinnerStyle::Rays => paint_rays(bounds, turn, color, window),
                                SpinnerStyle::Arc => paint_arc(bounds, turn, color, window),
                                SpinnerStyle::Dots => paint_dots(bounds, turn, color, window),
                            },
                        )
                        .size_full(),
                    )
                },
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Render, TestAppContext, px, size};

    struct SpinnerHost;

    impl Render for SpinnerHost {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            Spinner::new()
        }
    }

    struct ArcSpinnerHost;

    impl Render for ArcSpinnerHost {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            Spinner::new().arc()
        }
    }

    #[test]
    fn dots_brighten_in_turn() {
        for ix in 0..3 {
            let moment = (ix as f32 + 0.5) / 3.;
            assert!((dot_strength(ix, moment) - 1.).abs() < 1e-5);
            assert!((dot_strength((ix + 1) % 3, moment) - 0.25).abs() < 1e-5);
        }
        // The cycle wraps: just past the end looks like just past the start.
        assert!((dot_strength(0, 1.02) - dot_strength(0, 0.02)).abs() < 1e-5);
    }

    #[test]
    fn arc_spinner_matches_the_icon_sizes() {
        assert_eq!(diameter(Size::XSmall), px(12.));
        assert_eq!(diameter(Size::Small), px(14.));
        assert_eq!(diameter(Size::Medium), px(16.));
        assert_eq!(diameter(Size::Large), px(20.));
        assert_eq!(arc_stroke(px(12.)), px(1.5));
        assert_eq!(arc_stroke(px(24.)), px(3.));
        assert_eq!(Spinner::new().arc().style, SpinnerStyle::Arc);
    }

    #[gpui::test]
    fn reduced_motion_arc_spinner_requests_no_frame(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init(cx);
            cx.set_reduce_motion(true);
        });
        let window = cx.open_window(size(px(100.), px(100.)), |_, _| ArcSpinnerHost);
        cx.run_until_parked();

        assert_eq!(
            window
                .update(cx, |_, window, cx| window.simulate_next_frame(cx))
                .unwrap(),
            0
        );
    }

    #[gpui::test]
    fn reduced_motion_spinner_is_static_and_requests_no_frame(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init(cx);
            cx.set_reduce_motion(true);
        });
        let window = cx.open_window(size(px(100.), px(100.)), |_, _| SpinnerHost);
        cx.run_until_parked();

        assert_eq!(
            window
                .update(cx, |_, window, cx| window.simulate_next_frame(cx))
                .unwrap(),
            0
        );
    }
}
