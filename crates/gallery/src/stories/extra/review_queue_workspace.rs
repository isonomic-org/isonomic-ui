//! Gallery story for the review queue workspace.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::review_queue_workspace::{
    Proposal, ProposalRisk, ProposedChange, ReviewQueueWorkspace, next_open,
};

/// Shows a morning's proposals from a bookkeeping assistant. The story keeps
/// the decisions and moves on to the next open proposal after each one.
pub struct ReviewQueueWorkspaceStory {
    proposals: Vec<Proposal>,
    current: Option<SharedString>,
}

fn proposals() -> Vec<Proposal> {
    vec![
        Proposal::new(
            "pay-harbour",
            "Pay Harbour Supply 1,240.50",
            ProposalRisk::High,
        )
        .reason("Invoice 2291 is 14 days overdue and matches the purchase order.")
        .from("Payables assistant", "6 min", 2)
        .change(ProposedChange::new("Status", Some("Unpaid"), "Scheduled"))
        .change(ProposedChange::new("Pay on", None, "Tomorrow, 09:00"))
        .change(ProposedChange::new(
            "From account",
            None,
            "Operating, \u{2026}4417",
        )),
        Proposal::new("void-2240", "Void invoice 2240", ProposalRisk::High)
            .reason("It was issued twice; 2241 carries the same lines and was paid.")
            .from("Billing assistant", "22 min", 1)
            .change(ProposedChange::new("Status", Some("Open"), "Void")),
        Proposal::new(
            "merge-dana",
            "Merge two contacts named Dana Whitlock",
            ProposalRisk::Medium,
        )
        .reason("Same email address and the same company.")
        .from("Contacts assistant", "1 h", 3)
        .change(ProposedChange::new(
            "Phone",
            Some("(none)"),
            "+1 716 555 0148",
        ))
        .change(ProposedChange::new("Invoices", Some("4 and 2"), "6")),
        Proposal::new(
            "recat-fuel",
            "Move 3 fuel receipts to Vehicle costs",
            ProposalRisk::Low,
        )
        .reason("They were filed under Office supplies.")
        .from("Tidy-up assistant", "2 h", 4)
        .change(ProposedChange::new(
            "Category",
            Some("Office supplies"),
            "Vehicle costs",
        )),
        Proposal::new(
            "rename-tag",
            "Rename the tag 'misc' to 'Sundries'",
            ProposalRisk::Low,
        )
        .from("Tidy-up assistant", "3 h", 5)
        .change(ProposedChange::new("Tag", Some("misc"), "Sundries")),
    ]
}

impl ReviewQueueWorkspaceStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            proposals: proposals(),
            current: None,
        })
    }
}

impl Story for ReviewQueueWorkspaceStory {
    fn title() -> &'static str {
        "Review queue workspace"
    }

    fn description() -> &'static str {
        "An assistant's proposals reviewed one at a time, riskiest first."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ReviewQueueWorkspaceStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let select = story.clone();
        v_flex()
            .w_full()
            .gap_3()
            .child(
                ReviewQueueWorkspace::new("morning-queue", self.proposals.clone())
                    .title("Proposals this morning")
                    .current(self.current.clone())
                    .on_select(move |id, _, cx| {
                        select.update(cx, |this, cx| {
                            this.current = Some(id.clone());
                            cx.notify();
                        })
                    })
                    .on_decide(move |id, decision, _, cx| {
                        story.update(cx, |this, cx| {
                            if let Some(proposal) = this.proposals.iter_mut().find(|p| &p.id == id)
                            {
                                proposal.decision = decision;
                            }
                            if decision.is_some() {
                                this.current =
                                    next_open(&this.proposals, Some(id)).or(Some(id.clone()));
                            }
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Tab to the queue, then use J, K, A, R, S and U."),
            )
    }
}
