// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Segmented bar: one total split into coloured parts, side by side.
//!
//! Each part is as wide as its share of the maximum, with a small gap between
//! parts, and the unused rest shows as track. Use it for a budget or a context
//! window split by source.

use gpui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, IsZero as _, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};
use isonomic_ui_base::{Transition, transition};

use super::progress::stripe_pattern;
use crate::{ActiveTheme as _, Sizable, Size, StyledExt as _};

/// The gap between neighbouring parts.
const GAP: f32 = 4.;
/// The narrowest a part is drawn, so a tiny share is still visible.
const MIN_PART: f32 = 6.;

/// One part of the total.
#[derive(Clone, Debug)]
pub struct BarSegment {
    label: SharedString,
    value: f64,
    color: Hsla,
}

impl BarSegment {
    /// A part with its name, amount and colour.
    pub fn new(label: impl Into<SharedString>, value: f64, color: impl Into<Hsla>) -> Self {
        Self {
            label: label.into(),
            value: value.max(0.),
            color: color.into(),
        }
    }
}

/// What a screen reader hears: every part with its amount, such as
/// "Paper 1,200, Ink 340".
pub fn spoken_parts(segments: &[BarSegment]) -> String {
    segments
        .iter()
        .map(|segment| format!("{} {}", segment.label, grouped(segment.value)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A rounded amount with a comma between each group of three digits.
fn grouped(value: f64) -> String {
    let digits = (value.round() as u64).to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (ix, digit) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The share of `max` a part takes, between 0 and 1.
fn share(value: f64, max: f64) -> f32 {
    if max <= 0. {
        return 0.;
    }
    (value / max).clamp(0., 1.) as f32
}

/// A total split into coloured parts.
#[derive(IntoElement)]
pub struct SegmentedBar {
    id: ElementId,
    style: StyleRefinement,
    segments: Vec<BarSegment>,
    max: f64,
    size: Size,
    striped: bool,
    label: SharedString,
    value_text: Option<SharedString>,
}

impl SegmentedBar {
    /// A bar whose full width stands for `max`.
    pub fn new(id: impl Into<ElementId>, max: f64) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            segments: Vec::new(),
            max,
            size: Size::default(),
            striped: false,
            label: "Breakdown".into(),
            value_text: None,
        }
    }

    /// Add a part.
    pub fn segment(mut self, segment: BarSegment) -> Self {
        self.segments.push(segment);
        self
    }

    /// Add several parts.
    pub fn segments(mut self, segments: impl IntoIterator<Item = BarSegment>) -> Self {
        self.segments.extend(segments);
        self
    }

    /// Draw diagonal stripes over the parts.
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Name the bar for screen readers.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Say the value in your own words, such as "1,540 of 4,000 sheets".
    /// Without it, every part is read with its amount.
    pub fn value_text(mut self, text: impl Into<SharedString>) -> Self {
        self.value_text = Some(text.into());
        self
    }
}

impl Styled for SegmentedBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for SegmentedBar {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for SegmentedBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (height, pill_radius) = match self.size {
            // A thin rule by default: 4px, with 2 and 3 for dense rows and 6
            // where the bar is the headline.
            Size::XSmall => (px(2.), px(1.)),
            Size::Small => (px(3.), px(1.5)),
            Size::Medium => (px(4.), px(2.)),
            Size::Large => (px(6.), px(3.)),
            Size::Size(s) => (s, s / 2.),
        };
        let radius = if cx.theme().radius.is_zero() {
            px(0.)
        } else {
            pill_radius
        };
        let track = crate::look::ink::hairline(cx);
        let stripes = self.striped.then(|| stripe_pattern(cx));
        let motion = cx.theme().motion_tokens();
        let grow = Transition::new(motion.duration_slow).easing(motion.easing_move.clone());
        let total: f64 = self.segments.iter().map(|segment| segment.value).sum();
        let spoken = self
            .value_text
            .clone()
            .unwrap_or_else(|| spoken_parts(&self.segments).into());

        let mut parts = Vec::with_capacity(self.segments.len());
        for (ix, segment) in self.segments.iter().enumerate() {
            let width = transition(
                (self.id.clone(), format!("part-{ix}")),
                share(segment.value, self.max),
                grow.clone(),
                window,
                cx,
            );
            parts.push(
                div()
                    .h_full()
                    .flex_none()
                    .min_w(px(MIN_PART))
                    .w(relative(width))
                    .rounded(radius)
                    .bg(segment.color)
                    .when_some(stripes, |this, stripes| {
                        this.overflow_hidden()
                            .child(div().size_full().rounded(radius).bg(stripes))
                    }),
            );
        }

        div()
            .id(self.id)
            .flex()
            .flex_row()
            .w_full()
            .h(height)
            .gap(px(GAP))
            .role(Role::Meter)
            .aria_label(format!("{}: {}", self.label, spoken))
            .aria_min_numeric_value(0.)
            .aria_max_numeric_value(self.max)
            .aria_numeric_value(total)
            .children(parts)
            .child(div().h_full().flex_1().min_w_0().rounded(radius).bg(track))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::black;

    #[test]
    fn every_part_is_read_with_its_amount() {
        let parts = [
            BarSegment::new("Paper", 1200., black()),
            BarSegment::new("Ink", 340.4, black()),
        ];
        assert_eq!(spoken_parts(&parts), "Paper 1,200, Ink 340");
        assert_eq!(spoken_parts(&[]), "");
    }

    #[test]
    fn a_share_stays_between_nothing_and_everything() {
        assert_eq!(share(25., 100.), 0.25);
        assert_eq!(share(250., 100.), 1.);
        assert_eq!(share(5., 0.), 0.);
        assert_eq!(BarSegment::new("Waste", -3., black()).value, 0.);
    }
}
