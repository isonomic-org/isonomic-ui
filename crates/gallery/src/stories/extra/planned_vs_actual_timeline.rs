//! Gallery story for the planned vs actual timeline.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Window,
};
use gpui_component::v_flex;
use isonomic_ui_extra::planned_vs_actual_timeline::{PlanStep, PlannedVsActualTimeline};

/// Shows a month-end close running late after the bank import overran.
pub struct PlannedVsActualTimelineStory;

impl PlannedVsActualTimelineStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self)
    }
}

fn secs(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

impl Render for PlannedVsActualTimelineStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex().child(
            PlannedVsActualTimeline::new("close")
                .task("Close the March books")
                .now(secs(131))
                .step(
                    PlanStep::new("fetch", "Fetch statements", secs(20))
                        .started(secs(0))
                        .ended(secs(16)),
                )
                .step(
                    PlanStep::new("import", "Import bank lines", secs(30))
                        .waits_for("fetch")
                        .started(secs(16))
                        .ended(secs(58)),
                )
                .step(
                    PlanStep::new("receipts", "Match receipts", secs(35))
                        .waits_for("fetch")
                        .started(secs(16))
                        .ended(secs(49)),
                )
                .step(
                    PlanStep::new("reconcile", "Reconcile accounts", secs(40))
                        .waits_for("import")
                        .waits_for("receipts")
                        .started(secs(58))
                        .ended(secs(104)),
                )
                .step(
                    PlanStep::new("accruals", "Post accruals", secs(25))
                        .waits_for("reconcile")
                        .started(secs(104)),
                )
                .step(
                    PlanStep::new("report", "Draft the close report", secs(30))
                        .waits_for("accruals"),
                )
                .step(PlanStep::new("send", "Send for sign-off", secs(10)).waits_for("report")),
        )
    }
}
