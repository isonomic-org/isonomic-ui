//! Gallery story for the fault tree.

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window};
use isonomic_ui_extra::fault_tree::{FaultNode, FaultTree, Gate, Verdict};

/// Why 201 card matches failed to post. The story keeps the verdicts.
pub struct FaultTreeStory {
    verdicts: Vec<(SharedString, Verdict)>,
}

impl FaultTreeStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { verdicts: Vec::new() })
    }

    fn leaf(&self, id: &str, label: &str, verdict: Verdict, evidence: &str) -> FaultNode {
        let verdict = self
            .verdicts
            .iter()
            .find(|(v, _)| v.as_ref() == id)
            .map(|(_, v)| *v)
            .unwrap_or(verdict);
        let node = FaultNode::new(id.to_string(), label.to_string());
        if evidence.is_empty() && verdict == Verdict::Open {
            node
        } else {
            node.verdict(verdict, evidence.to_string())
        }
    }

    fn tree(&self) -> FaultNode {
        FaultNode::new("top", "201 card matches didn't post to the ledger").gate(
            Gate::Any,
            vec![
                FaultNode::new("access", "The ledger refused the write").gate(
                    Gate::Any,
                    vec![
                        self.leaf("locked", "The March period is locked", Verdict::Open, "The error names a closed period"),
                        self.leaf("token", "The ledger connection expired", Verdict::RuledOut, "Bank lines posted a minute earlier"),
                    ],
                ),
                FaultNode::new("data", "The matches were bad").gate(
                    Gate::All,
                    vec![
                        self.leaf("dupes", "Some matches were duplicates", Verdict::Open, ""),
                        self.leaf("check", "Duplicates stop the whole batch", Verdict::RuledOut, "The ledger skips duplicates one by one"),
                    ],
                ),
                self.leaf("size", "The batch was too large", Verdict::Open, "201 is under the 500 limit, but the limit may have changed"),
            ],
        )
    }
}

impl Story for FaultTreeStory {
    fn title() -> &'static str {
        "Fault tree"
    }

    fn description() -> &'static str {
        "The possible causes of a failure, joined by and/or gates, each with a verdict and its evidence."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for FaultTreeStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let set = cx.entity();
        FaultTree::new("posting-failure", self.tree()).on_verdict(move |id, verdict, _, cx| {
            set.update(cx, |this, cx| {
                this.verdicts.retain(|(v, _)| v != id);
                this.verdicts.push((id.clone(), verdict));
                cx.notify();
            })
        })
    }
}
