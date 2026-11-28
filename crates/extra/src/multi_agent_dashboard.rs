// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-agent dashboard: every assistant at work, as a filterable grid of tiles with totals.
//!
//! The strip at the top counts assistants by state and adds up what they've
//! spent. Each tile shows one assistant's task, how far along it is and when
//! it last reported, with pause, resume, stop and open buttons. Filtering and
//! every action go to the owner; the dashboard changes nothing itself.
//!
//! Working tiles pulse a live dot and can show the line the assistant is on.
//! As a dock, the dashboard folds to one compact row of pills that leaves
//! room for the rest of the screen; pressing a pill expands that assistant
//! into a panel with its progress, recent steps and actions. Left and Right
//! move between pills, Escape folds the panel away.

use std::{rc::Rc, time::Duration};

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, look, v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

/// One beat of the live dot on a working assistant.
const PULSE: Duration = Duration::from_millis(1600);

use crate::common::{Tone, resting_shadow};

/// What an assistant on the dashboard is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FleetState {
    /// Working.
    Working,
    /// Waiting on a person.
    Waiting,
    /// Paused by someone.
    Paused,
    /// Stopped with a problem.
    Failed,
    /// Finished.
    Done,
}

impl FleetState {
    /// Every state, in the order the dashboard counts them.
    pub const ALL: [FleetState; 5] = [
        Self::Working,
        Self::Waiting,
        Self::Paused,
        Self::Failed,
        Self::Done,
    ];

    fn tone(self) -> Tone {
        match self {
            Self::Working => Tone::Info,
            Self::Waiting => Tone::Warning,
            Self::Paused => Tone::Neutral,
            Self::Failed => Tone::Danger,
            Self::Done => Tone::Success,
        }
    }

    /// The state's name.
    pub fn word(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::Waiting => "Waiting",
            Self::Paused => "Paused",
            Self::Failed => "Failed",
            Self::Done => "Done",
        }
    }
}

/// Something the owner can ask of an assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetAction {
    /// Hold it where it is.
    Pause,
    /// Let it carry on.
    Resume,
    /// End its run.
    Stop,
    /// Look at it closely.
    Open,
}

/// One assistant's tile.
#[derive(Debug, Clone, PartialEq)]
pub struct FleetAgent {
    /// Identifies it.
    pub id: SharedString,
    /// Its name.
    pub name: SharedString,
    /// What it's doing, in a sentence.
    pub task: SharedString,
    /// Its state.
    pub state: FleetState,
    /// How far along, from 0 to 1, if known.
    pub progress: Option<f32>,
    /// Steps taken.
    pub steps: u32,
    /// What it has spent, in cents.
    pub spent_cents: u32,
    /// When it last reported, as written, such as "12s ago".
    pub last_seen: SharedString,
    /// The line it is on right now, such as the step it is running.
    pub live: Option<SharedString>,
    /// Its latest steps, oldest first, as when and what.
    pub activity: Vec<(SharedString, SharedString)>,
}

impl FleetAgent {
    /// Create a working assistant.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        task: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            task: task.into(),
            state: FleetState::Working,
            progress: None,
            steps: 0,
            spent_cents: 0,
            last_seen: "just now".into(),
            live: None,
            activity: Vec::new(),
        }
    }

    /// Set the line it is on right now. Working tiles show it under the task.
    pub fn live(mut self, line: impl Into<SharedString>) -> Self {
        self.live = Some(line.into());
        self
    }

    /// Add a step to its recent activity, shown when it is expanded.
    pub fn step(mut self, when: impl Into<SharedString>, what: impl Into<SharedString>) -> Self {
        self.activity.push((when.into(), what.into()));
        self
    }

    /// Set its state.
    pub fn state(mut self, state: FleetState) -> Self {
        self.state = state;
        self
    }

    /// Set how far along it is, from 0 to 1.
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = Some(progress.clamp(0., 1.));
        self
    }

    /// Set its steps, spend and when it last reported.
    pub fn usage(
        mut self,
        steps: u32,
        spent_cents: u32,
        last_seen: impl Into<SharedString>,
    ) -> Self {
        self.steps = steps;
        self.spent_cents = spent_cents;
        self.last_seen = last_seen.into();
        self
    }
}

/// How many assistants are in a state.
pub fn count_in(agents: &[FleetAgent], state: FleetState) -> usize {
    agents.iter().filter(|a| a.state == state).count()
}

/// Write cents as dollars.
pub fn dollars(cents: u32) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

/// The actions a tile offers in a state.
pub fn actions_for(state: FleetState) -> &'static [FleetAction] {
    match state {
        FleetState::Working | FleetState::Waiting => {
            &[FleetAction::Pause, FleetAction::Stop, FleetAction::Open]
        }
        FleetState::Paused => &[FleetAction::Resume, FleetAction::Stop, FleetAction::Open],
        FleetState::Failed => &[FleetAction::Resume, FleetAction::Open],
        FleetState::Done => &[FleetAction::Open],
    }
}

/// How the dashboard lays its assistants out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FleetLayout {
    /// Totals, filters and a grid of tiles.
    #[default]
    Grid,
    /// One compact row of pills that expands into one assistant.
    Dock,
}

/// The pill an arrow key moves to, among `count`; it wraps around.
fn dock_step(current: usize, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match key {
        "right" => Some((current + 1) % count),
        "left" => Some((current + count - 1) % count),
        "home" => Some(0),
        "end" => Some(count - 1),
        _ => None,
    }
}

/// How far along an assistant is, as a whole percent, if it says.
fn percent(agent: &FleetAgent) -> Option<u32> {
    match (agent.state, agent.progress) {
        (FleetState::Done, _) => Some(100),
        (_, Some(p)) => Some((p * 100.).round() as u32),
        (_, None) => None,
    }
}

/// What a screen reader hears for a dock pill.
fn pill_words(agent: &FleetAgent, expanded: bool) -> String {
    let mut words = format!("{}, {}", agent.name, agent.state.word().to_lowercase());
    if let Some(percent) = percent(agent).filter(|_| agent.state != FleetState::Done) {
        words.push_str(&format!(", {percent} percent"));
    }
    words.push_str(if expanded {
        ", expanded"
    } else {
        ", collapsed"
    });
    words
}

/// The dock's one-line summary: "6 assistants, 2 working, 1 needs you".
fn dock_summary(agents: &[FleetAgent]) -> String {
    let mut parts = vec![match agents.len() {
        1 => "1 assistant".to_string(),
        n => format!("{n} assistants"),
    }];
    let working = count_in(agents, FleetState::Working);
    if working > 0 {
        parts.push(format!("{working} working"));
    }
    let waiting = count_in(agents, FleetState::Waiting);
    if waiting > 0 {
        parts.push(if waiting == 1 {
            "1 needs you".to_string()
        } else {
            format!("{waiting} need you")
        });
    }
    let failed = count_in(agents, FleetState::Failed);
    if failed > 0 {
        parts.push(format!("{failed} failed"));
    }
    parts.join(", ")
}

/// The icon and word for an action.
fn action_face(action: FleetAction) -> (IconName, &'static str) {
    match action {
        FleetAction::Pause => (IconName::Pause, "Pause"),
        FleetAction::Resume => (IconName::Play, "Resume"),
        FleetAction::Stop => (IconName::Square, "Stop"),
        FleetAction::Open => (IconName::ExternalLink, "Open"),
    }
}

/// A dot that beats while the assistant works, and holds still under
/// reduced motion or in any other state.
fn live_dot(id: ElementId, color: Hsla, live: bool, still: bool) -> AnyElement {
    let dot = div().size(px(6.)).flex_none().rounded_full().bg(color);
    if live && !still {
        dot.with_animation(id, Animation::new(PULSE).repeat(), |this, t| {
            // Ease out from full to faint and back, once a beat.
            let wave = (t * std::f32::consts::TAU).cos() * 0.5 + 0.5;
            this.opacity(0.35 + 0.65 * wave)
        })
        .into_any_element()
    } else {
        dot.into_any_element()
    }
}

type FilterHandler = Rc<dyn Fn(Option<FleetState>, &mut Window, &mut App)>;
type ExpandHandler = Rc<dyn Fn(Option<&SharedString>, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&SharedString, FleetAction, &mut Window, &mut App)>;

/// A grid of assistants with totals and a state filter.
#[derive(IntoElement)]
pub struct MultiAgentDashboard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    agents: Vec<FleetAgent>,
    filter: Option<FleetState>,
    budget_cents: Option<u32>,
    on_filter: Option<FilterHandler>,
    on_action: Option<ActionHandler>,
    layout: FleetLayout,
    expanded: Option<SharedString>,
    on_expand: Option<ExpandHandler>,
}

impl MultiAgentDashboard {
    /// Create a dashboard.
    pub fn new(id: impl Into<ElementId>, agents: Vec<FleetAgent>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Assistants".into(),
            agents,
            filter: None,
            budget_cents: None,
            on_filter: None,
            on_action: None,
            layout: FleetLayout::Grid,
            expanded: None,
            on_expand: None,
        }
    }

    /// Lay the assistants out as a grid of tiles (the default) or a dock.
    pub fn layout(mut self, layout: FleetLayout) -> Self {
        self.layout = layout;
        self
    }

    /// The assistant the dock shows expanded, by id.
    pub fn expanded(mut self, id: Option<SharedString>) -> Self {
        self.expanded = id;
        self
    }

    /// Run when a dock pill is pressed, with the assistant to expand, or
    /// `None` to fold the panel away.
    pub fn on_expand(
        mut self,
        handler: impl Fn(Option<&SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_expand = Some(Rc::new(handler));
        self
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Show only assistants in this state.
    pub fn filter(mut self, state: Option<FleetState>) -> Self {
        self.filter = state;
        self
    }

    /// Compare total spend with a budget, in cents.
    pub fn budget(mut self, cents: u32) -> Self {
        self.budget_cents = Some(cents);
        self
    }

    /// Run when a filter chip is clicked; `None` shows everyone.
    pub fn on_filter(
        mut self,
        handler: impl Fn(Option<FleetState>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_filter = Some(Rc::new(handler));
        self
    }

    /// Run when a tile's button is clicked.
    pub fn on_action(
        mut self,
        handler: impl Fn(&SharedString, FleetAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl Styled for MultiAgentDashboard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct Paint {
    ink: Hsla,
    quiet: Hsla,
    fill: Hsla,
    line: Hsla,
    chip: Hsla,
}

impl RenderOnce for MultiAgentDashboard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.layout == FleetLayout::Dock {
            return self.dock(window, cx).into_any_element();
        }
        let still = cx.reduce_motion();
        let theme = cx.theme();
        let paint = Paint {
            ink: theme.foreground,
            quiet: theme.muted_foreground,
            fill: theme.background,
            line: theme.border,
            chip: theme.muted,
        };
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let danger = theme.danger;
        let tones: Vec<(FleetState, Hsla, Hsla)> = FleetState::ALL
            .iter()
            .map(|s| (*s, s.tone().color(cx), s.tone().tint(cx)))
            .collect();
        let tone_of = |state: FleetState| {
            tones
                .iter()
                .find(|(s, _, _)| *s == state)
                .map(|(_, color, tint)| (*color, *tint))
                .unwrap_or((paint.quiet, paint.chip))
        };

        let spent: u32 = self.agents.iter().map(|a| a.spent_cents).sum();
        let over_budget = self.budget_cents.is_some_and(|b| spent > b);

        let chip = |label: String, state: Option<FleetState>, count: usize| {
            let active = self.filter == state;
            let filter = self.on_filter.clone();
            let color = state.map(|s| tone_of(s).0);
            h_flex()
                .id(ElementId::NamedChild(
                    self.id.clone().into(),
                    format!("filter-{}", state.map(|s| s.word()).unwrap_or("all")).into(),
                ))
                .gap_1p5()
                .h(px(26.))
                .px_2p5()
                .rounded_full()
                .border_1()
                .border_color(if active { paint.ink } else { paint.line })
                .when(active, |this| this.bg(paint.chip))
                .cursor_pointer()
                .text_caption()
                .role(Role::Button)
                .aria_label(format!("{label}, {count}"))
                .aria_selected(active)
                .on_click(move |_, window, cx| {
                    if let Some(filter) = &filter {
                        filter(state, window, cx);
                    }
                })
                .children(color.map(|color| div().size(px(7.)).rounded_full().bg(color)))
                .child(label)
                .child(div().text_color(paint.quiet).child(count.to_string()))
        };

        let mut chips = vec![chip("All".into(), None, self.agents.len())];
        for state in FleetState::ALL {
            let count = count_in(&self.agents, state);
            if count > 0 || self.filter == Some(state) {
                chips.push(chip(state.word().into(), Some(state), count));
            }
        }

        let shown: Vec<&FleetAgent> = self
            .agents
            .iter()
            .filter(|a| self.filter.is_none_or(|f| a.state == f))
            .collect();

        let tiles: Vec<_> = shown
            .iter()
            .map(|agent| {
                let (color, tint) = tone_of(agent.state);
                let buttons: Vec<_> = actions_for(agent.state)
                    .iter()
                    .map(|action| {
                        let on_action = self.on_action.clone();
                        let id = agent.id.clone();
                        let action = *action;
                        let (icon, word) = action_face(action);
                        Button::new(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("{word}-{}", agent.id).into(),
                        ))
                        .xsmall()
                        .ghost()
                        .icon(icon)
                        .when(action == FleetAction::Stop, |this| this.danger())
                        .accessibility_label(format!("{word} {}", agent.name))
                        .on_click(move |_, window, cx| {
                            if let Some(on_action) = &on_action {
                                on_action(&id, action, window, cx);
                            }
                        })
                    })
                    .collect();
                v_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("tile-{}", agent.id).into(),
                    ))
                    .w(px(232.))
                    .flex_grow(1.)
                    .max_w(px(320.))
                    .gap_2()
                    .p_3()
                    .rounded(radius_lg)
                    .bg(paint.fill)
                    .shadow(resting_shadow(cx))
                    .role(Role::Group)
                    .aria_label(format!(
                        "{}, {}, {}",
                        agent.name,
                        agent.state.word(),
                        agent.task
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .size(px(26.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(7.))
                                    .bg(tint)
                                    .child(Icon::new(IconName::Bot).xsmall().text_color(color)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_medium()
                                    .child(agent.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .px_1p5()
                                    .rounded(px(4.))
                                    .bg(tint)
                                    .text_caption()
                                    .text_color(color)
                                    .child(agent.state.word()),
                            ),
                    )
                    .child(
                        div()
                            .h(px(36.))
                            .overflow_hidden()
                            .text_color(if agent.state == FleetState::Failed {
                                color
                            } else {
                                paint.ink
                            })
                            .line_height(relative(1.35))
                            .child(agent.task.clone()),
                    )
                    .children(
                        agent
                            .live
                            .clone()
                            .filter(|_| agent.state == FleetState::Working)
                            .map(|line| {
                                div()
                                    .truncate()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .text_caption()
                                    .text_color(paint.quiet)
                                    .child(line)
                            }),
                    )
                    .child(div().h(px(4.)).rounded_full().bg(paint.chip).child(
                        div().h_full().rounded_full().bg(color).w(relative(
                            match (agent.state, agent.progress) {
                                (FleetState::Done, _) => 1.,
                                (_, Some(p)) => p,
                                (_, None) => 0.,
                            },
                        )),
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .text_caption()
                            .text_color(paint.quiet)
                            .child(format!("{} steps", agent.steps))
                            .child(dollars(agent.spent_cents))
                            .child(div().flex_1())
                            .child(live_dot(
                                ElementId::NamedChild(
                                    self.id.clone().into(),
                                    format!("live-{}", agent.id).into(),
                                ),
                                color,
                                agent.state == FleetState::Working,
                                still,
                            ))
                            .child(
                                div()
                                    .flex_none()
                                    .max_w(px(96.))
                                    .truncate()
                                    .child(agent.last_seen.clone()),
                            ),
                    )
                    .child(h_flex().gap_0p5().justify_end().children(buttons))
            })
            .collect();

        let stat = |label: &'static str, value: String, color: Hsla| {
            v_flex()
                .gap_0p5()
                .child(div().text_caption().text_color(paint.quiet).child(label))
                .child(div().text_lg().font_medium().text_color(color).child(value))
        };

        v_flex()
            .id(self.id.clone())
            .w_full()
            .gap_3()
            .text_body()
            .text_color(paint.ink)
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_6()
                    .px_4()
                    .py_3()
                    .rounded(radius_lg)
                    .border_1()
                    .border_color(paint.line)
                    .bg(paint.fill)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(160.))
                            .child(div().font_medium().child(self.title.clone()))
                            .child(div().text_caption().text_color(paint.quiet).child(format!(
                                "{} running now",
                                count_in(&self.agents, FleetState::Working)
                            ))),
                    )
                    .child(stat(
                        "Working",
                        count_in(&self.agents, FleetState::Working).to_string(),
                        tone_of(FleetState::Working).0,
                    ))
                    .child(stat(
                        "Need you",
                        count_in(&self.agents, FleetState::Waiting).to_string(),
                        tone_of(FleetState::Waiting).0,
                    ))
                    .child(stat(
                        "Failed",
                        count_in(&self.agents, FleetState::Failed).to_string(),
                        tone_of(FleetState::Failed).0,
                    ))
                    .child(stat(
                        "Spent",
                        match self.budget_cents {
                            Some(budget) => format!("{} of {}", dollars(spent), dollars(budget)),
                            None => dollars(spent),
                        },
                        if over_budget { danger } else { paint.ink },
                    )),
            )
            .child(h_flex().flex_wrap().gap_1p5().children(chips))
            .child(
                h_flex()
                    .flex_wrap()
                    .items_start()
                    .gap_3()
                    .children(tiles)
                    .when(shown.is_empty(), |this| {
                        this.child(
                            div()
                                .w_full()
                                .py_6()
                                .rounded(radius)
                                .border_1()
                                .border_dashed()
                                .border_color(paint.line)
                                .text_center()
                                .text_color(paint.quiet)
                                .child("No assistants in this state."),
                        )
                    }),
            )
            .refine_style(&self.style)
            .into_any_element()
    }
}

/// The keyed focus state of the dock's pills.
#[derive(Default)]
struct DockUi {
    handles: Vec<FocusHandle>,
    hovered: Option<usize>,
}

impl MultiAgentDashboard {
    /// The compact dock and, below it, the one assistant it expands into.
    fn dock(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let still = cx.reduce_motion();
        let child_id = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let chip = theme.muted;
        let line = theme.border;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();

        let count = self.agents.len();
        let ui = window.use_keyed_state(child_id("dock".into()), cx, |_, _| DockUi::default());
        ui.update(cx, |ui, cx| {
            while ui.handles.len() < count {
                ui.handles.push(cx.focus_handle());
            }
        });
        let (handles, hovered) = {
            let ui = ui.read(cx);
            (Rc::new(ui.handles.clone()), ui.hovered)
        };
        let expanded_at = self
            .expanded
            .as_ref()
            .and_then(|id| self.agents.iter().position(|a| &a.id == id));
        let expand = self.on_expand.clone();

        let pills: Vec<_> = self
            .agents
            .iter()
            .enumerate()
            .map(|(n, agent)| {
                let tone = agent.state.tone();
                let color = tone.color(cx);
                let is_open = expanded_at == Some(n);
                let focused = handles[n].is_focused(window);
                let hover = look::hover_fade(
                    child_id(format!("pill-hover-{}", agent.id)),
                    hovered == Some(n),
                    window,
                    cx,
                );
                let rest = if is_open { chip } else { fill };
                div()
                    .id(child_id(format!("pill-area-{}", agent.id)))
                    .flex_none()
                    .on_hover({
                        let ui = ui.clone();
                        move |over, _, cx| {
                            ui.update(cx, |ui, cx| {
                                if *over {
                                    ui.hovered = Some(n);
                                } else if ui.hovered == Some(n) {
                                    ui.hovered = None;
                                }
                                cx.notify();
                            })
                        }
                    })
                    .child(
                        Pressable::new(child_id(format!("pill-{}", agent.id)))
                            .track_focus(&handles[n])
                            .selected(is_open)
                            .accessibility_label(pill_words(agent, is_open))
                            .on_click({
                                let expand = expand.clone();
                                let id = agent.id.clone();
                                move |_, window, cx| {
                                    if let Some(expand) = &expand {
                                        expand((!is_open).then_some(&id), window, cx);
                                    }
                                }
                            })
                            .gap_1p5()
                            .h(px(28.))
                            .pl_2()
                            .pr_2p5()
                            .rounded_full()
                            .bg(look::mix(rest, rest.blend(chip), hover))
                            .shadow(resting_shadow(cx))
                            .cursor_pointer()
                            .text_caption()
                            .when(focused, |this| this.focus_ring_style(window, cx))
                            .child(live_dot(
                                child_id(format!("pill-live-{}", agent.id)),
                                color,
                                agent.state == FleetState::Working,
                                still,
                            ))
                            .child(
                                div()
                                    .max_w(px(128.))
                                    .truncate()
                                    .font_medium()
                                    .child(agent.name.clone()),
                            )
                            .children(
                                percent(agent)
                                    .filter(|_| agent.state != FleetState::Done)
                                    .map(|p| div().text_color(quiet).child(format!("{p}%"))),
                            )
                            .when(agent.state != FleetState::Working, |this| {
                                this.child(div().text_color(color).child(agent.state.word()))
                            }),
                    )
            })
            .collect();

        let panel = expanded_at.map(|n| {
            let agent = &self.agents[n];
            let tone = agent.state.tone();
            let color = tone.color(cx);
            let tint = tone.tint(cx);
            let buttons: Vec<_> = actions_for(agent.state)
                .iter()
                .map(|action| {
                    let on_action = self.on_action.clone();
                    let id = agent.id.clone();
                    let action = *action;
                    let (icon, word) = action_face(action);
                    Button::new(child_id(format!("panel-{word}-{}", agent.id)))
                        .xsmall()
                        .map(|this| match action {
                            FleetAction::Stop => this.outline().danger(),
                            FleetAction::Open => this.primary(),
                            _ => this.outline(),
                        })
                        .icon(icon)
                        .label(word)
                        .accessibility_label(format!("{word} {}", agent.name))
                        .on_click(move |_, window, cx| {
                            if let Some(on_action) = &on_action {
                                on_action(&id, action, window, cx);
                            }
                        })
                })
                .collect();
            let fraction = percent(agent).map(|p| p as f32 / 100.).unwrap_or(0.);
            v_flex()
                .id(child_id(format!("panel-{}", agent.id)))
                .role(Role::Region)
                .aria_label(format!(
                    "{}, {}, {}",
                    agent.name,
                    agent.state.word().to_lowercase(),
                    agent.task
                ))
                .gap_3()
                .p_4()
                .rounded(radius_lg)
                .bg(fill)
                .shadow(look::surface::floating(cx))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .size(px(28.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(8.))
                                .bg(tint)
                                .child(Icon::new(IconName::Bot).small().text_color(color)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().truncate().font_medium().child(agent.name.clone()))
                                .child(
                                    div()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(format!("Last report {}", agent.last_seen)),
                                ),
                        )
                        .child(
                            h_flex()
                                .flex_none()
                                .gap_1p5()
                                .px_1p5()
                                .rounded(px(4.))
                                .bg(tint)
                                .text_caption()
                                .text_color(color)
                                .child(live_dot(
                                    child_id(format!("panel-live-{}", agent.id)),
                                    color,
                                    agent.state == FleetState::Working,
                                    still,
                                ))
                                .child(agent.state.word()),
                        )
                        .child(
                            Button::new(child_id("panel-close".into()))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .accessibility_label(format!("Fold {} away", agent.name))
                                .on_click({
                                    let expand = expand.clone();
                                    move |_, window, cx| {
                                        if let Some(expand) = &expand {
                                            expand(None, window, cx);
                                        }
                                    }
                                }),
                        ),
                )
                .child(
                    div()
                        .text_color(if agent.state == FleetState::Failed {
                            color
                        } else {
                            ink
                        })
                        .child(agent.task.clone()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div().flex_1().h(px(4.)).rounded_full().bg(chip).child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(color)
                                    .w(relative(fraction)),
                            ),
                        )
                        .child(div().flex_none().text_caption().text_color(quiet).child(
                            match percent(agent) {
                                Some(p) => format!("{p}%"),
                                None => "No estimate".to_string(),
                            },
                        )),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{} steps", agent.steps))
                        .child(format!("{} spent", dollars(agent.spent_cents))),
                )
                .children(
                    agent
                        .live
                        .clone()
                        .filter(|_| agent.state == FleetState::Working)
                        .map(|line| {
                            h_flex()
                                .gap_2()
                                .px_2p5()
                                .py_1p5()
                                .rounded(px(8.))
                                .bg(look::ink::wash(cx))
                                .font_family(mono.clone())
                                .text_caption()
                                .child(div().flex_1().min_w_0().truncate().child(line))
                        }),
                )
                .when(!agent.activity.is_empty(), |this| {
                    this.child(
                        v_flex()
                            .id(child_id(format!("panel-steps-{}", agent.id)))
                            .role(Role::List)
                            .aria_label(format!("Recent steps of {}", agent.name))
                            .gap_1()
                            .pt_2()
                            .border_t_1()
                            .border_color(line)
                            .child(div().text_eyebrow().text_color(quiet).child("RECENT STEPS"))
                            .children(agent.activity.iter().map(|(when, what)| {
                                h_flex()
                                    .gap_3()
                                    .text_caption()
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(px(56.))
                                            .text_color(quiet)
                                            .child(when.clone()),
                                    )
                                    .child(div().flex_1().min_w_0().truncate().child(what.clone()))
                            })),
                    )
                })
                .child(h_flex().gap_1p5().justify_end().children(buttons))
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .gap_2()
            .text_body()
            .text_color(ink)
            .on_key_down({
                let expand = expand.clone();
                let open = expanded_at.is_some();
                move |event, window, cx| {
                    let key = event.keystroke.key.as_str();
                    if key == "escape" && open {
                        if let Some(expand) = &expand {
                            expand(None, window, cx);
                        }
                        if let Some(at) = expanded_at
                            && let Some(handle) = handles.get(at)
                        {
                            handle.focus(window, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    let Some(current) = handles.iter().position(|h| h.is_focused(window)) else {
                        return;
                    };
                    if let Some(next) = dock_step(current, handles.len(), key) {
                        handles[next].focus(window, cx);
                        cx.stop_propagation();
                    }
                }
            })
            .child(
                h_flex()
                    .id(child_id("dock-row".into()))
                    .role(Role::Toolbar)
                    .aria_label(format!("{}: {}", self.title, dock_summary(&self.agents)))
                    .flex_wrap()
                    .gap_2()
                    .p_1p5()
                    .rounded(px(20.))
                    .bg(chip)
                    .child(
                        h_flex()
                            .gap_1p5()
                            .pl_1p5()
                            .pr_1()
                            .text_caption()
                            .text_color(quiet)
                            .child(Icon::new(IconName::Bot).xsmall())
                            .child(dock_summary(&self.agents)),
                    )
                    .children(pills),
            )
            .children(panel)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_money_add_up() {
        let agents = vec![
            FleetAgent::new("a", "A", "x").usage(3, 125, "now"),
            FleetAgent::new("b", "B", "y")
                .state(FleetState::Failed)
                .usage(1, 5, "now"),
            FleetAgent::new("c", "C", "z"),
        ];
        assert_eq!(count_in(&agents, FleetState::Working), 2);
        assert_eq!(count_in(&agents, FleetState::Failed), 1);
        assert_eq!(dollars(agents.iter().map(|a| a.spent_cents).sum()), "$1.30");
    }

    #[test]
    fn dock_arrows_wrap_around() {
        assert_eq!(dock_step(2, 3, "right"), Some(0));
        assert_eq!(dock_step(0, 3, "left"), Some(2));
        assert_eq!(dock_step(1, 3, "home"), Some(0));
        assert_eq!(dock_step(1, 3, "end"), Some(2));
        assert_eq!(dock_step(1, 3, "up"), None);
        assert_eq!(dock_step(0, 0, "right"), None);
    }

    #[test]
    fn pills_and_the_dock_read_in_words() {
        let working = FleetAgent::new("a", "Reconciler", "x").progress(0.62);
        let done = FleetAgent::new("b", "Filer", "y").state(FleetState::Done);
        let waiting = FleetAgent::new("c", "Payables", "z").state(FleetState::Waiting);
        assert_eq!(
            pill_words(&working, false),
            "Reconciler, working, 62 percent, collapsed"
        );
        assert_eq!(pill_words(&done, true), "Filer, done, expanded");
        assert_eq!(percent(&done), Some(100));
        assert_eq!(percent(&waiting), None);
        assert_eq!(
            dock_summary(&[working, done, waiting]),
            "3 assistants, 1 working, 1 needs you"
        );
    }

    #[test]
    fn an_agent_keeps_its_live_line_and_steps() {
        let agent = FleetAgent::new("a", "A", "x")
            .live("Matching payout 128 of 214")
            .step("09:02", "Pulled card payouts")
            .step("09:05", "Matched 120");
        assert_eq!(agent.live.as_deref(), Some("Matching payout 128 of 214"));
        assert_eq!(agent.activity.len(), 2);
    }

    #[test]
    fn each_state_offers_sensible_actions() {
        assert!(actions_for(FleetState::Working).contains(&FleetAction::Pause));
        assert!(actions_for(FleetState::Paused).contains(&FleetAction::Resume));
        assert_eq!(actions_for(FleetState::Done), &[FleetAction::Open]);
    }
}
