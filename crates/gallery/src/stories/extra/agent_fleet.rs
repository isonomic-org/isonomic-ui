// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the agent fleet.

use std::time::Duration;

use crate::Story;
use isonomic_ui::assets::IconName as AssetIcon;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::agent_fleet::{
    AgentFleet, FleetAgent, FleetAnswer, FleetEvent, FleetStatus, FleetStep,
};

/// Four leads at work on a Friday, one of them waiting on you.
pub struct AgentFleetStory {
    fleet: Entity<AgentFleet>,
    last: SharedString,
    _events: Subscription,
}

fn mins(m: u64, s: u64) -> Duration {
    Duration::from_secs(m * 60 + s)
}

fn fleet() -> Vec<FleetAgent> {
    vec![
        FleetAgent::new("books", "Books lead", "Close the September books by Monday")
            .model("Large model")
            .status(FleetStatus::Running)
            .tool(AssetIcon::Document)
            .tool(AssetIcon::Calendar)
            .cost(0.62)
            .took(mins(14, 6))
            .child(
                FleetAgent::new("matcher", "Matcher", "Match 1,204 bank lines to invoices")
                    .model("Small model")
                    .status(FleetStatus::Done)
                    .tool(AssetIcon::Document)
                    .cost(0.04)
                    .took(mins(5, 0))
                    .step(FleetStep::new("Export bank lines", FleetStatus::Done).took("40s"))
                    .step(FleetStep::new("Match by amount", FleetStatus::Done).took("3m 02s"))
                    .step(FleetStep::new("Flag the leftovers", FleetStatus::Done).took("1m 18s")),
            )
            .child(
                FleetAgent::new("accruals", "Accruals", "Book the month's accrued costs")
                    .model("Large model")
                    .status(FleetStatus::Waiting)
                    .tool(AssetIcon::Calculator)
                    .cost(0.49)
                    .took(mins(9, 12))
                    .ask("Book 3,400.00 of unbilled hosting to September?")
                    .step(
                        FleetStep::new("Read open purchase orders", FleetStatus::Done)
                            .took("2m 10s"),
                    )
                    .step(FleetStep::new(
                        "Draft accrual entries",
                        FleetStatus::Waiting,
                    ))
                    .step(FleetStep::new("Post to the ledger", FleetStatus::Queued)),
            )
            .child(
                FleetAgent::new("fx", "Rates", "Revalue the euro and pound balances")
                    .model("Small model")
                    .status(FleetStatus::Running)
                    .tool(AssetIcon::Currency)
                    .cost(0.29)
                    .took(mins(4, 51)),
            )
            .child(
                FleetAgent::new("report", "Reporter", "Write the close summary")
                    .model("Large model")
                    .status(FleetStatus::Queued)
                    .step(FleetStep::new("Collect the numbers", FleetStatus::Queued)),
            ),
        FleetAgent::new(
            "support",
            "Support lead",
            "Answer the 48 open tickets in the queue",
        )
        .model("Large model")
        .status(FleetStatus::Running)
        .tool(AssetIcon::Email)
        .cost(0.98)
        .took(mins(26, 6))
        .child(
            FleetAgent::new(
                "triage",
                "Triage",
                "Sort the tickets by product and urgency",
            )
            .status(FleetStatus::Done)
            .cost(0.04),
        )
        .child(
            FleetAgent::new("billing", "Billing", "Reply to questions about invoices")
                .status(FleetStatus::Running)
                .cost(0.49),
        ),
        FleetAgent::new(
            "release",
            "Release manager",
            "Ship version 2.4 to the store",
        )
        .model("Large model")
        .status(FleetStatus::Running)
        .tool(AssetIcon::Rocket)
        .cost(0.61)
        .took(mins(38, 6))
        .child(
            FleetAgent::new("build", "Build", "Sign and upload the build")
                .status(FleetStatus::Failed)
                .error("The signing certificate expired yesterday.")
                .cost(0.12),
        )
        .child(
            FleetAgent::new("notes", "Release notes", "Write what's new")
                .status(FleetStatus::Done)
                .cost(0.08),
        ),
        FleetAgent::new(
            "research",
            "Market analyst",
            "Write the weekly competitor report",
        )
        .model("Large model")
        .status(FleetStatus::Done)
        .cost(0.82)
        .took(mins(42, 0))
        .child(FleetAgent::new("prices", "Prices", "Check prices").status(FleetStatus::Done))
        .child(FleetAgent::new("news", "News", "Read the news").status(FleetStatus::Done))
        .child(FleetAgent::new("draft", "Draft", "Write it up").status(FleetStatus::Done)),
    ]
}

impl AgentFleetStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx: &mut Context<Self>| {
            let fleet = cx.new(|cx| AgentFleet::new(fleet(), cx).title("Fleet"));
            let events = cx.subscribe_in(
                &fleet,
                window,
                |this: &mut Self, _, event: &FleetEvent, _, cx| {
                    this.last = match event {
                        FleetEvent::Open(Some(id)) => format!("Opened {id}"),
                        FleetEvent::Open(None) => "All folded".into(),
                        FleetEvent::Answer(id, answer) => {
                            let word = match answer {
                                FleetAnswer::Approve => "approved",
                                FleetAnswer::Reject => "rejected",
                                FleetAnswer::Retry => "retried",
                                FleetAnswer::Dismiss => "dismissed",
                            };
                            format!("{id}: {word}")
                        }
                    }
                    .into();
                    cx.notify();
                },
            );
            Self {
                fleet,
                last: "Click a lead to open its sub-agents.".into(),
                _events: events,
            }
        })
    }
}

impl Story for AgentFleetStory {
    fn title() -> &'static str {
        "Agent fleet"
    }

    fn description() -> &'static str {
        "Several agents side by side, each summing up its sub-agents, one tree open at a time."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AgentFleetStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .max_w(px(600.))
            .child(self.fleet.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
