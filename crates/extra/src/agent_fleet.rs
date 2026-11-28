// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Agent fleet: several agents side by side, each summing up its sub-agents.
//!
//! The bar on top adds up the whole fleet: agents, sub-agents, how many run,
//! how many need you and what it all costs. Every main agent is a card with
//! its status, task and a ring that shows how its sub-agents stand. Opening
//! one unfolds its sub-agents under it, joined by a line, while the other
//! cards fade back; opening another folds the first one away.
//!
//! A sub-agent with tasks, a question for you or an error opens a small window
//! under its card with its tasks and, where it needs you, the answer buttons.
//!
//! Keys: Up and Down move between the main agents, Home and End jump to the
//! ends, Enter or Space opens or folds the one with focus, and Escape closes
//! a sub-agent's window.

use std::time::Duration;

use isonomic_ui::assets::IconName as AssetIcon;
use isonomic_ui::base::{MotionReveal, Presence, PressableExt as _, transition};
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, h_flex, look, v_flex,
};
use isonomic_ui::{
    AnyElement, App, ClickEvent, Context, ElementId, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, ParentElement as _,
    PathBuilder, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _, Window, canvas, div, point, prelude::FluentBuilder as _, px,
};

use crate::common::{ease, ease_out, over, paint_share_ring};

/// Fixed height of a main agent's card.
const MAIN_H: f32 = 96.;
/// Fixed height of a sub-agent's card.
const SUB_H: f32 = 94.;
/// Space between sub-agent cards.
const SUB_GAP: f32 = 10.;
/// How far the sub-agents sit in from their main agent.
const INDENT: f32 = 32.;

/// Where an agent stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FleetStatus {
    /// Not started yet.
    #[default]
    Queued,
    /// At work.
    Running,
    /// Waiting for you to decide something.
    Waiting,
    /// Finished.
    Done,
    /// Stopped with an error.
    Failed,
    /// Stopped on purpose.
    Stopped,
}

impl FleetStatus {
    /// The status in words, as the card shows it.
    pub fn word(self) -> &'static str {
        match self {
            FleetStatus::Queued => "Queued",
            FleetStatus::Running => "Running",
            FleetStatus::Waiting => "Needs you",
            FleetStatus::Done => "Done",
            FleetStatus::Failed => "Failed",
            FleetStatus::Stopped => "Stopped",
        }
    }

    fn quiet(self) -> bool {
        matches!(self, FleetStatus::Queued | FleetStatus::Stopped)
    }

    /// Text and fill of the status chip.
    fn colors(self, cx: &App) -> (Hsla, Hsla) {
        let theme = cx.theme();
        let tint = |color: Hsla, alpha: f32| (color, color.alpha(alpha));
        match self {
            FleetStatus::Running => tint(theme.info, 0.16),
            FleetStatus::Waiting => tint(theme.warning, 0.16),
            FleetStatus::Done => tint(theme.success, 0.13),
            FleetStatus::Failed => tint(theme.danger, 0.15),
            FleetStatus::Queued | FleetStatus::Stopped => {
                (look::ink::muted(cx), look::ink::wash_strong(cx))
            }
        }
    }
}

/// One task a sub-agent worked through.
#[derive(Debug, Clone, PartialEq)]
pub struct FleetStep {
    /// What the task was.
    pub title: SharedString,
    /// Where it stands.
    pub status: FleetStatus,
    /// How long it took, such as "1m 12s".
    pub took: Option<SharedString>,
}

impl FleetStep {
    /// A task.
    pub fn new(title: impl Into<SharedString>, status: FleetStatus) -> Self {
        Self {
            title: title.into(),
            status,
            took: None,
        }
    }

    /// Set how long it took.
    pub fn took(mut self, took: impl Into<SharedString>) -> Self {
        self.took = Some(took.into());
        self
    }
}

/// An agent of the fleet, main or sub.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FleetAgent {
    /// Unique in the fleet.
    pub id: SharedString,
    /// Its name.
    pub name: SharedString,
    /// What it was asked to do.
    pub task: SharedString,
    /// The model it runs on.
    pub model: Option<SharedString>,
    /// Where it stands.
    pub status: FleetStatus,
    /// Its own cost, in dollars.
    pub cost: f64,
    /// How long it has run.
    pub took: Option<Duration>,
    /// Marks of the tools it uses, shown as overlapping tiles.
    pub tools: Vec<AssetIcon>,
    /// The tasks it worked through.
    pub steps: Vec<FleetStep>,
    /// What a waiting agent needs you to decide.
    pub ask: Option<SharedString>,
    /// What went wrong for a failed agent.
    pub error: Option<SharedString>,
    /// Its sub-agents.
    pub children: Vec<FleetAgent>,
}

impl FleetAgent {
    /// A queued agent with a task.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        task: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            task: task.into(),
            ..Self::default()
        }
    }

    /// Set where it stands.
    pub fn status(mut self, status: FleetStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the model it runs on.
    pub fn model(mut self, model: impl Into<SharedString>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set its own cost, in dollars.
    pub fn cost(mut self, cost: f64) -> Self {
        self.cost = cost;
        self
    }

    /// Set how long it has run.
    pub fn took(mut self, took: Duration) -> Self {
        self.took = Some(took);
        self
    }

    /// Add a tool mark.
    pub fn tool(mut self, tool: AssetIcon) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add a task it worked through.
    pub fn step(mut self, step: FleetStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Set what a waiting agent needs you to decide.
    pub fn ask(mut self, ask: impl Into<SharedString>) -> Self {
        self.ask = Some(ask.into());
        self
    }

    /// Set what went wrong for a failed agent.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Add a sub-agent.
    pub fn child(mut self, child: FleetAgent) -> Self {
        self.children.push(child);
        self
    }

    /// Its own cost and all its sub-agents' together.
    pub fn branch_cost(&self) -> f64 {
        self.cost
            + self
                .children
                .iter()
                .map(FleetAgent::branch_cost)
                .sum::<f64>()
    }

    fn has_window(&self) -> bool {
        !self.steps.is_empty() || self.needs_answer()
    }

    fn needs_answer(&self) -> bool {
        (self.status == FleetStatus::Waiting && self.ask.is_some())
            || (self.status == FleetStatus::Failed && self.error.is_some())
    }
}

/// An answer given for a sub-agent that needs you or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetAnswer {
    /// Go ahead with what it asked.
    Approve,
    /// Don't.
    Reject,
    /// Run it again.
    Retry,
    /// Leave it failed.
    Dismiss,
}

/// What the fleet reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FleetEvent {
    /// A main agent's tree was opened, or all were folded.
    Open(Option<SharedString>),
    /// An answer was given for a sub-agent.
    Answer(SharedString, FleetAnswer),
}

/// "$0.42", "$12.30".
pub fn format_cost(amount: f64) -> String {
    format!("${amount:.2}")
}

/// "14m 06s", "42s", "1h 02m".
pub fn format_took(took: Duration) -> String {
    let secs = took.as_secs();
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}

/// What the line under a main agent says about its sub-agents, and the share
/// of them that are done for its ring.
pub fn summary(agent: &FleetAgent) -> (String, f32) {
    let kids = &agent.children;
    let total = kids.len();
    if total == 0 {
        return (agent.status.word().to_string(), 0.);
    }
    let count = |status: FleetStatus| kids.iter().filter(|kid| kid.status == status).count();
    let done = count(FleetStatus::Done);
    let share = done as f32 / total as f32;
    let waiting = count(FleetStatus::Waiting);
    let failed = count(FleetStatus::Failed);
    let running = count(FleetStatus::Running);
    let line = if waiting > 0 {
        format!("{waiting} needs you")
    } else if failed > 0 {
        format!("{failed} failed")
    } else if done == total {
        format!("All {total} done")
    } else {
        format!("{running}/{total} running")
    };
    (line, share)
}

/// The bar's totals: agents, sub-agents, running, needing you, and cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FleetTotals {
    /// Main agents.
    pub agents: usize,
    /// Sub-agents at every depth.
    pub subagents: usize,
    /// Agents running, main and sub.
    pub running: usize,
    /// Agents waiting for you, main and sub.
    pub waiting: usize,
    /// Everything together, in dollars.
    pub cost: f64,
}

/// Add up a fleet.
pub fn totals(agents: &[FleetAgent]) -> FleetTotals {
    fn walk(agents: &[FleetAgent], totals: &mut FleetTotals, depth: usize) {
        for agent in agents {
            if depth > 0 {
                totals.subagents += 1;
            }
            totals.running += usize::from(agent.status == FleetStatus::Running);
            totals.waiting += usize::from(agent.status == FleetStatus::Waiting);
            totals.cost += agent.cost;
            walk(&agent.children, totals, depth + 1);
        }
    }
    let mut out = FleetTotals {
        agents: agents.len(),
        subagents: 0,
        running: 0,
        waiting: 0,
        cost: 0.,
    };
    walk(agents, &mut out, 0);
    out
}

/// Several agents side by side, one tree open at a time.
pub struct AgentFleet {
    title: SharedString,
    agents: Vec<FleetAgent>,
    open: Option<SharedString>,
    /// The sub-agent whose window is open.
    popped: Option<SharedString>,
    /// The main agent the arrows are on.
    cursor: usize,
    focus: FocusHandle,
}

impl EventEmitter<FleetEvent> for AgentFleet {}

impl Focusable for AgentFleet {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl AgentFleet {
    /// Create a fleet with the first agent's tree open.
    pub fn new(agents: Vec<FleetAgent>, cx: &mut Context<Self>) -> Self {
        let open = agents.first().map(|agent| agent.id.clone());
        Self {
            title: "Agents".into(),
            agents,
            open,
            popped: None,
            cursor: 0,
            focus: cx.focus_handle().tab_stop(true),
        }
    }

    /// Set the bar's heading. Defaults to "Agents".
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Start with this agent's tree open, or none.
    pub fn open(mut self, id: Option<impl Into<SharedString>>) -> Self {
        self.open = id.map(Into::into);
        if let Some(ix) = self
            .open
            .as_ref()
            .and_then(|id| self.agents.iter().position(|agent| &agent.id == id))
        {
            self.cursor = ix;
        }
        self
    }

    /// Replace the agents, keeping what is open if it still exists.
    pub fn set_agents(&mut self, agents: Vec<FleetAgent>, cx: &mut Context<Self>) {
        self.agents = agents;
        if let Some(open) = &self.open
            && !self.agents.iter().any(|agent| &agent.id == open)
        {
            self.open = None;
        }
        self.cursor = self.cursor.min(self.agents.len().saturating_sub(1));
        cx.notify();
    }

    /// The main agent whose tree is open.
    pub fn opened(&self) -> Option<&SharedString> {
        self.open.as_ref()
    }

    fn toggle(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(agent) = self.agents.get(ix) else {
            return;
        };
        self.cursor = ix;
        self.popped = None;
        self.open = if self.open.as_ref() == Some(&agent.id) {
            None
        } else {
            Some(agent.id.clone())
        };
        cx.emit(FleetEvent::Open(self.open.clone()));
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let last = self.agents.len().saturating_sub(1);
        match event.keystroke.key.as_str() {
            "escape" if self.popped.is_some() => self.popped = None,
            "up" => self.cursor = self.cursor.saturating_sub(1),
            "down" => self.cursor = (self.cursor + 1).min(last),
            "home" => self.cursor = 0,
            "end" => self.cursor = last,
            "enter" | "space" => self.toggle(self.cursor, cx),
            _ => return,
        }
        cx.notify();
        cx.stop_propagation();
    }

    fn answer(&mut self, id: SharedString, answer: FleetAnswer, cx: &mut Context<Self>) {
        self.popped = None;
        cx.emit(FleetEvent::Answer(id, answer));
        cx.notify();
    }
}

/// The status chip: a mark and the status in words.
fn status_chip(status: FleetStatus, cx: &App) -> impl IntoElement {
    let (ink, fill) = status.colors(cx);
    let mark: AnyElement = match status {
        FleetStatus::Running => isonomic_ui::component::spinner::Spinner::new()
            .with_size(px(11.))
            .color(ink)
            .into_any_element(),
        FleetStatus::Done => Icon::new(AssetIcon::Checkmark)
            .size(px(12.))
            .into_any_element(),
        FleetStatus::Waiting => Icon::new(AssetIcon::WarningAlt)
            .size(px(12.))
            .into_any_element(),
        FleetStatus::Failed => Icon::new(AssetIcon::Close).size(px(12.)).into_any_element(),
        FleetStatus::Queued => Icon::new(AssetIcon::Time).size(px(12.)).into_any_element(),
        FleetStatus::Stopped => Icon::new(AssetIcon::Stop).size(px(12.)).into_any_element(),
    };
    h_flex()
        .flex_none()
        .gap(px(4.))
        .h(px(18.))
        .pl(px(4.))
        .pr(px(6.))
        .rounded(px(4.))
        .bg(fill)
        .text_color(ink)
        .text_size(px(11.))
        .line_height(px(16.))
        .font_medium()
        .child(mark)
        .child(status.word())
}

/// Overlapping tiles with the tools' marks.
fn tool_tiles(tools: &[AssetIcon], cx: &App) -> impl IntoElement {
    let card = cx.theme().background;
    h_flex()
        .flex_none()
        .children(tools.iter().take(3).enumerate().map(|(ix, tool)| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(16.))
                .when(ix > 0, |this| this.ml(px(-4.)))
                .rounded(px(4.))
                .bg(card)
                .shadow(look::surface::resting(cx))
                .text_color(look::ink::icon(cx))
                .child(Icon::new(*tool).size(px(10.)))
        }))
        .when(tools.len() > 3, |this| {
            this.child(
                div()
                    .ml(px(-4.))
                    .h(px(16.))
                    .min_w(px(16.))
                    .px(px(3.))
                    .rounded(px(4.))
                    .bg(card)
                    .shadow(look::surface::resting(cx))
                    .text_size(px(9.))
                    .line_height(px(16.))
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("+{}", tools.len() - 3)),
            )
        })
}

/// Name, model and status: the top of every card.
fn card_top(agent: &FleetAgent, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    h_flex()
        .items_start()
        .gap(px(8.))
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(22.))
                .rounded(px(6.))
                .bg(theme.background)
                .shadow(look::surface::resting(cx))
                .text_color(look::ink::icon(cx))
                .child(Icon::new(AssetIcon::Bot).size(px(13.))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .text_size(px(13.))
                        .line_height(px(16.))
                        .font_medium()
                        .text_color(look::ink::strong(cx))
                        .child(agent.name.clone()),
                )
                .children(agent.model.clone().map(|model| {
                    div()
                        .truncate()
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(theme.muted_foreground)
                        .child(model)
                })),
        )
        .child(status_chip(agent.status, cx))
}

fn task_line(agent: &FleetAgent, cx: &App) -> impl IntoElement {
    h_flex()
        .gap(px(6.))
        .min_w_0()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(look::ink::muted(cx))
        .when(!agent.tools.is_empty(), |this| {
            this.child(tool_tiles(&agent.tools, cx))
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(agent.task.clone()),
        )
}

fn cost_line(cost: f64, took: Option<Duration>, cx: &App) -> impl IntoElement {
    h_flex()
        .flex_none()
        .gap(px(4.))
        .text_size(px(11.))
        .line_height(px(16.))
        .child(
            div()
                .font_medium()
                .text_color(cx.theme().foreground)
                .child(format_cost(cost)),
        )
        .children(took.map(|took| {
            div()
                .text_color(cx.theme().muted_foreground)
                .child(format!("· {}", format_took(took)))
        }))
}

/// A small button inside a card or its window.
fn small_button(
    id: ElementId,
    label: &'static str,
    primary: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .flex()
        .items_center()
        .h(px(20.))
        .px(px(8.))
        .rounded(px(5.))
        .text_size(px(11.))
        .line_height(px(16.))
        .font_medium()
        .map(|this| {
            if primary {
                this.bg(theme.foreground).text_color(theme.background)
            } else {
                this.bg(theme.background)
                    .shadow(look::surface::resting(cx))
                    .text_color(theme.foreground)
                    .hover(|this| this.bg(look::ink::wash(cx)))
            }
        })
        .pressable(true)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(label)
}

impl AgentFleet {
    fn render_main(
        &self,
        ix: usize,
        agent: &FleetAgent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_open = self.open.as_ref() == Some(&agent.id);
        let someone_open = self.open.is_some();
        let focused = self.focus.is_focused(window) && self.cursor == ix;
        let hovered = window.use_keyed_state(
            ElementId::Name(format!("fleet-hover-{}", agent.id).into()),
            cx,
            |_, _| false,
        );
        let is_hovered = *hovered.read(cx);
        let target = if !someone_open || is_open {
            1.
        } else if is_hovered || focused {
            0.85
        } else {
            0.45
        };
        let opacity = transition(
            ElementId::Name(format!("fleet-dim-{}", agent.id).into()),
            target,
            over(320, ease_out()),
            window,
            cx,
        );
        let lift = transition(
            ElementId::Name(format!("fleet-lift-{}", agent.id).into()),
            if is_hovered { 1.0_f32 } else { 0. },
            over(200, ease()),
            window,
            cx,
        );
        let (say, share) = summary(agent);
        let say_color = if agent
            .children
            .iter()
            .any(|kid| kid.status == FleetStatus::Waiting)
        {
            cx.theme().warning
        } else if agent
            .children
            .iter()
            .any(|kid| kid.status == FleetStatus::Failed)
        {
            cx.theme().danger
        } else {
            cx.theme().muted_foreground
        };
        let ring_color = say_color;
        let entity = cx.entity().downgrade();
        let focus = self.focus.clone();
        v_flex()
            .id(ElementId::Name(format!("fleet-main-{}", agent.id).into()))
            .test_support()
            .role(Role::Button)
            .aria_label(format!("{}, {}, {say}", agent.name, agent.status.word()))
            .aria_expanded(is_open)
            .gap(px(6.))
            .h(px(MAIN_H))
            .pt(px(10.))
            .px(px(10.))
            .pb(px(8.))
            .rounded(px(10.))
            .bg(cx.theme().background)
            .shadow(if lift > 0.5 {
                look::surface::resting_hover(cx)
            } else {
                look::surface::resting(cx)
            })
            .opacity(opacity)
            .when(focused, |this| {
                this.border_2().border_color(cx.theme().ring)
            })
            .pressable(true)
            .on_hover({
                move |over, window, cx| {
                    hovered.update(cx, |state, _| *state = *over);
                    window.refresh();
                }
            })
            .on_click(move |_, window, cx| {
                focus.focus(window, cx);
                _ = entity.update(cx, |this, cx| this.toggle(ix, cx));
            })
            .child(card_top(agent, cx))
            .child(task_line(agent, cx))
            .child(
                h_flex()
                    .gap(px(8.))
                    .min_h(px(20.))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(5.))
                            .text_size(px(11.))
                            .line_height(px(16.))
                            .font_medium()
                            .text_color(say_color)
                            .child(
                                canvas(
                                    |_, _, _| {},
                                    move |bounds, _, window, _| {
                                        paint_share_ring(bounds, share, ring_color, window)
                                    },
                                )
                                .flex_none()
                                .size(px(10.)),
                            )
                            .child(div().min_w_0().truncate().child(say)),
                    )
                    .child(cost_line(agent.branch_cost(), agent.took, cx)),
            )
            .into_any_element()
    }

    fn render_sub(
        &self,
        ix: usize,
        agent: &FleetAgent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entered = Presence::new(
            ElementId::Name(format!("fleet-rise-{}", agent.id).into()),
            true,
        )
        .transition(over(360, ease_out()).delay(Duration::from_millis(60 * ix.min(8) as u64)))
        .sample(window, cx)
        .progress;
        let popped = self.popped.as_ref() == Some(&agent.id);
        let quiet = agent.status.quiet();
        let clickable = agent.has_window();
        let tasks = match agent.steps.len() {
            0 => None,
            1 => Some("1 task".to_string()),
            n => Some(format!("{n} tasks")),
        };
        let entity = cx.entity().downgrade();
        let id = agent.id.clone();
        let card = v_flex()
            .id(ElementId::Name(format!("fleet-sub-{}", agent.id).into()))
            .test_support()
            .role(Role::TreeItem)
            .aria_label(format!("{}, {}", agent.name, agent.status.word()))
            .when(clickable, |this| this.aria_expanded(popped))
            .relative()
            .gap(px(6.))
            .h(px(SUB_H))
            .pt(px(10.))
            .px(px(10.))
            .pb(px(8.))
            .rounded(px(8.))
            .bg(cx.theme().background)
            .shadow(if popped {
                look::surface::resting_hover(cx)
            } else {
                look::surface::resting(cx)
            })
            .opacity(entered)
            .ml(px(-8. * (1. - entered)))
            .when(clickable, |this| {
                this.pressable(true).on_click(move |_, _, cx| {
                    let id = id.clone();
                    _ = entity.update(cx, |this, cx| {
                        this.popped = if this.popped.as_ref() == Some(&id) {
                            None
                        } else {
                            Some(id)
                        };
                        cx.notify();
                    });
                })
            })
            .child(
                div()
                    .when(quiet, |this| this.opacity(0.6))
                    .child(card_top(agent, cx)),
            )
            .child(
                div()
                    .when(quiet, |this| this.opacity(0.6))
                    .child(task_line(agent, cx)),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .min_h(px(20.))
                    .when(quiet, |this| this.opacity(0.6))
                    .child(cost_line(agent.cost, agent.took, cx))
                    .child(div().flex_1())
                    .map(|this| {
                        if agent.needs_answer() {
                            let label = if agent.status == FleetStatus::Failed {
                                "Resolve"
                            } else {
                                "Answer"
                            };
                            let entity = cx.entity().downgrade();
                            let id = agent.id.clone();
                            this.child(small_button(
                                ElementId::Name(format!("fleet-review-{}", agent.id).into()),
                                label,
                                false,
                                move |_, _, cx| {
                                    let id = id.clone();
                                    _ = entity.update(cx, |this, cx| {
                                        this.popped = Some(id);
                                        cx.notify();
                                    });
                                },
                                cx,
                            ))
                        } else {
                            this.children(tasks.map(|tasks| {
                                div()
                                    .text_size(px(11.))
                                    .line_height(px(16.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tasks)
                            }))
                        }
                    }),
            );
        v_flex()
            .child(card)
            .when(popped, |this| {
                this.child(self.render_window(agent, window, cx))
            })
            .into_any_element()
    }

    fn render_window(
        &self,
        agent: &FleetAgent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shown = Presence::new(
            ElementId::Name(format!("fleet-window-{}", agent.id).into()),
            true,
        )
        .transition(over(220, ease_out()))
        .sample(window, cx)
        .progress;
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let text = look::ink::muted(cx);
        let ask = match agent.status {
            FleetStatus::Waiting => agent
                .ask
                .clone()
                .map(|ask| ("Needs your answer", ask, true)),
            FleetStatus::Failed => agent
                .error
                .clone()
                .map(|error| ("What went wrong", error, false)),
            _ => None,
        };
        let done = agent
            .steps
            .iter()
            .filter(|step| step.status == FleetStatus::Done)
            .count();
        let id = agent.id.clone();
        let owner = cx.entity().downgrade();
        let answer = |answer: FleetAnswer| {
            let entity = owner.clone();
            let id = id.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                let id = id.clone();
                _ = entity.update(cx, |this, cx| this.answer(id, answer, cx));
            }
        };
        let mut steps: Vec<AnyElement> = Vec::new();
        for (ix, step) in agent.steps.iter().enumerate() {
            let strong = matches!(
                step.status,
                FleetStatus::Running | FleetStatus::Waiting | FleetStatus::Failed
            );
            let faint = step.status.quiet();
            let appear = Presence::new(
                ElementId::Name(format!("fleet-step-{}-{ix}", agent.id).into()),
                true,
            )
            .transition(over(260, ease_out()).delay(Duration::from_millis(40 * ix.min(8) as u64)))
            .sample(window, cx)
            .progress;
            steps.push(
                h_flex()
                    .gap(px(8.))
                    .h(px(28.))
                    .px(px(6.))
                    .rounded(px(5.))
                    .opacity(appear)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(if faint {
                                look::ink::faint(cx)
                            } else if strong {
                                look::ink::strong(cx)
                            } else {
                                text
                            })
                            .when(strong, |this| this.font_medium())
                            .child(step.title.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .line_height(px(16.))
                            .text_color(if faint {
                                look::ink::faint(cx)
                            } else if strong {
                                theme.foreground
                            } else {
                                muted
                            })
                            .when(strong, |this| this.font_medium())
                            .child(match (&step.took, step.status) {
                                (Some(took), FleetStatus::Done) => took.clone(),
                                (_, status) => status.word().into(),
                            }),
                    )
                    .into_any_element(),
            );
        }
        v_flex()
            .id(ElementId::Name(format!("fleet-tasks-{}", agent.id).into()))
            .test_support()
            .role(Role::Dialog)
            .aria_label(format!("Tasks of {}", agent.name))
            .mt(px(6. - 4. * (1. - shown)))
            .gap(px(2.))
            .p(px(6.))
            .rounded(px(10.))
            .bg(theme.popover)
            .shadow(look::surface::floating(cx))
            .opacity(shown)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when_some(ask, |this, (label, body, waiting)| {
                this.child(
                    v_flex()
                        .gap(px(4.))
                        .pt(px(4.))
                        .px(px(6.))
                        .pb(px(8.))
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .font_medium()
                                .child(label),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(text)
                                .child(body),
                        )
                        .child(
                            h_flex()
                                .justify_end()
                                .gap(px(6.))
                                .mt(px(6.))
                                .child(small_button(
                                    ElementId::Name(format!("fleet-no-{}", agent.id).into()),
                                    if waiting { "Reject" } else { "Dismiss" },
                                    false,
                                    answer(if waiting {
                                        FleetAnswer::Reject
                                    } else {
                                        FleetAnswer::Dismiss
                                    }),
                                    cx,
                                ))
                                .child(small_button(
                                    ElementId::Name(format!("fleet-yes-{}", agent.id).into()),
                                    if waiting { "Approve" } else { "Retry" },
                                    true,
                                    answer(if waiting {
                                        FleetAnswer::Approve
                                    } else {
                                        FleetAnswer::Retry
                                    }),
                                    cx,
                                )),
                        ),
                )
            })
            .when(!steps.is_empty(), |this| {
                this.child(
                    h_flex()
                        .justify_between()
                        .gap(px(8.))
                        .pt(px(2.))
                        .px(px(6.))
                        .pb(px(6.))
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_medium()
                        .child("Tasks")
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_normal()
                                .text_color(muted)
                                .child(format!("{done} of {} done", agent.steps.len())),
                        ),
                )
                .child(
                    v_flex()
                        .p(px(2.))
                        .rounded(px(7.))
                        .bg(theme.foreground.opacity(0.04))
                        .children(steps),
                )
            })
            .into_any_element()
    }

    fn render_branch(
        &self,
        agent: &FleetAgent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let link = cx.theme().info;
        let link_muted = look::ink::hairline(cx);
        let card = cx.theme().background;
        let muted: Vec<bool> = agent
            .children
            .iter()
            .map(|kid| kid.status.quiet())
            .collect();
        let popped_ix = agent
            .children
            .iter()
            .position(|kid| self.popped.as_ref() == Some(&kid.id));
        let drawn = Presence::new(
            ElementId::Name(format!("fleet-links-{}", agent.id).into()),
            true,
        )
        .transition(over(420, ease_out()))
        .sample(window, cx)
        .progress;
        let subs: Vec<AnyElement> = agent
            .children
            .iter()
            .enumerate()
            .map(|(ix, kid)| self.render_sub(ix, kid, window, cx))
            .collect();
        // Lines from the main card down the left, curving into every card.
        // Cards have a fixed height, so the elbows sit at known places; an
        // open window below a card pushes the ones after it down, so the
        // lines stop at the open card.
        let links = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let x = bounds.origin.x + px(16.);
                let top = bounds.origin.y - px(10.);
                let elbow = |ix: usize| bounds.origin.y + px(ix as f32 * (SUB_H + SUB_GAP) + 22.);
                let last = match popped_ix {
                    Some(ix) => ix,
                    None => muted.len().saturating_sub(1),
                };
                let end = elbow(last);
                let reach = top + (end - top) * drawn;
                let mut trunk = PathBuilder::stroke(px(1.5));
                trunk.move_to(point(x, top));
                trunk.line_to(point(x, reach));
                if let Ok(path) = trunk.build() {
                    window.paint_path(path, link);
                }
                for (ix, quiet) in muted.iter().enumerate().take(last + 1) {
                    let y = elbow(ix);
                    if y > reach {
                        break;
                    }
                    let color = if *quiet { link_muted } else { link };
                    let mut arm = PathBuilder::stroke(px(1.5));
                    arm.move_to(point(x, y - px(8.)));
                    arm.curve_to(point(x + px(8.), y), point(x, y));
                    arm.line_to(point(bounds.origin.x + px(INDENT), y));
                    if let Ok(path) = arm.build() {
                        window.paint_path(path, color);
                    }
                }
                // A dot where the line leaves the main card.
                let r = px(3.);
                let points: Vec<_> = (0..16)
                    .map(|i| {
                        let a = i as f32 / 16. * std::f32::consts::TAU;
                        point(x + r * a.cos(), top + r * a.sin())
                    })
                    .collect();
                let mut dot = PathBuilder::fill();
                dot.add_polygon(&points, true);
                if let Ok(path) = dot.build() {
                    window.paint_path(path, card);
                }
                let mut ring = PathBuilder::stroke(px(1.5));
                ring.move_to(points[0]);
                for p in points.iter().skip(1) {
                    ring.line_to(*p);
                }
                ring.line_to(points[0]);
                if let Ok(path) = ring.build() {
                    window.paint_path(path, link);
                }
            },
        )
        .absolute()
        .top_0()
        .left(px(-INDENT))
        .w(px(INDENT))
        .h_full();
        v_flex()
            .id(ElementId::Name(format!("fleet-branch-{}", agent.id).into()))
            .role(Role::Tree)
            .aria_label(format!("Sub-agents of {}", agent.name))
            .relative()
            .pl(px(INDENT))
            .pt(px(10.))
            .gap(px(SUB_GAP))
            .child(
                div()
                    .absolute()
                    .top(px(10.))
                    .left(px(INDENT))
                    .bottom_0()
                    .w(px(INDENT))
                    .child(links),
            )
            .children(subs)
            .into_any_element()
    }
}

impl Render for AgentFleet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let totals = totals(&self.agents);
        let agents = self.agents.clone();

        let mut list: Vec<AnyElement> = Vec::new();
        for (ix, agent) in agents.iter().enumerate() {
            list.push(self.render_main(ix, agent, window, cx));
            let is_open = self.open.as_ref() == Some(&agent.id);
            let height = transition(
                ElementId::Name(format!("fleet-open-{}", agent.id).into()),
                if is_open { 1.0_f32 } else { 0. },
                over(if is_open { 360 } else { 300 }, ease_out()),
                window,
                cx,
            );
            if (is_open || height > 0.) && !agent.children.is_empty() {
                let branch = self.render_branch(agent, window, cx);
                list.push(
                    MotionReveal::new(
                        ElementId::Name(format!("fleet-reveal-{}", agent.id).into()),
                        height,
                        div()
                            .w_full()
                            .px(px(4.))
                            .pb(px(4.))
                            .child(branch)
                            .into_any_element(),
                    )
                    .into_any_element(),
                );
            }
        }
        if agents.is_empty() {
            list.push(
                div()
                    .py(px(28.))
                    .px(px(12.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(look::ink::hairline(cx))
                    .text_center()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(muted)
                    .child("No agents running")
                    .into_any_element(),
            );
        }

        let sums = [
            Some(format!(
                "{} {} / {} {}",
                totals.agents,
                if totals.agents == 1 {
                    "agent"
                } else {
                    "agents"
                },
                totals.subagents,
                if totals.subagents == 1 {
                    "sub-agent"
                } else {
                    "sub-agents"
                }
            )),
            (totals.running > 0).then(|| format!("{} running", totals.running)),
            (totals.waiting > 0).then(|| format!("{} needs you", totals.waiting)),
        ];

        v_flex()
            .id("agent-fleet")
            .test_support()
            .track_focus(&self.focus)
            .key_context("AgentFleet")
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, {
                let entity = cx.entity().downgrade();
                move |_, _, cx| {
                    _ = entity.update(cx, |this, cx| {
                        if this.popped.take().is_some() {
                            cx.notify();
                        }
                    });
                }
            })
            .w_full()
            .text_color(theme.foreground)
            .text_body()
            .child(
                h_flex()
                    .id("fleet-bar")
                    .role(Role::Status)
                    .aria_label(
                        sums.iter()
                            .flatten()
                            .cloned()
                            .chain(std::iter::once(format_cost(totals.cost)))
                            .collect::<Vec<_>>()
                            .join(", "),
                    )
                    .flex_wrap()
                    .gap(px(8.))
                    .mb(px(16.))
                    .child(
                        div()
                            .flex_none()
                            .w(px(16.))
                            .text_color(theme.info)
                            .child(Icon::new(AssetIcon::MagicWand).size(px(14.))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(120.))
                            .truncate()
                            .font_medium()
                            .text_color(look::ink::strong(cx))
                            .child(self.title.clone()),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .justify_end()
                            .gap_x(px(10.))
                            .gap_y(px(4.))
                            .ml_auto()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(muted)
                            .children(sums.into_iter().flatten().map(|sum| div().child(sum)))
                            .child(
                                div()
                                    .min_w(px(44.))
                                    .text_right()
                                    .font_medium()
                                    .text_color(theme.foreground)
                                    .child(format_cost(totals.cost)),
                            ),
                    ),
            )
            .child(v_flex().gap(px(8.)).children(list))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fleet() -> Vec<FleetAgent> {
        vec![
            FleetAgent::new("launch", "Launch lead", "Ship the pricing page")
                .status(FleetStatus::Running)
                .cost(0.4)
                .child(
                    FleetAgent::new("copy", "Copy", "Write it")
                        .status(FleetStatus::Waiting)
                        .cost(0.2),
                )
                .child(FleetAgent::new("art", "Art", "Draw it").status(FleetStatus::Done)),
            FleetAgent::new("support", "Support lead", "Answer the queue")
                .status(FleetStatus::Running)
                .child(
                    FleetAgent::new("a", "A", "a")
                        .status(FleetStatus::Done)
                        .cost(1.),
                )
                .child(FleetAgent::new("b", "B", "b").status(FleetStatus::Running))
                .child(FleetAgent::new("c", "C", "c").status(FleetStatus::Queued)),
            FleetAgent::new("report", "Analyst", "Weekly report")
                .status(FleetStatus::Done)
                .child(FleetAgent::new("d", "D", "d").status(FleetStatus::Done))
                .child(FleetAgent::new("e", "E", "e").status(FleetStatus::Done)),
        ]
    }

    #[test]
    fn the_line_under_a_card_says_what_matters_most() {
        let agents = fleet();
        assert_eq!(summary(&agents[0]), ("1 needs you".to_string(), 0.5));
        let (line, share) = summary(&agents[1]);
        assert_eq!(line, "1/3 running");
        assert!((share - 1. / 3.).abs() < 1e-6);
        assert_eq!(summary(&agents[2]), ("All 2 done".to_string(), 1.));
    }

    #[test]
    fn totals_add_up_the_whole_fleet() {
        let totals = totals(&fleet());
        assert_eq!(totals.agents, 3);
        assert_eq!(totals.subagents, 7);
        assert_eq!(totals.running, 3);
        assert_eq!(totals.waiting, 1);
        assert!((totals.cost - 1.6).abs() < 1e-9);
    }

    #[test]
    fn a_branch_costs_its_own_and_its_sub_agents() {
        assert!((fleet()[0].branch_cost() - 0.6).abs() < 1e-9);
    }

    #[test]
    fn costs_and_times_read_plainly() {
        assert_eq!(format_cost(0.042), "$0.04");
        assert_eq!(format_cost(12.3), "$12.30");
        assert_eq!(format_took(Duration::from_secs(42)), "42s");
        assert_eq!(format_took(Duration::from_secs(846)), "14m 06s");
        assert_eq!(format_took(Duration::from_secs(3720)), "1h 02m");
    }
}
