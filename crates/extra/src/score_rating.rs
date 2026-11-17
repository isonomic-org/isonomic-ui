// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Score rating: a row of stars for giving or showing a score, in whole or half steps, with an optional count of ratings beside it.
//!
//! Clicking a star sets the score; with half steps, the left half of a star
//! sets half. Clicking the current score again clears it when clearing is
//! on. Read-only ratings draw the same stars without handlers, and a screen
//! reader hears the score as "3.5 out of 5".

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, Role, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

/// How much of one star a score fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarFill {
    /// The whole star.
    Full,
    /// The left half.
    Half,
    /// None of it.
    Empty,
}

/// Round a score to the steps allowed and keep it between zero and `max`.
pub fn snap_score(raw: f32, max: u8, half_steps: bool) -> f32 {
    let snapped = if half_steps {
        (raw * 2.0).round() / 2.0
    } else {
        raw.round()
    };
    snapped.clamp(0.0, f32::from(max))
}

/// How much of star `ix` (from 0) a score fills.
pub fn star_fill(score: f32, ix: usize) -> StarFill {
    let ix = ix as f32;
    if score >= ix + 1.0 {
        StarFill::Full
    } else if score >= ix + 0.5 {
        StarFill::Half
    } else {
        StarFill::Empty
    }
}

/// A score in words, such as "3.5 out of 5" or "Not rated".
pub fn spoken_score(score: f32, max: u8) -> String {
    if score <= 0.0 {
        return "Not rated".into();
    }
    let shown = if score.fract() == 0.0 {
        format!("{score:.0}")
    } else {
        format!("{score:.1}")
    };
    format!("{shown} out of {max}")
}

/// The score a key sets from `score`: arrows step by a half or a whole star,
/// Home clears, End fills, and a digit picks that many stars.
pub fn keyed_score(score: f32, max: u8, half_steps: bool, key: &str) -> Option<f32> {
    let step = if half_steps { 0.5 } else { 1.0 };
    let next = match key {
        "right" | "up" => score + step,
        "left" | "down" => score - step,
        "home" => 0.0,
        "end" => f32::from(max),
        digit => {
            let n: u8 = digit.parse().ok()?;
            if digit.len() != 1 || n > max {
                return None;
            }
            f32::from(n)
        }
    };
    Some(snap_score(next, max, half_steps))
}

type ScoreHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

/// A row of stars.
#[derive(IntoElement)]
pub struct ScoreRating {
    id: ElementId,
    style: StyleRefinement,
    score: f32,
    max: u8,
    half_steps: bool,
    clearable: bool,
    count: Option<usize>,
    large: bool,
    on_change: Option<ScoreHandler>,
}

impl ScoreRating {
    /// Create a rating showing `score` out of five.
    pub fn new(id: impl Into<ElementId>, score: f32) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            score,
            max: 5,
            half_steps: false,
            clearable: true,
            count: None,
            large: false,
            on_change: None,
        }
    }

    /// Use this many stars.
    pub fn max(mut self, max: u8) -> Self {
        self.max = max.max(1);
        self
    }

    /// Allow half stars.
    pub fn half_steps(mut self, on: bool) -> Self {
        self.half_steps = on;
        self
    }

    /// Whether clicking the current score clears it. On by default.
    pub fn clearable(mut self, on: bool) -> Self {
        self.clearable = on;
        self
    }

    /// Show how many ratings the score is drawn from.
    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Draw larger stars, for a form.
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    /// Run with the new score when a star is clicked. Without it the rating is read-only.
    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Styled for ScoreRating {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ScoreRating {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = self.on_change.is_some().then(|| {
            window
                .use_keyed_state(
                    ElementId::NamedChild(self.id.clone().into(), "focus".into()),
                    cx,
                    |_, cx| cx.focus_handle().tab_stop(true),
                )
                .read(cx)
                .clone()
        });
        let focused = focus.as_ref().is_some_and(|f| f.is_focused(window));
        let theme = cx.theme();
        let ring = theme.ring;
        let lit = theme.warning;
        let unlit = theme.muted_foreground.opacity(0.5);
        let quiet = theme.muted_foreground;
        let score = snap_score(self.score, self.max, self.half_steps);
        let size = if self.large { px(24.) } else { px(16.) };
        let interactive = self.on_change.is_some();

        let stars = (0..usize::from(self.max)).map(|ix| {
            let fill = star_fill(score, ix);
            let zone = |part: &str, value: f32, left: bool| {
                let change = self.on_change.clone();
                let next = if self.clearable && (value - score).abs() < f32::EPSILON {
                    0.0
                } else {
                    value
                };
                div()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("star-{ix}-{part}").into(),
                    ))
                    .absolute()
                    .top_0()
                    .h_full()
                    .w(if self.half_steps { size / 2. } else { size })
                    .when(left, |this| this.left_0())
                    .when(!left, |this| this.right_0())
                    .cursor_pointer()
                    .role(Role::RadioButton)
                    .aria_label(format!("Rate {}", spoken_score(value, self.max)))
                    .on_click(move |_, window, cx| {
                        if let Some(change) = &change {
                            change(next, window, cx);
                        }
                    })
            };
            div()
                .relative()
                .size(size)
                .flex_none()
                .child(Icon::new(IconName::Star).with_size(size).text_color(unlit))
                .when(fill != StarFill::Empty, |this| {
                    this.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .h_full()
                            .w(if fill == StarFill::Full {
                                size
                            } else {
                                size / 2.
                            })
                            .overflow_hidden()
                            .child(
                                Icon::new(IconName::StarFill)
                                    .with_size(size)
                                    .text_color(lit),
                            ),
                    )
                })
                .when(interactive && self.half_steps, |this| {
                    this.child(zone("half", ix as f32 + 0.5, true)).child(zone(
                        "whole",
                        ix as f32 + 1.0,
                        false,
                    ))
                })
                .when(interactive && !self.half_steps, |this| {
                    this.child(zone("whole", ix as f32 + 1.0, true))
                })
        });

        h_flex()
            .id(self.id.clone())
            .gap_0p5()
            .role(if interactive {
                Role::Slider
            } else {
                Role::Image
            })
            .when(interactive, |this| {
                this.aria_numeric_value(f64::from(score))
                    .aria_min_numeric_value(0.0)
                    .aria_max_numeric_value(f64::from(self.max))
                    .aria_numeric_value_step(if self.half_steps { 0.5 } else { 1.0 })
            })
            .when_some(focus, |this, focus| this.track_focus(&focus))
            .when(focused, |this| {
                this.rounded(px(4.)).border_1().border_color(ring)
            })
            .when_some(self.on_change.clone(), |this, change| {
                let (max, half) = (self.max, self.half_steps);
                this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if let Some(next) = keyed_score(score, max, half, event.keystroke.key.as_str())
                    {
                        cx.stop_propagation();
                        change(next, window, cx);
                    }
                })
            })
            .aria_label(match self.count {
                Some(count) => format!("{}, from {count} ratings", spoken_score(score, self.max)),
                None => spoken_score(score, self.max),
            })
            .children(stars)
            .when_some(self.count, |this, count| {
                this.child(div().ml_1().text_sm().text_color(quiet).child(format!(
                    "{} ({count})",
                    if score > 0.0 {
                        format!("{score:.1}")
                    } else {
                        "\u{2013}".into()
                    }
                )))
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_snap_and_fill() {
        assert_eq!(snap_score(3.3, 5, true), 3.5);
        assert_eq!(snap_score(3.3, 5, false), 3.0);
        assert_eq!(snap_score(9.0, 5, false), 5.0);
        assert_eq!(star_fill(3.5, 2), StarFill::Full);
        assert_eq!(star_fill(3.5, 3), StarFill::Half);
        assert_eq!(star_fill(3.5, 4), StarFill::Empty);
        assert_eq!(spoken_score(3.5, 5), "3.5 out of 5");
        assert_eq!(spoken_score(4.0, 5), "4 out of 5");
        assert_eq!(spoken_score(0.0, 5), "Not rated");
        assert_eq!(keyed_score(3.0, 5, true, "right"), Some(3.5));
        assert_eq!(keyed_score(3.0, 5, false, "left"), Some(2.0));
        assert_eq!(keyed_score(5.0, 5, false, "right"), Some(5.0));
        assert_eq!(keyed_score(2.0, 5, false, "home"), Some(0.0));
        assert_eq!(keyed_score(2.0, 5, false, "end"), Some(5.0));
        assert_eq!(keyed_score(2.0, 5, false, "4"), Some(4.0));
        assert_eq!(keyed_score(2.0, 5, false, "7"), None);
        assert_eq!(keyed_score(2.0, 5, false, "x"), None);
    }
}
