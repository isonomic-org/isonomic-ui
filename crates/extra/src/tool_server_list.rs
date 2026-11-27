// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Tool server list: the tool servers an assistant is connected to, each with its state, a prompt to sign in or reconnect when it needs one, and a switch per tool.
//!
//! Grouped by server, one server opens at a time to show its tools and a
//! switch for all of them; a server that failed shows what went wrong. The
//! other view is one list of every tool with a search field. Switches are off
//! limits until their server is connected. Nothing here changes the data: the
//! host hears every switch, sign-in and reconnect and passes new data back.

use std::{rc::Rc, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{MotionReveal, TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    TextRole as _,
    button::{Button, ButtonVariants as _},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
    h_flex,
    input::{Input, InputState},
    look,
    rolling_number::RollingNumber,
    spinner::Spinner,
    switch::Switch,
    tag::Tag,
    v_flex,
};
use isonomic_ui::{
    AnyElement, App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

use crate::common::{inset, panel, shell, shell_header};

/// How long a server takes to open.
const OPEN: Duration = Duration::from_millis(300);

/// Where a server stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    /// Ready: its tools can be switched.
    Connected,
    /// Still starting up.
    Connecting,
    /// Waiting for the person to sign in.
    NeedsSignIn,
    /// The connection failed.
    Failed,
}

impl ServerStatus {
    /// The status in words.
    pub fn label(self) -> &'static str {
        match self {
            ServerStatus::Connected => "Connected",
            ServerStatus::Connecting => "Connecting",
            ServerStatus::NeedsSignIn => "Needs sign-in",
            ServerStatus::Failed => "Error",
        }
    }
}

/// How a server is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// A program on this machine, over its standard streams.
    Stdio,
    /// A service over the network.
    Http,
}

impl Transport {
    /// The short name in the server's tag.
    pub fn label(self) -> &'static str {
        match self {
            Transport::Stdio => "stdio",
            Transport::Http => "http",
        }
    }
}

/// One tool a server offers.
#[derive(Debug, Clone)]
pub struct ServerTool {
    name: SharedString,
    description: Option<SharedString>,
    enabled: bool,
}

impl ServerTool {
    /// A tool, switched on or off.
    pub fn new(name: impl Into<SharedString>, enabled: bool) -> Self {
        Self {
            name: name.into(),
            description: None,
            enabled,
        }
    }

    /// A line about what it does.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A tool server.
#[derive(Debug, Clone)]
pub struct ToolServer {
    id: SharedString,
    name: SharedString,
    transport: Transport,
    status: ServerStatus,
    error: Option<SharedString>,
    tools: Vec<ServerTool>,
}

impl ToolServer {
    /// A connected server with no tools yet.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        transport: Transport,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            transport,
            status: ServerStatus::Connected,
            error: None,
            tools: Vec::new(),
        }
    }

    /// Where it stands.
    pub fn status(mut self, status: ServerStatus) -> Self {
        self.status = status;
        self
    }

    /// What went wrong, shown under a failed server.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Add a tool.
    pub fn tool(mut self, tool: ServerTool) -> Self {
        self.tools.push(tool);
        self
    }
}

/// A tool switch that was flipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSwitch {
    /// The server's id.
    pub server: SharedString,
    /// The tool's name.
    pub tool: SharedString,
    /// What it was flipped to.
    pub enabled: bool,
}

/// How many of these tools are switched on.
pub fn enabled_count(tools: &[ServerTool]) -> usize {
    tools.iter().filter(|tool| tool.enabled).count()
}

/// How many servers are connected.
pub fn connected_count(servers: &[ToolServer]) -> usize {
    servers
        .iter()
        .filter(|server| server.status == ServerStatus::Connected)
        .count()
}

/// The count under a server's name: "3/4 tools".
pub fn tool_count_label(on: usize, total: usize) -> String {
    format!("{on}/{total} tools")
}

/// The tools of a server whose name holds the query, whatever the case.
pub fn matching_tools(server: &ToolServer, query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    server
        .tools
        .iter()
        .enumerate()
        .filter(|(_, tool)| query.is_empty() || tool.name.to_lowercase().contains(&query))
        .map(|(ix, _)| ix)
        .collect()
}

/// What needs attention, as it is spoken: every server that is not connected.
pub fn attention_summary(servers: &[ToolServer]) -> String {
    servers
        .iter()
        .filter(|server| server.status != ServerStatus::Connected)
        .map(|server| format!("{}: {}", server.name, server.status.label()))
        .collect::<Vec<_>>()
        .join(". ")
}

type ToolHandler = Rc<dyn Fn(&ToolSwitch, &mut Window, &mut App)>;
type ServerSwitchHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
type ServerHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// The tool servers and their tools.
#[derive(IntoElement)]
pub struct ToolServerList {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    servers: Vec<ToolServer>,
    by_tool: bool,
    default_expanded: Option<SharedString>,
    on_toggle_tool: Option<ToolHandler>,
    on_toggle_server: Option<ServerSwitchHandler>,
    on_sign_in: Option<ServerHandler>,
    on_reconnect: Option<ServerHandler>,
}

impl ToolServerList {
    /// Create the list, grouped by server.
    pub fn new(id: impl Into<ElementId>, servers: Vec<ToolServer>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Tool servers".into(),
            servers,
            by_tool: false,
            default_expanded: None,
            on_toggle_tool: None,
            on_toggle_server: None,
            on_sign_in: None,
            on_reconnect: None,
        }
    }

    /// The header's title.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Show every tool in one list with a search field instead.
    pub fn by_tool(mut self) -> Self {
        self.by_tool = true;
        self
    }

    /// The server that is open at first.
    pub fn default_expanded(mut self, server: impl Into<SharedString>) -> Self {
        self.default_expanded = Some(server.into());
        self
    }

    /// Run when one tool is switched.
    pub fn on_toggle_tool(
        mut self,
        handler: impl Fn(&ToolSwitch, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_tool = Some(Rc::new(handler));
        self
    }

    /// Run with a server's id when all of its tools are switched at once.
    pub fn on_toggle_server(
        mut self,
        handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_server = Some(Rc::new(handler));
        self
    }

    /// Run with a server's id when Sign in is pressed.
    pub fn on_sign_in(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sign_in = Some(Rc::new(handler));
        self
    }

    /// Run with a server's id when Reconnect is pressed.
    pub fn on_reconnect(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reconnect = Some(Rc::new(handler));
        self
    }
}

impl Styled for ToolServerList {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A status as an icon and a word in its colour.
fn status_mark(status: ServerStatus, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let (color, mark): (Hsla, AnyElement) = match status {
        ServerStatus::Connected => (
            theme.success,
            Icon::new(CatalogIcon::CheckmarkFilled)
                .size(px(14.))
                .into_any_element(),
        ),
        ServerStatus::Connecting => (
            theme.muted_foreground,
            Spinner::new().xsmall().into_any_element(),
        ),
        ServerStatus::NeedsSignIn => (
            theme.warning,
            Icon::new(CatalogIcon::Password)
                .size(px(14.))
                .into_any_element(),
        ),
        ServerStatus::Failed => (
            theme.danger,
            Icon::new(CatalogIcon::WarningFilled)
                .size(px(14.))
                .into_any_element(),
        ),
    };
    h_flex()
        .flex_none()
        .gap_1()
        .text_caption()
        .text_color(color)
        .child(mark)
        .child(status.label())
}

impl RenderOnce for ToolServerList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.id.clone();
        let part = |name: &str| ElementId::NamedChild(id.clone().into(), name.to_string().into());
        let still = cx.reduce_motion();
        let total_tools: usize = self.servers.iter().map(|server| server.tools.len()).sum();
        let enabled: usize = self
            .servers
            .iter()
            .map(|server| enabled_count(&server.tools))
            .sum();
        let connected = connected_count(&self.servers);

        let first = self.default_expanded.clone();
        let open = window.use_keyed_state(part("open"), cx, |_, _| first);
        let open_now = open.read(cx).clone();
        let search = window.use_keyed_state(part("search"), cx, |window, cx| {
            InputState::new(window, cx).placeholder("Search tools")
        });
        let query = search.read(cx).value();

        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let danger = theme.danger;
        let mono = theme.mono_font_family.clone();
        let wash = look::ink::wash(cx);

        let tool_row = |server: &ToolServer, ix: usize| {
            let tool = &server.tools[ix];
            let toggle = self.on_toggle_tool.clone();
            let flipped = ToolSwitch {
                server: server.id.clone(),
                tool: tool.name.clone(),
                enabled: !tool.enabled,
            };
            h_flex()
                .gap_3()
                .rounded(px(12.))
                .px_3()
                .py_2()
                .hover(|this| this.bg(wash))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .truncate()
                                .font_family(mono.clone())
                                .text_size(px(13.))
                                .line_height(px(20.))
                                .child(tool.name.clone()),
                        )
                        .when_some(tool.description.clone(), |this, description| {
                            this.child(
                                div()
                                    .truncate()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(description),
                            )
                        }),
                )
                .child(
                    Switch::new(part(&format!("tool-{}-{}", server.id, tool.name)))
                        .checked(tool.enabled)
                        .disabled(server.status != ServerStatus::Connected)
                        .accessibility_label(format!("{} on {}", tool.name, server.name))
                        .on_change(move |_, window, cx| {
                            if let Some(toggle) = &toggle {
                                toggle(&flipped, window, cx);
                            }
                        }),
                )
        };

        let aside = h_flex().gap_1().text_body().text_color(quiet).map(|this| {
            if self.by_tool {
                this.child(
                    RollingNumber::new(part("enabled"), enabled.to_string()).line_height(px(20.)),
                )
                .child(format!("of {total_tools} tools enabled"))
            } else {
                this.child(
                    RollingNumber::new(part("connected"), connected.to_string())
                        .line_height(px(20.)),
                )
                .child(format!("of {} connected", self.servers.len()))
            }
        });

        let body: AnyElement = if self.by_tool {
            let groups = self
                .servers
                .iter()
                .filter_map(|server| {
                    let tools = matching_tools(server, &query);
                    (!tools.is_empty()).then(|| {
                        v_flex()
                            .mt_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .px_3()
                                    .py(px(6.))
                                    .text_caption()
                                    .child(div().font_medium().child(server.name.clone()))
                                    .when(server.status != ServerStatus::Connected, |this| {
                                        this.child(status_mark(server.status, cx))
                                    }),
                            )
                            .children(tools.into_iter().map(|ix| tool_row(server, ix)))
                    })
                })
                .collect::<Vec<_>>();
            let nothing = groups.is_empty() && !query.trim().is_empty();
            panel(cx)
                .p_2()
                .child(
                    div().id(part("search")).test_support().child(
                        Input::new(&search)
                            .prefix(Icon::new(IconName::Search).xsmall().text_color(quiet))
                            .cleanable(true),
                    ),
                )
                .children(groups)
                .when(nothing, |this| {
                    this.child(
                        Empty::new().py_6().header(
                            EmptyHeader::new()
                                .media(
                                    EmptyMedia::new()
                                        .with_variant(EmptyMediaVariant::Icon)
                                        .child(Icon::new(IconName::Search)),
                                )
                                .title(EmptyTitle::new().child("No tools match"))
                                .description(
                                    EmptyDescription::new()
                                        .child(format!("Nothing matches “{}”.", query.trim())),
                                ),
                        ),
                    )
                })
                .into_any_element()
        } else {
            let mut cards: Vec<AnyElement> = Vec::new();
            for server in &self.servers {
                let expanded = open_now.as_ref() == Some(&server.id);
                let on = enabled_count(&server.tools);
                let shown = if still {
                    f32::from(expanded)
                } else {
                    transition(
                        part(&format!("reveal-{}", server.id)),
                        f32::from(expanded),
                        Transition::new(OPEN),
                        window,
                        cx,
                    )
                };
                let toggle_open = open.clone();
                let toggle_id = server.id.clone();
                let head = h_flex()
                    .id(part(&format!("server-{}", server.id)))
                    .test_support()
                    .role(Role::Button)
                    .aria_label(format!(
                        "{}, {}, {}, {}",
                        server.name,
                        server.transport.label(),
                        server.status.label(),
                        tool_count_label(on, server.tools.len())
                    ))
                    .aria_expanded(expanded)
                    .tab_index(0)
                    .min_w_0()
                    .flex_1()
                    .gap_3()
                    .cursor_pointer()
                    .child(
                        Icon::new(if expanded {
                            CatalogIcon::ChevronDown
                        } else {
                            CatalogIcon::ChevronRight
                        })
                        .size(px(16.))
                        .text_color(quiet),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_label()
                                            .child(server.name.clone()),
                                    )
                                    .child(
                                        Tag::secondary()
                                            .rounded_full()
                                            .child(server.transport.label()),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .mt(px(2.))
                                    .gap_2()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(status_mark(server.status, cx))
                                    .child("·")
                                    .child(tool_count_label(on, server.tools.len())),
                            ),
                    )
                    .on_click(move |_, _, cx| {
                        toggle_open.update(cx, |open, cx| {
                            *open = if expanded {
                                None
                            } else {
                                Some(toggle_id.clone())
                            };
                            cx.notify();
                        });
                    });
                let action = match server.status {
                    ServerStatus::NeedsSignIn => {
                        let sign_in = self.on_sign_in.clone();
                        let server_id = server.id.clone();
                        Some(
                            Button::new(part(&format!("sign-in-{}", server.id)))
                                .primary()
                                .small()
                                .label("Sign in")
                                .on_click(move |_, window, cx| {
                                    if let Some(sign_in) = &sign_in {
                                        sign_in(&server_id, window, cx);
                                    }
                                }),
                        )
                    }
                    ServerStatus::Failed => {
                        let reconnect = self.on_reconnect.clone();
                        let server_id = server.id.clone();
                        Some(
                            Button::new(part(&format!("reconnect-{}", server.id)))
                                .secondary()
                                .small()
                                .icon(Icon::new(CatalogIcon::Renew))
                                .label("Reconnect")
                                .on_click(move |_, window, cx| {
                                    if let Some(reconnect) = &reconnect {
                                        reconnect(&server_id, window, cx);
                                    }
                                }),
                        )
                    }
                    _ => None,
                };
                let failure = server
                    .error
                    .clone()
                    .filter(|_| server.status == ServerStatus::Failed)
                    .map(|error| {
                        div()
                            .mx(px(14.))
                            .mb_3()
                            .rounded(px(12.))
                            .bg(danger.opacity(0.1))
                            .px_3()
                            .py_2()
                            .font_family(mono.clone())
                            .text_caption()
                            .text_color(danger)
                            .child(error)
                    });
                let tools = (shown > 0.).then(|| {
                    let switch_all = self.on_toggle_server.clone();
                    let server_id = server.id.clone();
                    let all_on = on == server.tools.len() && on > 0;
                    let list = inset(cx)
                        .mx(px(6.))
                        .mb(px(6.))
                        .p_1()
                        .child(
                            h_flex()
                                .justify_between()
                                .px_3()
                                .py(px(6.))
                                .text_caption()
                                .text_color(quiet)
                                .child("All tools")
                                .child(
                                    Switch::new(part(&format!("all-{}", server.id)))
                                        .checked(all_on)
                                        .disabled(server.status != ServerStatus::Connected)
                                        .accessibility_label(format!(
                                            "All tools on {}",
                                            server.name
                                        ))
                                        .on_change(move |_, window, cx| {
                                            if let Some(switch_all) = &switch_all {
                                                switch_all(&server_id, !all_on, window, cx);
                                            }
                                        }),
                                ),
                        )
                        .children((0..server.tools.len()).map(|ix| tool_row(server, ix)));
                    MotionReveal::new(
                        part(&format!("tools-{}", server.id)),
                        shown,
                        div().opacity(shown).child(list).into_any_element(),
                    )
                });
                cards.push(
                    panel(cx)
                        .child(
                            h_flex()
                                .gap_3()
                                .py(px(10.))
                                .pl(px(14.))
                                .pr(px(10.))
                                .child(head)
                                .children(action),
                        )
                        .children(failure)
                        .children(tools)
                        .into_any_element(),
                );
            }
            v_flex().gap(px(6.)).children(cards).into_any_element()
        };

        let attention = attention_summary(&self.servers);
        shell(cx)
            .id(self.id.clone())
            .max_w(px(512.))
            .role(Role::Region)
            .aria_label(if attention.is_empty() {
                self.title.to_string()
            } else {
                format!("{}. {attention}", self.title)
            })
            .child(shell_header(
                Some(Icon::new(CatalogIcon::BareMetalServer)),
                self.title.clone(),
                None,
                Some(aside.into_any_element()),
                cx,
            ))
            .child(body)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<ToolServer> {
        vec![
            ToolServer::new("stock", "Stockroom", Transport::Stdio)
                .tool(ServerTool::new("count_flour", true))
                .tool(ServerTool::new("Order_Yeast", false))
                .tool(ServerTool::new("list_suppliers", true)),
            ToolServer::new("till", "Till", Transport::Http)
                .status(ServerStatus::NeedsSignIn)
                .tool(ServerTool::new("refund_order", false)),
            ToolServer::new("ovens", "Ovens", Transport::Http)
                .status(ServerStatus::Failed)
                .error("timed out after 30s"),
        ]
    }

    #[test]
    fn counts_cover_tools_and_servers() {
        let servers = sample();
        assert_eq!(enabled_count(&servers[0].tools), 2);
        assert_eq!(enabled_count(&servers[2].tools), 0);
        assert_eq!(connected_count(&servers), 1);
        assert_eq!(tool_count_label(2, 3), "2/3 tools");
    }

    #[test]
    fn search_ignores_case_and_outer_spaces() {
        let servers = sample();
        assert_eq!(matching_tools(&servers[0], ""), vec![0, 1, 2]);
        assert_eq!(matching_tools(&servers[0], "  ORDER "), vec![1]);
        assert_eq!(matching_tools(&servers[0], "o"), vec![0, 1]);
        assert!(matching_tools(&servers[1], "flour").is_empty());
    }

    #[test]
    fn servers_that_need_attention_are_spoken() {
        let servers = sample();
        assert_eq!(
            attention_summary(&servers),
            "Till: Needs sign-in. Ovens: Error"
        );
        assert_eq!(attention_summary(&servers[..1]), "");
    }

    #[test]
    fn statuses_and_transports_have_words() {
        assert_eq!(ServerStatus::Connecting.label(), "Connecting");
        assert_eq!(ServerStatus::Connected.label(), "Connected");
        assert_eq!(Transport::Stdio.label(), "stdio");
        assert_eq!(Transport::Http.label(), "http");
    }
}
