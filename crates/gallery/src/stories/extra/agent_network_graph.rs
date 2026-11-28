// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the agent network graph.

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, SharedString, Window,
};
use isonomic_ui_extra::agent_network_graph::{
    AgentNetworkGraph, NetworkLink, NetworkNode, NetworkNodeKind,
};
use isonomic_ui_extra::common::Tone;

/// Shows who a close coordinator's team talks to. The story keeps the
/// selected node.
pub struct AgentNetworkGraphStory {
    selected: Option<SharedString>,
}

fn nodes() -> Vec<NetworkNode> {
    use NetworkNodeKind::*;
    vec![
        NetworkNode::new("lead", "Close coordinator", Agent).tone(Tone::Info),
        NetworkNode::new("recon", "Reconciler", Agent).tone(Tone::Info),
        NetworkNode::new("ledger", "Ledger", Tool),
        NetworkNode::new("bank", "Bank feed", Tool),
        NetworkNode::new("cards", "Card matcher", Agent).tone(Tone::Danger),
        NetworkNode::new("payables", "Payables assistant", Agent).tone(Tone::Warning),
        NetworkNode::new("maya", "Maya Okafor", Person),
        NetworkNode::new("mail", "Mail", Tool),
    ]
}

fn links() -> Vec<NetworkLink> {
    vec![
        NetworkLink::new("lead", "recon", 14).live(),
        NetworkLink::new("lead", "payables", 6),
        NetworkLink::new("lead", "maya", 3),
        NetworkLink::new("recon", "ledger", 42).live(),
        NetworkLink::new("recon", "bank", 18),
        NetworkLink::new("recon", "cards", 9),
        NetworkLink::new("cards", "ledger", 4),
        NetworkLink::new("payables", "maya", 2).live(),
        NetworkLink::new("payables", "mail", 5),
        NetworkLink::new("lead", "mail", 1),
    ]
}

impl AgentNetworkGraphStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { selected: None })
    }
}

impl Story for AgentNetworkGraphStory {
    fn title() -> &'static str {
        "Agent network graph"
    }

    fn description() -> &'static str {
        "Assistants, people and tools as nodes, joined by how much they talk."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AgentNetworkGraphStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let select = cx.entity();
        AgentNetworkGraph::new("close-network", nodes(), links())
            .selected(self.selected.clone())
            .on_select(move |id, _, cx| {
                select.update(cx, |this, cx| {
                    this.selected = id.cloned();
                    cx.notify();
                })
            })
    }
}
