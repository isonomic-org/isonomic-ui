// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Thinking indicator: the row that shows an assistant is working between a message going out and the first words coming back, with a small orb that shows what kind of work it is.
//!
//! The orb has a look for each kind of work: reasoning, searching, using a
//! tool, waiting on the person, trying again after an error, and tidying a
//! long conversation. Its owner decides which applies; the indicator only
//! draws it. The label beside it shimmers while work goes on. Under reduced
//! motion the orb holds still and the label doesn't shimmer, and a screen
//! reader hears the label as a status.
//!
//! Once the work is done the row stays as a record of it: a light bulb and
//! the owner's words, such as "Thought for 2 seconds". Given the reasoning,
//! the row is a disclosure button, reached with Tab and pressed with Enter or
//! Space, that shows the reasoning under it in quiet text. A screen reader
//! hears whether it is expanded.

use std::{rc::Rc, time::Instant};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _, h_flex,
    shimmer::ShimmerText, v_flex,
};
use isonomic_ui::{
    AnyElement, App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, PathBuilder, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, size,
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

type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// The row shown while an assistant works before answering, and after it,
/// the line that says how long it thought.
///
/// While working it shows the orb and a shimmering label, with the time so
/// far after it when the owner passes one. Once finished it shows a light
/// bulb and the owner's words, such as "Thought for 2 seconds". Given the
/// reasoning, the row becomes a disclosure: a chevron button, collapsed at
/// first, that shows the reasoning under the row in quiet text and hides it
/// again. The owner formats every word; the indicator formats none.
#[derive(IntoElement)]
pub struct ThinkingIndicator {
    id: ElementId,
    style: StyleRefinement,
    state: OrbState,
    label: Option<SharedString>,
    elapsed: Option<SharedString>,
    finished: bool,
    reasoning: Option<AnyElement>,
    default_expanded: bool,
    on_toggle: Option<ToggleHandler>,
}

/// Whether the reasoning shows, kept between draws.
struct Disclosure {
    expanded: bool,
}

impl ThinkingIndicator {
    /// Create an indicator for one kind of work.
    pub fn new(id: impl Into<ElementId>, state: OrbState) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            state,
            label: None,
            elapsed: None,
            finished: false,
            reasoning: None,
            default_expanded: false,
            on_toggle: None,
        }
    }

    /// Say what's happening, such as "Reading 3 statements…", or once
    /// finished, how long it took: "Thought for 2 seconds".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Show the time so far after the label, in quieter text, such as
    /// "12s". The owner keeps the clock and passes the words on each draw.
    /// Only shown while working.
    pub fn elapsed(mut self, elapsed: impl Into<SharedString>) -> Self {
        self.elapsed = Some(elapsed.into());
        self
    }

    /// Mark the work finished. The orb gives way to a light bulb, the label
    /// stops shimmering, and the label should say how long it took.
    pub fn finished(mut self, finished: bool) -> Self {
        self.finished = finished;
        self
    }

    /// Give the reasoning, which makes the row a disclosure that shows it
    /// underneath. The owner draws the text, so it may stream or be styled
    /// as the owner likes; the indicator only places it and quiets it.
    pub fn reasoning(mut self, reasoning: impl IntoElement) -> Self {
        self.reasoning = Some(reasoning.into_any_element());
        self
    }

    /// Start with the reasoning showing. It starts hidden otherwise.
    pub fn default_expanded(mut self, expanded: bool) -> Self {
        self.default_expanded = expanded;
        self
    }

    /// Hear when the reasoning is shown or hidden, with whether it now shows.
    pub fn on_toggle(mut self, on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }
}

impl Styled for ThinkingIndicator {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// What a screen reader hears for the row: the label, then the time so far.
fn spoken(label: &str, elapsed: Option<&str>) -> String {
    match elapsed {
        Some(elapsed) if !elapsed.is_empty() => format!("{label}, {elapsed}"),
        _ => label.to_string(),
    }
}

impl RenderOnce for ThinkingIndicator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let still = cx.reduce_motion();
        let finished = self.finished;
        let label = self
            .label
            .unwrap_or_else(|| self.state.default_label().into());
        let elapsed = self.elapsed.filter(|_| !finished);
        let words = spoken(&label, elapsed.as_deref());
        let shimmer = !finished && self.state != OrbState::Waiting && !still;
        let id = self.id.clone();
        let part = |name: &str| ElementId::NamedChild(id.clone().into(), name.into());

        let mark = if finished {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(20.))
                .child(Icon::new(CatalogIcon::Idea).size(px(16.)).text_color(quiet))
                .into_any_element()
        } else {
            ActivityOrb::new(part("orb"), self.state)
                .color(quiet)
                .into_any_element()
        };
        let words_el = div().text_body().text_color(quiet).map(|this| {
            if shimmer {
                this.child(ShimmerText::new(label.clone()).id(part("shine")))
            } else {
                this.child(label.clone())
            }
        });
        let time_el = elapsed.map(|elapsed| {
            div()
                .flex_none()
                .text_body()
                .text_color(quiet.opacity(0.7))
                .child(elapsed)
        });

        let Some(reasoning) = self.reasoning else {
            return h_flex()
                .id(self.id.clone())
                .gap_2()
                .role(Role::Status)
                .aria_label(words)
                .child(mark)
                .child(words_el)
                .children(time_el)
                .refine_style(&self.style);
        };

        let default_expanded = self.default_expanded;
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| Disclosure {
            expanded: default_expanded,
        });
        let expanded = state.read(cx).expanded;
        let head_id = part("head");
        let focus = window
            .use_keyed_state((head_id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        let on_toggle = self.on_toggle;
        let head = Pressable::new(head_id)
            .accessibility_label(words)
            .aria_expanded(expanded)
            .track_focus(&focus)
            .on_click(move |_, window, cx| {
                let now = state.update(cx, |state, cx| {
                    state.expanded = !state.expanded;
                    cx.notify();
                    state.expanded
                });
                if let Some(on_toggle) = &on_toggle {
                    on_toggle(now, window, cx);
                }
            })
            .justify_start()
            .gap_2()
            .rounded(px(4.))
            .cursor_pointer()
            .when(focused, |this| this.focus_ring_style(window, cx))
            .child(mark)
            .child(words_el)
            .children(time_el)
            .child(
                Icon::new(if expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .small()
                .flex_none()
                .text_color(quiet),
            );

        v_flex()
            .id(self.id.clone())
            .items_start()
            .gap_1()
            .child(head)
            .when(expanded, |this| {
                this.child(
                    div()
                        .id(part("reasoning"))
                        .w_full()
                        // Under the label, past the 20 px mark and its gap.
                        .pl(px(28.))
                        .text_body()
                        .text_color(quiet)
                        .child(reasoning),
                )
            })
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

    #[test]
    fn the_time_so_far_is_spoken_after_the_label() {
        assert_eq!(spoken("Thinking", None), "Thinking");
        assert_eq!(spoken("Thinking", Some("")), "Thinking");
        assert_eq!(spoken("Thinking", Some("12s")), "Thinking, 12s");
    }
}
