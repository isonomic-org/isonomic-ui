// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A grid of images in equal tiles, such as attachments or a gallery.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, ImageSource, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement as _, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, StyledImage as _, Window, div, img, prelude::FluentBuilder as _,
};

use crate::{ActiveTheme as _, StyledExt as _, aspect_ratio::AspectRatio};

/// One tile of an [`ImageGrid`].
pub struct ImageGridItem {
    source: Option<ImageSource>,
    element: Option<AnyElement>,
    alt: SharedString,
}

impl ImageGridItem {
    /// A tile showing an image. `alt` describes it for screen readers.
    pub fn image(source: impl Into<ImageSource>, alt: impl Into<SharedString>) -> Self {
        Self {
            source: Some(source.into()),
            element: None,
            alt: alt.into(),
        }
    }

    /// A tile showing any element, such as a video poster or a file icon.
    pub fn element(element: impl IntoElement, alt: impl Into<SharedString>) -> Self {
        Self {
            source: None,
            element: Some(element.into_any_element()),
            alt: alt.into(),
        }
    }
}

/// Images in equal tiles on a fixed number of columns.
///
/// Every tile keeps the same ratio and crops its image to fill. With
/// [`ImageGrid::max_visible`] the last tile shown carries a "+3" over it for
/// the images that do not fit, as attachment previews do. Pressing a tile
/// reports its index; tiles are buttons for the keyboard and screen readers
/// only when a click handler is set.
#[derive(IntoElement)]
pub struct ImageGrid {
    id: ElementId,
    items: Vec<ImageGridItem>,
    columns: u16,
    ratio: f32,
    max_visible: Option<usize>,
    label: Option<SharedString>,
    on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App)>>,
    style: StyleRefinement,
}

impl ImageGrid {
    /// An empty grid with three square columns.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            columns: 3,
            ratio: 1.,
            max_visible: None,
            label: None,
            on_click: None,
            style: StyleRefinement::default(),
        }
    }

    /// The number of columns, at least one. Default three.
    pub fn columns(mut self, columns: u16) -> Self {
        self.columns = columns.max(1);
        self
    }

    /// The width-to-height ratio of every tile. Default square.
    pub fn ratio(mut self, ratio: f32) -> Self {
        self.ratio = ratio;
        self
    }

    /// Show at most this many tiles; the last one shown counts the rest.
    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = Some(max.max(1));
        self
    }

    /// Name the grid for screen readers, such as "Attachments".
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Add a tile.
    pub fn item(mut self, item: ImageGridItem) -> Self {
        self.items.push(item);
        self
    }

    /// Add tiles.
    pub fn items(mut self, items: impl IntoIterator<Item = ImageGridItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Called with a tile's index when it is pressed.
    pub fn on_click(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// How many tiles show, and how many are counted on the last of them.
    pub fn visible_and_hidden(&self) -> (usize, usize) {
        let total = self.items.len();
        match self.max_visible {
            Some(max) if total > max => (max, total - max),
            _ => (total, 0),
        }
    }
}

impl Styled for ImageGrid {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ImageGrid {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (visible, hidden) = self.visible_and_hidden();
        let total = self.items.len();
        let ratio = self.ratio;
        let on_click = self.on_click.clone();
        let ring = theme.ring;
        let placeholder = theme.muted;
        let radius = theme.radius_lg;

        div()
            .id(self.id)
            .role(Role::List)
            .when_some(self.label, |this, label| this.aria_label(label))
            .grid()
            .grid_cols(self.columns)
            .gap_1p5()
            .w_full()
            .refine_style(&self.style)
            .children(
                self.items
                    .into_iter()
                    .take(visible)
                    .enumerate()
                    .map(|(ix, item)| {
                        let more = (ix + 1 == visible && hidden > 0).then_some(hidden);
                        let spoken = match more {
                            Some(more) => format!("{}, and {more} more", item.alt),
                            None => item.alt.to_string(),
                        };
                        let content: AnyElement = match (item.source, item.element) {
                            (Some(source), _) => img(source)
                                .size_full()
                                .object_fit(ObjectFit::Cover)
                                .into_any_element(),
                            (None, Some(element)) => element,
                            (None, None) => div().into_any_element(),
                        };
                        let on_click = on_click.clone();
                        div()
                            .id(ix)
                            .role(if on_click.is_some() {
                                Role::Button
                            } else {
                                Role::Image
                            })
                            .aria_label(spoken)
                            .aria_position_in_set(ix + 1)
                            .aria_size_of_set(total)
                            .rounded(radius)
                            .when_some(on_click, |this, on_click| {
                                this.tab_index(0)
                                    .cursor_pointer()
                                    .focus_visible(move |style| style.border_2().border_color(ring))
                                    .on_click(move |_, window, cx| on_click(&ix, window, cx))
                            })
                            .child(
                                AspectRatio::new(ratio)
                                    .bg(placeholder)
                                    .child(content)
                                    .when_some(more, |this, more| {
                                        this.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .bg(gpui::black().opacity(0.55))
                                                .text_lg()
                                                .font_medium()
                                                .text_color(gpui::white())
                                                .child(format!("+{more}")),
                                        )
                                    }),
                            )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(count: usize) -> ImageGrid {
        ImageGrid::new("g")
            .items((0..count).map(|ix| ImageGridItem::element(div(), format!("Photo {ix}"))))
    }

    #[test]
    fn hidden_tiles_are_counted_on_the_last_one() {
        assert_eq!(grid(5).visible_and_hidden(), (5, 0));
        assert_eq!(grid(5).max_visible(4).visible_and_hidden(), (4, 1));
        assert_eq!(grid(9).max_visible(4).visible_and_hidden(), (4, 5));
        assert_eq!(grid(3).max_visible(4).visible_and_hidden(), (3, 0));
        assert_eq!(grid(3).max_visible(0).visible_and_hidden(), (1, 2));
    }

    #[test]
    fn columns_are_at_least_one() {
        assert_eq!(ImageGrid::new("g").columns(0).columns, 1);
        assert_eq!(ImageGrid::new("g").columns(4).columns, 4);
    }
}
