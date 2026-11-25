// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! A busy layer over content that is loading or saving.

use gpui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _,
};
use smallvec::SmallVec;

use crate::{ActiveTheme as _, Sizable as _, StyledExt as _, spinner::Spinner, v_flex};

/// Wraps content and, while loading, lays a quiet surface over it with a
/// spinner and an optional label.
///
/// While loading, the content stays in place and keeps its size, so the page
/// does not jump, and pointer input stops at the overlay. Screen readers hear
/// the label once, as a status. When loading ends the overlay goes and the
/// content is usable again.
///
/// ```ignore
/// LoadingOverlay::new("invoices")
///     .loading(self.saving)
///     .label("Saving")
///     .child(invoice_form)
/// ```
#[derive(IntoElement)]
pub struct LoadingOverlay {
    id: ElementId,
    loading: bool,
    label: Option<SharedString>,
    blur_content: bool,
    style: StyleRefinement,
    children: SmallVec<[AnyElement; 1]>,
}

impl LoadingOverlay {
    /// An overlay around the given content, not loading.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            loading: false,
            label: None,
            blur_content: true,
            style: StyleRefinement::default(),
            children: SmallVec::new(),
        }
    }

    /// Show the overlay.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Text under the spinner, such as "Saving". It is also what screen
    /// readers announce. Without it they hear "Loading".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Fade the content under the overlay. On by default; turn it off when
    /// the content should stay fully legible, such as a table being
    /// refreshed in place.
    pub fn dim_content(mut self, dim: bool) -> Self {
        self.blur_content = dim;
        self
    }

    /// Whether the overlay is showing.
    pub fn is_loading(&self) -> bool {
        self.loading
    }
}

impl Styled for LoadingOverlay {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for LoadingOverlay {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for LoadingOverlay {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let spoken = self.label.clone().unwrap_or_else(|| "Loading".into());
        let surface = theme
            .background
            .opacity(if self.blur_content { 0.72 } else { 0.4 });

        div()
            .id(self.id)
            .relative()
            .refine_style(&self.style)
            .children(self.children)
            .when(self.loading, |this| {
                this.child(
                    v_flex()
                        .id("loading-overlay")
                        .role(Role::Status)
                        .aria_label(spoken)
                        .absolute()
                        .inset_0()
                        .items_center()
                        .justify_center()
                        .bg(surface)
                        .rounded(theme.radius_lg)
                        // The overlay takes the pointer, so the content under it
                        // cannot be pressed or scrolled while it loads.
                        .occlude()
                        .child(
                            // A quiet chip, so the spinner and its label read
                            // clearly over any content.
                            v_flex()
                                .items_center()
                                .gap_2()
                                .px_4()
                                .py_3()
                                .rounded(theme.radius_lg)
                                .bg(theme.background)
                                .border_1()
                                .border_color(theme.border)
                                .child(Spinner::new().large().color(theme.muted_foreground))
                                .when_some(self.label, |this, label| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child(label),
                                    )
                                }),
                        ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext, VisualTestContext, point, px};
    use std::{cell::Cell, rc::Rc};

    struct Harness {
        loading: bool,
        clicks: Rc<Cell<usize>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            LoadingOverlay::new("overlay")
                .loading(self.loading)
                .label("Saving")
                .size(px(200.))
                .child(
                    div()
                        .id("content")
                        .size_full()
                        .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
                )
        }
    }

    fn harness(
        cx: &mut TestAppContext,
        loading: bool,
    ) -> (&mut VisualTestContext, Rc<Cell<usize>>) {
        cx.update(crate::init);
        let clicks = Rc::new(Cell::new(0));
        let (_, cx) = cx.add_window_view({
            let clicks = clicks.clone();
            move |_, _| Harness { loading, clicks }
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (cx, clicks)
    }

    #[gpui::test]
    fn content_takes_clicks_when_not_loading(cx: &mut TestAppContext) {
        let (cx, clicks) = harness(cx, false);
        cx.simulate_click(point(px(50.), px(50.)), Modifiers::default());
        assert_eq!(clicks.get(), 1);
    }

    #[gpui::test]
    fn the_overlay_stops_clicks_while_loading(cx: &mut TestAppContext) {
        let (cx, clicks) = harness(cx, true);
        cx.simulate_click(point(px(50.), px(50.)), Modifiers::default());
        assert_eq!(clicks.get(), 0);
    }

    #[test]
    fn builder_keeps_its_state() {
        assert!(!LoadingOverlay::new("a").is_loading());
        assert!(LoadingOverlay::new("a").loading(true).is_loading());
    }
}
