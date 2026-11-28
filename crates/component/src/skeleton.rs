// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{ActiveTheme, StyledExt};
use gpui::{
    Animation, AnimationExt, ElementId, InteractiveElement as _, IntoElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, bounce, div,
    ease_in_out, prelude::FluentBuilder as _,
};
use instant::Duration;

/// A skeleton loading placeholder element.
#[derive(IntoElement)]
pub struct Skeleton {
    style: StyleRefinement,
    secondary: bool,
    circle: Option<gpui::Pixels>,
    announce: Option<(ElementId, SharedString)>,
}

impl Skeleton {
    /// Create a new Skeleton element.
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            secondary: false,
            circle: None,
            announce: None,
        }
    }

    /// Set use secondary color.
    pub fn secondary(mut self) -> Self {
        self.secondary = true;
        self
    }

    /// A round placeholder `size` across, for an avatar or an icon; it does
    /// not shrink in a row.
    pub fn circle(mut self, size: impl Into<gpui::Pixels>) -> Self {
        self.circle = Some(size.into());
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
        let style = &self.style;
        let circle = self.circle;
        let still = cx.reduce_motion();
        match self.announce {
            Some((id, label)) => pulse(
                div().id(id).role(Role::ProgressIndicator).aria_label(label),
                color,
                circle,
                style,
                still,
            ),
            None => pulse(div(), color, circle, style, still),
        }
    }
}

/// Draws a skeleton block on `el` and pulses it, or holds it still under
/// reduced motion.
fn pulse<E>(
    el: E,
    color: gpui::Hsla,
    circle: Option<gpui::Pixels>,
    style: &StyleRefinement,
    still: bool,
) -> gpui::AnyElement
where
    E: Styled + IntoElement + 'static,
{
    // A text bar by default: 10px tall, so it sits inside a 20px line with
    // room above and below, with soft 6px ends.
    let el = el
        .w_full()
        .h(gpui::px(10.))
        .rounded(gpui::px(6.))
        .bg(color)
        .map(|this| match circle {
            Some(size) => this.size(size).flex_none().rounded(size / 2.),
            None => this,
        })
        .refine_style(style);
    if still {
        return el.into_any_element();
    }
    el.with_animation(
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, ParentElement as _, Render, TestAppContext, Window, px, size};

    struct Host;

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui::div()
                .flex()
                .child(Skeleton::new().circle(px(24.)))
                .child(Skeleton::new().w(px(40.)))
        }
    }

    #[gpui::test]
    fn reduced_motion_holds_skeletons_still(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init(cx);
            cx.set_reduce_motion(true);
        });
        let window = cx.open_window(size(px(200.), px(100.)), |_, _| Host);
        cx.run_until_parked();

        assert_eq!(
            window
                .update(cx, |_, window, cx| window.simulate_next_frame(cx))
                .unwrap(),
            0
        );
    }
}
