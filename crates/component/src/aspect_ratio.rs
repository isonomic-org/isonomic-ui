// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! A box that keeps a width-to-height ratio, for media and previews.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _,
};
use smallvec::SmallVec;

use crate::{ActiveTheme as _, StyledExt as _};

/// A box whose height follows its width at a fixed ratio.
///
/// It fills the width it is given and sets its height from `ratio`
/// (width divided by height), so a 16:9 video or a square thumbnail keeps
/// its shape at any size. Its content is centred in the box, clipped to it,
/// and can fill it with `size_full`.
///
/// ```ignore
/// AspectRatio::new(16. / 9.).child(img("cover.png").size_full())
/// ```
#[derive(IntoElement)]
pub struct AspectRatio {
    ratio: f32,
    rounded: bool,
    style: StyleRefinement,
    children: SmallVec<[AnyElement; 1]>,
}

impl AspectRatio {
    /// A box with the given width-to-height ratio, such as `16. / 9.`.
    ///
    /// A ratio that is not a positive, finite number is treated as square.
    pub fn new(ratio: f32) -> Self {
        Self {
            ratio: Self::valid(ratio),
            rounded: true,
            style: StyleRefinement::default(),
            children: SmallVec::new(),
        }
    }

    /// A square box.
    pub fn square() -> Self {
        Self::new(1.)
    }

    /// A 16:9 box, for video.
    pub fn widescreen() -> Self {
        Self::new(16. / 9.)
    }

    /// A 4:3 box, for photos.
    pub fn photo() -> Self {
        Self::new(4. / 3.)
    }

    /// Round the corners with the theme's large radius. On by default.
    pub fn rounded(mut self, rounded: bool) -> Self {
        self.rounded = rounded;
        self
    }

    /// The ratio the box keeps, width divided by height.
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    fn valid(ratio: f32) -> f32 {
        if ratio.is_finite() && ratio > 0. {
            ratio
        } else {
            1.
        }
    }
}

impl Styled for AspectRatio {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for AspectRatio {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for AspectRatio {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let radius = cx.theme().radius_lg;
        div()
            .relative()
            .w_full()
            .aspect_ratio(self.ratio)
            .overflow_hidden()
            .when(self.rounded, |this| this.rounded(radius))
            .refine_style(&self.style)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(self.children),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Bounds, TestAppContext, VisualTestContext, px, size};

    #[test]
    fn invalid_ratios_fall_back_to_square() {
        assert_eq!(AspectRatio::new(0.).ratio(), 1.);
        assert_eq!(AspectRatio::new(-2.).ratio(), 1.);
        assert_eq!(AspectRatio::new(f32::NAN).ratio(), 1.);
        assert_eq!(AspectRatio::new(f32::INFINITY).ratio(), 1.);
        assert_eq!(AspectRatio::widescreen().ratio(), 16. / 9.);
        assert_eq!(AspectRatio::photo().ratio(), 4. / 3.);
        assert_eq!(AspectRatio::square().ratio(), 1.);
    }

    #[gpui::test]
    fn height_follows_width(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let window = cx.add_empty_window();
        let cx: &mut VisualTestContext = window;
        let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::default()));
        cx.draw(
            gpui::point(px(0.), px(0.)),
            size(px(320.), px(600.)),
            |_, _| {
                let bounds = bounds.clone();
                div().w(px(320.)).child(
                    AspectRatio::widescreen().child(
                        gpui::canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {}).size_full(),
                    ),
                )
            },
        );
        let measured = bounds.get();
        assert_eq!(measured.size.width, px(320.));
        assert_eq!(measured.size.height, px(180.));
    }
}
