// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reasoning steps panel: shows what an assistant is doing while it works.
//!
//! A header counts the seconds. Under it the steps sit on a timeline, the
//! newest one shimmering while the work goes on, and a step that searched lists
//! what it looked at as chips. The owner adds steps as they happen and marks
//! the work done; then the header reads "Took 7s" and folds the steps away on
//! demand.

use std::time::{Duration, Instant};

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _, h_flex,
    shimmer::ShimmerText, spinner::Spinner, tooltip::Tooltip, v_flex,
};
use isonomic_ui::{
    App, Context, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

/// How often the clock is redrawn while the work goes on.
const TICK: Duration = Duration::from_millis(250);

/// Something a step looked at.
#[derive(Debug, Clone)]
pub struct ReasoningSource {
    title: SharedString,
    detail: Option<SharedString>,
    url: Option<SharedString>,
}

impl ReasoningSource {
    /// Create a source with the name shown on its chip.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            detail: None,
            url: None,
        }
    }

    /// A line for the tooltip, such as where the source lives.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Open this address when the chip is pressed.
    pub fn url(mut self, url: impl Into<SharedString>) -> Self {
        self.url = Some(url.into());
        self
    }
}

/// One step of the work.
#[derive(Debug, Clone)]
pub struct ReasoningStep {
    label: SharedString,
    finished_label: Option<SharedString>,
    sources: Vec<ReasoningSource>,
    hidden_sources: Vec<ReasoningSource>,
}

impl ReasoningStep {
    /// Create a step with the label shown while it is the current one.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            finished_label: None,
            sources: Vec::new(),
            hidden_sources: Vec::new(),
        }
    }

    /// The label once the step is behind us, such as "Searched" for "Searching".
    pub fn finished_label(mut self, label: impl Into<SharedString>) -> Self {
        self.finished_label = Some(label.into());
        self
    }

    /// Add a source shown as a chip under the step.
    pub fn source(mut self, source: ReasoningSource) -> Self {
        self.sources.push(source);
        self
    }

    /// Add a source kept behind a "+N more" chip until it is asked for.
    pub fn hidden_source(mut self, source: ReasoningSource) -> Self {
        self.hidden_sources.push(source);
        self
    }
}

/// A live trail of what an assistant is doing, with a clock.
#[derive(IntoElement)]
pub struct ReasoningStepsPanel {
    id: ElementId,
    style: StyleRefinement,
    steps: Vec<ReasoningStep>,
    working_label: SharedString,
    done_label: SharedString,
    started_at: Option<Instant>,
    took: Option<Duration>,
    collapse_when_done: bool,
}

impl ReasoningStepsPanel {
    /// Create a panel with no steps, still working.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            steps: Vec::new(),
            working_label: "Working it out".into(),
            done_label: "Worked out".into(),
            started_at: None,
            took: None,
            collapse_when_done: false,
        }
    }

    /// Add a step after the ones already added. While the work goes on, the
    /// last step is the current one.
    pub fn step(mut self, step: ReasoningStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Set the header label shown while working.
    pub fn working_label(mut self, label: impl Into<SharedString>) -> Self {
        self.working_label = label.into();
        self
    }

    /// Set the label of the last row, shown once every step is done.
    pub fn done_label(mut self, label: impl Into<SharedString>) -> Self {
        self.done_label = label.into();
        self
    }

    /// When the work started. Without it the clock counts from when the
    /// panel was first drawn working.
    pub fn started_at(mut self, started_at: Instant) -> Self {
        self.started_at = Some(started_at);
        self
    }

    /// Mark the work done, with how long it took.
    pub fn done(mut self, took: Duration) -> Self {
        self.took = Some(took);
        self
    }

    /// Fold the steps away when the work is marked done. The header stays.
    pub fn collapse_when_done(mut self, collapse: bool) -> Self {
        self.collapse_when_done = collapse;
        self
    }
}

impl Styled for ReasoningStepsPanel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct PanelState {
    /// When the panel was first drawn working, for a clock without a start.
    first_seen: Option<Instant>,
    /// Whether the work was done at the last draw.
    was_done: bool,
    collapsed: bool,
    /// Steps whose hidden sources were asked for.
    all_sources: Vec<usize>,
    ticker: Option<Task<()>>,
}

/// A duration as the header shows it: "7s", then "1m 05s".
fn elapsed_label(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    }
}

/// Whether the steps show after a draw: work marked done folds them when the
/// panel asks for that, and work that starts again unfolds them.
fn collapsed_after(collapsed: bool, was_done: bool, done: bool, collapse_when_done: bool) -> bool {
    match (was_done, done) {
        (false, true) => collapsed || collapse_when_done,
        (true, false) => false,
        _ => collapsed,
    }
}

/// Redraw on every tick for as long as the task is kept.
fn ticker(cx: &mut Context<PanelState>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(TICK).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    })
}

impl RenderOnce for ReasoningStepsPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let accent = theme.info;
        let radius = theme.radius;

        let done = self.took.is_some();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| PanelState {
            first_seen: None,
            was_done: done,
            collapsed: done && self.collapse_when_done,
            all_sources: Vec::new(),
            ticker: None,
        });
        let (elapsed, collapsed, all_sources) = state.update(cx, |state, cx| {
            state.collapsed = collapsed_after(
                state.collapsed,
                state.was_done,
                done,
                self.collapse_when_done,
            );
            if done {
                state.first_seen = None;
                state.ticker = None;
            } else {
                if state.was_done {
                    // The work started again: so do the extra sources.
                    state.all_sources.clear();
                }
                state.first_seen.get_or_insert_with(Instant::now);
                if state.ticker.is_none() {
                    state.ticker = Some(ticker(cx));
                }
            }
            state.was_done = done;
            let started = self.started_at.or(state.first_seen);
            (
                self.took
                    .or_else(|| started.map(|at| at.elapsed()))
                    .unwrap_or_default(),
                state.collapsed,
                state.all_sources.clone(),
            )
        });
        let finished = self.took;
        let shown = self.steps.len();

        let child_id = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let focus = window
            .use_keyed_state((self.id.clone(), "header-focus"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);

        // Read as the line shows: "Took 5s", or "Working it out, 6s".
        let header_name = match finished {
            Some(total) => format!("Took {}", elapsed_label(total)),
            None => format!("{}, {}", self.working_label, elapsed_label(elapsed)),
        };
        let header = Pressable::new((self.id.clone(), "header"))
            .accessibility_label(header_name)
            .aria_expanded(!collapsed)
            .track_focus(&focus)
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |state, cx| {
                        state.collapsed = !state.collapsed;
                        cx.notify();
                    })
                }
            })
            .justify_start()
            .gap_2()
            .h(px(28.))
            .pr_1()
            .rounded(radius)
            .cursor_pointer()
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size_4()
                    .map(|this| match finished {
                        Some(_) => {
                            this.child(Icon::new(IconName::CircleCheck).small().text_color(accent))
                        }
                        None => this.child(Spinner::new().small().color(accent)),
                    }),
            )
            .map(|this| match finished {
                Some(total) => this.child(
                    div()
                        .text_color(quiet)
                        .child(format!("Took {}", elapsed_label(total))),
                ),
                None => this
                    .child(
                        ShimmerText::new(self.working_label.clone()).id(child_id("working".into())),
                    )
                    .child(div().text_color(quiet).child(elapsed_label(elapsed))),
            })
            .child(
                Icon::new(if collapsed {
                    IconName::ChevronRight
                } else {
                    IconName::ChevronDown
                })
                .xsmall()
                .flex_none()
                .text_color(quiet),
            );

        let chip = |key: String, source: ReasoningSource| {
            div()
                .id(child_id(key))
                .flex()
                .flex_none()
                .items_center()
                .gap_1p5()
                .h(px(26.))
                .px_2()
                .rounded(radius)
                .border_1()
                .border_color(line)
                .bg(fill)
                .text_caption()
                .text_color(ink)
                .child(Icon::new(IconName::Globe).xsmall().text_color(quiet))
                .child(div().max_w(px(220.)).truncate().child(source.title))
                .when_some(source.detail, |this, detail| {
                    this.tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx))
                })
                .when_some(source.url, |this, url| {
                    this.cursor_pointer()
                        .on_click(move |_, _, cx| cx.open_url(&url))
                })
        };
        // Every row has a marker on the timeline and its content beside it.
        let row =
            |marker: isonomic_ui::AnyElement, joined: bool, content: isonomic_ui::AnyElement| {
                h_flex()
                    .items_stretch()
                    .gap_2()
                    .child(
                        v_flex()
                            .flex_none()
                            .w_4()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .justify_center()
                                    .h(px(22.))
                                    .child(marker),
                            )
                            .when(joined, |this| this.child(div().flex_1().w(px(1.)).bg(line))),
                    )
                    .child(div().flex_1().min_w_0().pb_2().child(content))
            };

        let step_count = self.steps.len();
        let rows =
            self.steps
                .into_iter()
                .take(shown)
                .enumerate()
                .map(|(ix, step)| {
                    let is_current = finished.is_none() && ix + 1 == shown;
                    let searched = !step.sources.is_empty() || !step.hidden_sources.is_empty();
                    let label = match step.finished_label {
                        Some(label) if !is_current => label,
                        _ => step.label,
                    };

                    let marker = if searched {
                        Icon::new(IconName::Search)
                            .small()
                            .text_color(quiet)
                            .into_any_element()
                    } else if is_current {
                        div()
                            .size(px(6.))
                            .rounded(px(1.))
                            .bg(accent)
                            .into_any_element()
                    } else {
                        div()
                            .size(px(5.))
                            .rounded_full_style(cx)
                            .bg(quiet.opacity(0.5))
                            .into_any_element()
                    };

                    let hidden = step.hidden_sources.len();
                    let show_all = all_sources.contains(&ix);
                    let mut chips: Vec<isonomic_ui::AnyElement> = step
                        .sources
                        .into_iter()
                        .enumerate()
                        .map(|(n, source)| {
                            chip(format!("source-{ix}-{n}"), source).into_any_element()
                        })
                        .collect();
                    if show_all {
                        chips.extend(step.hidden_sources.into_iter().enumerate().map(
                            |(n, source)| chip(format!("more-{ix}-{n}"), source).into_any_element(),
                        ));
                    } else if hidden > 0 {
                        let state = state.clone();
                        chips.push(
                            Pressable::new(child_id(format!("show-more-{ix}")))
                                .accessibility_label(more_sources_label(hidden))
                                .on_click(move |_, _, cx| {
                                    state.update(cx, |state, cx| {
                                        state.all_sources.push(ix);
                                        cx.notify();
                                    })
                                })
                                .flex_none()
                                .h(px(26.))
                                .px_2()
                                .rounded(radius)
                                .border_1()
                                .border_color(line)
                                .text_caption()
                                .text_color(quiet)
                                .cursor_pointer()
                                .child(format!("+{hidden} more"))
                                .into_any_element(),
                        );
                    }

                    let content = v_flex()
                        .gap_1p5()
                        .child(h_flex().h(px(22.)).map(|this| {
                            if is_current {
                                this.child(
                                    ShimmerText::new(label).id(child_id(format!("label-{ix}"))),
                                )
                            } else {
                                this.child(div().truncate().text_color(quiet).child(label))
                            }
                        }))
                        .when(!chips.is_empty(), |this| {
                            this.child(h_flex().flex_wrap().gap_1p5().children(chips))
                        })
                        .into_any_element();

                    // The line runs on to the next row, or to the last row once done.
                    row(marker, !is_current, content)
                });
        let rows: Vec<_> = rows.collect();

        let done_row = finished.filter(|_| step_count > 0).map(|_| {
            row(
                Icon::new(IconName::Check)
                    .small()
                    .text_color(accent)
                    .into_any_element(),
                false,
                h_flex()
                    .h(px(22.))
                    .font_medium()
                    .child(self.done_label.clone())
                    .into_any_element(),
            )
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(520.))
            .text_body()
            .line_height(relative(1.5))
            .text_color(ink)
            .child(h_flex().child(header))
            .when(!collapsed, |this| {
                this.child(v_flex().pt_1().children(rows).children(done_row))
            })
            .refine_style(&self.style)
    }
}

/// What a screen reader hears for the chip that shows a step's other sources.
fn more_sources_label(hidden: usize) -> String {
    match hidden {
        1 => "Show 1 more source".into(),
        n => format!("Show {n} more sources"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_more_chip_says_how_many_sources_it_shows() {
        assert_eq!(more_sources_label(1), "Show 1 more source");
        assert_eq!(more_sources_label(2), "Show 2 more sources");
    }

    #[test]
    fn marking_the_work_done_folds_the_steps_when_asked() {
        // Done for the first time: folds only if the panel asks for it.
        assert!(collapsed_after(false, false, true, true));
        assert!(!collapsed_after(false, false, true, false));
        // Still done: a fold by hand stays as it is.
        assert!(!collapsed_after(false, true, true, true));
        // The work starts again: the steps show.
        assert!(!collapsed_after(true, true, false, true));
        // Still working: whatever the header says.
        assert!(collapsed_after(true, false, false, false));
    }

    #[test]
    fn the_clock_switches_to_minutes() {
        assert_eq!(elapsed_label(Duration::from_secs(7)), "7s");
        assert_eq!(elapsed_label(Duration::from_secs(59)), "59s");
        assert_eq!(elapsed_label(Duration::from_secs(65)), "1m 05s");
    }
}
