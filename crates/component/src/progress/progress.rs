// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{ActiveTheme, Sizable, Size, StyledExt};
use gpui::{
    Animation, AnimationExt as _, App, Background, ElementId, Hsla, IntoElement, IsZero as _,
    ParentElement, RenderOnce, SharedString, StyleRefinement, Styled, Window, div, ease_in_out,
    pattern_slash, prelude::FluentBuilder, px, relative,
};
use instant::Duration;
use isonomic_ui_base::{
    LayoutDirection, LogicalStyledExt as _, Progress as BaseProgress, ProgressIndicator,
    ProgressTrack, Transition, transition,
};

/// A linear horizontal progress bar element.
#[derive(IntoElement)]
pub struct Progress {
    id: ElementId,
    style: StyleRefinement,
    color: Option<Hsla>,
    value: f32,
    accessibility_label: Option<SharedString>,
    size: Size,
    loading: bool,
    striped: bool,
}

impl Progress {
    /// Create a new Progress bar.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: Default::default(),
            color: None,
            accessibility_label: None,
            style: StyleRefinement::default(),
            size: Size::default(),
            loading: false,
            striped: false,
        }
    }

    /// Enable indeterminate loading animation.
    ///
    /// When `loading` is `true`, the `value` is ignored and an infinite
    /// sliding animation is shown instead.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Draw diagonal stripes over the filled part.
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }

    /// Set the color of the progress bar.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Set the percentage value of the progress bar.
    ///
    /// The value should be between 0.0 and 100.0.
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(0., 100.);
        self
    }

    /// Set the accessible name exposed by the progress indicator.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }
}

impl Styled for Progress {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for Progress {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Progress {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let bg = self
            .color
            .map(Background::from)
            .unwrap_or(cx.theme().tokens.progress_bar.into());
        let value = self.value;
        let loading = self.loading;
        let stripes = self.striped.then(|| stripe_pattern(cx));
        let accessibility_label = self.accessibility_label;
        let reduce_motion = cx.reduce_motion();
        let rtl = LayoutDirection::global(cx).is_rtl();

        let radius = self.style.corner_radii.clone();
        let mut inner_style = StyleRefinement::default();
        inner_style.corner_radii = radius;

        let (height, pill_radius) = match self.size {
            // A thin rule by default: 4px, with 2 and 3 for dense rows and 6
            // where the bar is the headline.
            Size::XSmall => (px(2.), px(1.)),
            Size::Small => (px(3.), px(1.5)),
            Size::Medium => (px(4.), px(2.)),
            Size::Large => (px(6.), px(3.)),
            Size::Size(s) => (s, s / 2.),
        };
        // The bar reads as a pill of half its own height, and squares off with
        // the rest of the UI when the theme has no radius.
        let radius = if cx.theme().radius.is_zero() {
            px(0.)
        } else {
            pill_radius
        };

        let animated_value = transition(
            (self.id.clone(), "indicator"),
            value,
            Transition::new(cx.theme().motion_tokens().duration_normal)
                .easing(cx.theme().motion_tokens().easing_move.clone()),
            window,
            cx,
        );

        BaseProgress::new(self.id)
            .value(value)
            .indeterminate(loading)
            .when_some(accessibility_label, |this, label| {
                this.accessibility_label(label)
            })
            .w_full()
            .relative()
            .h(height)
            .rounded(radius)
            .refine_style(&self.style)
            .child(
                ProgressTrack::new()
                    .absolute()
                    .size_full()
                    .bg(crate::look::ink::hairline(cx))
                    .rounded(radius)
                    .refine_style(&inner_style),
            )
            .child(
                ProgressIndicator::new()
                    .absolute()
                    .top_0()
                    // Progress fills from the start edge: the right in RTL.
                    .inset_s(px(0.), cx)
                    .h_full()
                    .bg(bg)
                    .rounded(radius)
                    .refine_style(&inner_style)
                    .when_some(stripes, |this, stripes| {
                        this.overflow_hidden()
                            .child(div().size_full().rounded(radius).bg(stripes))
                    })
                    .map(|this| {
                        if loading && !reduce_motion {
                            this.with_animation(
                                "progress-loading",
                                Animation::new(Duration::from_secs(1)).repeat(),
                                move |this, delta| {
                                    let start =
                                        relative(ease_in_out(((delta - 0.5) / 0.5).clamp(0., 1.)));
                                    let end = relative(ease_in_out(1.0 - delta));
                                    if rtl {
                                        this.when(delta > 0.5, |this| this.right(start)).left(end)
                                    } else {
                                        this.when(delta > 0.5, |this| this.left(start)).right(end)
                                    }
                                },
                            )
                            .into_any_element()
                        } else if loading {
                            this.left(relative(0.325))
                                .right(relative(0.325))
                                .into_any_element()
                        } else {
                            this.w(relative((animated_value / 100.).clamp(0., 1.)))
                                .into_any_element()
                        }
                    }),
            )
    }
}

/// Diagonal stripes for a filled bar: 3px lines 4px apart, in the colour that
/// sits on the primary fill, at low alpha.
pub(super) fn stripe_pattern(cx: &App) -> Background {
    pattern_slash(cx.theme().tokens.primary_foreground.opacity(0.16), 3., 4.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_an_explicit_accessibility_label() {
        let plain = Progress::new("upload");
        assert_eq!(plain.accessibility_label, None);

        let named = Progress::new("upload").accessibility_label("Upload progress");
        assert_eq!(
            named.accessibility_label.as_deref(),
            Some("Upload progress")
        );
    }
}
