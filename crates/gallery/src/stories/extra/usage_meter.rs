// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the usage meter.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::usage_meter::{
    UsageHistory, UsageLevel, UsageLimit, UsageMeter, UsageModel, UsagePart,
};

/// Tokens (thousands) a day for three made-up models over 18 days, with the
/// price per thousand tokens of each.
const MODEL_DAYS: [(&str, f64, [f64; 18]); 3] = [
    (
        "Large",
        0.012,
        [
            210., 260., 240., 90., 60., 330., 310., 360., 290., 340., 120., 80., 420., 460., 440.,
            480., 150., 110.,
        ],
    ),
    (
        "Medium",
        0.004,
        [
            320., 300., 340., 120., 90., 360., 380., 350., 400., 390., 150., 100., 410., 380.,
            420., 430., 160., 120.,
        ],
    ),
    (
        "Small",
        0.001,
        [
            520., 540., 600., 300., 240., 610., 650., 640., 700., 690., 310., 260., 720., 760.,
            740., 780., 330., 280.,
        ],
    ),
];

/// Shows a month with room left and a month that ran low, switched by period,
/// and under it a team's model spend over a month against its budget, by model,
/// on pace to run over.
pub struct UsageMeterStory {
    period: SharedString,
    last: SharedString,
}

impl UsageMeterStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            period: "this".into(),
            last: "Nothing pressed yet".into(),
        })
    }
}

impl Story for UsageMeterStory {
    fn title() -> &'static str {
        "Usage meter"
    }

    fn description() -> &'static str {
        "How much of a plan's allowance is used, and on what."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for UsageMeterStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let periods = cx.entity();
        let actions = periods.clone();
        let this_month = self.period.as_ref() == "this";
        // (invoices, receipts, statements, exports) and the days of the period.
        let (amounts, days): ([f64; 4], Vec<Option<f64>>) = if this_month {
            (
                [1840., 960., 410., 130.],
                [
                    150., 210., 190., 40., 30., 260., 240., 280., 230., 250., 60., 50., 300., 310.,
                    330.,
                ]
                .into_iter()
                .map(Some)
                .chain([None; 6])
                .collect(),
            )
        } else {
            (
                [2380., 1270., 560., 210.],
                [
                    180., 220., 60., 40., 250., 270., 240., 260., 230., 70., 50., 280., 300., 290.,
                    270., 250., 80., 60., 310., 320., 104.,
                ]
                .into_iter()
                .map(Some)
                .collect(),
            )
        };

        v_flex()
            .gap_3()
            .child(
                UsageMeter::new("plan-usage", 5000.)
                    .title("Harbour Books")
                    .plan("Team plan")
                    .renews_in(Duration::from_secs(12 * 86_400))
                    .unit("documents")
                    .period("this", "This month")
                    .period("last", "Last month")
                    .selected_period(self.period.clone())
                    .part(UsagePart::new("Invoices read", amounts[0]))
                    .part(UsagePart::new("Receipts matched", amounts[1]))
                    .part(UsagePart::new("Statements imported", amounts[2]))
                    .part(UsagePart::new("Exports", amounts[3]))
                    .daily(days)
                    .limit(UsageLimit::new("Seats", 9.).cap(10.))
                    .limit(UsageLimit::new("Storage", 2.4).cap(8.).unit("GB"))
                    .limit(UsageLimit::new("Ledgers", 6.))
                    .on_period_change(move |id, _, cx| {
                        let id = id.clone();
                        periods.update(cx, |this, cx| {
                            this.period = id;
                            cx.notify();
                        })
                    })
                    .on_action(move |level, _, cx| {
                        let line = match level {
                            UsageLevel::Fine => "Pressed with room left",
                            UsageLevel::Low => "Pressed while running low",
                            UsageLevel::Out => "Pressed with nothing left",
                        };
                        actions.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
            .child({
                let theme = cx.theme();
                let colors = [theme.chart_1, theme.chart_2, theme.chart_3];
                let days: Vec<String> = (1..=30).map(|d| format!("Jun {d}")).collect();
                let history = MODEL_DAYS.iter().fold(
                    UsageHistory::new(days).budget(140.),
                    |history, (name, price, days)| {
                        history.model(UsageModel::new(
                            *name,
                            days.iter().map(|k| (k * 1000., k * price)),
                        ))
                    },
                );
                UsageMeter::new("model-spend", 4_000_000.)
                    .title("Research team")
                    .plan("Model usage, June")
                    .renews_in(Duration::from_secs(12 * 86_400))
                    .unit("tokens")
                    .part(UsagePart::new("Large", 4_350_000. * 0.2).color(colors[0]))
                    .part(UsagePart::new("Medium", 4_720_000. * 0.2).color(colors[1]))
                    .part(UsagePart::new("Small", 9_870_000. * 0.2).color(colors[2]))
                    .history(history)
            })
    }
}
