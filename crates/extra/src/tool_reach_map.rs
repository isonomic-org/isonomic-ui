// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tool reach map: every tool an assistant can reach, grouped by service, showing what it used, what failed and what it's not allowed to touch, with each call ready to replay.
//!
//! Each service is a group of tool chips. A chip is quiet when the tool was
//! never called, green with a count when it was, red when a call failed,
//! and dashed when the assistant isn't allowed to use it. Selecting a chip
//! lists its calls underneath, newest first, each with a Replay button.
//! Filters narrow the map to used, failed or blocked tools.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use crate::common::{Tone, resting_shadow};

/// One call to a tool.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// Identifies it.
    pub id: SharedString,
    /// When, as written.
    pub when: SharedString,
    /// Whether it worked.
    pub ok: bool,
    /// What went in and came out, in a line.
    pub summary: SharedString,
}

impl ToolCall {
    /// Create a call that worked.
    pub fn ok(
        id: impl Into<SharedString>,
        when: impl Into<SharedString>,
        summary: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            when: when.into(),
            ok: true,
            summary: summary.into(),
        }
    }

    /// Create a call that failed.
    pub fn failed(
        id: impl Into<SharedString>,
        when: impl Into<SharedString>,
        summary: impl Into<SharedString>,
    ) -> Self {
        Self {
            ok: false,
            ..Self::ok(id, when, summary)
        }
    }
}

/// One tool the assistant could reach.
#[derive(Debug, Clone, PartialEq)]
pub struct ReachTool {
    /// Identifies it.
    pub id: SharedString,
    /// The service it belongs to.
    pub service: SharedString,
    /// Its name.
    pub name: SharedString,
    /// Whether the assistant may use it.
    pub allowed: bool,
    /// Its calls, oldest first.
    pub calls: Vec<ToolCall>,
}

impl ReachTool {
    /// Create an allowed tool with no calls.
    pub fn new(
        id: impl Into<SharedString>,
        service: impl Into<SharedString>,
        name: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            service: service.into(),
            name: name.into(),
            allowed: true,
            calls: Vec::new(),
        }
    }

    /// Mark it as off-limits.
    pub fn blocked(mut self) -> Self {
        self.allowed = false;
        self
    }

    /// Add its calls.
    pub fn calls(mut self, calls: Vec<ToolCall>) -> Self {
        self.calls = calls;
        self
    }

    /// Where it stands.
    pub fn standing(&self) -> ReachStanding {
        if !self.allowed {
            ReachStanding::Blocked
        } else if self.calls.iter().any(|c| !c.ok) {
            ReachStanding::Failed
        } else if self.calls.is_empty() {
            ReachStanding::Unused
        } else {
            ReachStanding::Used
        }
    }
}

/// Where a tool stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachStanding {
    /// Never called.
    Unused,
    /// Called, every call worked.
    Used,
    /// A call failed.
    Failed,
    /// Off-limits.
    Blocked,
}

/// Services in the order they first appear.
pub fn services(tools: &[ReachTool]) -> Vec<SharedString> {
    let mut out: Vec<SharedString> = Vec::new();
    for tool in tools {
        if !out.contains(&tool.service) {
            out.push(tool.service.clone());
        }
    }
    out
}

type ToolHandler = Rc<dyn Fn(Option<&SharedString>, &mut Window, &mut App)>;
type FilterHandler = Rc<dyn Fn(Option<ReachStanding>, &mut Window, &mut App)>;
type ReplayHandler = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;

/// Every tool an assistant can reach.
#[derive(IntoElement)]
pub struct ToolReachMap {
    id: ElementId,
    style: StyleRefinement,
    worker: SharedString,
    tools: Vec<ReachTool>,
    filter: Option<ReachStanding>,
    selected: Option<SharedString>,
    on_select: Option<ToolHandler>,
    on_filter: Option<FilterHandler>,
    on_replay: Option<ReplayHandler>,
}

impl ToolReachMap {
    /// Create a map of an assistant's tools.
    pub fn new(
        id: impl Into<ElementId>,
        worker: impl Into<SharedString>,
        tools: Vec<ReachTool>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            worker: worker.into(),
            tools,
            filter: None,
            selected: None,
            on_select: None,
            on_filter: None,
            on_replay: None,
        }
    }

    /// Show only tools in this standing.
    pub fn filter(mut self, filter: Option<ReachStanding>) -> Self {
        self.filter = filter;
        self
    }

    /// The selected tool.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Run when a chip is clicked; `None` when the selected one is clicked again.
    pub fn on_select(
        mut self,
        handler: impl Fn(Option<&SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Run when a filter is picked.
    pub fn on_filter(
        mut self,
        handler: impl Fn(Option<ReachStanding>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_filter = Some(Rc::new(handler));
        self
    }

    /// Run with a tool and one of its calls to replay.
    pub fn on_replay(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_replay = Some(Rc::new(handler));
        self
    }
}

impl Styled for ToolReachMap {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for ToolReachMap {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();
        let ok_c = Tone::Success.color(cx);
        let bad_c = Tone::Danger.color(cx);
        let count = |s: ReachStanding| self.tools.iter().filter(|t| t.standing() == s).count();

        let filter_chip = |label: &'static str, want: Option<ReachStanding>, n: usize| {
            let on_filter = self.on_filter.clone();
            let active = self.filter == want;
            h_flex()
                .id(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("filter-{label}").into(),
                ))
                .gap_1()
                .h(px(24.))
                .px_2()
                .rounded_full()
                .border_1()
                .border_color(if active { ink } else { line })
                .when(active, |this| this.bg(chip))
                .cursor_pointer()
                .text_caption()
                .aria_selected(active)
                .on_click(move |_, window, cx| {
                    if let Some(on_filter) = &on_filter {
                        on_filter(want, window, cx);
                    }
                })
                .child(label)
                .child(div().text_color(quiet).child(n.to_string()))
        };

        let groups: Vec<_> = services(&self.tools)
            .into_iter()
            .filter_map(|service| {
                let tools: Vec<&ReachTool> = self
                    .tools
                    .iter()
                    .filter(|t| {
                        t.service == service && self.filter.is_none_or(|f| t.standing() == f)
                    })
                    .collect();
                if tools.is_empty() {
                    return None;
                }
                let chips: Vec<_> = tools
                    .iter()
                    .map(|tool| {
                        let standing = tool.standing();
                        let is_selected = self.selected.as_ref() == Some(&tool.id);
                        let fails = tool.calls.iter().filter(|c| !c.ok).count();
                        let (border, tint): (Hsla, Hsla) = match standing {
                            ReachStanding::Unused => (line, fill),
                            ReachStanding::Used => (ok_c.opacity(0.6), ok_c.opacity(0.1)),
                            ReachStanding::Failed => (bad_c.opacity(0.7), bad_c.opacity(0.1)),
                            ReachStanding::Blocked => (line, chip.opacity(0.4)),
                        };
                        let select = self.on_select.clone();
                        let id = tool.id.clone();
                        h_flex()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("tool-{}", tool.id).into(),
                            ))
                            .gap_1p5()
                            .h(px(28.))
                            .px_2()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(if is_selected { ink } else { border })
                            .when(standing == ReachStanding::Blocked, |this| {
                                this.border_dashed()
                            })
                            .bg(tint)
                            .cursor_pointer()
                            .role(Role::Button)
                            .aria_label(format!(
                                "{}, {}",
                                tool.name,
                                match standing {
                                    ReachStanding::Unused => "never called".to_string(),
                                    ReachStanding::Used =>
                                        format!("called {} times", tool.calls.len()),
                                    ReachStanding::Failed =>
                                        format!("{fails} of {} calls failed", tool.calls.len()),
                                    ReachStanding::Blocked => "not allowed".to_string(),
                                }
                            ))
                            .aria_selected(is_selected)
                            .on_click(move |_, window, cx| {
                                if let Some(select) = &select {
                                    select(if is_selected { None } else { Some(&id) }, window, cx);
                                }
                            })
                            .when(standing == ReachStanding::Blocked, |this| {
                                this.child(Icon::new(IconName::Ban).xsmall().text_color(quiet))
                            })
                            .child(
                                div()
                                    .text_caption()
                                    .font_family(mono.clone())
                                    .text_color(
                                        if standing == ReachStanding::Unused
                                            || standing == ReachStanding::Blocked
                                        {
                                            quiet
                                        } else {
                                            ink
                                        },
                                    )
                                    .child(tool.name.clone()),
                            )
                            .when(!tool.calls.is_empty(), |this| {
                                this.child(
                                    div()
                                        .text_label_small()
                                        .text_color(if fails > 0 { bad_c } else { ok_c })
                                        .child(if fails > 0 {
                                            format!("{fails}/{}", tool.calls.len())
                                        } else {
                                            tool.calls.len().to_string()
                                        }),
                                )
                            })
                    })
                    .collect();
                Some(
                    v_flex()
                        .gap_1p5()
                        .child(div().text_label_small().text_color(quiet).child(service))
                        .child(h_flex().flex_wrap().gap_1p5().children(chips)),
                )
            })
            .collect();

        let calls = self
            .selected
            .as_ref()
            .and_then(|id| self.tools.iter().find(|t| &t.id == id))
            .map(|tool| {
                let rows: Vec<_> =
                    tool.calls
                        .iter()
                        .rev()
                        .map(|call| {
                            let replay = self.on_replay.clone();
                            let (tool_id, call_id) = (tool.id.clone(), call.id.clone());
                            h_flex()
                                .gap_2()
                                .py_1p5()
                                .border_t_1()
                                .border_color(line)
                                .child(
                                    Icon::new(if call.ok {
                                        IconName::Check
                                    } else {
                                        IconName::Close
                                    })
                                    .xsmall()
                                    .text_color(if call.ok { ok_c } else { bad_c }),
                                )
                                .child(
                                    div()
                                        .w(px(64.))
                                        .flex_none()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(call.when.clone()),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_caption()
                                        .font_family(mono.clone())
                                        .child(call.summary.clone()),
                                )
                                .child(
                                    Button::new(ElementId::NamedChild(
                                        self.id.clone().into(),
                                        format!("replay-{}", call.id).into(),
                                    ))
                                    .xsmall()
                                    .ghost()
                                    .icon(IconName::RotateCw)
                                    .label("Replay")
                                    .accessibility_label(format!(
                                        "Replay the {} call to {}",
                                        call.when, tool.name
                                    ))
                                    .on_click(
                                        move |_, window, cx| {
                                            if let Some(replay) = &replay {
                                                replay(&tool_id, &call_id, window, cx);
                                            }
                                        },
                                    ),
                                )
                        })
                        .collect();
                v_flex()
                    .gap_1()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .font_medium()
                                    .font_family(mono.clone())
                                    .child(tool.name.clone()),
                            )
                            .child(
                                div()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(tool.service.clone()),
                            ),
                    )
                    .when(!tool.allowed, |this| {
                        this.child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .child("This assistant isn't allowed to use this tool."),
                        )
                    })
                    .when(tool.allowed && tool.calls.is_empty(), |this| {
                        this.child(
                            div()
                                .text_caption()
                                .text_color(quiet)
                                .child("Never called."),
                        )
                    })
                    .children(rows)
            });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(760.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::Bot).small().text_color(quiet))
                            .child(
                                div()
                                    .flex_1()
                                    .font_medium()
                                    .child(format!("What {} can reach", self.worker)),
                            )
                            .child(
                                div()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(format!("{} tools", self.tools.len())),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1p5()
                            .child(filter_chip("All", None, self.tools.len()))
                            .child(filter_chip(
                                "Used",
                                Some(ReachStanding::Used),
                                count(ReachStanding::Used),
                            ))
                            .child(filter_chip(
                                "Failed",
                                Some(ReachStanding::Failed),
                                count(ReachStanding::Failed),
                            ))
                            .child(filter_chip(
                                "Never called",
                                Some(ReachStanding::Unused),
                                count(ReachStanding::Unused),
                            ))
                            .child(filter_chip(
                                "Not allowed",
                                Some(ReachStanding::Blocked),
                                count(ReachStanding::Blocked),
                            )),
                    ),
            )
            .child(v_flex().gap_3().p_4().children(groups))
            .children(calls)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_follows_permission_and_calls() {
        let used = ReachTool::new("a", "Ledger", "read").calls(vec![ToolCall::ok("1", "now", "x")]);
        let failed = ReachTool::new("b", "Ledger", "post").calls(vec![
            ToolCall::ok("1", "now", "x"),
            ToolCall::failed("2", "now", "y"),
        ]);
        assert_eq!(used.standing(), ReachStanding::Used);
        assert_eq!(failed.standing(), ReachStanding::Failed);
        assert_eq!(
            ReachTool::new("c", "Mail", "send").blocked().standing(),
            ReachStanding::Blocked
        );
        assert_eq!(
            ReachTool::new("d", "Mail", "read").standing(),
            ReachStanding::Unused
        );
        assert_eq!(services(&[used, failed]), ["Ledger"]);
    }
}
