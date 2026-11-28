// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the tool server list.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window,
};
use isonomic_ui_extra::tool_server_list::{
    ServerStatus, ServerTool, ToolServer, ToolServerList, Transport,
};

/// How long the made-up servers take to connect.
const CONNECT: Duration = Duration::from_millis(1200);

struct Server {
    id: &'static str,
    name: &'static str,
    transport: Transport,
    status: ServerStatus,
    error: Option<&'static str>,
    tools: Vec<(&'static str, &'static str, bool)>,
}

/// The tool servers of a made-up bakery's back office.
fn bakery() -> Vec<Server> {
    vec![
        Server {
            id: "stockroom",
            name: "Stockroom",
            transport: Transport::Stdio,
            status: ServerStatus::Connected,
            error: None,
            tools: vec![
                ("count_stock", "Count what is on a shelf", true),
                ("order_flour", "Place a flour order with the mill", true),
                ("list_suppliers", "Suppliers and their delivery days", true),
                ("write_off", "Record spoiled stock", false),
            ],
        },
        Server {
            id: "till",
            name: "Till",
            transport: Transport::Http,
            status: ServerStatus::NeedsSignIn,
            error: None,
            tools: vec![
                ("daily_takings", "Takings for a day, by hour", true),
                (
                    "refund_sale",
                    "Refund a sale to the card it came from",
                    false,
                ),
            ],
        },
        Server {
            id: "ovens",
            name: "Oven monitors",
            transport: Transport::Http,
            status: ServerStatus::Failed,
            error: Some("no answer from ovens.bakery.example:8443 after 30s"),
            tools: vec![
                ("read_temperature", "Temperature of each deck", true),
                ("set_timer", "Start a bake timer", true),
            ],
        },
        Server {
            id: "rota",
            name: "Staff rota",
            transport: Transport::Stdio,
            status: ServerStatus::Connecting,
            error: None,
            tools: vec![("who_is_in", "Who works a given shift", true)],
        },
    ]
}

/// Tool servers grouped by server, and as one searchable list of tools.
pub struct ToolServerListStory {
    servers: Vec<Server>,
}

impl ToolServerListStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { servers: bakery() })
    }

    fn data(&self) -> Vec<ToolServer> {
        self.servers
            .iter()
            .map(|server| {
                let mut out =
                    ToolServer::new(server.id, server.name, server.transport).status(server.status);
                if let Some(error) = server.error {
                    out = out.error(error);
                }
                for (name, description, enabled) in &server.tools {
                    out = out.tool(ServerTool::new(*name, *enabled).description(*description));
                }
                out
            })
            .collect()
    }

    fn server(&mut self, id: &SharedString) -> Option<&mut Server> {
        self.servers
            .iter_mut()
            .find(|server| server.id == id.as_ref())
    }

    /// Sign-in and reconnect both lead to a short wait, then a connection.
    fn connect(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if let Some(server) = self.server(id) {
            server.status = ServerStatus::Connecting;
            server.error = None;
        }
        cx.notify();
        let id = id.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CONNECT).await;
            this.update(cx, |this: &mut Self, cx| {
                if let Some(server) = this.server(&id) {
                    server.status = ServerStatus::Connected;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn list(&self, id: &'static str, cx: &mut Context<Self>) -> ToolServerList {
        let toggle = cx.entity();
        let toggle_all = cx.entity();
        let sign_in = cx.entity();
        let reconnect = cx.entity();
        ToolServerList::new(id, self.data())
            .on_toggle_tool(move |flip, _, cx| {
                toggle.update(cx, |this, cx| {
                    if let Some(server) = this.server(&flip.server) {
                        for tool in &mut server.tools {
                            if tool.0 == flip.tool.as_ref() {
                                tool.2 = flip.enabled;
                            }
                        }
                    }
                    cx.notify();
                })
            })
            .on_toggle_server(move |server, enabled, _, cx| {
                toggle_all.update(cx, |this, cx| {
                    if let Some(server) = this.server(server) {
                        for tool in &mut server.tools {
                            tool.2 = enabled;
                        }
                    }
                    cx.notify();
                })
            })
            .on_sign_in(move |server, _, cx| {
                sign_in.update(cx, |this, cx| this.connect(server, cx))
            })
            .on_reconnect(move |server, _, cx| {
                reconnect.update(cx, |this, cx| this.connect(server, cx))
            })
    }
}

impl Story for ToolServerListStory {
    fn title() -> &'static str {
        "Tool server list"
    }

    fn description() -> &'static str {
        "Connected tool servers with their state, sign-in and reconnect prompts, and a switch per tool."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ToolServerListStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_6()
            .child(self.list("servers", cx).default_expanded("stockroom"))
            .child(self.list("tools", cx).by_tool())
    }
}
