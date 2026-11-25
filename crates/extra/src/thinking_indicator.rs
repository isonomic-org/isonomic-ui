// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Thinking indicator: the row that shows an assistant is working between a message going out and the first words coming back, with a small orb that shows what kind of work it is.
//!
//! The orb has a look for each kind of work: reasoning, searching, using a
//! tool, waiting on the person, trying again after an error, and tidying a
//! long conversation. Its owner decides which applies; the indicator only
//! draws it. The label beside it shimmers while work goes on. Under reduced
//! motion the orb holds still and the label doesn't shimmer, and a screen
//! reader hears the label as a status.

use std::time::Instant;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{ActiveTheme as _, StyledExt as _, h_flex, shimmer::ShimmerText};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    PathBuilder, Pixels, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, canvas, div, point, prelude::FluentBuilder as _, px, size,
};

/// What kind of work the orb shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrbState {
    /// Thinking it through: three dots circling.
    #[default]
    Reasoning,
    /// Looking things up: an arc sweeping a ring.
    Searching,
    /// Running a tool: a core with a ring that pulses outwards.
    Using,
    /// Waiting on the person or an approval: a still ring that breathes.
    Waiting,
    /// Trying again after an error: a broken ring turning.
    Retrying,
    /// Tidying a long conversation: rings closing in.
    Compacting,
}

impl OrbState {
    /// The label shown when the owner gives none.
    pub fn default_label(self) -> &'static str {
        match self {
            Self::Reasoning => "Thinking\u{2026}",
            Self::Searching => "Searching\u{2026}",
            Self::Using => "Using a tool\u{2026}",
            Self::Waiting => "Waiting for you",
            Self::Retrying => "Trying again\u{2026}",
            Self::Compacting => "Tidying the conversation\u{2026}",
        }
    }

    /// Seconds for one turn of the orb's motion.
    pub fn period(self) -> f32 {
        match self {
            Self::Reasoning => 1.2,
            Self::Searching => 1.6,
            Self::Using => 1.0,
            Self::Waiting => 2.4,
            Self::Retrying => 0.9,
            Self::Compacting => 1.8,
        }
    }
}

/// Where in its motion an orb is, from 0 to 1, after `elapsed` seconds.
pub fn orb_phase(elapsed: f32, period: f32) -> f32 {
    if period <= 0.0 {
        0.0
    } else {
        (elapsed / period).fract()
    }
}

/// The point at `turn` (0 at the top, clockwise) on a circle.
fn on_circle(center: (f32, f32), radius: f32, turn: f32) -> (f32, f32) {
    let angle = turn * std::f32::consts::TAU;
    (
        center.0 + radius * angle.sin(),
        center.1 - radius * angle.cos(),
    )
}

fn arc(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radius: f32,
    start: f32,
    sweep: f32,
    width: f32,
    color: Hsla,
) {
    let side = f32::from(bounds.size.width);
    let center = (side / 2.0, side / 2.0);
    let steps = ((sweep * 48.0).ceil() as usize).max(2);
    let mut path = PathBuilder::stroke(px(width));
    for ix in 0..=steps {
        let (x, y) = on_circle(center, radius, start + sweep * ix as f32 / steps as f32);
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

fn dot(window: &mut Window, bounds: Bounds<Pixels>, at: (f32, f32), diameter: f32, color: Hsla) {
    let center = point(bounds.left() + px(at.0), bounds.top() + px(at.1));
    window.paint_quad(
        isonomic_ui::fill(
            Bounds::centered_at(center, size(px(diameter), px(diameter))),
            color,
        )
        .corner_radii(px(diameter / 2.0)),
    );
}

fn paint_orb(window: &mut Window, bounds: Bounds<Pixels>, state: OrbState, phase: f32, ink: Hsla) {
    let side = f32::from(bounds.size.width);
    let c = (side / 2.0, side / 2.0);
    let r = side / 2.0 - 2.0;
    let wave = 1.0 - (2.0 * phase - 1.0).abs();
    match state {
        OrbState::Reasoning => {
            for ix in 0..3 {
                let turn = phase + ix as f32 / 3.0;
                dot(
                    window,
                    bounds,
                    on_circle(c, r * 0.6, turn),
                    side * 0.2,
                    ink.opacity(1.0 - ix as f32 * 0.25),
                );
            }
        }
        OrbState::Searching => {
            arc(window, bounds, r, 0.0, 1.0, 1.0, ink.opacity(0.3));
            arc(window, bounds, r, phase, 0.25, 2.0, ink);
            dot(window, bounds, c, side * 0.2, ink);
        }
        OrbState::Using => {
            dot(window, bounds, c, side * 0.36, ink);
            arc(
                window,
                bounds,
                r * (0.55 + 0.45 * phase),
                0.0,
                1.0,
                1.5,
                ink.opacity(1.0 - phase),
            );
        }
        OrbState::Waiting => {
            arc(
                window,
                bounds,
                r,
                0.0,
                1.0,
                1.5,
                ink.opacity(0.45 + 0.4 * wave),
            );
            dot(window, bounds, c, side * 0.18, ink.opacity(0.6));
        }
        OrbState::Retrying => {
            arc(window, bounds, r * 0.8, phase, 0.72, 2.0, ink);
            let head = on_circle(c, r * 0.8, phase + 0.72);
            dot(window, bounds, head, side * 0.2, ink);
        }
        OrbState::Compacting => {
            for ix in 0..3 {
                let shrink = (phase + ix as f32 / 3.0).fract();
                arc(
                    window,
                    bounds,
                    r * (1.0 - shrink * 0.8),
                    0.0,
                    1.0,
                    1.2,
                    ink.opacity(shrink),
                );
            }
        }
    }
}

/// A small orb that shows what kind of work is going on.
#[derive(IntoElement)]
pub struct ActivityOrb {
    id: ElementId,
    style: StyleRefinement,
    state: OrbState,
    size: f32,
    color: Option<Hsla>,
}

impl ActivityOrb {
    /// Create a 20 px orb.
    pub fn new(id: impl Into<ElementId>, state: OrbState) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            state,
            size: 20.0,
            color: None,
        }
    }

    /// Draw it this many pixels across.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(12.0);
        self
    }

    /// Draw it in this colour instead of the theme's ink.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl Styled for ActivityOrb {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ActivityOrb {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ink = self.color.unwrap_or(cx.theme().foreground);
        let still = cx.reduce_motion();
        let started = window.use_keyed_state(self.id.clone(), cx, |_, _| Instant::now());
        let state = self.state;
        let phase = if still {
            0.0
        } else {
            window.request_animation_frame();
            orb_phase(started.read(cx).elapsed().as_secs_f32(), state.period())
        };
        div()
            .id(self.id.clone())
            .flex_none()
            .size(px(self.size))
            .role(Role::Image)
            .aria_label(
                state
                    .default_label()
                    .trim_end_matches('\u{2026}')
                    .to_string(),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| paint_orb(window, bounds, state, phase, ink),
                )
                .size_full(),
            )
            .refine_style(&self.style)
    }
}

/// The row shown while an assistant works before answering.
#[derive(IntoElement)]
pub struct ThinkingIndicator {
    id: ElementId,
    style: StyleRefinement,
    state: OrbState,
    label: Option<SharedString>,
}

impl ThinkingIndicator {
    /// Create an indicator for one kind of work.
    pub fn new(id: impl Into<ElementId>, state: OrbState) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            state,
            label: None,
        }
    }

    /// Say what's happening, such as "Reading 3 statements…".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Styled for ThinkingIndicator {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ThinkingIndicator {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let still = cx.reduce_motion();
        let label = self
            .label
            .unwrap_or_else(|| self.state.default_label().into());
        let working = self.state != OrbState::Waiting;
        h_flex()
            .id(self.id.clone())
            .gap_2()
            .role(Role::Status)
            .aria_label(label.clone())
            .child(
                ActivityOrb::new(
                    ElementId::NamedChild(self.id.clone().into(), "orb".into()),
                    self.state,
                )
                .color(quiet),
            )
            .child(div().text_body().text_color(quiet).map(|this| {
                if working && !still {
                    this.child(ShimmerText::new(label.clone()).id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "shine".into(),
                    )))
                } else {
                    this.child(label.clone())
                }
            }))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_carry_labels_and_phases_wrap() {
        assert_eq!(OrbState::default(), OrbState::Reasoning);
        assert_eq!(OrbState::Waiting.default_label(), "Waiting for you");
        assert_eq!(
            OrbState::Compacting.default_label(),
            "Tidying the conversation\u{2026}"
        );
        assert!((orb_phase(1.8, 1.2) - 0.5).abs() < 1e-5);
        assert_eq!(orb_phase(3.0, 0.0), 0.0);
        let (x, y) = on_circle((10.0, 10.0), 5.0, 0.25);
        assert!((x - 15.0).abs() < 1e-4 && (y - 10.0).abs() < 1e-4);
        assert!(OrbState::Retrying.period() < OrbState::Waiting.period());
    }
}
