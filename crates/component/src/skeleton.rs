// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{ActiveTheme, StyledExt};
use gpui::{
    Animation, AnimationExt, ElementId, InteractiveElement as _, IntoElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, bounce, div,
    ease_in_out,
};
use instant::Duration;

/// A skeleton loading placeholder element.
#[derive(IntoElement)]
pub struct Skeleton {
    style: StyleRefinement,
    secondary: bool,
    announce: Option<(ElementId, SharedString)>,
}

impl Skeleton {
    /// Create a new Skeleton element.
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            secondary: false,
            announce: None,
        }
    }

    /// Set use secondary color.
    pub fn secondary(mut self) -> Self {
        self.secondary = true;
        self
    }
}

impl Skeleton {
    /// Announce the loading state on this block, such as "Loading invoices".
    ///
    /// A group of skeletons stands for one thing loading, so set this on one
    /// of them; the rest stay silent. The id must be unique among siblings.
    pub fn loading_label(
        mut self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
    ) -> Self {
        self.announce = Some((id.into(), label.into()));
        self
    }
}

impl Styled for Skeleton {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let color = if self.secondary {
            cx.theme().skeleton.opacity(0.5).into()
        } else {
            cx.theme().skeleton
        };
        match self.announce {
            Some((id, label)) => pulse(
                div().id(id).role(Role::ProgressIndicator).aria_label(label),
                color,
                &self.style,
            ),
            None => pulse(div(), color, &self.style),
        }
    }
}

/// Draws a skeleton block on `el` and pulses it.
fn pulse<E>(el: E, color: gpui::Hsla, style: &StyleRefinement) -> gpui::AnyElement
where
    E: Styled + IntoElement + 'static,
{
    el.w_full()
        .h_4()
        .bg(color)
        .refine_style(style)
        .with_animation(
            "skeleton",
            Animation::new(Duration::from_secs(2))
                .repeat()
                .with_easing(bounce(ease_in_out)),
            move |this, delta| {
                let v = 1.0 - delta * 0.5;
                this.opacity(v)
            },
        )
        .into_any_element()
}
