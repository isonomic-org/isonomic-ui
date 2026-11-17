// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the metric change list.

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, px,
};
use isonomic_ui_extra::{
    common::Tone,
    metric_change_list::{MetricChange, MetricChangeList, TrendDirection},
};

/// Shows the list side by side and stacked for a narrow column.
pub struct MetricChangeListStory;

impl MetricChangeListStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }

    fn changes() -> Vec<MetricChange> {
        vec![
            MetricChange::new("Orders", "Up 9% to 1,204").tone(Tone::Success),
            MetricChange::new("Late deliveries", "Down to 18 from 31")
                .tone(Tone::Success)
                .direction(TrendDirection::Down),
            MetricChange::new("Refund requests", "Up 22% to 67")
                .tone(Tone::Danger)
                .direction(TrendDirection::Up),
            MetricChange::new("Average basket", "Holding at 48.20"),
        ]
    }
}

impl Story for MetricChangeListStory {
    fn title() -> &'static str {
        "Metric change list"
    }

    fn description() -> &'static str {
        "A short recap of what moved over a period, one metric per row."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for MetricChangeListStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_6()
            .child(
                MetricChangeList::new("wide")
                    .title("Since last week")
                    .period("7 days")
                    .changes(Self::changes()),
            )
            .child(
                MetricChangeList::new("narrow")
                    .w(px(260.))
                    .stacked(true)
                    .period("7 days")
                    .changes(Self::changes()),
            )
    }
}
