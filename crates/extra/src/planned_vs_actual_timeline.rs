// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Planned vs actual timeline: a run's plan against what really happened.
//!
//! Every step has a dashed bar for when it was planned and a solid bar for
//! when it really ran. Time past the plan is drawn in the warning colour,
//! the steps waiting on a late step move right, and the expected finish
//! moves away from the planned one.

use std::{rc::Rc, time::Duration};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, spinner::Spinner,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// The width of the step names.
const NAME_COLUMN: f32 = 180.;
/// The height of one row.
const ROW: f32 = 30.;

/// One step of the plan.
#[derive(Debug, Clone)]
pub struct PlanStep {
    id: SharedString,
    title: SharedString,
    planned: Duration,
    waits_for: Vec<SharedString>,
    started: Option<Duration>,
    ended: Option<Duration>,
}

impl PlanStep {
    /// Create a step with how long it was planned to take.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        planned: Duration,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            planned,
            waits_for: Vec::new(),
            started: None,
            ended: None,
        }
    }

    /// Name a step that has to finish first. List a step after the steps it
    /// waits for.
    pub fn waits_for(mut self, id: impl Into<SharedString>) -> Self {
        self.waits_for.push(id.into());
        self
    }

    /// Set when it really started, counted from the start of the run.
    pub fn started(mut self, at: Duration) -> Self {
        self.started = Some(at);
        self
    }

    /// Set when it really finished, counted from the start of the run.
    pub fn ended(mut self, at: Duration) -> Self {
        self.ended = Some(at);
        self
    }
}

/// Where one step stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepState {
    Waiting,
    Running,
    Done,
}

/// When a step was planned and when it ran or is expected to run.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Schedule {
    planned_start: Duration,
    planned_end: Duration,
    start: Duration,
    end: Duration,
    state: StepState,
}

/// Works out every step's planned and expected times. A step starts once
/// everything it waits for has finished, and never before `now` unless it
/// already has.
fn schedule(steps: &[PlanStep], now: Duration) -> Vec<Schedule> {
    let mut done: Vec<Schedule> = Vec::with_capacity(steps.len());
    for step in steps {
        let before = |pick: fn(&Schedule) -> Duration| {
            step.waits_for
                .iter()
                .filter_map(|id| steps.iter().position(|other| other.id == *id))
                .filter(|ix| *ix < done.len())
                .map(|ix| pick(&done[ix]))
                .max()
                .unwrap_or(Duration::ZERO)
        };
        let planned_start = before(|s| s.planned_end);
        let after = before(|s| s.end);
        let (state, start, end) = match (step.started, step.ended) {
            (Some(start), Some(end)) => (StepState::Done, start, end.max(start)),
            (Some(start), None) => (StepState::Running, start, (start + step.planned).max(now)),
            _ => {
                let start = after.max(now);
                (StepState::Waiting, start, start + step.planned)
            }
        };
        done.push(Schedule {
            planned_start,
            planned_end: planned_start + step.planned,
            start,
            end,
            state,
        });
    }
    done
}

/// Time as minutes and seconds: "0:42", "12:05".
fn clock(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// A difference with its sign: "+12s", "\u{2212}4s", "+1:05".
fn signed(late: Duration, early: Duration) -> String {
    let (sign, gap) = if late >= early {
        ("+", late - early)
    } else {
        ("\u{2212}", early - late)
    };
    let seconds = gap.as_secs();
    if seconds < 60 {
        format!("{sign}{seconds}s")
    } else {
        format!("{sign}{}", clock(gap))
    }
}

/// A step in words for screen readers, such as "Draft: done in 2:10, +10s against the plan".
fn spoken_step(
    title: &str,
    s: &Schedule,
    planned: Duration,
    now: Duration,
    format: &dyn Fn(Duration) -> String,
) -> String {
    match s.state {
        StepState::Done => format!(
            "{title}: done in {}, {} against the plan",
            format(s.end - s.start),
            signed(s.end - s.start, planned)
        ),
        StepState::Running => format!(
            "{title}: running, {} so far of {} planned",
            format(now.max(s.start) - s.start),
            format(planned)
        ),
        StepState::Waiting => format!(
            "{title}: waiting, {} planned, expected to start at {}",
            format(planned),
            format(s.start)
        ),
    }
}

/// The end of the axis: a whole number of steps of `tick` that holds `span`.
fn axis_end(span: Duration, tick: Duration) -> Duration {
    let tick = tick.max(Duration::from_secs(1));
    let ticks = span.as_secs_f64() / tick.as_secs_f64();
    tick * (ticks.ceil().max(1.) as u32)
}

/// A tick spacing that gives about six ticks over `span`.
fn tick_for(span: Duration) -> Duration {
    let target = span.as_secs() / 6;
    [5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600]
        .into_iter()
        .find(|step| *step >= target)
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(3600))
}

type Format = Rc<dyn Fn(Duration) -> String>;

/// Each step of a run against its plan, on one time axis.
#[derive(IntoElement)]
pub struct PlannedVsActualTimeline {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    task: Option<SharedString>,
    steps: Vec<PlanStep>,
    now: Duration,
    format: Format,
}

impl PlannedVsActualTimeline {
    /// Create a timeline with no steps.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Plan against run".into(),
            task: None,
            steps: Vec::new(),
            now: Duration::ZERO,
            format: Rc::new(clock),
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Say what the run is for.
    pub fn task(mut self, task: impl Into<SharedString>) -> Self {
        self.task = Some(task.into());
        self
    }

    /// Add a step after the steps it waits for.
    pub fn step(mut self, step: PlanStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Set where the run is, counted from its start.
    pub fn now(mut self, now: Duration) -> Self {
        self.now = now;
        self
    }

    /// Set how times are written. Minutes and seconds by default.
    pub fn format(mut self, format: impl Fn(Duration) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }
}

impl Styled for PlannedVsActualTimeline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct TimelineState {
    hovered: Option<usize>,
}

impl RenderOnce for PlannedVsActualTimeline {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.info;
        let success = theme.success;
        let warning = theme.warning;
        let radius_lg = theme.radius_lg;

        let state =
            window.use_keyed_state(self.id.clone(), cx, |_, _| TimelineState { hovered: None });
        let hovered = state.read(cx).hovered.filter(|ix| *ix < self.steps.len());
        let plan = schedule(&self.steps, self.now);
        let planned_finish = plan.iter().map(|s| s.planned_end).max().unwrap_or_default();
        let expected_finish = plan.iter().map(|s| s.end).max().unwrap_or_default();
        let span = planned_finish.max(expected_finish).max(self.now);
        let tick = tick_for(span);
        let end = axis_end(span + tick / 2, tick);
        let at = |time: Duration| (time.as_secs_f32() / end.as_secs_f32()).clamp(0., 1.);
        let format = self.format.clone();
        let behind = expected_finish > planned_finish;
        let gap = signed(expected_finish, planned_finish);
        // The steps linked to the one pointed at.
        let linked = |ix: usize| {
            hovered.is_some_and(|hovered| {
                let pointed = &self.steps[hovered];
                let other = &self.steps[ix];
                pointed.waits_for.contains(&other.id) || other.waits_for.contains(&pointed.id)
            })
        };

        let marker = |time: Duration, color, width: f32| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(at(time)))
                .w(px(width))
                .bg(color)
        };
        let ticks = (0..)
            .map(|n| tick * n)
            .take_while(|time| *time <= end)
            .skip(1)
            .map(|time| {
                div()
                    .absolute()
                    .left(relative(at(time)))
                    .ml(px(-16.))
                    .w(px(32.))
                    .text_center()
                    .text_caption()
                    .text_color(quiet)
                    .child(format(time))
            });

        let rows = self
            .steps
            .iter()
            .zip(&plan)
            .enumerate()
            .map(|(ix, (step, s))| {
                let is_lit = hovered == Some(ix) || linked(ix);
                let faded = hovered.is_some() && !is_lit;
                let icon = match s.state {
                    StepState::Done => Icon::new(IconName::Check)
                        .xsmall()
                        .text_color(success)
                        .into_any_element(),
                    StepState::Running => Spinner::new().xsmall().color(accent).into_any_element(),
                    StepState::Waiting => div()
                        .size(px(10.))
                        .rounded_full()
                        .border_1()
                        .border_color(quiet)
                        .into_any_element(),
                };
                // The solid bar ends at `now` while the step runs.
                let solid_end = match s.state {
                    StepState::Running => self.now.max(s.start),
                    _ => s.end,
                };
                let on_plan_end = (s.start + step.planned).min(solid_end);
                let over = solid_end > on_plan_end;
                let label = match s.state {
                    StepState::Done => Some((
                        format(s.end - s.start),
                        Some(signed(s.end - s.start, step.planned)),
                    )),
                    StepState::Running => Some((format(solid_end - s.start), None)),
                    StepState::Waiting => Some((format(step.planned), None)),
                };
                let late = s.state == StepState::Done && s.end - s.start > step.planned;
                let early = s.state == StepState::Done && s.end - s.start < step.planned;

                h_flex()
                    .id(("step", ix))
                    .role(Role::ListItem)
                    .aria_label(spoken_step(
                        &step.title,
                        s,
                        step.planned,
                        self.now,
                        format.as_ref(),
                    ))
                    .h(px(ROW))
                    .when(faded, |this| this.opacity(0.45))
                    .on_hover({
                        let state = state.clone();
                        move |hovered, _, cx| {
                            state.update(cx, |state, cx| {
                                if *hovered {
                                    state.hovered = Some(ix);
                                    cx.notify();
                                } else if state.hovered == Some(ix) {
                                    state.hovered = None;
                                    cx.notify();
                                }
                            })
                        }
                    })
                    .child(
                        h_flex()
                            .flex_none()
                            .w(px(NAME_COLUMN))
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .w(px(14.))
                                    .justify_center()
                                    .child(icon),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .when(is_lit, |this| this.font_medium())
                                    .when(s.state == StepState::Waiting, |this| {
                                        this.text_color(quiet)
                                    })
                                    .child(step.title.clone()),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .h_full()
                            // The planned ghost.
                            .child(
                                div()
                                    .absolute()
                                    .top(px(8.))
                                    .h(px(14.))
                                    .left(relative(at(s.planned_start)))
                                    .w(relative(at(s.planned_end) - at(s.planned_start)))
                                    .rounded(px(3.))
                                    .border_1()
                                    .border_dashed()
                                    .border_color(quiet.opacity(0.6)),
                            )
                            // What really ran, with the part past the plan in amber.
                            .when(s.state != StepState::Waiting, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .top(px(10.))
                                        .h(px(10.))
                                        .left(relative(at(s.start)))
                                        .w(relative(at(on_plan_end) - at(s.start)))
                                        .rounded_l(px(2.))
                                        .when(!over, |this| this.rounded_r(px(2.)))
                                        .bg(accent.opacity(0.55)),
                                )
                                .when(over, |this| {
                                    this.child(
                                        div()
                                            .absolute()
                                            .top(px(10.))
                                            .h(px(10.))
                                            .left(relative(at(on_plan_end)))
                                            .w(relative(at(solid_end) - at(on_plan_end)))
                                            .rounded_r(px(2.))
                                            .bg(warning.opacity(0.7)),
                                    )
                                })
                            })
                            .children(label.map(|(time, delta)| {
                                let edge = match s.state {
                                    StepState::Waiting => s.end,
                                    _ => solid_end.max(s.planned_end),
                                };
                                h_flex()
                                    .absolute()
                                    .top(px(6.))
                                    .left(relative(at(edge)))
                                    .ml_1p5()
                                    .gap_1()
                                    .text_caption()
                                    .child(
                                        div()
                                            .when(s.state == StepState::Waiting, |this| {
                                                this.text_color(quiet)
                                            })
                                            .child(time),
                                    )
                                    .children(delta.map(|delta| {
                                        div()
                                            .px_1()
                                            .rounded(px(4.))
                                            .when(late, |this| this.bg(warning.opacity(0.18)))
                                            .when(early, |this| this.bg(success.opacity(0.16)))
                                            .child(delta)
                                    }))
                            })),
                    )
            });
        let rows: Vec<_> = rows.collect();

        let summary = format!(
            "{}: expected to finish at {}, {} against the plan",
            self.title,
            format(expected_finish),
            gap
        );
        let headline = h_flex()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(line)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().font_medium().child(self.title))
                    .children(
                        self.task
                            .map(|task| div().min_w_0().truncate().text_color(quiet).child(task)),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .text_color(quiet)
                    .child("Time")
                    .child(div().font_medium().text_color(ink).child(format(self.now)))
                    .child(format!("of {}", format(expected_finish))),
            )
            .child(
                div()
                    .flex_none()
                    .px_1p5()
                    .rounded(px(5.))
                    .text_label_small()
                    .bg(if behind {
                        warning.opacity(0.18)
                    } else {
                        success.opacity(0.16)
                    })
                    .child(if behind {
                        format!("{gap} behind plan")
                    } else if expected_finish == planned_finish {
                        "On plan".to_string()
                    } else {
                        format!("{gap} ahead of plan")
                    }),
            );

        let chart = v_flex()
            .px_4()
            .py_2()
            .child(
                h_flex()
                    .h(px(20.))
                    .child(
                        div()
                            .flex_none()
                            .w(px(NAME_COLUMN))
                            .text_caption()
                            .text_color(quiet)
                            .child("STEP"),
                    )
                    .child(div().relative().flex_1().h_full().children(ticks)),
            )
            .child(
                div()
                    .relative()
                    .child(
                        v_flex()
                            .id("steps")
                            .role(Role::List)
                            .aria_label("Steps")
                            .children(rows),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(px(NAME_COLUMN))
                            .right_0()
                            .child(marker(planned_finish, quiet.opacity(0.5), 1.))
                            .when(expected_finish != planned_finish, |this| {
                                this.child(marker(expected_finish, warning, 1.))
                            })
                            .child(marker(self.now, accent, 2.)),
                    ),
            );

        let legend = h_flex()
            .gap_4()
            .px_4()
            .py_2()
            .border_t_1()
            .border_color(line)
            .text_caption()
            .text_color(quiet)
            .child(
                h_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .w(px(14.))
                            .h(px(8.))
                            .rounded(px(2.))
                            .border_1()
                            .border_dashed()
                            .border_color(quiet),
                    )
                    .child("Planned"),
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .w(px(14.))
                            .h(px(8.))
                            .rounded(px(2.))
                            .bg(accent.opacity(0.55)),
                    )
                    .child("Ran"),
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .w(px(14.))
                            .h(px(8.))
                            .rounded(px(2.))
                            .bg(warning.opacity(0.7)),
                    )
                    .child("Past the plan"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .px_1p5()
                    .rounded(px(4.))
                    .bg(chip)
                    .child(format!("Planned finish {}", format(planned_finish))),
            )
            .child(
                div()
                    .px_1p5()
                    .rounded(px(4.))
                    .when(behind, |this| this.bg(warning.opacity(0.18)))
                    .when(!behind, |this| this.bg(chip))
                    .child(format!("Expected finish {}", format(expected_finish))),
            );

        v_flex()
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label(summary)
            .w_full()
            .max_w(px(820.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.4))
            .shadow(resting_shadow(cx))
            .child(headline)
            .child(chart)
            .child(legend)
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_read_in_words() {
        let s = Schedule {
            planned_start: Duration::ZERO,
            planned_end: Duration::from_secs(120),
            start: Duration::ZERO,
            end: Duration::from_secs(130),
            state: StepState::Done,
        };
        assert_eq!(
            spoken_step(
                "Draft",
                &s,
                Duration::from_secs(120),
                Duration::ZERO,
                &clock
            ),
            "Draft: done in 2:10, +10s against the plan"
        );
    }

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    fn steps() -> Vec<PlanStep> {
        vec![
            PlanStep::new("a", "A", secs(20))
                .started(secs(0))
                .ended(secs(30)),
            PlanStep::new("b", "B", secs(10))
                .waits_for("a")
                .started(secs(30)),
            PlanStep::new("c", "C", secs(15)).waits_for("b"),
        ]
    }

    #[test]
    fn the_plan_follows_what_each_step_waits_for() {
        let plan = schedule(&steps(), secs(35));
        assert_eq!(plan[1].planned_start, secs(20));
        assert_eq!(plan[2].planned_start, secs(30));
        assert_eq!(plan[2].planned_end, secs(45));
    }

    #[test]
    fn a_late_step_pushes_the_steps_after_it() {
        let plan = schedule(&steps(), secs(35));
        assert_eq!(plan[0].state, StepState::Done);
        assert_eq!(plan[1].state, StepState::Running);
        // B started at 30 and is planned for 10, so it should end at 40.
        assert_eq!(plan[1].end, secs(40));
        assert_eq!(plan[2].start, secs(40));
        assert_eq!(plan[2].end, secs(55));
    }

    #[test]
    fn a_running_step_past_its_plan_ends_no_sooner_than_now() {
        let plan = schedule(&steps(), secs(50));
        assert_eq!(plan[1].end, secs(50));
        assert_eq!(plan[2].start, secs(50));
    }

    #[test]
    fn differences_carry_their_sign() {
        assert_eq!(signed(secs(30), secs(20)), "+10s");
        assert_eq!(signed(secs(16), secs(20)), "\u{2212}4s");
        assert_eq!(signed(secs(85), secs(20)), "+1:05");
    }

    #[test]
    fn the_axis_ends_on_a_whole_tick() {
        assert_eq!(tick_for(secs(200)), secs(60));
        assert_eq!(axis_end(secs(190), secs(30)), secs(210));
        assert_eq!(clock(secs(125)), "2:05");
    }
}
