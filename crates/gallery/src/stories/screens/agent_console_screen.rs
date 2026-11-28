// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! An operator's console for a team of agents: the fleet beside the run's
//! trace and its tool calls, every agent's files, and what the run has used
//! with its event log.

use std::time::Duration;

use crate::Story;
use isonomic_ui::assets::IconName as AssetIcon;
use isonomic_ui::component::{
    ActiveTheme as _, StyledExt as _, h_flex,
    tab::{Tab, TabBar},
    v_flex,
};
use isonomic_ui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};
use isonomic_ui_extra::agent_finder::{AgentFinder, FinderAgent, FinderFile, FinderStatus};
use isonomic_ui_extra::agent_fleet::{AgentFleet, FleetAgent, FleetStatus, FleetStep};
use isonomic_ui_extra::context_usage_grid::{BudgetItem, BudgetPart, ContextUsageGrid};
use isonomic_ui_extra::event_log_brush::{
    AttemptStatus, EventLogBrush, LogAttempt, LogEvent, LogLevel,
};
use isonomic_ui_extra::tool_fanout_graph::{FanoutCall, ToolFanoutGraph};
use isonomic_ui_extra::trace_timeline::{TraceSpan, TraceTimeline};
use isonomic_ui_extra::usage_meter::{UsageLimit, UsageMeter, UsagePart};

use super::window_frame;

const TABS: [&str; 3] = ["Agents", "Files", "Usage"];

fn mins(m: u64, s: u64) -> Duration {
    Duration::from_secs(m * 60 + s)
}

fn fleet() -> Vec<FleetAgent> {
    vec![
        FleetAgent::new(
            "catalogue",
            "Catalogue lead",
            "Refresh the spring plant catalogue",
        )
        .model("Large model")
        .status(FleetStatus::Running)
        .tool(AssetIcon::Document)
        .cost(0.74)
        .took(mins(12, 40))
        .child(
            FleetAgent::new("stock", "Stock", "Read stock levels from both greenhouses")
                .model("Small model")
                .status(FleetStatus::Done)
                .cost(0.05)
                .took(mins(2, 10))
                .step(FleetStep::new(
                    "Export greenhouse counts",
                    FleetStatus::Done,
                ))
                .step(FleetStep::new("Mark plants under five", FleetStatus::Done)),
        )
        .child(
            FleetAgent::new("prices", "Pricing", "Set spring prices from supplier lists")
                .model("Large model")
                .status(FleetStatus::Waiting)
                .tool(AssetIcon::Calculator)
                .cost(0.31)
                .took(mins(6, 2))
                .ask("Raise all fruit trees by 8% to match the new supplier list?")
                .step(FleetStep::new("Read supplier lists", FleetStatus::Done).took("1m 40s"))
                .step(FleetStep::new("Draft new prices", FleetStatus::Waiting))
                .step(FleetStep::new("Publish prices", FleetStatus::Queued)),
        )
        .child(
            FleetAgent::new("copy", "Descriptions", "Write care notes for 42 new plants")
                .model("Small model")
                .status(FleetStatus::Running)
                .cost(0.22)
                .took(mins(4, 31)),
        ),
        FleetAgent::new(
            "photos",
            "Photo desk",
            "Crop and tag this week's plant photos",
        )
        .model("Small model")
        .status(FleetStatus::Failed)
        .tool(AssetIcon::Image)
        .cost(0.12)
        .took(mins(3, 5))
        .child(
            FleetAgent::new("upload", "Uploader", "Upload 180 photos to the shop")
                .status(FleetStatus::Failed)
                .error("The image store refused files over 10 MB.")
                .cost(0.02),
        ),
        FleetAgent::new("support", "Support lead", "Answer 23 delivery questions")
            .model("Large model")
            .status(FleetStatus::Running)
            .tool(AssetIcon::Email)
            .cost(0.58)
            .took(mins(21, 0))
            .child(
                FleetAgent::new("tracking", "Tracking", "Look up parcels")
                    .status(FleetStatus::Done)
                    .cost(0.03),
            ),
        FleetAgent::new("report", "Weekly report", "Sum up sales by plant family")
            .model("Large model")
            .status(FleetStatus::Done)
            .cost(0.41)
            .took(mins(9, 12)),
    ]
}

fn spans() -> Vec<TraceSpan> {
    vec![
        TraceSpan::new("plan", "Plan the refresh", 0, 2_100).usage(5_800, 1_400),
        TraceSpan::new("lead", "Catalogue lead", 2_200, 0)
            .running()
            .usage(4_200, 1_100)
            .child(TraceSpan::new("stock", "Stock", 2_400, 14_800).usage(9_600, 2_200))
            .child(
                TraceSpan::new("prices", "Pricing", 2_600, 0)
                    .running()
                    .usage(18_400, 4_900)
                    .child(
                        TraceSpan::new("suppliers", "files.read_supplier_lists", 2_700, 9_900)
                            .usage(11_200, 300),
                    )
                    .child(
                        TraceSpan::new("draft", "prices.draft", 10_100, 31_500).usage(6_800, 4_300),
                    ),
            )
            .child(
                TraceSpan::new("copy", "Descriptions", 15_000, 0)
                    .running()
                    .usage(12_100, 8_700),
            ),
        TraceSpan::new("photos", "Photo desk", 3_000, 21_600)
            .failed()
            .usage(3_900, 800),
    ]
}

fn finder() -> Vec<FinderAgent> {
    vec![
        FinderAgent::new("catalogue", "Catalogue lead")
            .status(FinderStatus::Running)
            .progress(0.58)
            .model("Large model")
            .task("Refresh the spring plant catalogue")
            .tokens(36_480)
            .child(
                FinderAgent::new("stock", "Stock")
                    .status(FinderStatus::Done)
                    .model("Small model")
                    .tokens(11_800)
                    .file(
                        FinderFile::new("counts", "greenhouse-counts.csv")
                            .tool("sheets.export")
                            .size("42 KB")
                            .duration("1.2 s"),
                    ),
            )
            .child(
                FinderAgent::new("prices", "Pricing")
                    .status(FinderStatus::Waiting)
                    .model("Large model")
                    .tokens(23_300)
                    .file(
                        FinderFile::new("suppliers", "supplier-lists.pdf")
                            .tool("files.read")
                            .size("1.4 MB")
                            .duration("3.8 s"),
                    )
                    .file(
                        FinderFile::new("draft", "spring-prices.csv")
                            .tool("prices.draft")
                            .size("18 KB")
                            .duration("21.4 s")
                            .exchange(
                                "{ \"family\": \"fruit trees\", \"rise\": 0.08 }",
                                "Drafted 64 prices; 12 changed by more than 10%",
                            ),
                    ),
            )
            .file(
                FinderFile::new("plan", "refresh-plan.md")
                    .tool("docs.create_page")
                    .size("6 KB")
                    .duration("0.9 s"),
            ),
        FinderAgent::new("photos", "Photo desk")
            .status(FinderStatus::Failed)
            .model("Small model")
            .tokens(4_700)
            .file(
                FinderFile::new("upload-log", "upload.log")
                    .tool("shop.upload_images")
                    .size("3 KB")
                    .duration("18.6 s")
                    .failed(true)
                    .exchange(
                        "shop upload photos/week-46/*.jpg",
                        "error: 7 files are over the 10 MB limit",
                    ),
            ),
        FinderAgent::new("support", "Support lead")
            .status(FinderStatus::Running)
            .model("Large model")
            .tokens(52_010),
        FinderAgent::new("report", "Weekly report").status(FinderStatus::Done),
    ]
}

fn log_events() -> Vec<LogEvent> {
    let mut events = Vec::new();
    let mut at = 0.8;
    for batch in 1..=18u32 {
        let slow = batch % 6 == 0;
        let mut event = LogEvent::new(
            format!("photo-{batch:02}"),
            at,
            if slow {
                LogLevel::Warning
            } else {
                LogLevel::Info
            },
            "upload",
            if slow {
                format!("Batch {batch} uploaded on the third try")
            } else {
                format!("Uploaded photo batch {batch}")
            },
        );
        event = if slow {
            event
                .attempt(LogAttempt::new(AttemptStatus::TimedOut, 6000.).error("no reply in 6 s"))
                .attempt(LogAttempt::new(AttemptStatus::Failed, 380.).error("503 from the store"))
                .attempt(LogAttempt::new(AttemptStatus::Succeeded, 1_900.))
        } else {
            event.attempt(LogAttempt::new(
                AttemptStatus::Succeeded,
                300. + f64::from(batch % 5) * 90.,
            ))
        };
        events.push(event);
        events.push(LogEvent::new(
            format!("tag-{batch:02}"),
            at + 1.1,
            LogLevel::Debug,
            "tag",
            format!("Batch {batch}: {} photos tagged", 8 + batch % 4),
        ));
        at += 4.5 + f64::from(batch % 3);
    }
    events.push(
        LogEvent::new(
            "too-big",
            at + 1.,
            LogLevel::Error,
            "upload",
            "Seven photos are over the 10 MB limit",
        )
        .attempt(LogAttempt::new(AttemptStatus::Failed, 120.).error("413 payload too large")),
    );
    events
}

/// The console of a plant nursery's agents during the weekly refresh.
pub struct AgentConsoleScreenStory {
    tab: usize,
    fleet: Entity<AgentFleet>,
    finder: Entity<AgentFinder>,
    log: Entity<EventLogBrush>,
    open: Vec<SharedString>,
    selected: Option<SharedString>,
}

impl AgentConsoleScreenStory {
    /// Create the screen.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let fleet = cx.new(|cx| AgentFleet::new(fleet(), cx).title("Fleet"));
            let finder = cx.new(|cx| {
                AgentFinder::new("Weekly refresh", finder(), cx)
                    .subtitle("Started today, 07:30 \u{b7} running for 34 min")
            });
            let log = cx.new(|cx| {
                let mut log =
                    EventLogBrush::new("console-log", "Photo upload log", log_events(), window, cx)
                        .subtitle("Photo desk, this run");
                log.pick("too-big", cx);
                log
            });
            Self {
                tab: 0,
                fleet,
                finder,
                log,
                open: vec!["lead".into(), "prices".into()],
                selected: Some("draft".into()),
            }
        })
    }

    fn agents_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let (select, toggle) = (cx.entity(), cx.entity());
        h_flex()
            .size_full()
            .items_start()
            .child(
                div()
                    .id("console-fleet")
                    .flex_none()
                    .w(px(340.))
                    .h_full()
                    .overflow_y_scroll()
                    .p_4()
                    .child(self.fleet.clone()),
            )
            .child(
                div()
                    .id("console-trace")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .p_4()
                    .child(
                        v_flex()
                            .gap_4()
                            .child(
                                TraceTimeline::new("console-trace-timeline", spans(), 34_000)
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
                                            if let Some(at) = this.open.iter().position(|o| o == id)
                                            {
                                                this.open.remove(at);
                                            } else {
                                                this.open.push(id.clone());
                                            }
                                            cx.notify();
                                        })
                                    }),
                            )
                            .child(
                                ToolFanoutGraph::new(
                                    "console-fanout",
                                    "Tool calls",
                                    "Catalogue lead",
                                )
                                .subtitle("This run, 34 minutes")
                                .call(
                                    FanoutCall::new("Prices", 64, 640.)
                                        .errors(1)
                                        .child(FanoutCall::new("Supplier lists", 12, 2_100.))
                                        .child(FanoutCall::new("Price history", 52, 180.)),
                                )
                                .call(FanoutCall::new("Stock", 38, 220.))
                                .call(FanoutCall::new("Docs", 21, 410.).errors(2))
                                .call(FanoutCall::new("Shop", 180, 950.).errors(7)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn usage_tab(&self) -> AnyElement {
        let days: Vec<Option<f64>> = [
            1.9, 2.4, 2.2, 0.6, 0.4, 2.8, 3.1, 2.6, 2.9, 3.4, 0.7, 0.5, 3.6, 3.2, 4.1, 3.8,
        ]
        .into_iter()
        .map(Some)
        .chain([None; 14])
        .collect();
        div()
            .id("console-usage")
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .child(
                v_flex()
                    .gap_4()
                    .child(
                        h_flex()
                            .w_full()
                            .items_start()
                            .gap_4()
                            .child(
                                div().flex_1().min_w_0().child(
                                    UsageMeter::new("console-plan", 60.)
                                        .title("Nursery agents")
                                        .plan("Team plan")
                                        .renews_in(Duration::from_secs(14 * 86_400))
                                        .unit("M tokens")
                                        .part(UsagePart::new("Catalogue", 19.4))
                                        .part(UsagePart::new("Support", 14.2))
                                        .part(UsagePart::new("Photos", 6.1))
                                        .part(UsagePart::new("Reports", 4.0))
                                        .daily(days)
                                        .limit(UsageLimit::new("Agents running", 6.).cap(8.))
                                        .limit(UsageLimit::new("Storage", 3.2).cap(10.).unit("GB")),
                                ),
                            )
                            .child(
                                div().flex_1().min_w_0().child(
                                    ContextUsageGrid::new("console-context", 128_000)
                                        .model("Catalogue lead, large model")
                                        .part(
                                            BudgetPart::new("rules", "Instructions", 4_200)
                                                .locked(true),
                                        )
                                        .part(BudgetPart::new("tools", "Tools", 11_300))
                                        .part(
                                            BudgetPart::new("files", "Attached files", 0)
                                                .item(BudgetItem::new(
                                                    "suppliers",
                                                    "supplier-lists.pdf",
                                                    24_600,
                                                ))
                                                .item(BudgetItem::new(
                                                    "counts",
                                                    "greenhouse-counts.csv",
                                                    9_100,
                                                )),
                                        )
                                        .part(
                                            BudgetPart::new("chat", "Conversation", 38_900)
                                                .compactable(true),
                                        ),
                                ),
                            ),
                    )
                    .child(self.log.clone()),
            )
            .into_any_element()
    }
}

impl Story for AgentConsoleScreenStory {
    fn title() -> &'static str {
        "Agent console screen"
    }

    fn description() -> &'static str {
        "An operator's console built from the library: the fleet, the run's trace and tool calls, each agent's files, and usage with the event log."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AgentConsoleScreenStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.tab {
            0 => self.agents_tab(cx),
            1 => div()
                .id("console-files")
                .size_full()
                .overflow_y_scroll()
                .p_4()
                .child(self.finder.clone())
                .into_any_element(),
            _ => self.usage_tab(),
        };
        let theme = cx.theme();
        let line = theme.border;
        let quiet = theme.muted_foreground;

        let header = h_flex()
            .flex_none()
            .h(px(48.))
            .px_4()
            .gap_4()
            .border_b_1()
            .border_color(line)
            .child(div().text_sm().font_medium().child("Agent console"))
            .child(
                div()
                    .text_sm()
                    .text_color(quiet)
                    .child("Weekly refresh, 4 leads, 7 helpers"),
            )
            .child(div().flex_1())
            .child(
                TabBar::new("console-tabs")
                    .underline()
                    .selected_index(self.tab)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.tab = *ix;
                        cx.notify();
                    }))
                    .children(TABS.map(|label| Tab::new().label(label))),
            );

        window_frame(cx)
            .child(header)
            .child(div().flex_1().min_h_0().w_full().child(body))
    }
}
