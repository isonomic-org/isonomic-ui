// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the variant comparison card.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::{
    common::Tone,
    variant_comparison_card::{ComparisonMetric, ComparisonVariant, VariantComparisonCard},
};

/// Shows a reminder wording test, open, and reports picks and the action.
pub struct VariantComparisonCardStory {
    last: SharedString,
}

impl VariantComparisonCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            last: "Nothing picked yet".into(),
        })
    }
}

impl Story for VariantComparisonCardStory {
    fn title() -> &'static str {
        "Variant comparison card"
    }

    fn description() -> &'static str {
        "Where an experiment stands, and which way to go."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for VariantComparisonCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let picks = cx.entity();
        let actions = picks.clone();

        v_flex()
            .gap_3()
            .child(
                VariantComparisonCard::new("reminder-wording", "Reminder email wording")
                    .status("Collecting", Tone::Success)
                    .note("Week 3 of 4")
                    .default_open(true)
                    .metric(ComparisonMetric::new("Paid in 7 days").format(|v| format!("{v:.1}%")))
                    .metric(ComparisonMetric::new("Replies").format(|v| format!("{v:.1}%")))
                    .metric(
                        ComparisonMetric::new("Days to pay")
                            .format(|v| format!("{v:.1} d"))
                            .lower_is_better(true),
                    )
                    .variant(
                        ComparisonVariant::new("plain", "A", "Plain reminder")
                            .values([31.4, 6.2, 11.8])
                            .control(true)
                            .sample(1840),
                    )
                    .variant(
                        ComparisonVariant::new("due-date", "B", "Leads with the due date")
                            .values([36.9, 5.8, 9.9])
                            .sample(1812)
                            .certainty([96., 61., 93.]),
                    )
                    .variant(
                        ComparisonVariant::new("pay-link", "C", "Pay link at the top")
                            .values([33.0, 4.1, 11.2])
                            .sample(1795)
                            .certainty([72., 88., 64.]),
                    )
                    .on_pick(move |id, _, cx| {
                        let line = match id {
                            Some(id) => format!("Picked {id}"),
                            None => "Pick taken back".to_string(),
                        };
                        picks.update(cx, |this, cx| {
                            this.last = line.into();
                            cx.notify();
                        })
                    })
                    .on_action(move |id, _, cx| {
                        let line = format!("Going with {id}");
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
    }
}
