//! Gallery story for the multi-agent dashboard.

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window};
use isonomic_ui_extra::multi_agent_dashboard::{
    FleetAction, FleetAgent, FleetState, MultiAgentDashboard,
};

/// Shows a bookkeeping team's assistants on a busy morning. The story
/// applies the filter and the pause, resume and stop buttons.
pub struct MultiAgentDashboardStory {
    agents: Vec<FleetAgent>,
    filter: Option<FleetState>,
}

fn agents() -> Vec<FleetAgent> {
    vec![
        FleetAgent::new(
            "recon",
            "Reconciler",
            "Matching 214 card payouts against the ledger",
        )
        .progress(0.62)
        .usage(38, 27, "4s ago"),
        FleetAgent::new(
            "payables",
            "Payables assistant",
            "Waiting for Maya to approve Friday's supplier run",
        )
        .state(FleetState::Waiting)
        .progress(0.8)
        .usage(11, 8, "6 min ago"),
        FleetAgent::new(
            "cards",
            "Card matcher",
            "The March ledger is locked, so 201 matches couldn't post",
        )
        .state(FleetState::Failed)
        .progress(0.94)
        .usage(9, 4, "12 min ago"),
        FleetAgent::new(
            "collections",
            "Collections",
            "Drafting reminders for 7 overdue invoices",
        )
        .progress(0.35)
        .usage(16, 12, "18s ago"),
        FleetAgent::new(
            "receipts",
            "Receipt filer",
            "Filing 43 receipts from the shared inbox",
        )
        .state(FleetState::Paused)
        .progress(0.5)
        .usage(21, 6, "1 h ago"),
        FleetAgent::new(
            "bank",
            "Bank matcher",
            "Matched all 61 operating account lines",
        )
        .state(FleetState::Done)
        .usage(22, 9, "25 min ago"),
    ]
}

impl MultiAgentDashboardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            agents: agents(),
            filter: None,
        })
    }
}

impl Story for MultiAgentDashboardStory {
    fn title() -> &'static str {
        "Multi-agent dashboard"
    }

    fn description() -> &'static str {
        "Every assistant at work, as a filterable grid of tiles with totals."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for MultiAgentDashboardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filter = cx.entity();
        let act = cx.entity();
        MultiAgentDashboard::new("morning-fleet", self.agents.clone())
            .title("Bookkeeping assistants")
            .filter(self.filter)
            .budget(100)
            .on_filter(move |state, _, cx| {
                filter.update(cx, |this, cx| {
                    this.filter = state;
                    cx.notify();
                })
            })
            .on_action(move |id, action, _, cx| {
                act.update(cx, |this, cx| {
                    if let Some(agent) = this.agents.iter_mut().find(|a| &a.id == id) {
                        match action {
                            FleetAction::Pause => agent.state = FleetState::Paused,
                            FleetAction::Resume => agent.state = FleetState::Working,
                            FleetAction::Stop => {
                                agent.state = FleetState::Done;
                                agent.task = "Stopped by you".into();
                            }
                            FleetAction::Open => {}
                        }
                    }
                    cx.notify();
                })
            })
    }
}
