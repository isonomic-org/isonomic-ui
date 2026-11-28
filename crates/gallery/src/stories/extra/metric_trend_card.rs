// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the metric trend card.

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window,
};
use isonomic_ui_extra::metric_trend_card::{MetricTrendCard, TrendSeries};

/// Shows a week of collections by customer group, and a card with nothing to plot.
pub struct MetricTrendCardStory;

impl MetricTrendCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

impl Story for MetricTrendCardStory {
    fn title() -> &'static str {
        "Metric trend card"
    }

    fn description() -> &'static str {
        "One headline number and how its parts moved."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for MetricTrendCardStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_4()
            .child(
                MetricTrendCard::new("collected", "Collected this week", "41.8k")
                    .delta(3350.)
                    .labels([
                        "12 May", "13 May", "14 May", "15 May", "16 May", "17 May", "18 May",
                    ])
                    .long_labels([
                        "Monday 12 May",
                        "Tuesday 13 May",
                        "Wednesday 14 May",
                        "Thursday 15 May",
                        "Friday 16 May",
                        "Saturday 17 May",
                        "Sunday 18 May",
                    ])
                    .series(TrendSeries::new(
                        "Retail",
                        [2900., 3150., 2700., 3400., 3900., 1800., 1350.],
                    ))
                    .series(TrendSeries::new(
                        "Wholesale",
                        [2100., 1750., 2450., 2300., 2650., 900., 600.],
                    ))
                    .series(TrendSeries::new(
                        "Subscriptions",
                        [1200., 1200., 1250., 1250., 1300., 1300., 1300.],
                    ))
                    .series(TrendSeries::new(
                        "Late fees",
                        [150., 400., 250., 500., 350., 100., 50.],
                    ))
                    .default_point(4)
                    .footer("Four customer groups, payments received per day"),
            )
            .child(
                MetricTrendCard::new("refunds", "Refunds this week", "0")
                    .delta(-420.)
                    .series(TrendSeries::new("Retail", [0., 0., 0.]))
                    .footer("No refunds were paid out"),
            )
    }
}
