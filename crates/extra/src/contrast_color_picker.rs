// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Contrast colour picker: a palette of swatches that checks each one as text on a background, with the ratio and whether it passes.
//!
//! Every swatch shows a tick or a warning for readability against the
//! chosen background. The picked colour's panel shows sample text on that
//! background, its contrast ratio, and whether it passes for normal and
//! large text. Recently picked colours sit in a row of their own.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Toggled, Window, div, prelude::FluentBuilder as _, px,
};

use crate::common::{Tone, resting_shadow};

/// Relative luminance of a colour, from 0 for black to 1 for white.
pub fn luminance(color: Hsla) -> f32 {
    let rgb = color.to_rgb();
    let channel = |c: f32| {
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b)
}

/// The contrast ratio between two colours, from 1 to 21.
pub fn contrast_ratio(a: Hsla, b: Hsla) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// The swatch an arrow key moves to from `picked`, wrapping at the ends.
pub fn step_swatch(picked: Option<usize>, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let last = count - 1;
    Some(match (key, picked) {
        ("home", _) => 0,
        ("end", _) => last,
        ("right" | "down", None) => 0,
        ("left" | "up", None) => last,
        ("right" | "down", Some(ix)) => {
            if ix >= last {
                0
            } else {
                ix + 1
            }
        }
        ("left" | "up", Some(ix)) => {
            if ix == 0 {
                last
            } else {
                ix - 1
            }
        }
        _ => return None,
    })
}

/// Whether a ratio passes for normal text (4.5) and for large text (3).
pub fn passes(ratio: f32) -> (bool, bool) {
    (ratio >= 4.5, ratio >= 3.)
}

type PickHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Swatches checked for contrast against a background.
#[derive(IntoElement)]
pub struct ContrastColorPicker {
    id: ElementId,
    style: StyleRefinement,
    palette: Vec<(SharedString, Hsla)>,
    picked: Option<usize>,
    recent: Vec<usize>,
    background: Option<Hsla>,
    on_pick: Option<PickHandler>,
}

impl ContrastColorPicker {
    /// Create a picker over named colours.
    pub fn new(id: impl Into<ElementId>, palette: Vec<(SharedString, Hsla)>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            palette,
            picked: None,
            recent: Vec::new(),
            background: None,
            on_pick: None,
        }
    }

    /// The colour picked, by its place in the palette.
    pub fn picked(mut self, ix: Option<usize>) -> Self {
        self.picked = ix;
        self
    }

    /// Recently picked colours, newest first, by their place in the palette.
    pub fn recent(mut self, recent: Vec<usize>) -> Self {
        self.recent = recent;
        self
    }

    /// The background to check against. The theme's background by default.
    pub fn background(mut self, color: Hsla) -> Self {
        self.background = Some(color);
        self
    }

    /// Run when a swatch is picked.
    pub fn on_pick(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl Styled for ContrastColorPicker {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ContrastColorPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(
                ElementId::NamedChild(self.id.clone().into(), "focus".into()),
                cx,
                |_, cx| cx.focus_handle().tab_stop(true),
            )
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        let theme = cx.theme();
        let ring = theme.ring;
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let ok = Tone::Success.color(cx);
        let warn = Tone::Warning.color(cx);
        let bad = Tone::Danger.color(cx);
        let background = self.background.unwrap_or(fill);

        let swatch = |ix: usize, size: f32, key: &str| {
            let (name, color) = self.palette[ix].clone();
            let ratio = contrast_ratio(color, background);
            let (normal, large) = passes(ratio);
            let picked = self.picked == Some(ix);
            let pick = self.on_pick.clone();
            let main = key == "swatch";
            div()
                .id(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("{key}-{ix}").into(),
                ))
                .role(if main {
                    Role::RadioButton
                } else {
                    Role::Button
                })
                .relative()
                .size(px(size))
                .rounded(px(6.))
                .bg(color)
                .border_1()
                .border_color(line)
                .when(picked, |this| {
                    this.border_2()
                        .border_color(if focused && main { ring } else { ink })
                })
                .cursor_pointer()
                .aria_label(format!(
                    "{}{name}, contrast {ratio:.1} to 1, {}",
                    if main { "" } else { "Use again: " },
                    if normal {
                        "readable"
                    } else if large {
                        "large text only"
                    } else {
                        "not readable"
                    }
                ))
                .when(main, |this| {
                    this.aria_toggled(if picked {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                })
                .on_click(move |_, window, cx| {
                    if let Some(pick) = &pick {
                        pick(ix, window, cx);
                    }
                })
                .child(
                    div()
                        .absolute()
                        .right(px(-4.))
                        .bottom(px(-4.))
                        .size(px(14.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(fill)
                        .border_1()
                        .border_color(line)
                        .child(
                            Icon::new(if normal {
                                IconName::Check
                            } else {
                                IconName::TriangleAlert
                            })
                            .size(px(9.))
                            .text_color(if normal {
                                ok
                            } else if large {
                                warn
                            } else {
                                bad
                            }),
                        ),
                )
        };

        let panel =
            self.picked
                .and_then(|ix| self.palette.get(ix))
                .map(|(name, color)| {
                    let ratio = contrast_ratio(*color, background);
                    let (normal, large) = passes(ratio);
                    let verdict = |label: &'static str, pass: bool| {
                        h_flex()
                            .gap_1()
                            .text_caption()
                            .text_color(if pass { ok } else { bad })
                            .child(
                                Icon::new(if pass {
                                    IconName::Check
                                } else {
                                    IconName::Close
                                })
                                .xsmall(),
                            )
                            .child(label)
                    };
                    v_flex()
                        .id(ElementId::NamedChild(
                            self.id.clone().into(),
                            "verdict".into(),
                        ))
                        .role(Role::Status)
                        .aria_label(format!(
                            "{name}: {ratio:.2} to 1. Normal text {}, large text {}",
                            if normal { "passes" } else { "fails" },
                            if large { "passes" } else { "fails" }
                        ))
                        .gap_2()
                        .p_3()
                        .rounded(radius)
                        .border_1()
                        .border_color(line)
                        .child(
                            v_flex()
                                .gap_1()
                                .p_3()
                                .rounded(radius)
                                .bg(background)
                                .child(
                                    div()
                                        .text_color(*color)
                                        .text_lg()
                                        .font_medium()
                                        .child("Invoice 2291 is overdue"),
                                )
                                .child(
                                    div().text_color(*color).text_body().child(
                                        "Paid amounts show in this colour on this background.",
                                    ),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_3()
                                .child(div().font_medium().child(name.clone()))
                                .child(
                                    div()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(format!("{ratio:.2} : 1")),
                                )
                                .child(div().flex_1())
                                .child(verdict("Normal text", normal))
                                .child(verdict("Large text", large)),
                        )
                });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(460.))
            .gap_3()
            .p_4()
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().font_medium().child("Text colour"))
                    .child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child("Ticks pass for normal text"),
                    ),
            )
            .child({
                let pick = self.on_pick.clone();
                let picked = self.picked;
                let count = self.palette.len();
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        "swatches".into(),
                    ))
                    .role(Role::RadioGroup)
                    .aria_label("Text colour")
                    .track_focus(&focus)
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if let Some(next) = step_swatch(picked, count, event.keystroke.key.as_str())
                        {
                            cx.stop_propagation();
                            if let Some(pick) = &pick {
                                pick(next, window, cx);
                            }
                        }
                    })
                    .flex_wrap()
                    .gap_2p5()
                    .children((0..count).map(|ix| swatch(ix, 28., "swatch")))
            })
            .when(!self.recent.is_empty(), |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .child(div().text_caption().text_color(quiet).child("Recent"))
                        .children(
                            self.recent
                                .iter()
                                .filter(|ix| **ix < self.palette.len())
                                .take(6)
                                .map(|ix| swatch(*ix, 20., "recent")),
                        ),
                )
            })
            .children(panel)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use isonomic_ui::hsla;

    #[test]
    fn black_on_white_is_the_widest_contrast() {
        let ratio = contrast_ratio(hsla(0., 0., 0., 1.), hsla(0., 0., 1., 1.));
        assert!((ratio - 21.).abs() < 0.05);
        assert!((contrast_ratio(hsla(0., 0., 0.5, 1.), hsla(0., 0., 0.5, 1.)) - 1.).abs() < 0.001);
        assert_eq!(passes(4.6), (true, true));
        assert_eq!(passes(3.2), (false, true));
        assert_eq!(step_swatch(None, 4, "right"), Some(0));
        assert_eq!(step_swatch(Some(3), 4, "right"), Some(0));
        assert_eq!(step_swatch(Some(0), 4, "up"), Some(3));
        assert_eq!(step_swatch(Some(1), 4, "end"), Some(3));
        assert_eq!(step_swatch(Some(1), 4, "x"), None);
        assert_eq!(step_swatch(None, 0, "right"), None);
    }
}
