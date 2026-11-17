// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the segmented switcher.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div, px,
};
use isonomic_ui_extra::segmented_switcher::{Segment, SegmentedSwitcher};

/// Filtering invoices by status, and switching a report between periods.
pub struct SegmentedSwitcherStory {
    status: usize,
    period: usize,
}

impl SegmentedSwitcherStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            status: 1,
            period: 0,
        })
    }
}

impl Story for SegmentedSwitcherStory {
    fn title() -> &'static str {
        "Segmented switcher"
    }

    fn description() -> &'static str {
        "A short row of joined choices where exactly one is on, each with an optional icon and count."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SegmentedSwitcherStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let status = cx.entity();
        let period = cx.entity();
        let statuses = ["All", "Unpaid", "Overdue", "Paid", "Void"];
        v_flex()
            .gap_4()
            .child(
                SegmentedSwitcher::new(
                    "invoice-status",
                    vec![
                        Segment::new("All").count(1250),
                        Segment::new("Unpaid").count(12),
                        Segment::new("Overdue").count(3),
                        Segment::new("Paid").count(1231),
                        Segment::new("Void").count(4).disabled(true),
                    ],
                    self.status,
                )
                .label("Invoice status")
                .on_select(move |ix, _, cx| {
                    status.update(cx, |this, cx| {
                        this.status = ix;
                        cx.notify();
                    })
                }),
            )
            .child(div().text_xs().text_color(quiet).child(format!(
                "Showing {} invoices.",
                statuses[self.status].to_lowercase()
            )))
            .child(
                div().w(px(360.)).child(
                    SegmentedSwitcher::new(
                        "report-period",
                        vec![
                            Segment::new("Month").icon(IconName::Calendar),
                            Segment::new("Quarter"),
                            Segment::new("Year"),
                        ],
                        self.period,
                    )
                    .label("Report period")
                    .fill()
                    .on_select(move |ix, _, cx| {
                        period.update(cx, |this, cx| {
                            this.period = ix;
                            cx.notify();
                        })
                    }),
                ),
            )
    }
}
