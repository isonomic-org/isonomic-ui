// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-agent dashboard: every assistant at work, as a filterable grid of tiles with totals.
//!
//! The strip at the top counts assistants by state and adds up what they've
//! spent. Each tile shows one assistant's task, how far along it is and when
//! it last reported, with pause, resume, stop and open buttons. Filtering and
//! every action go to the owner; the dashboard changes nothing itself.

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
    prelude::FluentBuilder as _, px, relative,
};

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
        }
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

type FilterHandler = Rc<dyn Fn(Option<FleetState>, &mut Window, &mut App)>;
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
        }
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
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
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
                        let (icon, word) = match action {
                            FleetAction::Pause => (IconName::Pause, "Pause"),
                            FleetAction::Resume => (IconName::Play, "Resume"),
                            FleetAction::Stop => (IconName::Square, "Stop"),
                            FleetAction::Open => (IconName::ExternalLink, "Open"),
                        };
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
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_right()
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
    fn each_state_offers_sensible_actions() {
        assert!(actions_for(FleetState::Working).contains(&FleetAction::Pause));
        assert!(actions_for(FleetState::Paused).contains(&FleetAction::Resume));
        assert_eq!(actions_for(FleetState::Done), &[FleetAction::Open]);
    }
}
