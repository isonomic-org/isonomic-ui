// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the agent issue stack.

use crate::Story;
use isonomic_ui::assets::IconName as AssetIcon;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::agent_issue_stack::{
    AgentIssueStack, IssueOwner, IssuePriority, IssueStackEvent, IssueStackVariant, StackIssue,
};

/// Six issues a small team of agents is working through, in both looks.
pub struct AgentIssueStackStory {
    stack: Entity<AgentIssueStack>,
    cycle: Entity<AgentIssueStack>,
    last: SharedString,
    _events: Vec<Subscription>,
}

fn issues(cx: &App) -> Vec<StackIssue> {
    let theme = cx.theme();
    vec![
        StackIssue::new("backoff", "Researcher loops on a rate-limited search")
            .text("The code search tool keeps answering with a rate limit and the researcher retries at once. It should wait longer each time.")
            .icon(AssetIcon::Debug)
            .tint(theme.chart_4)
            .branch("fix/tool-backoff")
            .time("Just now")
            .priority(IssuePriority::Urgent)
            .owner(IssueOwner::new("Watchdog").color(theme.danger).working(true)),
        StackIssue::new("rounding", "Round invoice totals once, not per line")
            .text("Some totals were a cent off because every line was rounded before adding them up.")
            .icon(AssetIcon::Receipt)
            .tint(theme.chart_2)
            .pull("#409")
            .time("2 h ago")
            .priority(IssuePriority::Medium)
            .users(207)
            .owner(IssueOwner::new("Ledger").color(theme.chart_2).working(true)),
        StackIssue::new("signup", "Fix sign-up after the auth key rotation")
            .text("New accounts failed to create sessions once the signing key rotated. Triage found the cause and is writing the fix.")
            .icon(AssetIcon::Locked)
            .tint(theme.chart_5)
            .pull("#412")
            .time("4 min ago")
            .priority(IssuePriority::High)
            .users(78)
            .owner(IssueOwner::new("Triage").color(theme.info).working(true)),
        StackIssue::new("csv", "Add CSV export to the billing page")
            .text("Customers asked to download their invoices as a spreadsheet for their own books.")
            .icon(AssetIcon::Table)
            .tint(theme.warning)
            .branch("feat/csv-export")
            .time("18 min ago")
            .priority(IssuePriority::Medium)
            .diff(214, 12)
            .owner(IssueOwner::new("Coder").color(theme.chart_3).working(true)),
        StackIssue::new("cleanup", "Remove the old onboarding flags")
            .text("Three flags from the spring launch are still read on every request.")
            .icon(AssetIcon::Clean)
            .tint(theme.info)
            .merged("#415")
            .time("32 min ago")
            .priority(IssuePriority::Low)
            .diff(96, 131)
            .owner(IssueOwner::new("Upkeep").color(theme.warning)),
        StackIssue::new("docs", "Document the new webhook retries")
            .text("Explain how often a failed delivery is tried again and how to replay one by hand.")
            .icon(AssetIcon::Document)
            .tint(theme.success)
            .branch("docs/webhooks")
            .time("1 h ago")
            .priority(IssuePriority::Low)
            .owner(IssueOwner::new("Docs").color(theme.success)),
    ]
}

impl AgentIssueStackStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx: &mut Context<Self>| {
            let stack = cx.new(|cx| AgentIssueStack::new(issues(cx), cx));
            let cycle =
                cx.new(|cx| AgentIssueStack::new(issues(cx), cx).variant(IssueStackVariant::Cycle));
            let events = [&stack, &cycle]
                .into_iter()
                .map(|entity| {
                    cx.subscribe_in(
                        entity,
                        window,
                        |this: &mut Self, _, event: &IssueStackEvent, _, cx| {
                            let IssueStackEvent::Show(id) = event;
                            this.last = format!("Showing {id}").into();
                            cx.notify();
                        },
                    )
                })
                .collect();
            Self {
                stack,
                cycle,
                last: "Hover to hold the stack still, or use the arrow keys.".into(),
                _events: events,
            }
        })
    }
}

impl Story for AgentIssueStackStory {
    fn title() -> &'static str {
        "Agent issue stack"
    }

    fn description() -> &'static str {
        "What your agents are on right now, one issue at a time, as a stack or a cycle."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AgentIssueStackStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .flex_wrap()
                    .items_start()
                    .gap(px(40.))
                    .child(div().w(px(360.)).child(self.stack.clone()))
                    .child(div().w(px(360.)).child(self.cycle.clone())),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
