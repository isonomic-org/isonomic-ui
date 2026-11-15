//! Fading scroll area: a box that scrolls its content and fades the edge where more is hidden, with a button back to the top after a long scroll.
//!
//! The top edge fades once anything is scrolled away above, and the bottom
//! edge fades while more waits below, so it's clear the box scrolls before
//! anyone tries. Past two screens down, a "Back to top" button appears.

use isonomic_ui::component::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
};
use isonomic_ui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, linear_color_stop, linear_gradient,
    point, prelude::FluentBuilder as _, px,
};

/// Which edges hide more content, as (top, bottom), from the scroll offset and the most it can scroll (both positive).
pub fn hidden_edges(scrolled: f32, most: f32) -> (bool, bool) {
    let slack = 1.0;
    (scrolled > slack, most - scrolled > slack)
}

/// Whether to offer the way back up: once scrolled past `screens` heights of the box.
pub fn offer_back_to_top(scrolled: f32, viewport: f32, screens: f32) -> bool {
    viewport > 0.0 && scrolled > viewport * screens
}

/// A box that scrolls and fades its edges.
#[derive(IntoElement)]
pub struct FadingScrollArea {
    id: ElementId,
    style: StyleRefinement,
    children: Vec<AnyElement>,
    max_height: Pixels,
    fade: Pixels,
    label: Option<SharedString>,
}

impl FadingScrollArea {
    /// Create a box no taller than `max_height`.
    pub fn new(id: impl Into<ElementId>, max_height: Pixels) -> Self {
        Self { id: id.into(), style: StyleRefinement::default(), children: Vec::new(), max_height, fade: px(28.), label: None }
    }

    /// Fade this far in from each edge.
    pub fn fade(mut self, fade: Pixels) -> Self {
        self.fade = fade;
        self
    }

    /// Name the region for screen readers.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl ParentElement for FadingScrollArea {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for FadingScrollArea {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for FadingScrollArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let fill = cx.theme().background;
        let handle = window.use_keyed_state(self.id.clone(), cx, |_, _| ScrollHandle::new()).read(cx).clone();
        let scrolled = -f32::from(handle.offset().y);
        let most = f32::from(handle.max_offset().y);
        let viewport = f32::from(handle.bounds().size.height);
        let (above, below) = hidden_edges(scrolled, most);
        let back = offer_back_to_top(scrolled, viewport, 2.0);
        let fade = self.fade;
        let up = handle.clone();

        div()
            .relative()
            .w_full()
            .child(
                div()
                    .id(self.id.clone())
                    .w_full()
                    .max_h(self.max_height)
                    .overflow_y_scroll()
                    .track_scroll(&handle)
                    .when_some(self.label, |this, label| this.aria_label(label))
                    .on_scroll(|_, window, _| window.refresh())
                    .children(self.children)
                    .refine_style(&self.style),
            )
            .when(above, |this| {
                this.child(
                    div().absolute().top_0().left_0().right_0().h(fade).bg(linear_gradient(
                        180.,
                        linear_color_stop(fill, 0.),
                        linear_color_stop(fill.opacity(0.), 1.),
                    )),
                )
            })
            .when(below, |this| {
                this.child(
                    div().absolute().bottom_0().left_0().right_0().h(fade).bg(linear_gradient(
                        0.,
                        linear_color_stop(fill, 0.),
                        linear_color_stop(fill.opacity(0.), 1.),
                    )),
                )
            })
            .when(back, |this| {
                this.child(
                    div().absolute().bottom_2().left_0().right_0().flex().justify_center().child(
                        Button::new(ElementId::NamedChild(self.id.clone().into(), "top".into()))
                            .xsmall()
                            .primary()
                            .icon(IconName::ArrowUp)
                            .label("Back to top")
                            .on_click(move |_, window, _| {
                                up.set_offset(point(px(0.), px(0.)));
                                window.refresh();
                            }),
                    ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_fade_only_where_content_hides() {
        assert_eq!(hidden_edges(0.0, 400.0), (false, true));
        assert_eq!(hidden_edges(120.0, 400.0), (true, true));
        assert_eq!(hidden_edges(400.0, 400.0), (true, false));
        assert_eq!(hidden_edges(0.0, 0.0), (false, false));
        assert!(!offer_back_to_top(300.0, 200.0, 2.0));
        assert!(offer_back_to_top(450.0, 200.0, 2.0));
        assert!(!offer_back_to_top(450.0, 0.0, 2.0));
    }
}
