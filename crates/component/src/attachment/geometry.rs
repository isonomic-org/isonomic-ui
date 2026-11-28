// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The card's geometry per named size, and the upload bar that follows its
//! rounded bottom edge.

use gpui::{
    AbsoluteLength, App, Bounds, Hsla, IntoElement, Path, PathBuilder, Pixels, Styled as _, Window,
    canvas, point, px, rems,
};

use crate::Size;

/// The geometry a named size resolves to, in rems so it follows the root font size.
pub(super) struct CardMetrics {
    /// The height of a horizontal card and the side of a square image tile.
    pub(super) height: AbsoluteLength,
    /// The fixed width of a horizontal card that carries content.
    pub(super) chip_width: AbsoluteLength,
    /// The side of the square media slot of a horizontal card.
    pub(super) media: AbsoluteLength,
    /// The glyph size inside the media slot: an icon child without its own size,
    /// and the status glyphs.
    pub(super) media_glyph: AbsoluteLength,
    /// The padding before the media slot.
    pub(super) padding_start: AbsoluteLength,
    /// The padding after the content and actions.
    pub(super) padding_end: AbsoluteLength,
    /// The padding of a vertical card that carries content.
    pub(super) card_padding: AbsoluteLength,
    pub(super) gap: AbsoluteLength,
    pub(super) text: AbsoluteLength,
    pub(super) description: AbsoluteLength,
}

pub(super) fn card_metrics(size: Size) -> CardMetrics {
    let r = |value: f32| AbsoluteLength::Rems(rems(value));
    match size {
        Size::XSmall => CardMetrics {
            height: r(2.5),
            chip_width: r(11.),
            media: r(1.75),
            media_glyph: r(0.875),
            padding_start: r(0.25),
            padding_end: r(0.375),
            card_padding: r(0.25),
            gap: r(0.375),
            text: r(0.6875),
            description: r(0.625),
        },
        Size::Small => CardMetrics {
            height: r(3.),
            chip_width: r(12.5),
            media: r(2.),
            media_glyph: r(1.),
            padding_start: r(0.375),
            padding_end: r(0.5),
            card_padding: r(0.375),
            gap: r(0.5),
            text: r(0.75),
            description: r(0.6875),
        },
        Size::Medium => CardMetrics {
            height: r(3.5),
            chip_width: r(14.5),
            media: r(2.375),
            media_glyph: r(1.25),
            padding_start: r(0.5),
            padding_end: r(0.75),
            card_padding: r(0.5),
            gap: r(0.625),
            text: r(0.8125),
            description: r(0.75),
        },
        Size::Large => CardMetrics {
            height: r(4.),
            chip_width: r(17.),
            media: r(2.75),
            media_glyph: r(1.5),
            padding_start: r(0.625),
            padding_end: r(1.),
            card_padding: r(0.75),
            gap: r(0.75),
            text: r(0.875),
            description: r(0.8125),
        },
        // A custom density scales the medium geometry from its base value.
        Size::Size(value) => CardMetrics {
            height: (value * 3.5).into(),
            chip_width: (value * 14.5).into(),
            media: (value * 2.375).into(),
            media_glyph: (value * 1.25).into(),
            padding_start: (value * 0.5).into(),
            padding_end: (value * 0.75).into(),
            card_padding: (value * 0.5).into(),
            gap: (value * 0.625).into(),
            text: (value * 0.8125).into(),
            description: (value * 0.75).into(),
        },
    }
}

/// The upload bar's thickness.
const UPLOAD_BAR_THICKNESS: f32 = 2.;
/// The card's border width; the padding box's corners are this much tighter.
const CARD_BORDER: f32 = 1.;

/// The upload bar along a card's bottom edge. gpui clips rectangularly, so a
/// plain rectangle could not follow the corner curve; the bar is a filled path
/// whose ends trace the inner corner arcs, as CSS `overflow: hidden` would.
pub(super) fn upload_bar(percent: f32, radius: Pixels, color: Hsla) -> impl IntoElement {
    canvas(
        |_: Bounds<Pixels>, _: &mut Window, _: &mut App| (),
        move |bounds: Bounds<Pixels>, _: (), window: &mut Window, _: &mut App| {
            if let Some(path) = upload_bar_path(bounds, percent, radius) {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .inset_0()
}

/// The bar as the intersection of its rectangle with the card's inner rounded
/// rectangle: both ends are sampled along the corner arcs. An absolutely
/// positioned child is laid out in the card's padding box, so `bounds` already
/// sits inside the border; only the corner radius shrinks by the border width.
pub(super) fn upload_bar_path(
    bounds: Bounds<Pixels>,
    percent: f32,
    radius: Pixels,
) -> Option<Path<Pixels>> {
    const STEPS: usize = 6;
    let width = bounds.size.width.as_f32();
    let height = bounds.size.height.as_f32();
    let radius = (radius.as_f32() - CARD_BORDER).clamp(0., width.min(height) / 2.);
    let inner_left = 0.;
    let inner_right = width;
    let inner_bottom = height;
    let top = inner_bottom - UPLOAD_BAR_THICKNESS;
    let end = inner_left + (inner_right - inner_left) * (percent / 100.).clamp(0., 1.);
    if end <= inner_left || top <= 0. {
        return None;
    }
    // The corner circles' centre line; rows below it lie in the arcs.
    let centre_y = inner_bottom - radius;
    let arc_dx = |y: f32| {
        if radius > 0. && y > centre_y {
            (radius * radius - (y - centre_y) * (y - centre_y))
                .max(0.)
                .sqrt()
        } else {
            radius
        }
    };
    let left_at = |y: f32| inner_left + radius - arc_dx(y);
    let right_at = |y: f32| (inner_right - radius + arc_dx(y)).min(end);
    let at = |x: f32, y: f32| point(bounds.origin.x + px(x), bounds.origin.y + px(y));
    let rows = (0..=STEPS).map(|i| top + UPLOAD_BAR_THICKNESS * i as f32 / STEPS as f32);

    let mut builder = PathBuilder::fill();
    builder.move_to(at(left_at(top), top));
    // Down the right end, then back up the left arc.
    for y in rows.clone() {
        builder.line_to(at(right_at(y), y));
    }
    for y in rows.rev() {
        builder.line_to(at(left_at(y), y));
    }
    builder.build().ok()
}
