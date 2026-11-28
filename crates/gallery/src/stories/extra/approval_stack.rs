// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the approval stack.

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window,
};
use isonomic_ui_extra::approval_stack::{
    ApprovalStack, GrantScope, PendingRequest, RequestRisk, grant_note, reject_note,
};

/// One undoable step: the requests it took off, where they were, and the
/// words for the undo line.
struct Step {
    taken: Vec<(usize, PendingRequest)>,
    note: SharedString,
}

/// A deck and a queue of requests that the story owns.
#[derive(Default)]
struct Pile {
    waiting: Vec<PendingRequest>,
    history: Vec<Step>,
}

impl Pile {
    fn new(waiting: Vec<PendingRequest>) -> Self {
        Self {
            waiting,
            history: Vec::new(),
        }
    }

    /// Take these requests off and remember how to put them back.
    fn take(&mut self, ids: &[SharedString], note: impl Fn(&[SharedString]) -> String) {
        let mut taken = Vec::new();
        let mut actions = Vec::new();
        for id in ids {
            if let Some(at) = self.waiting.iter().position(|r| &r.id == id) {
                let request = self.waiting.remove(at);
                actions.push(request.action.clone());
                taken.push((at, request));
            }
        }
        if !taken.is_empty() {
            self.history.push(Step {
                taken,
                note: note(&actions).into(),
            });
        }
    }

    fn undo(&mut self) {
        if let Some(step) = self.history.pop() {
            for (at, request) in step.taken.into_iter().rev() {
                let at = at.min(self.waiting.len());
                self.waiting.insert(at, request);
            }
        }
    }

    fn decided(&self) -> usize {
        self.history.iter().map(|step| step.taken.len()).sum()
    }

    fn note(&self) -> Option<SharedString> {
        self.history.last().map(|step| step.note.clone())
    }
}

/// Requests from bookkeeping assistants and from a coding assistant's tools.
/// The deck grants with a scope; the queue triages by risk and acts on many
/// at once. Every step can be undone.
pub struct ApprovalStackStory {
    deck: Pile,
    queue: Pile,
}

/// Tool calls a coding assistant waits on, in the order they came.
fn tool_requests() -> Vec<PendingRequest> {
    vec![
        PendingRequest::new(
            "read",
            "Code assistant",
            "Read src/pump.rs",
            RequestRisk::Low,
        )
        .asked("just now"),
        PendingRequest::new(
            "push",
            "Code assistant",
            "Force-push to the release branch",
            RequestRisk::High,
        )
        .asked("1 min ago"),
        PendingRequest::new(
            "install",
            "Code assistant",
            "Install two packages from the registry",
            RequestRisk::Medium,
        )
        .asked("1 min ago"),
        PendingRequest::new(
            "tests",
            "Code assistant",
            "Run the test suite",
            RequestRisk::Low,
        )
        .asked("2 min ago"),
        PendingRequest::new(
            "drop",
            "Code assistant",
            "Drop the staging database table",
            RequestRisk::High,
        )
        .asked("3 min ago"),
        PendingRequest::new(
            "fetch",
            "Code assistant",
            "Fetch the pump vendor's manual",
            RequestRisk::Medium,
        )
        .asked("4 min ago"),
        PendingRequest::new(
            "list",
            "Code assistant",
            "List files in the docs folder",
            RequestRisk::Low,
        )
        .asked("4 min ago"),
    ]
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
            deck: Pile::new(requests()),
            queue: Pile::new(tool_requests()),
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
        let view = cx.entity();
        let pile = move |queue: bool| {
            let view = view.clone();
            move |cx: &mut App, change: &dyn Fn(&mut Pile)| {
                view.update(cx, |this, cx| {
                    change(if queue {
                        &mut this.queue
                    } else {
                        &mut this.deck
                    });
                    cx.notify();
                })
            }
        };
        let stack = |queue: bool, data: &Pile| {
            let decide = pile(queue);
            let grant = pile(queue);
            let reject = pile(queue);
            let undo = pile(queue);
            ApprovalStack::new(
                if queue { "tool-queue" } else { "approvals" },
                data.waiting.clone(),
            )
            .decided(data.decided())
            .note(data.note())
            .grant_scopes(true)
            .queue(queue)
            .on_decide(move |id, decision, _, cx| {
                let id = id.clone();
                decide(cx, &move |pile| {
                    pile.take(std::slice::from_ref(&id), |actions| {
                        format!(
                            "{}: {}",
                            isonomic_ui_extra::approval_stack::decision_words(decision),
                            actions.join(", ")
                        )
                    })
                })
            })
            .on_grant(move |ids, scope: GrantScope, _, cx| {
                let ids = ids.to_vec();
                grant(cx, &move |pile| {
                    pile.take(&ids, |actions| grant_note(actions, scope))
                })
            })
            .on_reject_many(move |ids, _, cx| {
                let ids = ids.to_vec();
                reject(cx, &move |pile| pile.take(&ids, reject_note))
            })
            .on_undo(move |_, cx| undo(cx, &|pile| pile.undo()))
        };
        v_flex()
            .gap_6()
            .child(stack(false, &self.deck))
            .child(stack(true, &self.queue))
    }
}
