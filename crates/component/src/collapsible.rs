// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled,
    Window,
};
use isonomic_ui_base::spring;

use crate::{ActiveTheme as _, StyledExt};

/// An interactive element which expands/collapses.
#[derive(IntoElement)]
pub struct Collapsible {
    base: isonomic_ui_base::Collapsible,
    style: StyleRefinement,
    motion_id: Option<ElementId>,
    open: bool,
    header: Option<HeaderBuilder>,
}

type HeaderBuilder = Box<dyn FnOnce(f32, &mut Window, &mut App) -> AnyElement>;

impl Collapsible {
    /// Creates a new `Collapsible` instance.
    pub fn new() -> Self {
        Self {
            base: isonomic_ui_base::Collapsible::new(),
            style: StyleRefinement::default(),
            motion_id: None,
            open: false,
            header: None,
        }
    }

    /// Sets whether the collapsible is open. default is false.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self.base = self.base.open(open);
        self
    }

    /// Enables a reversible measured reveal under a stable identity.
    pub fn motion_id(mut self, id: impl Into<ElementId>) -> Self {
        self.motion_id = Some(id.into());
        self
    }

    /// Sets a header drawn above everything else, built from how far the
    /// section is open: 0 closed, 1 open, following the height reveal under
    /// [`Self::motion_id`] (without one it is 0 or 1). A chevron that turns
    /// with `progress` moves in step with the content instead of snapping.
    pub fn header<E: IntoElement>(
        mut self,
        header: impl FnOnce(f32, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.header = Some(Box::new(move |progress, window, cx| {
            header(progress, window, cx).into_any_element()
        }));
        self
    }

    /// Sets the content of the collapsible.
    ///
    /// If `open` is false, content will be hidden.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.base = self.base.content(content);
        self
    }
}

/// How far the collapsible keyed by `motion_id` is open, from 0 to 1. Under
/// reduced motion it jumps.
fn reveal_progress(
    motion_id: impl Into<ElementId>,
    open: bool,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    spring(
        (motion_id.into(), "reveal"),
        if open { 1.0 } else { 0.0 },
        cx.theme().motion_tokens().spring_control,
        window,
        cx,
    )
}

impl Styled for Collapsible {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Collapsible {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for Collapsible {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (base, progress) = match self.motion_id {
            Some(id) => {
                let progress = reveal_progress(id.clone(), self.open, window, cx);
                (self.base.reveal(id, progress), progress)
            }
            None => (self.base, if self.open { 1.0 } else { 0.0 }),
        };
        let base = match self.header {
            Some(header) => base.prepend(header(progress, window, cx)),
            None => base,
        };
        base.v_flex().refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Context, InteractiveElement as _, Render, TestAppContext, div, px};

    use super::*;
    use crate::Theme;

    struct Harness(bool);

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Collapsible::new()
                .open(self.0)
                .child(
                    div()
                        .debug_selector(|| "collapsible-trigger".into())
                        .size(px(10.)),
                )
                .content(
                    div()
                        .debug_selector(|| "collapsible-content".into())
                        .size(px(10.)),
                )
        }
    }

    #[gpui::test]
    fn facade_preserves_vertical_layout_and_visibility(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Harness(true));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let trigger = cx.debug_bounds("collapsible-trigger").unwrap();
        let content = cx.debug_bounds("collapsible-content").unwrap();
        assert!(trigger.origin.y < content.origin.y);

        let (_, cx) = cx.add_window_view(|_, _| Harness(false));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("collapsible-trigger").is_some());
        assert!(cx.debug_bounds("collapsible-content").is_none());
    }

    struct HeaderHarness(bool);

    impl Render for HeaderHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Collapsible::new()
                .open(self.0)
                .header(|progress, _, _| {
                    div()
                        .debug_selector(|| "header".into())
                        .h(px(10.))
                        // The header's width reports the progress it was given.
                        .w(px(10. + 100. * progress))
                })
                .child(div().debug_selector(|| "trigger".into()).size(px(10.)))
                .content(div().debug_selector(|| "body".into()).size(px(10.)))
        }
    }

    #[gpui::test]
    fn header_comes_first_and_sees_the_open_state(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::default()));
        let (_, cx) = cx.add_window_view(|_, _| HeaderHarness(true));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let header = cx.debug_bounds("header").unwrap();
        let trigger = cx.debug_bounds("trigger").unwrap();
        let body = cx.debug_bounds("body").unwrap();
        assert!(header.origin.y < trigger.origin.y);
        assert!(trigger.origin.y < body.origin.y);
        assert_eq!(header.size.width, px(110.));

        let (_, cx) = cx.add_window_view(|_, _| HeaderHarness(false));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(cx.debug_bounds("header").unwrap().size.width, px(10.));
        assert!(cx.debug_bounds("body").is_none());
    }

    #[gpui::test]
    fn motion_id_keeps_closed_content_mounted_for_reversible_reveal(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::default()));

        struct MotionHarness;

        impl Render for MotionHarness {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                Collapsible::new()
                    .motion_id("details-motion")
                    .open(false)
                    .content(
                        div()
                            .debug_selector(|| "motion-content".into())
                            .size(px(10.)),
                    )
            }
        }

        let (_, cx) = cx.add_window_view(|_, _| MotionHarness);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("motion-content").is_some());
    }
}
