//! Gallery story for the agent run tree.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window,
};
use isonomic_ui_extra::agent_run_tree::{AgentRunTree, RunStep, RunStepKind, RunStepOutcome};

/// Shows one reconciliation run. The story keeps what's open and selected.
pub struct AgentRunTreeStory {
    open: Vec<SharedString>,
    selected: Option<SharedString>,
}

fn run() -> Vec<RunStep> {
    use RunStepKind::*;
    use RunStepOutcome::*;
    vec![
        RunStep::new("plan", Thought, "Work out what's unmatched for March")
            .took(Ok, 1_840)
            .io(
                "214 card payouts, 61 bank lines",
                "Match cards first, then bank, then report",
            ),
        RunStep::new("cards", Helper, "Card matcher")
            .took(Failed, 38_200)
            .child(
                RunStep::new(
                    "cards-fetch",
                    Tool,
                    "ledger.list_unmatched(account=\"cards\")",
                )
                .took(Ok, 920)
                .io("account: cards, month: 2026-03", "214 lines, $48,210.55"),
            )
            .child(
                RunStep::new("cards-match", Tool, "match.by_amount_and_date(window=3d)")
                    .took(Ok, 6_300)
                    .io("214 lines", "201 matched, 13 left"),
            )
            .child(
                RunStep::new("cards-post", Tool, "ledger.post_matches(count=201)")
                    .took(Failed, 410)
                    .io(
                        "201 matches",
                        "The March period is locked. Unlock it or post to April.",
                    ),
            ),
        RunStep::new("bank", Helper, "Bank matcher")
            .took(Ok, 22_900)
            .child(
                RunStep::new("bank-fetch", Tool, "bank.statement(account=\"operating\")")
                    .took(Ok, 1_150)
                    .io("operating, March", "61 lines"),
            )
            .child(
                RunStep::new("bank-match", Tool, "match.by_reference()")
                    .took(Ok, 3_400)
                    .io("61 lines", "61 matched"),
            ),
        RunStep::new("report", Message, "Tell Maya what's left").took(Skipped, 0),
        RunStep::new("retry", Thought, "Decide whether to post to April").took(Running, 0),
    ]
}

impl AgentRunTreeStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            open: vec!["cards".into()],
            selected: Some("cards-post".into()),
        })
    }
}

impl Story for AgentRunTreeStory {
    fn title() -> &'static str {
        "Agent run tree"
    }

    fn description() -> &'static str {
        "Everything one run did, as a folding outline with a detail pane."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AgentRunTreeStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.entity();
        let toggle = cx.entity();
        AgentRunTree::new("march-run", run())
            .title("Reconcile March")
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
    }
}
