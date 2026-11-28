// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the cost breakdown waterfall.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window,
};
use isonomic_ui_extra::cost_breakdown_waterfall::{CostBreakdownWaterfall, CostStep};

/// What a March reconciliation run cost, step by step. The story keeps the
/// measure, the open steps and the selection.
pub struct CostBreakdownWaterfallStory {
    tokens: bool,
    open: Vec<SharedString>,
    selected: Option<SharedString>,
}

fn run() -> Vec<CostStep> {
    vec![
        CostStep::new("plan", "Plan the close", 1_850, 6_400),
        CostStep::new("recon", "Reconciler", 900, 3_100)
            .child(CostStep::new("cards", "Card matcher", 11_200, 41_800))
            .child(CostStep::new("bank", "Bank matcher", 6_300, 22_900))
            .child(CostStep::new("post", "Post matches", 420, 1_500)),
        CostStep::new("payables", "Payables assistant", 4_100, 14_700),
        CostStep::new("reminders", "Draft reminders", 3_600, 12_900),
        CostStep::new("note", "Write the close note", 1_200, 4_300),
    ]
}

impl CostBreakdownWaterfallStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            tokens: false,
            open: vec!["recon".into()],
            selected: Some("cards".into()),
        })
    }
}

impl Story for CostBreakdownWaterfallStory {
    fn title() -> &'static str {
        "Cost breakdown waterfall"
    }

    fn description() -> &'static str {
        "What each step of a run cost, stacked up to the total."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CostBreakdownWaterfallStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.entity();
        let toggle = cx.entity();
        let measure = cx.entity();
        CostBreakdownWaterfall::new("march-cost", run())
            .tokens(self.tokens)
            .open(self.open.clone())
            .selected(self.selected.clone())
            .on_select(move |id, _, cx| {
                select.update(cx, |this, cx| {
                    this.selected = Some(id.clone());
                    cx.notify();
                })
            })
            .on_toggle(move |id, _, cx| {
                toggle.update(cx, |this, cx| {
                    if let Some(at) = this.open.iter().position(|o| o == id) {
                        this.open.remove(at);
                    } else {
                        this.open.push(id.clone());
                    }
                    cx.notify();
                })
            })
            .on_measure(move |tokens, _, cx| {
                measure.update(cx, |this, cx| {
                    this.tokens = tokens;
                    cx.notify();
                })
            })
    }
}
