// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the tool fan-out graph.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use isonomic_ui_extra::tool_fanout_graph::{FanPick, FanoutCall, ToolFanoutGraph};

/// A trip planner's calls over one afternoon, two hops deep, and a small
/// helper with one hop.
pub struct ToolFanoutGraphStory {
    picked: SharedString,
}

impl ToolFanoutGraphStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            picked: "Nothing picked".into(),
        })
    }
}

impl Story for ToolFanoutGraphStory {
    fn title() -> &'static str {
        "Tool fan-out graph"
    }

    fn description() -> &'static str {
        "An agent's calls fanning out to tools, with call counts, latency and error rates on every line and the hottest path marked."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ToolFanoutGraphStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let this = cx.entity();
        v_flex()
            .gap_6()
            .child(
                ToolFanoutGraph::new("planner", "Tool calls", "Trip planner")
                    .subtitle("Saturday afternoon, 14:00 to 18:00")
                    .call(
                        FanoutCall::new("Route search", 148, 420.)
                            .errors(3)
                            .child(FanoutCall::new("Timetable", 212, 180.).errors(2))
                            .child(FanoutCall::new("Live delays", 96, 1350.).errors(14))
                            .child(FanoutCall::new("Fare table", 40, 35.)),
                    )
                    .call(
                        FanoutCall::new("Place lookup", 64, 260.)
                            .child(FanoutCall::new("Map tiles", 58, 90.).errors(1)),
                    )
                    .call(FanoutCall::new("Weather", 22, 610.).errors(4))
                    .call(FanoutCall::new("Booking", 9, 2400.))
                    .on_select(move |pick, _, cx| {
                        this.update(cx, |this, cx| {
                            this.picked = match pick {
                                None => "Nothing picked".into(),
                                Some(FanPick::Node(ix)) => format!("Node {ix} picked").into(),
                                Some(FanPick::Edge(ix)) => {
                                    format!("Line into node {ix} picked").into()
                                }
                            };
                            cx.notify();
                        })
                    }),
            )
            .child(
                div().max_w(px(420.)).child(
                    ToolFanoutGraph::new("notes", "Note helper", "Notes helper")
                        .subtitle("Last hour")
                        .call(FanoutCall::new("Read note", 31, 40.))
                        .call(FanoutCall::new("Spell check", 18, 120.).errors(1))
                        .call(FanoutCall::new("Translate", 6, 900.).errors(2)),
                ),
            )
            .child(div().text_sm().text_color(quiet).child(self.picked.clone()))
    }
}
