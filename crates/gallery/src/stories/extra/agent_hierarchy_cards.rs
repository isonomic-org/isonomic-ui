//! Gallery story for the agent hierarchy cards.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::agent_hierarchy_cards::{AgentHierarchyCards, AgentNode, AgentState};

/// Shows a month-end close run by a lead assistant and five helpers. The
/// story keeps the folded branches and the selected card.
pub struct AgentHierarchyCardsStory {
    folded: Vec<SharedString>,
    selected: Option<SharedString>,
}

fn team() -> AgentNode {
    AgentNode::new("lead", "Close coordinator", "Runs the March close")
        .state(AgentState::Working("Waiting for two helpers".into()))
        .progress(14, "$0.61")
        .helper(
            AgentNode::new("recon", "Reconciler", "Matches bank and card lines")
                .state(AgentState::Working("Matching 214 card payouts".into()))
                .progress(38, "$0.27")
                .helper(
                    AgentNode::new("bank", "Bank matcher", "Operating account")
                        .state(AgentState::Done)
                        .progress(22, "$0.09"),
                )
                .helper(
                    AgentNode::new("cards", "Card matcher", "Company cards")
                        .state(AgentState::Failed("The March ledger is locked".into()))
                        .progress(9, "$0.04"),
                ),
        )
        .helper(
            AgentNode::new("payables", "Payables assistant", "Supplier bills")
                .state(AgentState::Waiting("Maya's approval".into()))
                .progress(11, "$0.08"),
        )
        .helper(
            AgentNode::new("report", "Report writer", "Drafts the close summary")
                .state(AgentState::Idle),
        )
}

impl AgentHierarchyCardsStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            folded: Vec::new(),
            selected: Some("lead".into()),
        })
    }
}

impl Story for AgentHierarchyCardsStory {
    fn title() -> &'static str {
        "Agent hierarchy cards"
    }

    fn description() -> &'static str {
        "A lead assistant and the helpers it handed work to, as a tree of cards."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AgentHierarchyCardsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.entity();
        let toggle = cx.entity();
        let team = team();
        let detail = self
            .selected
            .as_ref()
            .and_then(|id| team.find(id))
            .map(|node| format!("Selected: {}, {}", node.name, node.role))
            .unwrap_or_else(|| "Click a card to select it.".into());
        v_flex()
            .w_full()
            .gap_3()
            .child(
                AgentHierarchyCards::new("close-team", team)
                    .folded(self.folded.clone())
                    .selected(self.selected.clone())
                    .on_select(move |id, _, cx| {
                        select.update(cx, |this, cx| {
                            this.selected = Some(id.clone());
                            cx.notify();
                        })
                    })
                    .on_toggle(move |id, _, cx| {
                        toggle.update(cx, |this, cx| {
                            if let Some(at) = this.folded.iter().position(|f| f == id) {
                                this.folded.remove(at);
                            } else {
                                this.folded.push(id.clone());
                            }
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(detail),
            )
    }
}
