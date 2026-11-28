// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the stacked breakdown chart.

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Window,
};
use isonomic_ui_extra::stacked_breakdown_chart::{
    BreakdownDimension, BreakdownMeasure, BreakdownRecord, StackedBreakdownChart,
};

/// Shows this quarter's invoices by state and customer group, by count or by amount.
pub struct StackedBreakdownChartStory;

impl StackedBreakdownChartStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

/// How many invoices each state has per customer group, and their usual amount.
const INVOICES: [(&str, [usize; 3], f64); 4] = [
    ("Draft", [6, 2, 1], 180.),
    ("Sent", [14, 9, 4], 240.),
    ("Overdue", [5, 7, 0], 410.),
    ("Paid", [31, 18, 12], 220.),
];
const GROUPS: [&str; 3] = ["Retail", "Wholesale", "Subscription"];

impl Story for StackedBreakdownChartStory {
    fn title() -> &'static str {
        "Stacked breakdown chart"
    }

    fn description() -> &'static str {
        "One set of records, split the way the reader asks."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for StackedBreakdownChartStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let records = INVOICES.iter().flat_map(|(state, counts, amount)| {
            counts.iter().zip(GROUPS).flat_map(move |(count, group)| {
                (0..*count).map(move |n| {
                    BreakdownRecord::new()
                        .field("state", *state)
                        .field("group", group)
                        .number("amount", amount + 15. * n as f64)
                })
            })
        });

        v_flex().child(
            StackedBreakdownChart::new("invoices", "Invoices this quarter")
                .records(records)
                .dimension(
                    BreakdownDimension::new("state", "State")
                        .value("Draft")
                        .value("Sent")
                        .value("Overdue")
                        .value("Paid"),
                )
                .dimension(
                    BreakdownDimension::new("group", "Customer group")
                        .value("Retail")
                        .value("Wholesale")
                        .value("Subscription"),
                )
                .measure(BreakdownMeasure::count("Invoices", "invoices"))
                .measure(BreakdownMeasure::sum("Amount", "billed", "amount")),
        )
    }
}
