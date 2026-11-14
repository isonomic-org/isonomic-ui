//! Reasoning steps panel: shows what an assistant is doing while it works.
//!
//! A header counts the seconds. Under it the steps appear one by one on a
//! timeline, the newest one shimmering, and a step that searched lists what it
//! looked at as chips. When the work is done the header reads "Worked for 7s"
//! and folds the steps away on demand.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    App, Context, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};
use gpui_base::Button as Pressable;
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, ThemeStyled as _, h_flex,
    shimmer::ShimmerText, spinner::Spinner, tooltip::Tooltip, v_flex,
};

/// How often the clock and the steps are checked.
const TICK: Duration = Duration::from_millis(100);
/// How long a looping panel rests on its finished state before starting over.
const RESTART_AFTER: Duration = Duration::from_secs(3);

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

type DoneHandler = Rc<dyn Fn(Duration, &mut Window, &mut App)>;

/// A live trail of what an assistant is doing, with a clock.
#[derive(IntoElement)]
pub struct ReasoningStepsPanel {
    id: ElementId,
    style: StyleRefinement,
    steps: Vec<ReasoningStep>,
    working_label: SharedString,
    done_label: SharedString,
    interval: Duration,
    looping: bool,
    collapse_when_done: bool,
    on_done: Option<DoneHandler>,
}

impl ReasoningStepsPanel {
    /// Create a panel with no steps. The clock starts when it is first drawn.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            steps: Vec::new(),
            working_label: "Thinking".into(),
            done_label: "Finished thinking".into(),
            interval: Duration::from_millis(1300),
            looping: false,
            collapse_when_done: false,
            on_done: None,
        }
    }

    /// Add a step after the ones already added.
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

    /// Set the pause between two steps.
    pub fn step_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Start over a few seconds after finishing. Meant for previews.
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Fold the steps away once the work is done. The header stays.
    pub fn collapse_when_done(mut self, collapse: bool) -> Self {
        self.collapse_when_done = collapse;
        self
    }

    /// Run once all steps are done, with the time the work took.
    pub fn on_done(mut self, handler: impl Fn(Duration, &mut Window, &mut App) + 'static) -> Self {
        self.on_done = Some(Rc::new(handler));
        self
    }
}

impl Styled for ReasoningStepsPanel {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct PanelState {
    started: Instant,
    /// How many steps are showing; the last of them is the current one.
    shown: usize,
    seconds: u64,
    finished: Option<Duration>,
    collapsed: bool,
    /// Steps whose hidden sources were asked for.
    all_sources: Vec<usize>,
    // Copied from the element on every draw, for the timer to read.
    steps: usize,
    interval: Duration,
    looping: bool,
    collapse_when_done: bool,
    on_done: Option<DoneHandler>,
    _timer: Task<()>,
}

/// How many steps are showing after `elapsed`, and whether the work is done.
fn progress(elapsed: Duration, interval: Duration, steps: usize) -> (usize, bool) {
    let passed = (elapsed.as_millis() / interval.as_millis().max(1)) as usize;
    ((passed + 1).min(steps), passed >= steps)
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

impl PanelState {
    /// Advance the clock. Returns whether the timer should keep running, and
    /// the handler to call if the work finished on this tick.
    fn tick(&mut self, cx: &mut Context<Self>) -> (bool, Option<(Duration, DoneHandler)>) {
        let elapsed = self.started.elapsed();
        if let Some(total) = self.finished {
            if !self.looping {
                return (false, None);
            }
            if elapsed >= total + RESTART_AFTER {
                self.started = Instant::now();
                self.shown = 1.min(self.steps);
                self.seconds = 0;
                self.finished = None;
                self.collapsed = false;
                self.all_sources.clear();
                cx.notify();
            }
            return (true, None);
        }

        let (shown, finished) = progress(elapsed, self.interval, self.steps);
        if finished {
            self.shown = shown;
            self.finished = Some(elapsed);
            self.collapsed |= self.collapse_when_done;
            cx.notify();
            let done = self.on_done.clone().map(|handler| (elapsed, handler));
            return (self.looping, done);
        }
        if shown != self.shown || elapsed.as_secs() != self.seconds {
            self.shown = shown;
            self.seconds = elapsed.as_secs();
            cx.notify();
        }
        (true, None)
    }
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

        let state = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            let timer = cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor().timer(TICK).await;
                    let keep_going = cx.update(|window, cx| {
                        let Some(this) = this.upgrade() else {
                            return false;
                        };
                        let (keep_going, done) =
                            this.update(cx, |state: &mut PanelState, cx| state.tick(cx));
                        if let Some((elapsed, handler)) = done {
                            handler(elapsed, window, cx);
                        }
                        keep_going
                    });
                    if !keep_going.unwrap_or(false) {
                        break;
                    }
                }
            });
            PanelState {
                started: Instant::now(),
                shown: 0,
                seconds: 0,
                finished: None,
                collapsed: false,
                all_sources: Vec::new(),
                steps: 0,
                interval: Duration::from_secs(1),
                looping: false,
                collapse_when_done: false,
                on_done: None,
                _timer: timer,
            }
        });
        let (shown, seconds, finished, collapsed, all_sources) = state.update(cx, |state, _| {
            state.steps = self.steps.len();
            state.interval = self.interval;
            state.looping = self.looping;
            state.collapse_when_done = self.collapse_when_done;
            state.on_done = self.on_done.clone();
            if state.finished.is_none() {
                state.shown = progress(state.started.elapsed(), state.interval, state.steps).0;
            }
            (
                state.shown,
                state.seconds,
                state.finished,
                state.collapsed,
                state.all_sources.clone(),
            )
        });

        let child_id = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let focus = window
            .use_keyed_state((self.id.clone(), "header-focus"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);

        let header = Pressable::new((self.id.clone(), "header"))
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
                        .child(format!("Worked for {}", elapsed_label(total))),
                ),
                None => this
                    .child(
                        ShimmerText::new(self.working_label.clone()).id(child_id("working".into())),
                    )
                    .child(
                        div()
                            .text_color(quiet)
                            .child(elapsed_label(Duration::from_secs(seconds))),
                    ),
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
                .text_xs()
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
        let row = |marker: gpui::AnyElement, joined: bool, content: gpui::AnyElement| {
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
                    let mut chips: Vec<gpui::AnyElement> = step
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
                                .text_xs()
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
            .text_sm()
            .line_height(relative(1.5))
            .text_color(ink)
            .child(h_flex().child(header))
            .when(!collapsed, |this| {
                this.child(v_flex().pt_1().children(rows).children(done_row))
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: Duration = Duration::from_millis(1000);

    #[test]
    fn steps_appear_one_per_interval() {
        assert_eq!(progress(Duration::ZERO, STEP, 3), (1, false));
        assert_eq!(progress(Duration::from_millis(999), STEP, 3), (1, false));
        assert_eq!(progress(Duration::from_millis(1000), STEP, 3), (2, false));
        assert_eq!(progress(Duration::from_millis(2500), STEP, 3), (3, false));
    }

    #[test]
    fn the_work_is_done_after_the_last_step_had_its_turn() {
        assert_eq!(progress(Duration::from_millis(3000), STEP, 3), (3, true));
        assert_eq!(progress(Duration::from_secs(60), STEP, 3), (3, true));
    }

    #[test]
    fn nothing_to_do_is_done_at_once() {
        assert_eq!(progress(Duration::ZERO, STEP, 0), (0, true));
    }

    #[test]
    fn the_clock_switches_to_minutes() {
        assert_eq!(elapsed_label(Duration::from_secs(7)), "7s");
        assert_eq!(elapsed_label(Duration::from_secs(59)), "59s");
        assert_eq!(elapsed_label(Duration::from_secs(65)), "1m 05s");
    }
}
