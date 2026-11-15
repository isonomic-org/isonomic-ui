//! Task inbox board: every background task in one calm place.
//!
//! Tasks sit in three columns: what needs a person (waiting or failed), what
//! is running, and what is done. Running clocks tick. A card opens to its
//! last steps and the one thing that can be done now: approve or reject,
//! retry, or open. The same tasks can be read as a list instead.

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

/// How often the running clocks redraw.
const TICK: Duration = Duration::from_secs(1);

/// Where a task stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxStatus {
    /// Working in the background.
    Running,
    /// Stopped until a person approves or rejects.
    Waiting,
    /// Finished.
    Done,
    /// Stopped with an error.
    Failed,
}

/// How the board lays tasks out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InboxLayout {
    /// Three columns side by side.
    #[default]
    Board,
    /// One list with a heading per section.
    List,
}

/// One step of a task's history.
#[derive(Debug, Clone)]
pub struct InboxStep {
    label: SharedString,
    when: Option<SharedString>,
}

impl InboxStep {
    /// Create a step, with when it happened in words ("2m ago", "now").
    pub fn new(label: impl Into<SharedString>, when: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            when: Some(when.into()),
        }
    }
}

/// One background task.
#[derive(Debug, Clone)]
pub struct InboxTask {
    id: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    agent: Option<SharedString>,
    status: InboxStatus,
    started: Option<Instant>,
    elapsed: Option<Duration>,
    tools: Option<usize>,
    sources: Option<usize>,
    steps: Vec<InboxStep>,
}

impl InboxTask {
    /// Create a task.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        status: InboxStatus,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            agent: None,
            status,
            started: None,
            elapsed: None,
            tools: None,
            sources: None,
            steps: Vec::new(),
        }
    }

    /// Say what it is about or where it stands, in a sentence or two.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Name the agent or model doing it.
    pub fn agent(mut self, agent: impl Into<SharedString>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    /// Say when it started, so its clock ticks. Keep the same instant between
    /// renders.
    pub fn started(mut self, at: Instant) -> Self {
        self.started = Some(at);
        self
    }

    /// Say how long a finished task took.
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    /// Say how many tool calls and sources it used.
    pub fn usage(mut self, tools: usize, sources: usize) -> Self {
        self.tools = Some(tools);
        self.sources = Some(sources);
        self
    }

    /// Add a step to its history, oldest first.
    pub fn step(mut self, step: InboxStep) -> Self {
        self.steps.push(step);
        self
    }

    /// The task's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// The task's title.
    pub fn title(&self) -> &SharedString {
        &self.title
    }
}

type TaskHandler = Rc<dyn Fn(&InboxTask, &mut Window, &mut App)>;
type LayoutHandler = Rc<dyn Fn(InboxLayout, &mut Window, &mut App)>;

/// Background tasks by status, as a board or a list.
#[derive(IntoElement)]
pub struct TaskInboxBoard {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    tasks: Vec<InboxTask>,
    layout: Option<InboxLayout>,
    on_layout_change: Option<LayoutHandler>,
    on_approve: Option<TaskHandler>,
    on_reject: Option<TaskHandler>,
    on_retry: Option<TaskHandler>,
    on_open: Option<TaskHandler>,
}

impl TaskInboxBoard {
    /// Create an empty board.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Tasks".into(),
            tasks: Vec::new(),
            layout: None,
            on_layout_change: None,
            on_approve: None,
            on_reject: None,
            on_retry: None,
            on_open: None,
        }
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Add a task. Inside a column, tasks keep the order they were added.
    pub fn task(mut self, task: InboxTask) -> Self {
        self.tasks.push(task);
        self
    }

    /// Fix the layout. Without it the board keeps its own, starting as a
    /// board.
    pub fn layout(mut self, layout: InboxLayout) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Run when Board or List is picked.
    pub fn on_layout_change(
        mut self,
        handler: impl Fn(InboxLayout, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_layout_change = Some(Rc::new(handler));
        self
    }

    /// Run on Approve. Without it the task turns running inside the board.
    pub fn on_approve(
        mut self,
        handler: impl Fn(&InboxTask, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_approve = Some(Rc::new(handler));
        self
    }

    /// Run on Reject. Without it the task turns failed inside the board.
    pub fn on_reject(
        mut self,
        handler: impl Fn(&InboxTask, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reject = Some(Rc::new(handler));
        self
    }

    /// Run on Retry. Without it the task runs again inside the board.
    pub fn on_retry(
        mut self,
        handler: impl Fn(&InboxTask, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    /// Show Open on running and done tasks.
    pub fn on_open(
        mut self,
        handler: impl Fn(&InboxTask, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

impl Styled for TaskInboxBoard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The three sections, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    NeedsYou,
    Running,
    Done,
}

impl Section {
    fn of(status: InboxStatus) -> Self {
        match status {
            InboxStatus::Waiting | InboxStatus::Failed => Section::NeedsYou,
            InboxStatus::Running => Section::Running,
            InboxStatus::Done => Section::Done,
        }
    }

    fn heading(self) -> &'static str {
        match self {
            Section::NeedsYou => "Needs you",
            Section::Running => "Running",
            Section::Done => "Done",
        }
    }
}

#[derive(Default)]
struct InboxUi {
    layout: InboxLayout,
    open: Option<SharedString>,
    /// Changes the board made itself, for tasks without handlers.
    changed: HashMap<SharedString, (InboxStatus, Option<Instant>)>,
    ticker: Option<Task<()>>,
}

/// A clock: "45s", "6m 01s", "1h 04m".
fn clock(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, secs % 3600 / 60)
    }
}

/// What the bar at the bottom says: "1 needs you, 2 running".
fn summary(tasks: &[InboxTask]) -> String {
    let needs = tasks
        .iter()
        .filter(|task| Section::of(task.status) == Section::NeedsYou)
        .count();
    let running = tasks
        .iter()
        .filter(|task| task.status == InboxStatus::Running)
        .count();
    let needs_words = match needs {
        1 => "1 needs you".to_string(),
        n => format!("{n} need you"),
    };
    match (needs, running) {
        (0, 0) => "Nothing waiting or running".into(),
        (0, running) => format!("{running} running"),
        (_, 0) => needs_words,
        (_, running) => format!("{needs_words}, {running} running"),
    }
}

impl RenderOnce for TaskInboxBoard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let success = theme.success;
        let warning = theme.warning;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let ui = window.use_keyed_state(self.id.clone(), cx, |_, _| InboxUi::default());
        let (own_layout, open, changed) = {
            let ui = ui.read(cx);
            (ui.layout, ui.open.clone(), ui.changed.clone())
        };
        let layout = self.layout.unwrap_or(own_layout);

        // The tasks as shown, with any change the board made itself.
        let tasks: Vec<InboxTask> = self
            .tasks
            .iter()
            .cloned()
            .map(|mut task| {
                if let Some((status, started)) = changed.get(&task.id) {
                    task.status = *status;
                    if started.is_some() {
                        task.started = *started;
                    }
                }
                task
            })
            .collect();
        let any_running = tasks
            .iter()
            .any(|task| task.status == InboxStatus::Running && task.started.is_some());
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

        let tone = |status: InboxStatus| -> Hsla {
            match status {
                InboxStatus::Running => accent,
                InboxStatus::Waiting => warning,
                InboxStatus::Done => success,
                InboxStatus::Failed => danger,
            }
        };

        // An action: the owner's handler, or the board's own change.
        let act = |handler: Option<TaskHandler>,
                   fallback: Option<(InboxStatus, bool)>,
                   task: InboxTask| {
            let ui = ui.clone();
            move |_: &isonomic_ui::ClickEvent, window: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                match (&handler, fallback) {
                    (Some(handler), _) => handler(&task, window, cx),
                    (None, Some((status, restart))) => ui.update(cx, |ui, cx| {
                        let started = restart.then(Instant::now);
                        ui.changed.insert(task.id.clone(), (status, started));
                        cx.notify();
                    }),
                    (None, None) => {}
                }
            }
        };

        let card = |ix: usize, task: &InboxTask, compact: bool| {
            let is_open = open.as_ref() == Some(&task.id);
            let color = tone(task.status);
            let state: isonomic_ui::AnyElement = match task.status {
                InboxStatus::Running => h_flex()
                    .gap_1()
                    .child(
                        Icon::new(IconName::LoaderCircle)
                            .xsmall()
                            .text_color(accent),
                    )
                    .children(task.started.map(|at| clock(at.elapsed())))
                    .into_any_element(),
                InboxStatus::Done => h_flex()
                    .gap_1p5()
                    .child(div().size(px(7.)).rounded_full().bg(success))
                    .children(task.elapsed.map(clock))
                    .into_any_element(),
                InboxStatus::Waiting => h_flex()
                    .gap_1p5()
                    .text_color(warning)
                    .child(div().size(px(7.)).rounded_full().bg(warning))
                    .child("Waiting for you")
                    .into_any_element(),
                InboxStatus::Failed => h_flex()
                    .gap_1p5()
                    .text_color(danger)
                    .child(Icon::new(IconName::CircleAlert).xsmall())
                    .child("Failed")
                    .into_any_element(),
            };
            let usage = h_flex()
                .gap_2()
                .text_color(quiet)
                .children(task.sources.map(|sources| {
                    h_flex()
                        .gap_0p5()
                        .child(Icon::new(IconName::Search).xsmall())
                        .child(sources.to_string())
                }))
                .children(task.tools.map(|tools| {
                    h_flex()
                        .gap_0p5()
                        .child(Icon::new(IconName::Settings2).xsmall())
                        .child(tools.to_string())
                }));
            let details = is_open.then(|| {
                let actions = match task.status {
                    InboxStatus::Waiting => h_flex()
                        .gap_1p5()
                        .child(
                            Button::new(("inbox-approve", ix))
                                .primary()
                                .xsmall()
                                .label("Approve")
                                .on_click(act(
                                    self.on_approve.clone(),
                                    Some((InboxStatus::Running, false)),
                                    task.clone(),
                                )),
                        )
                        .child(
                            Button::new(("inbox-reject", ix))
                                .outline()
                                .xsmall()
                                .label("Reject")
                                .on_click(act(
                                    self.on_reject.clone(),
                                    Some((InboxStatus::Failed, false)),
                                    task.clone(),
                                )),
                        )
                        .into_any_element(),
                    InboxStatus::Failed => Button::new(("inbox-retry", ix))
                        .outline()
                        .xsmall()
                        .icon(IconName::RotateCw)
                        .label("Retry")
                        .on_click(act(
                            self.on_retry.clone(),
                            Some((InboxStatus::Running, true)),
                            task.clone(),
                        ))
                        .into_any_element(),
                    InboxStatus::Running | InboxStatus::Done if self.on_open.is_some() => {
                        Button::new(("inbox-open", ix))
                            .outline()
                            .xsmall()
                            .icon(IconName::ExternalLink)
                            .label("Open")
                            .on_click(act(self.on_open.clone(), None, task.clone()))
                            .into_any_element()
                    }
                    _ => div().into_any_element(),
                };
                v_flex()
                    .gap_1p5()
                    .pt_2()
                    .mt_1()
                    .border_t_1()
                    .border_color(line)
                    .children(task.agent.clone().map(|agent| {
                        h_flex()
                            .gap_1()
                            .text_xs()
                            .text_color(quiet)
                            .child(Icon::new(IconName::Bot).xsmall())
                            .child(agent)
                    }))
                    .children(task.steps.iter().enumerate().map(|(at, step)| {
                        let last = at + 1 == task.steps.len();
                        h_flex()
                            .gap_2()
                            .text_xs()
                            .child(div().flex_none().size(px(6.)).rounded_full().bg(if last {
                                color
                            } else {
                                quiet.opacity(0.4)
                            }))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .when(!last, |this| this.text_color(quiet))
                                    .child(step.label.clone()),
                            )
                            .children(
                                step.when
                                    .clone()
                                    .map(|when| div().flex_none().text_color(quiet).child(when)),
                            )
                    }))
                    .child(h_flex().pt_1().child(actions))
            });
            let ui_toggle = ui.clone();
            let id = task.id.clone();
            v_flex()
                .id(("inbox-task", ix))
                .gap_1()
                .p_3()
                .rounded(radius)
                .border_1()
                .border_color(if is_open { accent.opacity(0.5) } else { line })
                .bg(fill)
                .cursor_pointer()
                .hover(|style| style.border_color(accent.opacity(0.35)))
                .on_click(move |_, _, cx| {
                    ui_toggle.update(cx, |ui, cx| {
                        ui.open = if ui.open.as_ref() == Some(&id) {
                            None
                        } else {
                            Some(id.clone())
                        };
                        cx.notify();
                    })
                })
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_none().size(px(8.)).rounded(px(2.)).bg(color))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_medium()
                                .child(task.title.clone()),
                        )
                        .when(compact, |this| {
                            this.child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(color)
                                    .child(state_words(task)),
                            )
                        }),
                )
                .children(task.description.clone().map(|description| {
                    div()
                        .text_xs()
                        .text_color(quiet)
                        .line_height(relative(1.45))
                        .line_clamp(if compact { 1 } else { 2 })
                        .text_ellipsis()
                        .child(description)
                }))
                .when(!compact, |this| {
                    this.child(
                        h_flex()
                            .gap_2()
                            .pt_1()
                            .text_xs()
                            .child(div().flex_1().child(state))
                            .child(usage),
                    )
                })
                .children(details)
        };

        let sections = [Section::NeedsYou, Section::Running, Section::Done].map(|section| {
            let here: Vec<(usize, &InboxTask)> = tasks
                .iter()
                .enumerate()
                .filter(|(_, task)| Section::of(task.status) == section)
                .collect();
            (section, here)
        });

        let heading = |section: Section, count: usize| {
            h_flex()
                .gap_2()
                .px_1()
                .text_xs()
                .font_medium()
                .text_color(quiet)
                .child(div().flex_1().child(section.heading()))
                .child(
                    div()
                        .px_1p5()
                        .rounded(px(4.))
                        .bg(if section == Section::NeedsYou && count > 0 {
                            warning.opacity(0.18)
                        } else {
                            chip
                        })
                        .child(count.to_string()),
                )
        };

        let body = match layout {
            InboxLayout::Board => h_flex()
                .items_start()
                .gap_3()
                .children(sections.iter().map(|(section, here)| {
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .p_2()
                        .rounded(radius_lg)
                        .bg(chip.opacity(0.5))
                        .child(heading(*section, here.len()))
                        .children(here.iter().map(|(ix, task)| card(*ix, task, false)))
                        .when(here.is_empty(), |this| {
                            this.child(
                                div()
                                    .px_1()
                                    .py_2()
                                    .text_xs()
                                    .text_color(quiet)
                                    .child("Nothing here"),
                            )
                        })
                }))
                .into_any_element(),
            InboxLayout::List => v_flex()
                .gap_3()
                .children(sections.iter().filter(|(_, here)| !here.is_empty()).map(
                    |(section, here)| {
                        v_flex()
                            .gap_1p5()
                            .child(heading(*section, here.len()))
                            .children(here.iter().map(|(ix, task)| card(*ix, task, true)))
                    },
                ))
                .into_any_element(),
        };

        let switch = |label: &'static str, icon: IconName, which: InboxLayout| {
            let ui = ui.clone();
            let handler = self.on_layout_change.clone();
            Button::new(label)
                .xsmall()
                .icon(icon)
                .label(label)
                .map(|this| {
                    if layout == which {
                        this.outline()
                    } else {
                        this.ghost()
                    }
                })
                .on_click(move |_, window, cx| {
                    ui.update(cx, |ui, cx| {
                        ui.layout = which;
                        cx.notify();
                    });
                    if let Some(handler) = &handler {
                        handler(which, window, cx);
                    }
                })
        };

        v_flex()
            .id(self.id.clone())
            .w_full()
            .gap_3()
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.3))
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().font_semibold().child(self.title))
                    .child(
                        h_flex()
                            .gap_0p5()
                            .p_0p5()
                            .rounded(radius)
                            .bg(chip)
                            .child(switch(
                                "Board",
                                IconName::LayoutDashboard,
                                InboxLayout::Board,
                            ))
                            .child(switch("List", IconName::Menu, InboxLayout::List)),
                    ),
            )
            .child(body)
            .child(div().text_xs().text_color(quiet).child(summary(&tasks)))
            .refine_style(&self.style)
    }
}

/// A task's state in a few words, for the list rows.
fn state_words(task: &InboxTask) -> String {
    match task.status {
        InboxStatus::Running => task
            .started
            .map(|at| clock(at.elapsed()))
            .unwrap_or_else(|| "Running".into()),
        InboxStatus::Done => task.elapsed.map(clock).unwrap_or_else(|| "Done".into()),
        InboxStatus::Waiting => "Waiting for you".into(),
        InboxStatus::Failed => "Failed".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_read_short() {
        assert_eq!(clock(Duration::from_secs(45)), "45s");
        assert_eq!(clock(Duration::from_secs(361)), "6m 01s");
        assert_eq!(clock(Duration::from_secs(3840)), "1h 04m");
    }

    #[test]
    fn waiting_and_failed_both_need_you() {
        assert_eq!(Section::of(InboxStatus::Waiting), Section::NeedsYou);
        assert_eq!(Section::of(InboxStatus::Failed), Section::NeedsYou);
        assert_eq!(Section::of(InboxStatus::Running), Section::Running);
    }

    #[test]
    fn the_summary_counts_what_matters() {
        let tasks = [
            InboxTask::new("a", "A", InboxStatus::Failed),
            InboxTask::new("b", "B", InboxStatus::Running),
            InboxTask::new("c", "C", InboxStatus::Running),
            InboxTask::new("d", "D", InboxStatus::Done),
        ];
        assert_eq!(summary(&tasks), "1 needs you, 2 running");
        assert_eq!(summary(&tasks[3..]), "Nothing waiting or running");
    }
}
