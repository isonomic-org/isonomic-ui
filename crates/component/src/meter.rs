// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! A meter: a measured value within a known range, such as disk use or a
//! score. Unlike a progress bar it does not show a task moving towards done.

use gpui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, relative,
};

use crate::{ActiveTheme as _, Sizable, Size, StyledExt as _, h_flex, v_flex};

/// How a meter's value reads against its thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterLevel {
    /// Within the good range.
    Good,
    /// Past the warning threshold.
    Warning,
    /// Past the critical threshold.
    Critical,
}

/// A bar that shows where a value sits in a range, with optional warning
/// and critical thresholds that colour it.
///
/// It is named for screen readers by its label and reports its value, range
/// and the text it shows, so "Storage, 72 GB of 100 GB" is read as one
/// meter.
///
/// ```ignore
/// Meter::new("storage", 72.)
///     .label("Storage")
///     .max(100.)
///     .value_text("72 GB of 100 GB")
///     .warning_at(80.)
///     .critical_at(95.)
/// ```
#[derive(IntoElement)]
pub struct Meter {
    id: ElementId,
    value: f64,
    min: f64,
    max: f64,
    warning_at: Option<f64>,
    critical_at: Option<f64>,
    higher_is_better: bool,
    label: Option<SharedString>,
    value_text: Option<SharedString>,
    show_value: bool,
    size: Size,
    style: StyleRefinement,
}

impl Meter {
    /// A meter showing `value` in the range 0 to 100.
    pub fn new(id: impl Into<ElementId>, value: f64) -> Self {
        Self {
            id: id.into(),
            value,
            min: 0.,
            max: 100.,
            warning_at: None,
            critical_at: None,
            higher_is_better: false,
            label: None,
            value_text: None,
            show_value: true,
            size: Size::Medium,
            style: StyleRefinement::default(),
        }
    }

    /// The lowest value. Default 0.
    pub fn min(mut self, min: f64) -> Self {
        self.min = min;
        self
    }

    /// The highest value. Default 100.
    pub fn max(mut self, max: f64) -> Self {
        self.max = max;
        self
    }

    /// The value at which the meter turns to the warning colour.
    pub fn warning_at(mut self, value: f64) -> Self {
        self.warning_at = Some(value);
        self
    }

    /// The value at which the meter turns to the danger colour.
    pub fn critical_at(mut self, value: f64) -> Self {
        self.critical_at = Some(value);
        self
    }

    /// Read the thresholds downwards: the meter warns as the value falls
    /// below them, as for battery or a score. Off by default, where higher
    /// values are worse, as for disk use.
    pub fn higher_is_better(mut self, higher_is_better: bool) -> Self {
        self.higher_is_better = higher_is_better;
        self
    }

    /// The meter's name, shown above the bar and read by screen readers.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The value as people should read it, such as "72 GB of 100 GB".
    /// Without it the meter shows a percentage of the range.
    pub fn value_text(mut self, text: impl Into<SharedString>) -> Self {
        self.value_text = Some(text.into());
        self
    }

    /// Show the value beside the label. On by default.
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// The share of the range the value fills, from 0 to 1.
    pub fn fraction(&self) -> f32 {
        let span = self.max - self.min;
        if !span.is_finite() || span <= 0. || !self.value.is_finite() {
            return 0.;
        }
        ((self.value - self.min) / span).clamp(0., 1.) as f32
    }

    /// How the value reads against the thresholds.
    pub fn level(&self) -> MeterLevel {
        let past = |threshold: Option<f64>| match threshold {
            Some(at) if self.higher_is_better => self.value <= at,
            Some(at) => self.value >= at,
            None => false,
        };
        if past(self.critical_at) {
            MeterLevel::Critical
        } else if past(self.warning_at) {
            MeterLevel::Warning
        } else {
            MeterLevel::Good
        }
    }

    /// The text shown beside the label and read by screen readers.
    pub fn display_value(&self) -> SharedString {
        self.value_text
            .clone()
            .unwrap_or_else(|| format!("{}%", (self.fraction() * 100.).round() as i64).into())
    }
}

impl Sizable for Meter {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for Meter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Meter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let fraction = self.fraction();
        let level = self.level();
        let display = self.display_value();
        let theme = cx.theme();
        let fill = match level {
            MeterLevel::Good => theme.progress_bar,
            MeterLevel::Warning => theme.warning,
            MeterLevel::Critical => theme.danger,
        };
        let height = match self.size {
            Size::XSmall | Size::Small => gpui::px(4.),
            Size::Large => gpui::px(10.),
            _ => gpui::px(6.),
        };
        let spoken = match &self.label {
            Some(label) => format!("{label}, {display}"),
            None => display.to_string(),
        };

        v_flex()
            .id(self.id)
            .role(Role::Meter)
            .aria_label(spoken)
            .aria_value(display.clone())
            .aria_numeric_value(self.value)
            .aria_min_numeric_value(self.min)
            .aria_max_numeric_value(self.max)
            .w_full()
            .gap_1p5()
            .refine_style(&self.style)
            .when(self.label.is_some() || self.show_value, |this| {
                this.child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .gap_2()
                        .text_sm()
                        .when_some(self.label.clone(), |this, label| {
                            this.child(div().text_color(theme.foreground).child(label))
                        })
                        .when(self.show_value, |this| {
                            this.child(
                                div()
                                    .text_color(theme.muted_foreground)
                                    .when(level != MeterLevel::Good, |this| this.text_color(fill))
                                    .child(display),
                            )
                        }),
                )
            })
            .child(
                div()
                    .w_full()
                    .h(height)
                    .rounded_full()
                    .bg(theme.progress_bar.opacity(0.15))
                    .overflow_hidden()
                    .child(div().h_full().w(relative(fraction)).rounded_full().bg(fill)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_clamps_to_the_range() {
        assert_eq!(Meter::new("m", 50.).fraction(), 0.5);
        assert_eq!(Meter::new("m", 150.).fraction(), 1.);
        assert_eq!(Meter::new("m", -5.).fraction(), 0.);
        assert_eq!(Meter::new("m", 30.).min(20.).max(40.).fraction(), 0.5);
        assert_eq!(Meter::new("m", 5.).min(10.).max(10.).fraction(), 0.);
        assert_eq!(Meter::new("m", f64::NAN).fraction(), 0.);
    }

    #[test]
    fn levels_follow_the_thresholds() {
        let disk = |v| Meter::new("m", v).warning_at(80.).critical_at(95.);
        assert_eq!(disk(50.).level(), MeterLevel::Good);
        assert_eq!(disk(80.).level(), MeterLevel::Warning);
        assert_eq!(disk(97.).level(), MeterLevel::Critical);

        let battery = |v| {
            Meter::new("m", v)
                .higher_is_better(true)
                .warning_at(20.)
                .critical_at(10.)
        };
        assert_eq!(battery(60.).level(), MeterLevel::Good);
        assert_eq!(battery(15.).level(), MeterLevel::Warning);
        assert_eq!(battery(5.).level(), MeterLevel::Critical);
    }

    #[test]
    fn display_value_prefers_the_given_text() {
        assert_eq!(Meter::new("m", 42.4).display_value().as_ref(), "42%");
        assert_eq!(
            Meter::new("m", 72.)
                .value_text("72 GB of 100 GB")
                .display_value()
                .as_ref(),
            "72 GB of 100 GB"
        );
    }
}
