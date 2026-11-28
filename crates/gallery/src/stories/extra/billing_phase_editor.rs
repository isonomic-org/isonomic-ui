// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the billing phase editor.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::billing_phase_editor::{
    BillingPhase, BillingPhaseEditor, BillingPhaseEvent, LineItem, money,
};

/// Shows a bookkeeping firm's two-phase contract with a client: a pilot with
/// a minimum, then the full service with a discount. The story keeps the
/// plan the editor reports.
pub struct BillingPhaseEditorStory {
    editor: Entity<BillingPhaseEditor>,
    last: SharedString,
    _events: Subscription,
}

impl BillingPhaseEditorStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let phases = vec![
                BillingPhase::new("pilot", "Pilot", "2027-01-01", 3)
                    .item(LineItem::new("books", "Monthly bookkeeping", 1, 90_000))
                    .item(LineItem::new("seats", "Client portal seats", 3, 2_500))
                    .minimum(120_000),
                BillingPhase::new("full", "Full service", "2027-04-01", 9)
                    .item(LineItem::new("books", "Monthly bookkeeping", 1, 90_000))
                    .item(LineItem::new("payroll", "Payroll runs", 2, 18_000))
                    .item(LineItem::new("seats", "Client portal seats", 8, 2_500))
                    .discount(10),
            ];
            let editor = cx.new(|cx| BillingPhaseEditor::new(phases, window, cx));
            let events = cx.subscribe(&editor, |this: &mut Self, _, event, cx| {
                let BillingPhaseEvent::Changed(phases) = event;
                let total: i64 = phases.iter().map(|phase| phase.totals().total).sum();
                this.last =
                    format!("Saved draft: {} phases, ${}", phases.len(), money(total)).into();
                cx.notify();
            });
            Self {
                editor,
                last: "Nothing changed yet".into(),
                _events: events,
            }
        })
    }
}

impl Story for BillingPhaseEditorStory {
    fn title() -> &'static str {
        "Billing phase editor"
    }

    fn description() -> &'static str {
        "The phases of a contract, each with its dates, line items, discount and minimum."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for BillingPhaseEditorStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().w_full().gap_3().child(self.editor.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
