// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Job queue view: the work waiting, the workers taking it, and how deep the backlog has run.
//!
//! The top row shows each worker slot: busy with a job and its progress,
//! free, or off. Below, the waiting jobs in the order they'll be taken,
//! each with how long it has waited, a button to move it to the front and
//! one to cancel it. A small chart shows the backlog's depth over the last
//! samples the owner passes in, against the number of workers.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, canvas, div, point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// One worker slot.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerSlot {
    /// Identifies it.
    pub id: SharedString,
    /// Its name.
    pub name: SharedString,
    /// The job it's on and how far along, from 0 to 1.
    pub job: Option<(SharedString, f32)>,
    /// Whether it takes work at all.
    pub online: bool,
}

impl WorkerSlot {
    /// Create a free worker.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            job: None,
            online: true,
        }
    }

    /// Put it on a job.
    pub fn busy(mut self, job: impl Into<SharedString>, progress: f32) -> Self {
        self.job = Some((job.into(), progress.clamp(0., 1.)));
        self
    }

    /// Take it offline.
    pub fn offline(mut self) -> Self {
        self.online = false;
        self
    }
}

/// One waiting job.
#[derive(Debug, Clone, PartialEq)]
pub struct WaitingJob {
    /// Identifies it.
    pub id: SharedString,
    /// What it is.
    pub label: SharedString,
    /// How long it has waited, in seconds.
    pub waited_secs: u32,
    /// Whether it jumps the queue.
    pub urgent: bool,
}

impl WaitingJob {
    /// Create a job.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        waited_secs: u32,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            waited_secs,
            urgent: false,
        }
    }

    /// Mark it urgent.
    pub fn urgent(mut self) -> Self {
        self.urgent = true;
        self
    }
}

/// Write a wait short: "40s", "3m", "1h 5m".
pub fn waited_words(secs: u32) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        _ => format!("{}h {}m", secs / 3600, (secs % 3600) / 60),
    }
}

/// A rough wait for the job at `position` in the queue, in seconds, given
/// free workers and the typical job length.
pub fn expected_wait(position: usize, workers: usize, typical_secs: u32) -> Option<u32> {
    (workers > 0).then(|| (position / workers) as u32 * typical_secs)
}

type JobHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Waiting work, worker slots and backlog depth.
#[derive(IntoElement)]
pub struct JobQueueView {
    id: ElementId,
    style: StyleRefinement,
    workers: Vec<WorkerSlot>,
    jobs: Vec<WaitingJob>,
    depth: Vec<u32>,
    typical_secs: u32,
    on_front: Option<JobHandler>,
    on_cancel: Option<JobHandler>,
}

impl JobQueueView {
    /// Create a view.
    pub fn new(id: impl Into<ElementId>, workers: Vec<WorkerSlot>, jobs: Vec<WaitingJob>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            workers,
            jobs,
            depth: Vec::new(),
            typical_secs: 60,
            on_front: None,
            on_cancel: None,
        }
    }

    /// The backlog's depth over time, oldest first.
    pub fn depth(mut self, samples: Vec<u32>) -> Self {
        self.depth = samples;
        self
    }

    /// How long a job usually takes, for the expected waits. One minute by default.
    pub fn typical(mut self, secs: u32) -> Self {
        self.typical_secs = secs.max(1);
        self
    }

    /// Run when a job is moved to the front.
    pub fn on_front(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_front = Some(Rc::new(handler));
        self
    }

    /// Run when a job is cancelled.
    pub fn on_cancel(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }
}

impl Styled for JobQueueView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for JobQueueView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let warning = Tone::Warning.color(cx);

        let online = self.workers.iter().filter(|w| w.online).count();
        let busy = self.workers.iter().filter(|w| w.job.is_some()).count();

        let slots: Vec<_> =
            self.workers
                .iter()
                .map(|worker| {
                    let (state, color) = match (&worker.job, worker.online) {
                        (_, false) => ("Off", quiet),
                        (Some(_), true) => ("Busy", accent),
                        (None, true) => ("Free", Tone::Success.color(cx)),
                    };
                    v_flex()
                        .id(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("slot-{}", worker.id).into(),
                        ))
                        .w(px(150.))
                        .flex_grow(1.)
                        .gap_1()
                        .p_2p5()
                        .rounded(radius)
                        .border_1()
                        .border_color(line)
                        .when(!worker.online, |this| this.border_dashed().opacity(0.6))
                        .role(Role::Group)
                        .aria_label(format!(
                            "{}, {}{}",
                            worker.name,
                            state,
                            worker
                                .job
                                .as_ref()
                                .map(|(job, _)| format!(", {job}"))
                                .unwrap_or_default()
                        ))
                        .child(
                            h_flex()
                                .gap_1p5()
                                .text_caption()
                                .child(div().size(px(7.)).rounded_full().bg(color))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_medium()
                                        .child(worker.name.clone()),
                                )
                                .child(div().text_color(quiet).child(state)),
                        )
                        .child(
                            div()
                                .text_caption()
                                .truncate()
                                .text_color(if worker.job.is_some() { ink } else { quiet })
                                .child(
                                    worker
                                        .job
                                        .as_ref()
                                        .map(|(job, _)| job.clone())
                                        .unwrap_or_else(|| "\u{2014}".into()),
                                ),
                        )
                        .child(
                            div().h(px(3.)).rounded_full().bg(chip).child(
                                div().h_full().rounded_full().bg(accent).w(relative(
                                    worker.job.as_ref().map(|(_, p)| *p).unwrap_or(0.),
                                )),
                            ),
                        )
                })
                .collect();

        let free = online.saturating_sub(busy).max(online.min(1));
        let rows: Vec<_> = self
            .jobs
            .iter()
            .enumerate()
            .map(|(ix, job)| {
                let front = self.on_front.clone();
                let cancel = self.on_cancel.clone();
                let (a, b) = (job.id.clone(), job.id.clone());
                let eta = expected_wait(ix, free, self.typical_secs);
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("job-{}", job.id).into(),
                    ))
                    .gap_2()
                    .px_2()
                    .py_1p5()
                    .when(ix > 0, |this| this.border_t_1().border_color(line))
                    .role(Role::ListItem)
                    .aria_label(format!(
                        "{}, waited {}",
                        job.label,
                        waited_words(job.waited_secs)
                    ))
                    .child(
                        div()
                            .w(px(18.))
                            .flex_none()
                            .text_caption()
                            .text_color(quiet)
                            .child((ix + 1).to_string()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .child(div().min_w_0().truncate().child(job.label.clone()))
                                    .when(job.urgent, |this| {
                                        this.child(
                                            div()
                                                .flex_none()
                                                .text_caption()
                                                .text_color(warning)
                                                .font_medium()
                                                .child("Urgent"),
                                        )
                                    }),
                            )
                            .child(div().text_caption().text_color(quiet).child(format!(
                                "waited {}{}",
                                waited_words(job.waited_secs),
                                match eta {
                                    Some(0) => ", next up".to_string(),
                                    Some(secs) =>
                                        format!(", starts in about {}", waited_words(secs)),
                                    None => ", no worker online".to_string(),
                                }
                            ))),
                    )
                    .child(
                        Button::new(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("front-{}", job.id).into(),
                        ))
                        .xsmall()
                        .ghost()
                        .icon(IconName::ArrowUp)
                        .disabled(ix == 0)
                        .accessibility_label(format!("Move {} to the front", job.label))
                        .on_click(move |_, window, cx| {
                            if let Some(front) = &front {
                                front(&a, window, cx);
                            }
                        }),
                    )
                    .child(
                        Button::new(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("cancel-{}", job.id).into(),
                        ))
                        .xsmall()
                        .ghost()
                        .icon(IconName::Close)
                        .accessibility_label(format!("Cancel {}", job.label))
                        .on_click(move |_, window, cx| {
                            if let Some(cancel) = &cancel {
                                cancel(&b, window, cx);
                            }
                        }),
                    )
            })
            .collect();

        let samples = self.depth.clone();
        let peak = samples
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .max(online as u32)
            .max(1);
        let latest = samples.last().copied().unwrap_or(self.jobs.len() as u32);
        let first = samples.first().copied().unwrap_or(latest);
        let trend = latest as i64 - first as i64;
        let limit_share = online as f32 / peak as f32;
        let chart = (!samples.is_empty()).then(|| {
            let samples = samples.clone();
            let grid = line;
            v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .text_caption()
                        .text_color(quiet)
                        .child(div().flex_1().child("Backlog over time"))
                        .child(format!(
                            "{latest} now, {}",
                            match trend {
                                0 => "steady".to_string(),
                                t if t > 0 => format!("up {t}"),
                                t => format!("down {}", -t),
                            }
                        )),
                )
                .child(
                    div()
                        .id("backlog-chart")
                        .relative()
                        .h(px(64.))
                        .w_full()
                        .role(Role::Image)
                        .aria_label(format!(
                            "Backlog depth over {} samples, peak {peak}, now {latest}",
                            samples.len()
                        ))
                        .child(
                            canvas(
                                |_, _, _| {},
                                move |bounds, _, window, _| {
                                    let n = samples.len();
                                    let x = |ix: usize| {
                                        bounds.origin.x
                                            + if n > 1 {
                                                bounds.size.width * (ix as f32 / (n - 1) as f32)
                                            } else {
                                                bounds.size.width / 2.
                                            }
                                    };
                                    let y = |v: f32| {
                                        bounds.origin.y
                                            + bounds.size.height * (1. - v / peak as f32)
                                    };
                                    let mut base =
                                        PathBuilder::stroke(px(1.)).dash_array(&[px(3.), px(3.)]);
                                    base.move_to(point(bounds.origin.x, y(online as f32)));
                                    base.line_to(point(
                                        bounds.origin.x + bounds.size.width,
                                        y(online as f32),
                                    ));
                                    if let Ok(path) = base.build() {
                                        window.paint_path(path, grid);
                                    }
                                    let mut path = PathBuilder::stroke(px(2.));
                                    for (ix, v) in samples.iter().enumerate() {
                                        let p = point(x(ix), y(*v as f32));
                                        if ix == 0 {
                                            path.move_to(p);
                                        } else {
                                            path.line_to(p);
                                        }
                                    }
                                    if let Ok(path) = path.build() {
                                        window.paint_path(path, accent);
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
                                .right_0()
                                .bottom(relative(limit_share.min(1.)))
                                .text_caption()
                                .text_color(quiet)
                                .child(format!("{online} workers")),
                        ),
                )
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(720.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(line)
                    .child(Icon::new(IconName::Inbox).small().text_color(quiet))
                    .child(div().flex_1().font_medium().child("Job queue"))
                    .child(div().text_caption().text_color(quiet).child(format!(
                        "{} waiting, {busy} of {online} workers busy",
                        self.jobs.len()
                    ))),
            )
            .child(h_flex().flex_wrap().gap_2().p_3().children(slots))
            .children(chart.map(|chart| div().px_4().pb_3().child(chart)))
            .child(
                v_flex()
                    .px_2()
                    .py_1()
                    .border_t_1()
                    .border_color(line)
                    .children(rows)
                    .when(self.jobs.is_empty(), |this| {
                        this.child(
                            div()
                                .px_2()
                                .py_3()
                                .text_color(quiet)
                                .child("Nothing waiting."),
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
    fn waits_read_short_and_estimates_divide_by_workers() {
        assert_eq!(waited_words(40), "40s");
        assert_eq!(waited_words(180), "3m");
        assert_eq!(waited_words(3_900), "1h 5m");
        assert_eq!(expected_wait(0, 2, 60), Some(0));
        assert_eq!(expected_wait(5, 2, 60), Some(120));
        assert_eq!(expected_wait(3, 0, 60), None);
    }
}
