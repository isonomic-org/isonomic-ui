//! Progress ring: a circular indicator that fills as work completes, or circles while the amount is unknown, with an optional label beside it.
//!
//! Give it a share from 0 to 1 and the ring fills clockwise from the top;
//! the large size prints the percent in the middle. Leave the share out and
//! a short arc circles instead, holding still when the system asks for less
//! motion. A finished ring turns to the success color.

use std::time::Instant;

use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex};
use isonomic_ui::{
    Role,
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px,
};

/// How big the ring is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RingSize {
    /// 16 px, for inline use.
    Small,
    /// 28 px.
    #[default]
    Medium,
    /// 64 px, with the percent in the middle.
    Large,
}

impl RingSize {
    fn diameter(self) -> f32 {
        match self {
            Self::Small => 16.,
            Self::Medium => 28.,
            Self::Large => 64.,
        }
    }

    fn stroke(self) -> f32 {
        match self {
            Self::Small => 2.,
            Self::Medium => 3.,
            Self::Large => 5.,
        }
    }
}

/// A share as a whole percent, such as "42%".
pub fn percent_label(share: f32) -> String {
    format!("{:.0}%", (share.clamp(0.0, 1.0) * 100.0).floor())
}

/// Points along an arc, from `start` turning `sweep` (both in turns, 0 at the top, clockwise).
pub fn arc_points(center: (f32, f32), radius: f32, start: f32, sweep: f32, steps: usize) -> Vec<(f32, f32)> {
    let steps = steps.max(1);
    (0..=steps)
        .map(|ix| {
            let turn = start + sweep * ix as f32 / steps as f32;
            let angle = turn * std::f32::consts::TAU;
            (center.0 + radius * angle.sin(), center.1 - radius * angle.cos())
        })
        .collect()
}

fn stroke_arc(window: &mut Window, bounds: Bounds<Pixels>, width: f32, start: f32, sweep: f32, color: Hsla) {
    if sweep <= 0.0 {
        return;
    }
    let side = f32::from(bounds.size.width);
    let radius = (side - width) / 2.0;
    let steps = (sweep * 96.0).ceil() as usize;
    let mut path = PathBuilder::stroke(px(width));
    for (ix, (x, y)) in arc_points((side / 2.0, side / 2.0), radius, start, sweep, steps).into_iter().enumerate() {
        let at = point(bounds.left() + px(x), bounds.top() + px(y));
        if ix == 0 {
            path.move_to(at);
        } else {
            path.line_to(at);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// A circular progress indicator.
#[derive(IntoElement)]
pub struct ProgressRing {
    id: ElementId,
    style: StyleRefinement,
    share: Option<f32>,
    size: RingSize,
    label: Option<SharedString>,
}

impl ProgressRing {
    /// Create a ring that circles until a share is set.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into(), style: StyleRefinement::default(), share: None, size: RingSize::Medium, label: None }
    }

    /// Fill to this share, from 0 to 1.
    pub fn share(mut self, share: f32) -> Self {
        self.share = Some(share.clamp(0.0, 1.0));
        self
    }

    /// Set how big the ring is.
    pub fn size(mut self, size: RingSize) -> Self {
        self.size = size;
        self
    }

    /// Say what is happening, beside the ring.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Styled for ProgressRing {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ProgressRing {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let track = theme.muted;
        let done = self.share.is_some_and(|s| s >= 1.0);
        let ink = if done { theme.success } else { theme.info };
        let quiet = theme.muted_foreground;
        let still = cx.reduce_motion();
        let started = window.use_keyed_state(self.id.clone(), cx, |_, _| Instant::now());
        let turn = if self.share.is_none() && !still {
            window.request_animation_frame();
            started.read(cx).elapsed().as_secs_f32() % 1.0
        } else {
            0.0
        };
        let size = self.size;
        let share = self.share;
        let side = px(size.diameter());

        let ring = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                stroke_arc(window, bounds, size.stroke(), 0.0, 1.0, track);
                match share {
                    Some(share) => stroke_arc(window, bounds, size.stroke(), 0.0, share, ink),
                    None => stroke_arc(window, bounds, size.stroke(), turn, 0.28, ink),
                }
            },
        )
        .size(side);

        let spoken = match (&self.label, share) {
            (Some(label), Some(share)) => format!("{label}, {}", percent_label(share)),
            (Some(label), None) => label.to_string(),
            (None, Some(share)) => percent_label(share),
            (None, None) => "Working".into(),
        };

        h_flex()
            .id(self.id.clone())
            .gap_2()
            .flex_none()
            .role(Role::ProgressIndicator)
            .when_some(share, |this, share| {
                this.aria_numeric_value(f64::from(share * 100.0)).aria_min_numeric_value(0.0).aria_max_numeric_value(100.0)
            })
            .aria_label(spoken)
            .child(
                div()
                    .relative()
                    .size(side)
                    .flex_none()
                    .child(ring)
                    .when(size == RingSize::Large && share.is_some(), |this| {
                        this.child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_sm()
                                .font_medium()
                                .child(percent_label(share.unwrap_or(0.0))),
                        )
                    }),
            )
            .when_some(self.label, |this, label| this.child(div().text_sm().text_color(quiet).child(label)))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percents_floor_and_arcs_start_at_the_top() {
        assert_eq!(percent_label(0.426), "42%");
        assert_eq!(percent_label(0.999), "99%");
        assert_eq!(percent_label(1.4), "100%");
        let points = arc_points((10.0, 10.0), 8.0, 0.0, 0.25, 2);
        assert_eq!(points.len(), 3);
        assert!((points[0].0 - 10.0).abs() < 1e-4 && (points[0].1 - 2.0).abs() < 1e-4);
        assert!((points[2].0 - 18.0).abs() < 1e-4 && (points[2].1 - 10.0).abs() < 1e-4);
        assert_eq!(RingSize::default(), RingSize::Medium);
    }
}
