// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the approval stack.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window,
};
use isonomic_ui_extra::approval_stack::{
    ApprovalStack, PendingRequest, RequestRisk, StackDecision,
};

/// Five requests from bookkeeping assistants. The story takes the front card
/// off on each decision and puts it back on undo.
pub struct ApprovalStackStory {
    waiting: Vec<PendingRequest>,
    history: Vec<(PendingRequest, StackDecision)>,
}

fn requests() -> Vec<PendingRequest> {
    vec![
        PendingRequest::new(
            "pay",
            "Payables assistant",
            "Pay Harbour Supply $1,240.50",
            RequestRisk::High,
        )
        .detail("Invoice", "2291, due 14 days ago")
        .detail("From", "Operating, \u{2026}4417")
        .detail("When", "Tomorrow, 09:00")
        .asked("6 min ago"),
        PendingRequest::new(
            "void",
            "Billing assistant",
            "Void invoice 2240",
            RequestRisk::High,
        )
        .detail("Reason", "Issued twice; 2241 was paid")
        .asked("22 min ago"),
        PendingRequest::new(
            "email",
            "Collections",
            "Send a second reminder to Northwind Dental",
            RequestRisk::Medium,
        )
        .detail("Owed", "$3,120.00, 41 days")
        .detail("Tone", "Firm")
        .asked("1 h ago"),
        PendingRequest::new(
            "rule",
            "Tidy-up assistant",
            "File Shell receipts under Vehicle costs from now on",
            RequestRisk::Low,
        )
        .asked("2 h ago"),
        PendingRequest::new(
            "merge",
            "Contacts assistant",
            "Merge two contacts named Dana Whitlock",
            RequestRisk::Low,
        )
        .asked("3 h ago"),
    ]
}

impl ApprovalStackStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            waiting: requests(),
            history: Vec::new(),
        })
    }
}

impl Story for ApprovalStackStory {
    fn title() -> &'static str {
        "Approval stack"
    }

    fn description() -> &'static str {
        "Pending requests as a deck of cards to approve, edit or reject."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ApprovalStackStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let decide = cx.entity();
        let undo = cx.entity();
        let last = self
            .history
            .last()
            .map(|(request, decision)| (SharedString::from(request.action.to_string()), *decision));
        ApprovalStack::new("approvals", self.waiting.clone())
            .decided(self.history.len())
            .last(last)
            .on_decide(move |id, decision, _, cx| {
                decide.update(cx, |this, cx| {
                    if let Some(at) = this.waiting.iter().position(|r| &r.id == id) {
                        let request = this.waiting.remove(at);
                        this.history.push((request, decision));
                    }
                    cx.notify();
                })
            })
            .on_undo(move |_, cx| {
                undo.update(cx, |this, cx| {
                    if let Some((request, _)) = this.history.pop() {
                        this.waiting.insert(0, request);
                    }
                    cx.notify();
                })
            })
    }
}
