// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Step trail: the steps an agent works through, as one quiet line.
//!
//! Finished steps show a check and their time. The step being worked on says
//! what it is doing, with a live timer, and opens into its sub-steps. A step
//! that needs a person waits with "Needs you", which opens Approve and
//! Decline. What comes next is faded, and a step that needs approval says so
//! before it is reached.

use std::{
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How often a running step's timer redraws.
const TICK: Duration = Duration::from_millis(500);
/// Sub-step lists longer than this fold around the one being worked on.
const FOLD_OVER: usize = 6;
/// How many sub-steps a folded list shows.
const FOLDED_ROWS: usize = 5;
/// Sub-step details longer than this keep their end.
const DETAIL_CHARS: usize = 22;

/// Where a step stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailState {
    /// Finished.
    Done,
    /// Being worked on.
    Running,
    /// Still to come.
    Pending,
    /// Waiting for a person to approve it.
    Waiting,
    /// Stopped with an error.
    Failed,
    /// A person said no.
    Declined,
}

/// Where a sub-step stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstepState {
    /// Finished.
    Done,
    /// Being worked on.
    Running,
    /// Still to come.
    Pending,
}

/// Something the agent did inside a step.
#[derive(Debug, Clone)]
pub struct TrailSubstep {
    title: SharedString,
    state: SubstepState,
    detail: Option<SharedString>,
}

impl TrailSubstep {
    /// Create a finished sub-step. Say what happened: "Found the stale rate".
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            state: SubstepState::Done,
            detail: None,
        }
    }

    /// Set where the sub-step stands.
    pub fn state(mut self, state: SubstepState) -> Self {
        self.state = state;
        self
    }

    /// Add a short fact on the right: a file, a count, a change.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// One step of the trail.
#[derive(Clone)]
pub struct TrailStep {
    id: SharedString,
    title: SharedString,
    state: TrailState,
    took: Option<Duration>,
    status: Option<SharedString>,
    started: Option<Instant>,
    gate: bool,
    ask: Option<SharedString>,
    icon: Option<IconName>,
    substeps: Vec<TrailSubstep>,
}

impl TrailStep {
    /// Create a step. Keep the id stable: opening and closing follow it.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            state: TrailState::Pending,
            took: None,
            status: None,
            started: None,
            gate: false,
            ask: None,
            icon: None,
            substeps: Vec::new(),
        }
    }

    /// Set where the step stands.
    pub fn state(mut self, state: TrailState) -> Self {
        self.state = state;
        self
    }

    /// Mark the step done, with how long it took.
    pub fn done(mut self, took: Duration) -> Self {
        self.state = TrailState::Done;
        self.took = Some(took);
        self
    }

    /// Mark the step running: what it does now, in one word, and when it
    /// started, for the live timer. Keep the same instant between renders.
    pub fn running(mut self, status: impl Into<SharedString>, started: Instant) -> Self {
        self.state = TrailState::Running;
        self.status = Some(status.into());
        self.started = Some(started);
        self
    }

    /// Say the step needs approval before it runs.
    pub fn gate(mut self, gate: bool) -> Self {
        self.gate = gate;
        self
    }

    /// Say what a waiting step asks a person to approve.
    pub fn ask(mut self, ask: impl Into<SharedString>) -> Self {
        self.ask = Some(ask.into());
        self
    }

    /// Show the tool or system the step works in.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Add a sub-step.
    pub fn substep(mut self, substep: TrailSubstep) -> Self {
        self.substeps.push(substep);
        self
    }
}

type StepHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// The steps an agent works through, with the one it is on opened up.
#[derive(IntoElement)]
pub struct StepTrail {
    id: ElementId,
    style: StyleRefinement,
    title: Option<SharedString>,
    agent: Option<SharedString>,
    steps: Vec<TrailStep>,
    on_approve: Option<StepHandler>,
    on_decline: Option<StepHandler>,
}

impl StepTrail {
    /// Create an empty trail.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: None,
            agent: None,
            steps: Vec::new(),
            on_approve: None,
            on_decline: None,
        }
    }

    /// Show a header with the task and how many steps are done.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Name who works on it, in the header.
    pub fn agent(mut self, agent: impl Into<SharedString>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    /// Add a step, in the order the agent walks them.
    pub fn step(mut self, step: TrailStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Run when a waiting step is approved. With this or `on_decline`,
    /// "Needs you" opens Approve and Decline.
    pub fn on_approve(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_approve = Some(Rc::new(handler));
        self
    }

    /// Run when a waiting step is declined. Mark it declined in your data.
    pub fn on_decline(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decline = Some(Rc::new(handler));
        self
    }
}

impl Styled for StepTrail {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[derive(Default)]
struct TrailUi {
    /// Steps opened or closed by hand; the rest follow whether they run.
    opened: HashMap<SharedString, bool>,
    /// Steps whose long sub-step list is shown whole.
    unfolded: HashMap<SharedString, bool>,
    /// The waiting step whose answer row is open.
    answering: Option<SharedString>,
    ticker: Option<Task<()>>,
}

/// A step's time in a few characters: "<1s", "8s", "1m 4s", "1h 2m".
fn took_words(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs == 0 {
        "<1s".into()
    } else if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86_400 {
        format!("{}h {}m", secs / 3600, secs % 3600 / 60)
    } else {
        format!("{}d {}h", secs / 86_400, secs % 86_400 / 3600)
    }
}

/// A long detail keeps its end, where the line number or count is.
fn detail_tail(detail: &str) -> String {
    let count = detail.chars().count();
    if count <= DETAIL_CHARS {
        detail.to_string()
    } else {
        let tail: String = detail.chars().skip(count - (DETAIL_CHARS - 1)).collect();
        format!("\u{2026}{tail}")
    }
}

/// Which sub-steps a folded list shows: `FOLDED_ROWS` around the running one,
/// as a range, with how many are hidden before and after.
fn fold_window(len: usize, running: Option<usize>) -> (usize, usize) {
    if len <= FOLD_OVER {
        return (0, len);
    }
    let focus = running.unwrap_or(len - 1);
    let start = focus.saturating_sub(FOLDED_ROWS / 2).min(len - FOLDED_ROWS);
    (start, start + FOLDED_ROWS)
}

impl RenderOnce for StepTrail {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let warning = theme.warning;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let ui = window.use_keyed_state(self.id.clone(), cx, |_, _| TrailUi::default());
        let any_running = self
            .steps
            .iter()
            .any(|step| step.state == TrailState::Running && step.started.is_some());
        // Keep the running timer moving while something runs.
        ui.update(cx, |ui, cx| {
            if any_running && ui.ticker.is_none() {
                ui.ticker = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(TICK).await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                }));
            } else if !any_running {
                ui.ticker = None;
            }
        });
        let (opened, unfolded, answering) = {
            let ui = ui.read(cx);
            (ui.opened.clone(), ui.unfolded.clone(), ui.answering.clone())
        };
        let can_answer = self.on_approve.is_some() || self.on_decline.is_some();
        let done = self
            .steps
            .iter()
            .filter(|step| step.state == TrailState::Done)
            .count();
        let total = self.steps.len();

        let header = self.title.clone().map(|title| {
            h_flex()
                .gap_2()
                .px_4()
                .pt_3()
                .pb_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_semibold()
                        .child(title),
                )
                .children(self.agent.clone().map(|agent| {
                    h_flex()
                        .flex_none()
                        .gap_1()
                        .px_1p5()
                        .rounded(px(5.))
                        .bg(chip)
                        .text_xs()
                        .text_color(quiet)
                        .child(Icon::new(IconName::Bot).xsmall())
                        .child(agent)
                }))
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(quiet)
                        .child(format!("{done}/{total}")),
                )
        });

        let rows = self.steps.iter().enumerate().map(|(ix, step)| {
            let last = ix + 1 == total;
            let has_subs = !step.substeps.is_empty();
            let open = has_subs
                && opened
                    .get(&step.id)
                    .copied()
                    .unwrap_or(step.state == TrailState::Running);
            let to_come = matches!(step.state, TrailState::Pending | TrailState::Declined);

            let marker = {
                let ring = div()
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full();
                match step.state {
                    TrailState::Done => ring
                        .bg(accent.opacity(0.16))
                        .child(Icon::new(IconName::Check).xsmall().text_color(accent)),
                    TrailState::Running => ring.border_2().border_color(accent.opacity(0.3)).child(
                        Icon::new(IconName::LoaderCircle)
                            .xsmall()
                            .text_color(accent),
                    ),
                    TrailState::Waiting => ring
                        .bg(warning)
                        .child(Icon::new(IconName::Pause).xsmall().text_color(fill)),
                    TrailState::Failed => ring
                        .bg(danger)
                        .child(Icon::new(IconName::Close).xsmall().text_color(fill)),
                    TrailState::Declined => {
                        ring.child(div().w(px(8.)).h(px(1.5)).rounded_full().bg(quiet))
                    }
                    TrailState::Pending if step.gate => ring
                        .border_1()
                        .border_color(warning)
                        .child(div().size(px(6.)).rounded_full().bg(warning.opacity(0.5))),
                    TrailState::Pending => ring.border_1().border_color(line),
                }
            };
            // The thread: a piece above the marker joins the step before, and
            // a piece below runs on to the next one.
            let thread_color = |done: bool| if done { accent.opacity(0.5) } else { line };
            let before_done = ix > 0 && self.steps[ix - 1].state == TrailState::Done;
            let line_before = div()
                .w(px(1.5))
                .h(px(8.))
                .when(ix > 0, |this| this.bg(thread_color(before_done)));
            let line_after = div().w(px(1.5)).flex_1().min_h(px(8.)).when(!last, |this| {
                this.bg(thread_color(step.state == TrailState::Done))
            });

            let right: Option<isonomic_ui::AnyElement> = match step.state {
                TrailState::Done => step.took.map(|took| {
                    div()
                        .text_xs()
                        .text_color(quiet)
                        .child(took_words(took))
                        .into_any_element()
                }),
                TrailState::Running => Some(
                    h_flex()
                        .gap_1p5()
                        .text_xs()
                        .children(
                            step.status
                                .clone()
                                .map(|status| div().text_color(accent).font_medium().child(status)),
                        )
                        .children(
                            step.started
                                .map(|at| div().text_color(quiet).child(took_words(at.elapsed()))),
                        )
                        .into_any_element(),
                ),
                TrailState::Waiting if can_answer => {
                    let ui = ui.clone();
                    let id = step.id.clone();
                    Some(
                        Button::new(("trail-needs", ix))
                            .outline()
                            .xsmall()
                            .label("Needs you")
                            .on_click(move |_, _, cx| {
                                ui.update(cx, |ui, cx| {
                                    ui.answering = if ui.answering.as_ref() == Some(&id) {
                                        None
                                    } else {
                                        Some(id.clone())
                                    };
                                    cx.notify();
                                })
                            })
                            .into_any_element(),
                    )
                }
                TrailState::Waiting => Some(tag("Needs you", warning).into_any_element()),
                TrailState::Failed => Some(tag("Failed", danger).into_any_element()),
                TrailState::Declined => Some(
                    div()
                        .text_xs()
                        .text_color(quiet)
                        .child("Declined")
                        .into_any_element(),
                ),
                TrailState::Pending if step.gate => {
                    Some(tag("Approval", warning).into_any_element())
                }
                TrailState::Pending => None,
            };

            let head = h_flex()
                .id(("trail-step", ix))
                .h(px(32.))
                .gap_2()
                .min_w_0()
                .flex_1()
                .when(has_subs, |this| {
                    let ui = ui.clone();
                    let id = step.id.clone();
                    let running = step.state == TrailState::Running;
                    this.cursor_pointer().on_click(move |_, _, cx| {
                        ui.update(cx, |ui, cx| {
                            let now = ui.opened.get(&id).copied().unwrap_or(running);
                            ui.opened.insert(id.clone(), !now);
                            cx.notify();
                        })
                    })
                })
                .children(step.icon.clone().map(|icon| {
                    Icon::new(icon).small().text_color(if to_come {
                        quiet.opacity(0.6)
                    } else {
                        ink
                    })
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(to_come, |this| this.text_color(quiet))
                        .when(step.state == TrailState::Declined, |this| {
                            this.line_through()
                        })
                        .when(step.state == TrailState::Running, |this| this.font_medium())
                        .when(step.state == TrailState::Failed, |this| {
                            this.text_color(danger)
                        })
                        .child(step.title.clone()),
                )
                .children(right.map(|right| div().flex_none().max_w(relative(0.4)).child(right)));

            let substeps = open.then(|| {
                let running_sub = step
                    .substeps
                    .iter()
                    .position(|sub| sub.state == SubstepState::Running);
                let whole = unfolded.get(&step.id).copied().unwrap_or(false);
                let (from, to) = if whole {
                    (0, step.substeps.len())
                } else {
                    fold_window(step.substeps.len(), running_sub)
                };
                let folds = step.substeps.len() > FOLD_OVER;
                let toggle = |label: String, key: (&'static str, usize)| {
                    let ui = ui.clone();
                    let id = step.id.clone();
                    h_flex()
                        .id(key)
                        .h(px(24.))
                        .gap_2()
                        .cursor_pointer()
                        .text_xs()
                        .text_color(quiet)
                        .hover(|style| style.text_color(ink))
                        .child(div().w(px(16.)))
                        .child(label)
                        .on_click(move |_, _, cx| {
                            ui.update(cx, |ui, cx| {
                                let now = ui.unfolded.get(&id).copied().unwrap_or(false);
                                ui.unfolded.insert(id.clone(), !now);
                                cx.notify();
                            })
                        })
                };
                v_flex()
                    .when(from > 0, |this| {
                        this.child(toggle(format!("+{from} earlier"), ("trail-earlier", ix)))
                    })
                    .children(step.substeps[from..to].iter().map(|sub| {
                        let dot = match sub.state {
                            SubstepState::Done => quiet.opacity(0.6),
                            SubstepState::Running => accent,
                            SubstepState::Pending => line,
                        };
                        h_flex()
                            .h(px(24.))
                            .gap_2()
                            .text_xs()
                            .child(
                                div()
                                    .w(px(16.))
                                    .flex()
                                    .justify_center()
                                    .child(div().size(px(5.)).rounded_full().bg(dot)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(match sub.state {
                                        SubstepState::Running => ink,
                                        _ => quiet,
                                    })
                                    .child(sub.title.clone()),
                            )
                            .children(sub.detail.as_ref().map(|detail| {
                                div()
                                    .flex_none()
                                    .text_color(quiet)
                                    .child(detail_tail(detail))
                            }))
                    }))
                    .when(to < step.substeps.len(), |this| {
                        this.child(toggle(
                            format!("+{} more", step.substeps.len() - to),
                            ("trail-more", ix),
                        ))
                    })
                    .when(folds && whole, |this| {
                        this.child(toggle("Show fewer".into(), ("trail-fewer", ix)))
                    })
            });

            let answer = (step.state == TrailState::Waiting
                && answering.as_ref() == Some(&step.id))
            .then(|| {
                let approve = self.on_approve.clone();
                let decline = self.on_decline.clone();
                let (approve_id, decline_id) = (step.id.clone(), step.id.clone());
                let (ui_a, ui_d) = (ui.clone(), ui.clone());
                v_flex()
                    .gap_2()
                    .my_1()
                    .p_2p5()
                    .rounded(radius)
                    .bg(warning.opacity(0.08))
                    .border_1()
                    .border_color(warning.opacity(0.3))
                    .children(step.ask.clone().map(|ask| div().text_xs().child(ask)))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new(("trail-approve", ix))
                                    .primary()
                                    .xsmall()
                                    .label("Approve")
                                    .on_click(move |_, window, cx| {
                                        ui_a.update(cx, |ui, cx| {
                                            ui.answering = None;
                                            cx.notify();
                                        });
                                        if let Some(handler) = &approve {
                                            handler(&approve_id, window, cx);
                                        }
                                    }),
                            )
                            .child(
                                Button::new(("trail-decline", ix))
                                    .outline()
                                    .xsmall()
                                    .label("Decline")
                                    .on_click(move |_, window, cx| {
                                        ui_d.update(cx, |ui, cx| {
                                            ui.answering = None;
                                            cx.notify();
                                        });
                                        if let Some(handler) = &decline {
                                            handler(&decline_id, window, cx);
                                        }
                                    }),
                            ),
                    )
            });

            h_flex()
                .items_stretch()
                .gap_2p5()
                .child(
                    v_flex()
                        .flex_none()
                        .items_center()
                        .w(px(16.))
                        .child(line_before)
                        .child(marker)
                        .child(line_after),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(head)
                        .children(substeps)
                        .children(answer),
                )
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(420.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.3))
            .shadow(resting_shadow(cx))
            .children(header)
            .child(
                v_flex()
                    .px_4()
                    .pb_2()
                    .when(self.title.is_none(), |this| this.pt_2())
                    .children(rows)
                    .when(total == 0, |this| {
                        this.child(div().py_2().text_color(quiet).child("No steps yet"))
                    }),
            )
            .refine_style(&self.style)
    }
}

/// A small tinted label.
fn tag(label: &'static str, color: Hsla) -> impl IntoElement {
    div()
        .px_1p5()
        .rounded(px(4.))
        .bg(color.opacity(0.14))
        .text_xs()
        .font_medium()
        .text_color(color)
        .child(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_short() {
        assert_eq!(took_words(Duration::from_millis(300)), "<1s");
        assert_eq!(took_words(Duration::from_secs(8)), "8s");
        assert_eq!(took_words(Duration::from_secs(64)), "1m 4s");
        assert_eq!(took_words(Duration::from_secs(3720)), "1h 2m");
        assert_eq!(took_words(Duration::from_secs(273_600)), "3d 4h");
    }

    #[test]
    fn long_details_keep_their_end() {
        assert_eq!(detail_tail("rates.csv:42"), "rates.csv:42");
        let long = detail_tail("ledgers/2026/march/receivables.csv:1284");
        assert!(long.ends_with("receivables.csv:1284"));
        assert_eq!(long.chars().count(), DETAIL_CHARS);
    }

    #[test]
    fn long_lists_fold_around_the_running_one() {
        assert_eq!(fold_window(4, Some(1)), (0, 4));
        assert_eq!(fold_window(12, Some(6)), (4, 9));
        assert_eq!(fold_window(12, Some(0)), (0, 5));
        assert_eq!(fold_window(12, None), (7, 12));
    }
}
