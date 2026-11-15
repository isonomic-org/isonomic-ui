//! Gallery story for the agent timeline lanes.

use crate::Story;
use isonomic_ui::{App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window};
use isonomic_ui_extra::agent_timeline_lanes::{AgentTimelineLanes, LaneDecision, LaneHandoff, LaneTask};
use isonomic_ui_extra::common::Tone;

/// Fifteen minutes of a close run across four assistants. The story keeps
/// the selection.
pub struct AgentTimelineLanesStory {
    selected: Option<SharedString>,
}

impl AgentTimelineLanesStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { selected: None })
    }
}

impl Story for AgentTimelineLanesStory {
    fn title() -> &'static str {
        "Agent timeline lanes"
    }

    fn description() -> &'static str {
        "One lane per assistant, with its tasks as blocks, the handoffs between lanes and the decisions taken."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AgentTimelineLanesStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.entity();
        AgentTimelineLanes::new(
            "close-lanes",
            vec![
                ("lead".into(), "Coordinator".into()),
                ("recon".into(), "Reconciler".into()),
                ("cards".into(), "Card matcher".into()),
                ("pay".into(), "Payables".into()),
            ],
            840,
        )
        .tasks(vec![
            LaneTask::new("t1", "lead", 0, 70, "Plan the close"),
            LaneTask::new("t2", "recon", 80, 300, "Match the bank lines"),
            LaneTask::new("t3", "cards", 120, 610, "Match 214 card payouts").tone(Tone::Danger),
            LaneTask::new("t4", "pay", 90, 400, "Build Friday's run"),
            LaneTask::new("t5", "pay", 400, 0, "Wait for approval").running().tone(Tone::Warning),
            LaneTask::new("t6", "recon", 620, 0, "Decide where to post").running(),
            LaneTask::new("t7", "lead", 700, 0, "Draft the close note").running(),
        ])
        .handoffs(vec![
            LaneHandoff { at: 75, from: "lead".into(), to: "recon".into() },
            LaneHandoff { at: 85, from: "lead".into(), to: "pay".into() },
            LaneHandoff { at: 115, from: "recon".into(), to: "cards".into() },
            LaneHandoff { at: 615, from: "cards".into(), to: "recon".into() },
        ])
        .decisions(vec![
            LaneDecision { id: "d1".into(), lane: "lead".into(), at: 40, label: "Cards first, then bank".into() },
            LaneDecision { id: "d2".into(), lane: "cards".into(), at: 330, label: "Widen the date window to 3 days".into() },
            LaneDecision { id: "d3".into(), lane: "recon".into(), at: 660, label: "Ask Maya before posting to April".into() },
        ])
        .selected(self.selected.clone())
        .on_select(move |id, _, cx| {
            select.update(cx, |this, cx| {
                this.selected = Some(id.clone());
                cx.notify();
            })
        })
    }
}
