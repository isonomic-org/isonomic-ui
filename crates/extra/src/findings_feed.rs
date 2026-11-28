// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Findings feed: several workers checking something at once.
//!
//! Each worker gets one calm row with what it is doing and how far it is.
//! What they find lands in a short feed below, newest first, each with its
//! severity, who found it and where. Pointing a worker shows only its finds.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Task, Window,
    div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How often the "5s ago" times redraw.
const TICK: Duration = Duration::from_secs(1);

/// How much a finding matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Must be fixed.
    Critical,
    /// Should be looked at.
    Warning,
    /// Worth knowing.
    Note,
}

impl Severity {
    fn word(self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
}

/// One worker of the swarm.
#[derive(Clone)]
pub struct FindingWorker {
    id: SharedString,
    name: SharedString,
    step: Option<SharedString>,
    done: usize,
    total: usize,
    icon: Option<IconName>,
}

impl FindingWorker {
    /// Create a worker with how many checks it has done of its total.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        done: usize,
        total: usize,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            step: None,
            done,
            total,
            icon: None,
        }
    }

    /// Say what it is checking now.
    pub fn step(mut self, step: impl Into<SharedString>) -> Self {
        self.step = Some(step.into());
        self
    }

    /// Show a mark before its name.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    fn finished(&self) -> bool {
        self.total > 0 && self.done >= self.total
    }
}

/// Something a worker found.
#[derive(Debug, Clone)]
pub struct Finding {
    id: SharedString,
    title: SharedString,
    severity: Severity,
    worker: SharedString,
    location: Option<SharedString>,
    found_at: Instant,
}

impl Finding {
    /// Create a finding by the worker with id `worker`, found at `found_at`.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        severity: Severity,
        worker: impl Into<SharedString>,
        found_at: Instant,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            severity,
            worker: worker.into(),
            location: None,
            found_at,
        }
    }

    /// Say where it was found, such as a file and line.
    pub fn location(mut self, location: impl Into<SharedString>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// The finding's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// What was found.
    pub fn title(&self) -> &SharedString {
        &self.title
    }
}

type OpenHandler = Rc<dyn Fn(&Finding, &mut Window, &mut App)>;

/// Workers' progress rows above a feed of what they found.
#[derive(IntoElement)]
pub struct FindingsFeed {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    workers: Vec<FindingWorker>,
    findings: Vec<Finding>,
    rows: usize,
    on_open: Option<OpenHandler>,
}

impl FindingsFeed {
    /// Create an empty feed.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Checks".into(),
            workers: Vec::new(),
            findings: Vec::new(),
            rows: 4,
            on_open: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Add a worker row.
    pub fn worker(mut self, worker: FindingWorker) -> Self {
        self.workers.push(worker);
        self
    }

    /// Add a finding. The feed shows the newest first.
    pub fn finding(mut self, finding: Finding) -> Self {
        self.findings.push(finding);
        self
    }

    /// Set how many findings the feed shows at most. Four by default.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Run when a finding is clicked.
    pub fn on_open(mut self, handler: impl Fn(&Finding, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl Styled for FindingsFeed {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[derive(Default)]
struct FeedUi {
    only: Option<SharedString>,
    ticker: Option<Task<()>>,
}

/// How long ago, shortly: "now", "5s", "2m", "1h".
fn ago_words(ago: Duration) -> String {
    let secs = ago.as_secs();
    if secs < 3 {
        "now".into()
    } else if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

/// "1 critical", "3 critical", "2 warnings".
fn count_words(count: usize, severity: Severity) -> String {
    match (count, severity) {
        (1, _) | (_, Severity::Critical) => format!("{count} {}", severity.word()),
        _ => format!("{count} {}s", severity.word()),
    }
}

/// The findings to show: the worker's own if one is picked, newest first,
/// at most `rows`.
fn latest<'a>(
    findings: &'a [Finding],
    only: Option<&SharedString>,
    rows: usize,
) -> Vec<&'a Finding> {
    let mut shown: Vec<&Finding> = findings
        .iter()
        .filter(|finding| only.is_none_or(|only| &finding.worker == only))
        .collect();
    shown.sort_by_key(|finding| std::cmp::Reverse(finding.found_at));
    shown.truncate(rows);
    shown
}

/// A small square in the severity's colour; a hollow one when there is none.
fn mark(color: Hsla, filled: bool) -> impl IntoElement {
    div().flex_none().size(px(9.)).rounded(px(2.)).map(|this| {
        if filled {
            this.bg(color)
        } else {
            this.border_1().border_color(color.opacity(0.4))
        }
    })
}

impl RenderOnce for FindingsFeed {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        // A tint in the ring colour marks the part the keyboard is on.
        let focus_tint = theme.ring.opacity(0.25);
        let accent = theme.blue;
        let success = theme.success;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();
        let tones = [theme.danger, theme.warning, theme.blue];
        let tone = move |severity: Severity| tones[severity as usize];

        let ui = window.use_keyed_state(self.id.clone(), cx, |_, _| FeedUi::default());
        ui.update(cx, |ui, cx| {
            if ui.ticker.is_none() {
                ui.ticker = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(TICK).await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                }));
            }
        });
        let only = ui
            .read(cx)
            .only
            .clone()
            .filter(|only| self.workers.iter().any(|worker| &worker.id == only));
        let now = Instant::now();

        let done: usize = self
            .workers
            .iter()
            .map(|worker| worker.done.min(worker.total))
            .sum();
        let total: usize = self.workers.iter().map(|worker| worker.total).sum();
        let count = |severity: Severity| {
            self.findings
                .iter()
                .filter(|finding| finding.severity == severity)
                .count()
        };
        let (critical, warnings) = (count(Severity::Critical), count(Severity::Warning));

        let header = h_flex()
            .gap_2()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(line)
            .child(Icon::new(IconName::Bot).small().text_color(accent))
            .child(div().flex_none().font_medium().child(self.title))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_caption()
                    .text_color(quiet)
                    .child(format!("{done} of {total} checks")),
            )
            .children(
                [
                    (critical, Severity::Critical),
                    (warnings, Severity::Warning),
                ]
                .map(|(count, severity)| {
                    (count > 0).then(|| {
                        h_flex()
                            .flex_none()
                            .gap_1()
                            .text_caption()
                            .text_color(tone(severity))
                            .child(div().size(px(6.)).rounded_full().bg(tone(severity)))
                            .child(count_words(count, severity))
                    })
                })
                .into_iter()
                .flatten(),
            );

        let workers = self.workers.iter().enumerate().map(|(ix, worker)| {
            let picked = only.as_ref() == Some(&worker.id);
            let dimmed = only.is_some() && !picked;
            let share = if worker.total == 0 {
                0.
            } else {
                (worker.done as f32 / worker.total as f32).clamp(0., 1.)
            };
            let found = |severity: Severity| {
                self.findings
                    .iter()
                    .filter(|finding| finding.worker == worker.id && finding.severity == severity)
                    .count()
            };
            let ui = ui.clone();
            let id = worker.id.clone();
            let worker_words = format!(
                "{}, {} of {} done, {} critical, {} warnings{}",
                worker.name,
                worker.done.min(worker.total),
                worker.total,
                found(Severity::Critical),
                found(Severity::Warning),
                if picked {
                    ", showing only theirs"
                } else {
                    ". Show only theirs"
                }
            );
            let (hover_fade, hover_on_hover) = isonomic_ui::component::look::tracked_hover(
                ("finding-worker-hover", ix),
                true,
                window,
                cx,
            );
            h_flex()
                .id(("finding-worker", ix))
                .role(Role::Button)
                .aria_label(worker_words)
                .aria_selected(picked)
                .tab_index(0)
                .focus_visible(move |s| s.bg(focus_tint))
                .gap_3()
                .px_4()
                .py_2()
                .cursor_pointer()
                .bg(crate::common::faded(chip, hover_fade))
                .on_hover(hover_on_hover)
                .when(picked, |this| this.bg(chip))
                .when(dimmed, |this| this.opacity(0.5))
                .on_click(move |_, _, cx| {
                    ui.update(cx, |ui, cx| {
                        ui.only = if ui.only.as_ref() == Some(&id) {
                            None
                        } else {
                            Some(id.clone())
                        };
                        cx.notify();
                    })
                })
                .child(
                    div()
                        .flex_none()
                        .size(px(24.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(line)
                        .child(
                            Icon::new(worker.icon.clone().unwrap_or(IconName::Bot))
                                .small()
                                .text_color(quiet),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().font_medium().truncate().child(worker.name.clone()))
                        .children(worker.step.clone().map(|step| {
                            div().text_caption().text_color(quiet).truncate().child(
                                if worker.finished() {
                                    SharedString::from("Done")
                                } else {
                                    step
                                },
                            )
                        })),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(96.))
                        .h(px(5.))
                        .rounded_full()
                        .bg(chip)
                        .child(div().h_full().w(relative(share)).rounded_full().bg(
                            if worker.finished() {
                                success
                            } else {
                                accent.opacity(0.7)
                            },
                        )),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(40.))
                        .text_right()
                        .text_caption()
                        .text_color(quiet)
                        .child(format!(
                            "{}/{}",
                            worker.done.min(worker.total),
                            worker.total
                        )),
                )
                .children([Severity::Critical, Severity::Warning].map(|severity| {
                    let found = found(severity);
                    h_flex()
                        .flex_none()
                        .w(px(26.))
                        .gap_1()
                        .text_caption()
                        .text_color(if found > 0 {
                            tone(severity)
                        } else {
                            quiet.opacity(0.5)
                        })
                        .child(mark(tone(severity), found > 0))
                        .child(found.to_string())
                }))
        });
        let workers: Vec<_> = workers.collect();

        let shown = latest(&self.findings, only.as_ref(), self.rows);
        let picked_name = only.as_ref().and_then(|only| {
            self.workers
                .iter()
                .find(|worker| &worker.id == only)
                .map(|worker| worker.name.clone())
        });
        let feed_rows = shown.into_iter().enumerate().map(|(ix, finding)| {
            let who = self
                .workers
                .iter()
                .find(|worker| worker.id == finding.worker)
                .map(|worker| worker.name.clone())
                .unwrap_or_else(|| finding.worker.clone());
            let color = tones[finding.severity as usize];
            let open = self.on_open.clone();
            let finding_for_click = finding.clone();
            let finding_words = format!(
                "{}: {}, by {who}{}, {}",
                finding.severity.word(),
                finding.title,
                finding
                    .location
                    .as_ref()
                    .map(|l| format!(" in {l}"))
                    .unwrap_or_default(),
                ago_words(now.saturating_duration_since(finding.found_at))
            );
            let (hover_fade, hover_on_hover) = isonomic_ui::component::look::tracked_hover(
                ("finding-hover", ix),
                open.is_some(),
                window,
                cx,
            );
            h_flex()
                .id(("finding", ix))
                .role(if open.is_some() {
                    Role::Button
                } else {
                    Role::ListItem
                })
                .aria_label(finding_words)
                .when(open.is_some(), |this| {
                    this.tab_index(0).focus_visible(move |s| s.bg(focus_tint))
                })
                .gap_2()
                .h(px(32.))
                .px_4()
                .when(open.is_some(), |this| {
                    this.cursor_pointer()
                        .bg(crate::common::faded(chip, hover_fade))
                        .on_hover(hover_on_hover)
                        .on_click(move |_, window, cx| {
                            if let Some(open) = &open {
                                open(&finding_for_click, window, cx);
                            }
                        })
                })
                .child(mark(color, true))
                .child(
                    div()
                        .flex_none()
                        .max_w(relative(0.55))
                        .truncate()
                        .child(finding.title.clone()),
                )
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1p5()
                        .text_caption()
                        .text_color(quiet)
                        .child(div().flex_none().child(who))
                        .children(finding.location.clone().map(|location| {
                            div()
                                .min_w_0()
                                .truncate()
                                .font_family(mono.clone())
                                .child(location)
                        })),
                )
                .child(
                    div()
                        .flex_none()
                        .text_caption()
                        .text_color(quiet)
                        .child(ago_words(now.saturating_duration_since(finding.found_at))),
                )
        });
        let feed_rows: Vec<_> = feed_rows.collect();
        let empty = feed_rows.is_empty();

        v_flex()
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label("Findings")
            .w_full()
            .max_w(px(640.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.3))
            .overflow_hidden()
            .shadow(resting_shadow(cx))
            .child(header)
            .child(v_flex().py_1().children(workers))
            .child(
                v_flex()
                    .border_t_1()
                    .border_color(line)
                    .pb_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .px_4()
                            .pt_2p5()
                            .pb_1()
                            .text_label_small()
                            .text_color(quiet)
                            .child(match &picked_name {
                                Some(name) => format!("Found by {name}"),
                                None => "Latest findings".to_string(),
                            })
                            .when(picked_name.is_some(), |this| {
                                let ui = ui.clone();
                                this.child(
                                    div()
                                        .id("findings-show-all")
                                        .role(Role::Button)
                                        .aria_label("Show all findings")
                                        .tab_index(0)
                                        .focus_visible(move |s| s.bg(focus_tint))
                                        .text_color(accent)
                                        .cursor_pointer()
                                        .on_click(move |_, _, cx| {
                                            ui.update(cx, |ui, cx| {
                                                ui.only = None;
                                                cx.notify();
                                            })
                                        })
                                        .child("Show all"),
                                )
                            }),
                    )
                    .children(feed_rows)
                    .when(empty, |this| {
                        this.child(
                            div()
                                .px_4()
                                .py_2()
                                .text_caption()
                                .text_color(quiet)
                                .child("Nothing found yet"),
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
    fn ages_read_short() {
        assert_eq!(ago_words(Duration::from_secs(1)), "now");
        assert_eq!(ago_words(Duration::from_secs(48)), "48s");
        assert_eq!(ago_words(Duration::from_secs(150)), "2m");
        assert_eq!(count_words(1, Severity::Critical), "1 critical");
        assert_eq!(count_words(2, Severity::Warning), "2 warnings");
        assert_eq!(count_words(3, Severity::Critical), "3 critical");
    }

    #[test]
    fn the_feed_is_newest_first_and_short() {
        let start = Instant::now();
        let at = |secs| start + Duration::from_secs(secs);
        let findings = [
            Finding::new("a", "A", Severity::Note, "x", at(1)),
            Finding::new("b", "B", Severity::Warning, "y", at(3)),
            Finding::new("c", "C", Severity::Critical, "x", at(2)),
        ];
        let ids = |shown: Vec<&Finding>| -> String {
            shown.iter().map(|finding| finding.id.as_ref()).collect()
        };
        assert_eq!(ids(latest(&findings, None, 2)), "bc");
        assert_eq!(ids(latest(&findings, Some(&"x".into()), 4)), "ca");
    }
}
