// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use std::f32::consts::{FRAC_PI_2, TAU};

use crate::{Icon, Sizable, Size};
use gpui::{
    Animation, AnimationExt as _, App, Bounds, Hsla, IntoElement, ParentElement, PathBuilder,
    Pixels, Point, RenderOnce, Styled as _, Transformation, Window, canvas, div, ease_in_out,
    linear, percentage, point, prelude::FluentBuilder as _, px,
};
use instant::Duration;

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
        }
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

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
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
        div()
            .flex_none()
            .size(size)
            .with_animation(
                "ring",
                Animation::new(self.speed).repeat().with_easing(easing),
                move |this, turn| {
                    this.child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| paint_ring(bounds, turn, color, window),
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

    #[gpui::test]
    fn reduced_motion_spinner_is_static_and_requests_no_frame(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(true));
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
