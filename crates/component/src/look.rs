// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The finished look every component shares: text roles, ink levels, surface
//! shadows and the hover fade.
//!
//! Components reach for these instead of picking sizes, alphas and shadows
//! themselves, so the whole library reads as one system. The values are the
//! ones `docs/design-language.md` records.

use gpui::{App, BoxShadow, ElementId, FontWeight, Hsla, Styled, Window, hsla, point, px};
use isonomic_ui_base::{
    Easing,
    motion::{Transition, transition},
};

use crate::ActiveTheme as _;

/// Text roles. Each sets size, line height and weight together, so a box is
/// exactly as tall as the design says and baselines line up across components.
pub trait TextRole: Styled + Sized {
    /// Running text: 13px on a 20px line. Leaves the weight alone, so it
    /// combines with a weight set before or after it.
    fn text_body(self) -> Self {
        self.text_size(px(13.)).line_height(px(20.))
    }

    /// A control's label, a row title, a card title: 13px on 20px, medium.
    fn text_label(self) -> Self {
        self.text_size(px(13.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
    }

    /// Secondary lines, hints, meta: 12px on 16px. Leaves the weight alone.
    fn text_caption(self) -> Self {
        self.text_size(px(12.)).line_height(px(16.))
    }

    /// A small control's label: 12px on 16px, medium.
    fn text_label_small(self) -> Self {
        self.text_size(px(12.))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
    }

    /// Counts and status pills: 11px on 16px, medium.
    fn text_badge(self) -> Self {
        self.text_size(px(11.))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
    }

    /// A section's small heading: 10px on 16px, medium, spaced. Pass the text
    /// in capitals; the element does not change case.
    fn text_eyebrow(self) -> Self {
        self.text_size(px(10.))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
    }

    /// A panel or dialog title: 15px on 22px, medium.
    fn text_title(self) -> Self {
        self.text_size(px(15.))
            .line_height(px(22.))
            .font_weight(FontWeight::MEDIUM)
    }
}

impl<T: Styled> TextRole for T {}

/// Ink levels, all taken from the theme's foreground so they hold in light
/// and dark.
pub mod ink {
    use super::*;

    /// Titles and values.
    pub fn strong(cx: &App) -> Hsla {
        cx.theme().foreground
    }

    /// Body text in a quiet control, ghost buttons, menu items at rest.
    pub fn muted(cx: &App) -> Hsla {
        let fg = cx.theme().foreground;
        fg.alpha(fg.a * 0.69)
    }

    /// Icons at rest.
    pub fn icon(cx: &App) -> Hsla {
        let fg = cx.theme().foreground;
        fg.alpha(
            fg.a * if cx.theme().mode.is_dark() {
                0.56
            } else {
                0.51
            },
        )
    }

    /// Hints, meta and placeholders.
    pub fn subtle(cx: &App) -> Hsla {
        cx.theme().muted_foreground
    }

    /// Shortcuts and counters.
    pub fn faint(cx: &App) -> Hsla {
        let fg = cx.theme().foreground;
        fg.alpha(fg.a * 0.32)
    }

    /// The fill a quiet control takes under the pointer.
    pub fn wash(cx: &App) -> Hsla {
        if cx.theme().mode.is_dark() {
            hsla(0., 0., 1., 0.06)
        } else {
            hsla(0., 0., 0., 0.03)
        }
    }

    /// The fill of a quiet control that is open or pressed, and of a
    /// selected menu row.
    pub fn wash_strong(cx: &App) -> Hsla {
        if cx.theme().mode.is_dark() {
            hsla(0., 0., 1., 0.08)
        } else {
            hsla(0., 0., 0., 0.05)
        }
    }

    /// A hairline: separators and the inset rule between a card's sections.
    pub fn hairline(cx: &App) -> Hsla {
        if cx.theme().mode.is_dark() {
            hsla(0., 0., 1., 0.08)
        } else {
            hsla(0., 0., 0., 0.06)
        }
    }
}

// GPUI treats a shadow's blur as the gaussian's deviation, where CSS treats it
// as twice that, so every blur below is half the CSS value it stands for.
fn layer(y: f32, blur: f32, spread: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(y)),
        blur_radius: px(blur / 2.),
        spread_radius: px(spread),
        inset: false,
    }
}

fn ring(width: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(width),
        inset: false,
    }
}

/// Surface shadows. Raised surfaces have no border: their edge is a 0.5px
/// ring and their depth a soft shadow. Empty when the theme turns shadows
/// off, except for the ring, which is the edge and stays.
pub mod surface {
    use super::*;

    fn dark(cx: &App) -> bool {
        cx.theme().mode.is_dark()
    }

    /// A card, a panel, a secondary button at rest.
    pub fn resting(cx: &App) -> Vec<BoxShadow> {
        if dark(cx) {
            let mut v = vec![ring(0.5, hsla(0., 0., 1., 0.10))];
            if cx.theme().shadow {
                v.push(layer(1., 2., 0., hsla(0., 0., 0., 0.4)));
            }
            v
        } else {
            let mut v = vec![ring(0.5, hsla(0., 0., 0., 0.06))];
            if cx.theme().shadow {
                v.push(layer(1., 1., -1., hsla(0., 0., 0., 0.10)));
                v.push(layer(1., 2., 0., hsla(0., 0., 0., 0.05)));
            }
            v
        }
    }

    /// The same surface under the pointer: the ring and shadow step up.
    pub fn resting_hover(cx: &App) -> Vec<BoxShadow> {
        if dark(cx) {
            let mut v = vec![ring(0.5, hsla(0., 0., 1., 0.16))];
            if cx.theme().shadow {
                v.push(layer(1., 2., 0., hsla(0., 0., 0., 0.4)));
            }
            v
        } else {
            let mut v = vec![ring(0.5, hsla(0., 0., 0., 0.10))];
            if cx.theme().shadow {
                v.push(layer(1., 1., -1., hsla(0., 0., 0., 0.12)));
                v.push(layer(1., 3., 0., hsla(0., 0., 0., 0.07)));
            }
            v
        }
    }

    /// A text field's box: a slightly firmer ring than a card.
    pub fn field(cx: &App) -> Vec<BoxShadow> {
        field_with(cx, 0.08, 0.14, 0.5)
    }

    /// A text field's box under the pointer.
    pub fn field_hover(cx: &App) -> Vec<BoxShadow> {
        field_with(cx, 0.13, 0.20, 0.5)
    }

    /// A text field's box with the caret in it: the ring firms to a full
    /// pixel. Fields show no focus halo.
    pub fn field_focus(cx: &App) -> Vec<BoxShadow> {
        field_with(cx, 0.28, 0.30, 1.)
    }

    fn field_with(cx: &App, light: f32, dark_alpha: f32, width: f32) -> Vec<BoxShadow> {
        if dark(cx) {
            let mut v = vec![ring(width, hsla(0., 0., 1., dark_alpha))];
            if cx.theme().shadow {
                v.push(layer(1., 2., 0., hsla(0., 0., 0., 0.4)));
            }
            v
        } else {
            let mut v = vec![ring(width, hsla(0., 0., 0., light))];
            if cx.theme().shadow {
                if width < 1. {
                    v.push(layer(1., 1., -1., hsla(0., 0., 0., 0.10 + (light - 0.08))));
                }
                v.push(layer(
                    1.,
                    2.,
                    0.,
                    hsla(0., 0., 0., 0.05 + (light - 0.08) * 0.2),
                ));
            }
            v
        }
    }

    /// A text field's box when its value is wrong.
    pub fn field_invalid(cx: &App) -> Vec<BoxShadow> {
        let danger = cx.theme().danger;
        vec![
            ring(1., danger.alpha(if dark(cx) { 0.5 } else { 0.55 })),
            layer(1., 2., 0., hsla(0., 0., 0., 0.05)),
        ]
    }

    /// Menus, popovers and tooltips floating over the page.
    pub fn floating(cx: &App) -> Vec<BoxShadow> {
        if dark(cx) {
            vec![
                ring(0.5, hsla(0., 0., 1., 0.12)),
                layer(12., 32., -8., hsla(0., 0., 0., 0.6)),
            ]
        } else {
            vec![
                ring(0.5, hsla(0., 0., 0., 0.10)),
                layer(12., 32., -8., hsla(0., 0., 0., 0.16)),
            ]
        }
    }

    /// The thumb of a switch or slider, the lifted pill of a segmented control.
    pub fn knob(cx: &App) -> Vec<BoxShadow> {
        let _ = cx;
        vec![
            layer(1., 2., 0., hsla(0., 0., 0., 0.18)),
            ring(0.5, hsla(0., 0., 0., 0.04)),
        ]
    }
}

/// Eases `id`'s hover from 0 (at rest) to 1 (under the pointer) over the
/// theme's fast duration, so fills, rings and ink fade instead of snapping.
///
/// `hovered` is the state now; the returned value trails it. Under reduced
/// motion the value jumps.
pub fn hover_fade(
    id: impl Into<ElementId>,
    hovered: bool,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    // The browser's plain `ease`, which is what a hover colour change rides on.
    let ease = Easing::cubic_bezier(0.25, 0.1, 0.25, 1.0).expect("static curve is valid");
    let policy = Transition::new(cx.theme().motion.duration_fast).easing(ease);
    transition(
        ElementId::NamedChild(std::sync::Arc::new(id.into()), "hover-fade".into()),
        if hovered { 1.0_f32 } else { 0.0 },
        policy,
        window,
        cx,
    )
}

/// Mixes two colours for a fade.
pub fn mix(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0., 1.);
    if t <= 0. {
        return from;
    }
    if t >= 1. {
        return to;
    }
    // Interpolate through a transparent end without dragging the hue to red.
    let (fh, fs, fl) = if from.a == 0. {
        (to.h, to.s, to.l)
    } else {
        (from.h, from.s, from.l)
    };
    let (th, ts, tl) = if to.a == 0. {
        (from.h, from.s, from.l)
    } else {
        (to.h, to.s, to.l)
    };
    hsla(
        fh + (th - fh) * t,
        fs + (ts - fs) * t,
        fl + (tl - fl) * t,
        from.a + (to.a - from.a) * t,
    )
}

/// Mixes two shadow stacks layer by layer. Stacks of different lengths take
/// the longer one's extra layers faded in or out.
pub fn mix_shadows(from: &[BoxShadow], to: &[BoxShadow], t: f32) -> Vec<BoxShadow> {
    let n = from.len().max(to.len());
    (0..n)
        .map(|ix| {
            let a = from.get(ix);
            let b = to.get(ix);
            match (a, b) {
                (Some(a), Some(b)) => BoxShadow {
                    color: mix(a.color, b.color, t),
                    offset: point(
                        a.offset.x + (b.offset.x - a.offset.x) * t,
                        a.offset.y + (b.offset.y - a.offset.y) * t,
                    ),
                    blur_radius: a.blur_radius + (b.blur_radius - a.blur_radius) * t,
                    spread_radius: a.spread_radius + (b.spread_radius - a.spread_radius) * t,
                    inset: a.inset,
                },
                (Some(a), None) => BoxShadow {
                    color: a.color.alpha(a.color.a * (1. - t)),
                    ..a.clone()
                },
                (None, Some(b)) => BoxShadow {
                    color: b.color.alpha(b.color.a * t),
                    ..b.clone()
                },
                (None, None) => unreachable!(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixing_reaches_both_ends() {
        let a = hsla(0., 0., 0., 0.06);
        let b = hsla(0., 0., 0., 0.10);
        assert_eq!(mix(a, b, 0.), a);
        assert_eq!(mix(a, b, 1.), b);
        assert!((mix(a, b, 0.5).a - 0.08).abs() < 1e-6);
    }

    #[test]
    fn a_fade_through_transparent_keeps_the_colour() {
        let clear = hsla(0., 0., 0., 0.);
        let blue = hsla(0.6, 0.8, 0.5, 1.);
        let half = mix(clear, blue, 0.5);
        assert_eq!(half.h, blue.h);
        assert!((half.a - 0.5).abs() < 1e-6);
    }

    #[test]
    fn shadow_stacks_of_different_lengths_fade_the_extra_layer() {
        let one = vec![ring(0.5, hsla(0., 0., 0., 0.06))];
        let two = vec![
            ring(0.5, hsla(0., 0., 0., 0.10)),
            layer(1., 2., 0., hsla(0., 0., 0., 0.05)),
        ];
        let mid = mix_shadows(&one, &two, 0.5);
        assert_eq!(mid.len(), 2);
        assert!((mid[1].color.a - 0.025).abs() < 1e-6);
    }

    #[test]
    fn css_blur_is_halved() {
        assert_eq!(layer(1., 2., 0., hsla(0., 0., 0., 1.)).blur_radius, px(1.));
    }
}
