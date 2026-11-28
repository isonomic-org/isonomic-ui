// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the agent finder.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::agent_finder::{
    AgentFinder, FinderAgent, FinderEvent, FinderFile, FinderItem, FinderStatus,
};

/// A quarterly close run with six agents, their helpers and their tool calls.
pub struct AgentFinderStory {
    finder: Entity<AgentFinder>,
    last: SharedString,
    _events: Subscription,
}

fn run() -> Vec<FinderAgent> {
    vec![
        FinderAgent::new("planner", "Planner")
            .status(FinderStatus::Running)
            .progress(0.62)
            .model("Large model")
            .task("Break the quarter close into steps and hand them out")
            .tokens(48_210)
            .child(
                FinderAgent::new("researcher", "Researcher")
                    .status(FinderStatus::Done)
                    .model("Large model")
                    .tokens(18_240)
                    .file(
                        FinderFile::new("policy", "Close policy.md")
                            .tool("docs.read")
                            .size("14 KB")
                            .duration("0.8 s"),
                    ),
            )
            .child(
                FinderAgent::new("scheduler", "Scheduler")
                    .status(FinderStatus::Done)
                    .model("Small model")
                    .tokens(9_120),
            )
            .file(
                FinderFile::new("checklist", "Close checklist.pdf")
                    .tool("docs.create_page")
                    .size("212 KB")
                    .duration("2.1 s")
                    .exchange(
                        "{\n  \"parent\": \"Q3 close\",\n  \"title\": \"Close checklist\"\n}",
                        "Created page \"Close checklist\"\n1. Match bank lines – Matcher\n2. Accruals – Ledger\n3. Review variances – Reviewer\n4. Sign off – Controller",
                    ),
            )
            .file(
                FinderFile::new("ticket", "FIN-311.json")
                    .tool("tracker.get_issue")
                    .size("6 KB")
                    .duration("0.4 s")
                    .exchange(
                        "{ \"id\": \"FIN-311\" }",
                        "{\n  \"title\": \"Close Q3 books\",\n  \"state\": \"In progress\"\n}",
                    ),
            ),
        FinderAgent::new("matcher", "Matcher")
            .status(FinderStatus::Running)
            .model("Small model")
            .tokens(122_904)
            .file(
                FinderFile::new("bank", "bank-lines.csv")
                    .tool("bank.export")
                    .size("88 KB")
                    .duration("3.4 s"),
            ),
        FinderAgent::new("ledger", "Ledger")
            .status(FinderStatus::Waiting)
            .model("Large model")
            .tokens(31_007)
            .child(FinderAgent::new("accruals", "Accruals").status(FinderStatus::Waiting))
            .file(
                FinderFile::new("sync", "ledger-sync.log")
                    .tool("shell.run")
                    .size("2 KB")
                    .duration("12.0 s")
                    .failed(true)
                    .exchange(
                        "ledger sync --period 2026-09",
                        "error: period 2026-09 is locked by another session",
                    ),
            ),
        FinderAgent::new("reviewer", "Reviewer")
            .status(FinderStatus::Waiting)
            .model("Large model"),
        FinderAgent::new("controller", "Controller").status(FinderStatus::Done),
        FinderAgent::new("notifier", "Notifier"),
    ]
}

impl AgentFinderStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx: &mut Context<Self>| {
            let finder = cx.new(|cx| {
                AgentFinder::new("Q3 close", run(), cx)
                    .subtitle("Started today, 08:52 · running for 1 h 48 min")
            });
            let events = cx.subscribe_in(
                &finder,
                window,
                |this: &mut Self, _, event: &FinderEvent, _, cx| {
                    this.last = match event {
                        FinderEvent::Select(Some(FinderItem::Agent(id) | FinderItem::File(id))) => {
                            format!("Selected {id}")
                        }
                        FinderEvent::Select(None) => "Nothing selected".into(),
                        FinderEvent::Open(id) => format!("Opened {id}"),
                        FinderEvent::OpenFile(id, agent) => format!("Opened {id} from {agent}"),
                    }
                    .into();
                    cx.notify();
                },
            );
            Self {
                finder,
                last: "Double-click a folder to open it, or select a document and press Space."
                    .into(),
                _events: events,
            }
        })
    }
}

impl Story for AgentFinderStory {
    fn title() -> &'static str {
        "Agent finder"
    }

    fn description() -> &'static str {
        "A multi-agent run browsed like files: agents are folders, tool calls are documents."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AgentFinderStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .max_w(isonomic_ui::px(720.))
            .child(self.finder.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
