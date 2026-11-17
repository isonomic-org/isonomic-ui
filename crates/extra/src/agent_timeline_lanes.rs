// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Agent timeline lanes: one lane per assistant, with its tasks as blocks, the handoffs between lanes and the decisions taken.
//!
//! Time runs left to right with minute ticks. Each assistant's tasks are
//! blocks along its lane, coloured by how they ended. A handoff is an arrow
//! from one lane down or up to another at the moment work changed hands,
//! and a decision is a square mark on the lane that took it. A line marks now.
//! Selecting a block or a decision names it underneath.

use std::rc::Rc;

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, canvas, div, point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// One task on a lane.
#[derive(Debug, Clone, PartialEq)]
pub struct LaneTask {
    /// Identifies it.
    pub id: SharedString,
    /// The lane's id.
    pub lane: SharedString,
    /// When it started, in seconds.
    pub start: u32,
    /// When it ended, or `None` while running.
    pub end: Option<u32>,
    /// What it was.
    pub label: SharedString,
    /// How it went.
    pub tone: Tone,
}

impl LaneTask {
    /// Create a task that finished well.
    pub fn new(
        id: impl Into<SharedString>,
        lane: impl Into<SharedString>,
        start: u32,
        end: u32,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            lane: lane.into(),
            start,
            end: Some(end.max(start)),
            label: label.into(),
            tone: Tone::Success,
        }
    }

    /// Mark it as still running.
    pub fn running(mut self) -> Self {
        self.end = None;
        self.tone = Tone::Info;
        self
    }

    /// Set how it went.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

/// Work changing hands.
#[derive(Debug, Clone, PartialEq)]
pub struct LaneHandoff {
    /// When.
    pub at: u32,
    /// From which lane.
    pub from: SharedString,
    /// To which lane.
    pub to: SharedString,
}

/// A decision taken on a lane.
#[derive(Debug, Clone, PartialEq)]
pub struct LaneDecision {
    /// Identifies it.
    pub id: SharedString,
    /// The lane's id.
    pub lane: SharedString,
    /// When.
    pub at: u32,
    /// What was decided.
    pub label: SharedString,
}

/// The span the timeline covers: from the first start to the later of the
/// last event and now.
pub fn timeline_span(tasks: &[LaneTask], decisions: &[LaneDecision], now: u32) -> (u32, u32) {
    let start = tasks.iter().map(|t| t.start).min().unwrap_or(0);
    let end = tasks
        .iter()
        .map(|t| t.end.unwrap_or(now))
        .chain(decisions.iter().map(|d| d.at))
        .chain(std::iter::once(now))
        .max()
        .unwrap_or(start + 60);
    (start, end.max(start + 1))
}

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Lanes of assistants' work over time.
#[derive(IntoElement)]
pub struct AgentTimelineLanes {
    id: ElementId,
    style: StyleRefinement,
    lanes: Vec<(SharedString, SharedString)>,
    tasks: Vec<LaneTask>,
    handoffs: Vec<LaneHandoff>,
    decisions: Vec<LaneDecision>,
    now: u32,
    selected: Option<SharedString>,
    on_select: Option<SelectHandler>,
}

impl AgentTimelineLanes {
    /// Create lanes, each given as id and name, at a moment `now` in seconds.
    pub fn new(
        id: impl Into<ElementId>,
        lanes: Vec<(SharedString, SharedString)>,
        now: u32,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            lanes,
            tasks: Vec::new(),
            handoffs: Vec::new(),
            decisions: Vec::new(),
            now,
            selected: None,
            on_select: None,
        }
    }

    /// Set the tasks.
    pub fn tasks(mut self, tasks: Vec<LaneTask>) -> Self {
        self.tasks = tasks;
        self
    }

    /// Set the handoffs.
    pub fn handoffs(mut self, handoffs: Vec<LaneHandoff>) -> Self {
        self.handoffs = handoffs;
        self
    }

    /// Set the decisions.
    pub fn decisions(mut self, decisions: Vec<LaneDecision>) -> Self {
        self.decisions = decisions;
        self
    }

    /// The selected task or decision.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Run when a task or decision is clicked.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for AgentTimelineLanes {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

const LANE: f32 = 44.;
const NAME: f32 = 130.;

fn clock(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// The handoffs at a lane, in time order and in words, so the arrows are not
/// the only place they show.
pub fn handoff_words(
    lanes: &[(SharedString, SharedString)],
    handoffs: &[LaneHandoff],
    lane: &str,
) -> String {
    let name = |id: &SharedString| {
        lanes
            .iter()
            .find(|(l, _)| l == id)
            .map(|(_, n)| n.to_string())
            .unwrap_or_else(|| id.to_string())
    };
    let mut touching: Vec<&LaneHandoff> = handoffs
        .iter()
        .filter(|h| h.from.as_ref() == lane || h.to.as_ref() == lane)
        .collect();
    touching.sort_by_key(|h| h.at);
    if touching.is_empty() {
        return "No handoffs".into();
    }
    touching
        .iter()
        .map(|h| {
            if h.from.as_ref() == lane {
                format!("Hands off to {} at {}", name(&h.to), clock(h.at))
            } else {
                format!("Takes over from {} at {}", name(&h.from), clock(h.at))
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

impl RenderOnce for AgentTimelineLanes {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius_lg = theme.radius_lg;

        let (start, end) = timeline_span(&self.tasks, &self.decisions, self.now);
        let span = (end - start) as f32;
        let share = |t: u32| (t.saturating_sub(start)) as f32 / span;
        let lane_ix = |id: &SharedString| self.lanes.iter().position(|(l, _)| l == id);
        let tick_every = if span <= 300. {
            60
        } else if span <= 1200. {
            300
        } else {
            600
        };
        let ticks: Vec<u32> = ((start / tick_every)..=(end / tick_every))
            .map(|n| n * tick_every)
            .filter(|t| *t >= start && *t <= end)
            .collect();

        let arrows: Vec<(f32, usize, usize)> = self
            .handoffs
            .iter()
            .filter_map(|h| Some((share(h.at), lane_ix(&h.from)?, lane_ix(&h.to)?)))
            .collect();

        let lanes: Vec<_> = self
            .lanes
            .iter()
            .map(|(lane_id, name)| {
                let blocks: Vec<_> = self
                    .tasks
                    .iter()
                    .filter(|t| &t.lane == lane_id)
                    .map(|task| {
                        let to = task.end.unwrap_or(self.now);
                        let color = task.tone.color(cx);
                        let is_selected = self.selected.as_ref() == Some(&task.id);
                        let select = self.on_select.clone();
                        let id = task.id.clone();
                        div()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("task-{}", task.id).into(),
                            ))
                            .absolute()
                            .top(px(8.))
                            .h(px(LANE - 16.))
                            .left(relative(share(task.start)))
                            .w(relative(((to - task.start) as f32 / span).max(0.01)))
                            .px_1p5()
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .rounded(px(5.))
                            .bg(color.opacity(if is_selected { 0.4 } else { 0.18 }))
                            .border_1()
                            .border_color(color.opacity(0.6))
                            .when(task.end.is_none(), |this| {
                                this.border_r_2().border_color(color)
                            })
                            .cursor_pointer()
                            .role(Role::Button)
                            .aria_label(format!(
                                "{}, {} to {}",
                                task.label,
                                clock(task.start),
                                task.end.map(clock).unwrap_or_else(|| "now".into())
                            ))
                            .aria_selected(is_selected)
                            .on_click(move |_, window, cx| {
                                if let Some(select) = &select {
                                    select(&id, window, cx);
                                }
                            })
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(ink)
                                    .child(task.label.clone()),
                            )
                    })
                    .collect();
                let diamonds: Vec<_> = self
                    .decisions
                    .iter()
                    .filter(|d| &d.lane == lane_id)
                    .map(|decision| {
                        let is_selected = self.selected.as_ref() == Some(&decision.id);
                        let select = self.on_select.clone();
                        let id = decision.id.clone();
                        div()
                            .id(ElementId::NamedChild(
                                self.id.clone().into(),
                                format!("decision-{}", decision.id).into(),
                            ))
                            .absolute()
                            .top(px(LANE / 2. - 7.))
                            .left(relative(share(decision.at)))
                            .ml(px(-7.))
                            .size(px(14.))
                            .cursor_pointer()
                            .role(Role::Button)
                            .aria_label(format!(
                                "Decision at {}: {}",
                                clock(decision.at),
                                decision.label
                            ))
                            .aria_selected(is_selected)
                            .on_click(move |_, window, cx| {
                                if let Some(select) = &select {
                                    select(&id, window, cx);
                                }
                            })
                            .child(
                                div()
                                    .size(px(12.))
                                    .m(px(1.))
                                    .rounded(px(3.))
                                    .bg(if is_selected { accent } else { fill })
                                    .border_2()
                                    .border_color(accent),
                            )
                    })
                    .collect();
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("lane-{lane_id}").into(),
                    ))
                    .role(Role::Group)
                    .aria_label(name.clone())
                    .aria_description(handoff_words(&self.lanes, &self.handoffs, lane_id))
                    .h(px(LANE))
                    .border_b_1()
                    .border_color(line)
                    .child(
                        h_flex()
                            .w(px(NAME))
                            .flex_none()
                            .gap_1p5()
                            .px_3()
                            .child(Icon::new(IconName::Bot).xsmall().text_color(quiet))
                            .child(div().truncate().text_xs().font_medium().child(name.clone())),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .h_full()
                            .mr_3()
                            .children(blocks)
                            .children(diamonds),
                    )
            })
            .collect();

        let detail = self
            .selected
            .as_ref()
            .and_then(|id| {
                self.tasks
                    .iter()
                    .find(|t| &t.id == id)
                    .map(|t| {
                        let lane = self
                            .lanes
                            .iter()
                            .find(|(l, _)| l == &t.lane)
                            .map(|(_, n)| n.clone())
                            .unwrap_or_default();
                        format!(
                            "{lane}: {} from {} to {}",
                            t.label,
                            clock(t.start),
                            t.end.map(clock).unwrap_or_else(|| "now".into())
                        )
                    })
                    .or_else(|| {
                        self.decisions.iter().find(|d| &d.id == id).map(|d| {
                            let lane = self
                                .lanes
                                .iter()
                                .find(|(l, _)| l == &d.lane)
                                .map(|(_, n)| n.clone())
                                .unwrap_or_default();
                            format!("{lane} decided at {}: {}", clock(d.at), d.label)
                        })
                    })
            })
            .unwrap_or_else(|| {
                format!(
                    "{} tasks, {} handoffs, {} decisions",
                    self.tasks.len(),
                    self.handoffs.len(),
                    self.decisions.len()
                )
            });

        let now_share = share(self.now);
        let lanes_count = self.lanes.len();
        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(900.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .h(px(28.))
                    .border_b_1()
                    .border_color(line)
                    .child(div().w(px(NAME)).flex_none())
                    .child(div().relative().flex_1().h_full().mr_3().children(
                        ticks.into_iter().map(|t| {
                            div()
                                .absolute()
                                .top(px(7.))
                                .left(relative(share(t)))
                                .text_xs()
                                .text_color(quiet)
                                .child(clock(t))
                        }),
                    )),
            )
            .child(
                div().relative().children(lanes).child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(NAME))
                        .right(px(12.))
                        .child(
                            canvas(
                                |_, _, _| {},
                                move |bounds, _, window, _| {
                                    for (at, from, to) in &arrows {
                                        if from == to {
                                            continue;
                                        }
                                        let x = bounds.origin.x + bounds.size.width * *at;
                                        let y = |lane: usize| {
                                            bounds.origin.y + px(LANE * lane as f32 + LANE / 2.)
                                        };
                                        let (a, b) = (y(*from), y(*to));
                                        let down = b > a;
                                        let tip = if down { b - px(10.) } else { b + px(10.) };
                                        let mut path = PathBuilder::stroke(px(1.5));
                                        path.move_to(point(x, a));
                                        path.line_to(point(x, tip));
                                        if let Ok(path) = path.build() {
                                            window.paint_path(path, accent);
                                        }
                                        let dir = if down { -1. } else { 1. };
                                        let mut head = PathBuilder::fill();
                                        head.move_to(point(x, tip));
                                        head.line_to(point(x - px(4.), tip + px(6. * dir)));
                                        head.line_to(point(x + px(4.), tip + px(6. * dir)));
                                        head.close();
                                        if let Ok(head) = head.build() {
                                            window.paint_path(head, accent);
                                        }
                                    }
                                },
                            )
                            .absolute()
                            .inset_0()
                            .size_full(),
                        )
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .h(px(LANE * lanes_count as f32))
                                .left(relative(now_share))
                                .w(px(1.))
                                .bg(Tone::Danger.color(cx).opacity(0.7)),
                        ),
                ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .text_xs()
                    .text_color(quiet)
                    .bg(chip.opacity(0.3))
                    .child(div().flex_1().min_w_0().truncate().child(detail))
                    .child(format!("now {}", clock(self.now))),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoffs_are_told_from_each_lane_in_time_order() {
        let lanes = vec![
            ("plan".into(), "Planner".into()),
            ("write".into(), "Writer".into()),
        ];
        let handoffs = vec![
            LaneHandoff {
                at: 130,
                from: "write".into(),
                to: "plan".into(),
            },
            LaneHandoff {
                at: 65,
                from: "plan".into(),
                to: "write".into(),
            },
        ];
        assert_eq!(
            handoff_words(&lanes, &handoffs, "plan"),
            "Hands off to Writer at 1:05; Takes over from Writer at 2:10"
        );
        assert_eq!(handoff_words(&lanes, &[], "plan"), "No handoffs");
    }

    #[test]
    fn the_span_reaches_now_and_the_last_event() {
        let tasks = vec![
            LaneTask::new("a", "x", 30, 90, "A"),
            LaneTask::new("b", "y", 60, 0, "B").running(),
        ];
        let decisions = vec![LaneDecision {
            id: "d".into(),
            lane: "x".into(),
            at: 200,
            label: "go".into(),
        }];
        assert_eq!(timeline_span(&tasks, &decisions, 150), (30, 200));
        assert_eq!(timeline_span(&tasks, &[], 150), (30, 150));
    }
}
