//! Plan editor: the steps an agent means to take, reviewed before it runs.
//!
//! In review, people rename, reorder, remove and add steps, then press Run.
//! The same card then follows the run: which step is going, how long each one
//! took, and where it stopped, with a way to try again from a failed step.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How often the timers redraw while the plan runs.
const TICK: Duration = Duration::from_millis(100);

/// Where one step stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStepStatus {
    /// Not started.
    Pending,
    /// Going now.
    Running,
    /// Finished well.
    Done,
    /// Stopped with an error.
    Failed,
}

/// One step of the plan.
#[derive(Debug, Clone)]
pub struct PlanStep {
    id: SharedString,
    title: SharedString,
    detail: Option<SharedString>,
    tool: Option<SharedString>,
    status: PlanStepStatus,
    took: Option<Duration>,
    error: Option<SharedString>,
}

impl PlanStep {
    /// Create a pending step. The id must stay the same when the step is
    /// renamed or moved.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            tool: None,
            status: PlanStepStatus::Pending,
            took: None,
            error: None,
        }
    }

    /// Add a line under the title.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Name the tool the step will call, shown as a small code tag.
    pub fn tool(mut self, tool: impl Into<SharedString>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    /// Set where the step stands.
    pub fn status(mut self, status: PlanStepStatus) -> Self {
        self.status = status;
        self
    }

    /// Say how long the step took, when the agent reports it.
    pub fn took(mut self, took: Duration) -> Self {
        self.took = Some(took);
        self
    }

    /// Mark the step failed, with what went wrong.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.status = PlanStepStatus::Failed;
        self.error = Some(error.into());
        self
    }

    /// The step's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// The step's title, as people left it.
    pub fn title(&self) -> &SharedString {
        &self.title
    }

    /// Where the step stands.
    pub fn current_status(&self) -> PlanStepStatus {
        self.status
    }
}

/// What the card is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanPhase {
    /// People read and edit the plan.
    Review,
    /// The plan is being carried out.
    Running,
    /// Every step is done.
    Finished,
}

/// What people did with the plan.
#[derive(Debug, Clone)]
pub enum PlanEditorEvent {
    /// A step was renamed, moved, removed or added. Carries the whole plan.
    Edited(Vec<PlanStep>),
    /// Run was pressed. Carries the steps in their final order: run these,
    /// not the first proposal. Call [`PlanEditor::start`] to follow the run.
    Run(Vec<PlanStep>),
    /// Cancel was pressed in review.
    Cancel,
    /// Try again was pressed under a failed step. The card has already put
    /// that step and the ones after it back to pending.
    Retry(SharedString),
}

/// The thing a step's grip carries while it is dragged.
#[derive(Clone)]
struct DraggedStep {
    id: SharedString,
    title: SharedString,
}

impl Render for DraggedStep {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .px_3()
            .py_1p5()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_sm()
            .text_color(theme.foreground)
            .shadow_md()
            .child(self.title.clone())
    }
}

/// A plan of steps to review, edit and then follow while it runs.
pub struct PlanEditor {
    title: SharedString,
    estimate: Option<SharedString>,
    steps: Vec<PlanStep>,
    phase: PlanPhase,
    cancellable: bool,
    focus: FocusHandle,
    selected: Option<usize>,
    renaming: Option<SharedString>,
    rename: Entity<InputState>,
    add: Entity<InputState>,
    added: usize,
    run_started: Option<Instant>,
    run_took: Option<Duration>,
    stopped_at: Option<Instant>,
    paused: Duration,
    step_started: HashMap<SharedString, Instant>,
    _ticker: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PlanEditorEvent> for PlanEditor {}

impl Focusable for PlanEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PlanEditor {
    /// Create an empty plan in review.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let rename = cx.new(|cx| InputState::new(window, cx));
        let add = cx.new(|cx| InputState::new(window, cx).placeholder("Add a step"));
        let subscriptions = vec![
            cx.subscribe_in(&rename, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.save_rename(window, cx),
                InputEvent::Blur => this.save_rename(window, cx),
                _ => {}
            }),
            cx.subscribe_in(&add, window, |this, input, event, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let text = input.read(cx).value().trim().to_string();
                    if !text.is_empty() {
                        this.add_step(text, window, cx);
                    }
                }
            }),
        ];

        Self {
            title: "Plan".into(),
            estimate: None,
            steps: Vec::new(),
            phase: PlanPhase::Review,
            cancellable: false,
            focus: cx.focus_handle(),
            selected: None,
            renaming: None,
            rename,
            add,
            added: 0,
            run_started: None,
            run_took: None,
            stopped_at: None,
            paused: Duration::ZERO,
            step_started: HashMap::new(),
            _ticker: None,
            _subscriptions: subscriptions,
        }
    }

    /// Set the goal of the plan, shown as its heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Show an estimate of the run time in review, such as "about 2 min".
    pub fn estimate(mut self, estimate: impl Into<SharedString>) -> Self {
        self.estimate = Some(estimate.into());
        self
    }

    /// Add a step to the end of the plan.
    pub fn step(mut self, step: PlanStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Show a Cancel button next to Run.
    pub fn cancellable(mut self, cancellable: bool) -> Self {
        self.cancellable = cancellable;
        self
    }

    /// The steps as they stand.
    pub fn plan(&self) -> &[PlanStep] {
        &self.steps
    }

    /// What the card is doing.
    pub fn phase(&self) -> PlanPhase {
        self.phase
    }

    /// Replace every step, such as when the agent proposes a new plan.
    pub fn set_steps(&mut self, steps: Vec<PlanStep>, cx: &mut Context<Self>) {
        self.steps = steps;
        self.selected = self.selected.filter(|ix| *ix < self.steps.len());
        cx.notify();
    }

    /// Start following a run. Steps go back to pending and the clock starts.
    pub fn start(&mut self, cx: &mut Context<Self>) {
        for step in &mut self.steps {
            step.status = PlanStepStatus::Pending;
            step.took = None;
            step.error = None;
        }
        self.phase = PlanPhase::Running;
        self.renaming = None;
        self.selected = None;
        self.run_started = Some(Instant::now());
        self.run_took = None;
        self.stopped_at = None;
        self.paused = Duration::ZERO;
        self.step_started.clear();
        self._ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let going = this.update(cx, |this, cx| {
                    cx.notify();
                    this.phase == PlanPhase::Running
                });
                if !matches!(going, Ok(true)) {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// Report where a step stands. A step that turns running starts its own
    /// clock; one that ends keeps how long it took, unless it already says.
    /// Once every step is done, the run is finished.
    pub fn mark(&mut self, id: &str, status: PlanStepStatus, cx: &mut Context<Self>) {
        let now = Instant::now();
        if let Some(step) = self.steps.iter_mut().find(|step| step.id.as_ref() == id) {
            match status {
                PlanStepStatus::Running => {
                    self.step_started.insert(step.id.clone(), now);
                    step.took = None;
                    step.error = None;
                }
                PlanStepStatus::Done | PlanStepStatus::Failed => {
                    if status == PlanStepStatus::Failed {
                        self.stopped_at = Some(now);
                    }
                    if step.took.is_none() {
                        step.took = self.step_started.get(&step.id).map(|at| now - *at);
                    }
                }
                PlanStepStatus::Pending => step.took = None,
            }
            step.status = status;
        }
        if self.phase == PlanPhase::Running
            && self
                .steps
                .iter()
                .all(|step| step.status == PlanStepStatus::Done)
        {
            self.phase = PlanPhase::Finished;
            self.run_took = Some(self.run_time(now));
        }
        cx.notify();
    }

    /// Report that a step failed, and why.
    pub fn fail(&mut self, id: &str, error: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.mark(id, PlanStepStatus::Failed, cx);
        if let Some(step) = self.steps.iter_mut().find(|step| step.id.as_ref() == id) {
            step.error = Some(error.into());
        }
    }

    /// Put a step and every step after it back to pending.
    pub fn retry_from(&mut self, id: &str, cx: &mut Context<Self>) {
        retry_reset(&mut self.steps, id);
        if let Some(at) = self.stopped_at.take() {
            self.paused += at.elapsed();
        }
        cx.notify();
    }

    /// Go back to review, keeping the steps.
    pub fn review(&mut self, cx: &mut Context<Self>) {
        self.phase = PlanPhase::Review;
        self._ticker = None;
        for step in &mut self.steps {
            step.status = PlanStepStatus::Pending;
            step.took = None;
            step.error = None;
        }
        cx.notify();
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        cx.emit(PlanEditorEvent::Edited(self.steps.clone()));
        cx.notify();
    }

    fn move_step(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if move_within(&mut self.steps, from, to) {
            self.selected = Some(to.min(self.steps.len() - 1));
            self.edited(cx);
        }
    }

    fn drop_on(&mut self, dragged: &DraggedStep, target: usize, cx: &mut Context<Self>) {
        if let Some(from) = self.steps.iter().position(|step| step.id == dragged.id) {
            self.move_step(from, target, cx);
        }
    }

    fn remove(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.steps.len() {
            self.steps.remove(ix);
            self.selected = match self.steps.len() {
                0 => None,
                len => Some(ix.min(len - 1)),
            };
            self.edited(cx);
        }
    }

    fn add_step(&mut self, title: String, window: &mut Window, cx: &mut Context<Self>) {
        self.added += 1;
        self.steps
            .push(PlanStep::new(format!("added-{}", self.added), title));
        self.add
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.edited(cx);
    }

    fn begin_rename(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(step) = self.steps.get(ix) else {
            return;
        };
        self.selected = Some(ix);
        self.renaming = Some(step.id.clone());
        let title = step.title.clone();
        self.rename.update(cx, |input, cx| {
            input.set_value(title, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn save_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.renaming.take() else {
            return;
        };
        let text = self.rename.read(cx).value().trim().to_string();
        if let Some(step) = self.steps.iter_mut().find(|step| step.id == id)
            && !text.is_empty()
            && step.title.as_ref() != text
        {
            step.title = text.into();
            self.edited(cx);
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.renaming = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.phase != PlanPhase::Review {
            return;
        }
        let key = event.keystroke.key.as_str();
        if self.renaming.is_some() {
            if key == "escape" {
                self.cancel_rename(window, cx);
                cx.stop_propagation();
            }
            return;
        }
        if !self.focus.is_focused(window) || self.steps.is_empty() {
            return;
        }
        let last = self.steps.len() - 1;
        let at = self.selected.unwrap_or(0).min(last);
        let alt = event.keystroke.modifiers.alt;
        match key {
            "up" if alt => self.move_step(at, at.saturating_sub(1), cx),
            "down" if alt => self.move_step(at, (at + 1).min(last), cx),
            "up" => self.selected = Some(at.saturating_sub(1)),
            "down" => self.selected = Some((at + 1).min(last)),
            "enter" => self.begin_rename(at, window, cx),
            "delete" | "backspace" => self.remove(at, cx),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// How long the run has taken up to `until`, leaving out the time it
    /// waited on a failed step.
    fn run_time(&self, until: Instant) -> Duration {
        self.run_started
            .map(|at| {
                until
                    .saturating_duration_since(at)
                    .saturating_sub(self.paused)
            })
            .unwrap_or_default()
    }

    fn step_time(&self, step: &PlanStep) -> Option<Duration> {
        match step.status {
            PlanStepStatus::Running => self.step_started.get(&step.id).map(|at| at.elapsed()),
            _ => step.took,
        }
    }
}

/// Move the step at `from` to `to`. Says whether anything moved.
fn move_within<T>(items: &mut Vec<T>, from: usize, to: usize) -> bool {
    if from >= items.len() || from == to {
        return false;
    }
    let item = items.remove(from);
    items.insert(to.min(items.len()), item);
    true
}

/// Put the step `id` and every step after it back to pending.
fn retry_reset(steps: &mut [PlanStep], id: &str) {
    if let Some(from) = steps.iter().position(|step| step.id.as_ref() == id) {
        for step in &mut steps[from..] {
            step.status = PlanStepStatus::Pending;
            step.took = None;
            step.error = None;
        }
    }
}

/// A duration as the card shows it: "0.8s", "14s", "2m 05s".
fn clock(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 10. {
        format!("{secs:.1}s")
    } else if secs < 60. {
        format!("{}s", secs.floor() as u64)
    } else {
        let whole = secs.floor() as u64;
        format!("{}m {:02}s", whole / 60, whole % 60)
    }
}

/// The plan's count in words: "1 step", "5 steps".
fn steps_words(count: usize) -> String {
    match count {
        1 => "1 step".into(),
        n => format!("{n} steps"),
    }
}

/// Six small dots that show a row can be dragged.
fn grip(color: isonomic_ui::Hsla) -> impl IntoElement {
    let column = || {
        v_flex()
            .gap(px(2.))
            .children((0..3).map(move |_| div().size(px(2.5)).rounded_full().bg(color)))
    };
    h_flex().gap(px(2.)).child(column()).child(column())
}

impl Render for PlanEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let accent_ink = theme.primary_foreground;
        let success = theme.success;
        let danger = theme.danger;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();

        let review = self.phase == PlanPhase::Review;
        let on_rows = self.focus.is_focused(window);
        let done = self
            .steps
            .iter()
            .filter(|step| step.status == PlanStepStatus::Done)
            .count();
        let total = self.steps.len();
        let current = self
            .steps
            .iter()
            .position(|step| step.status == PlanStepStatus::Running);
        let failed = self
            .steps
            .iter()
            .position(|step| step.status == PlanStepStatus::Failed);
        let renaming = self.renaming.clone();

        let rows: Vec<_> = self
            .steps
            .iter()
            .enumerate()
            .map(|(ix, step)| {
                let is_selected = review && on_rows && self.selected == Some(ix);
                let is_renaming = renaming.as_ref() == Some(&step.id);
                let is_last = ix + 1 == total;
                let marker = match step.status {
                    PlanStepStatus::Pending => div()
                        .size_5()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(line)
                        .text_xs()
                        .text_color(quiet)
                        .child((ix + 1).to_string())
                        .into_any_element(),
                    PlanStepStatus::Running => div()
                        .size_5()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(IconName::LoaderCircle).small().text_color(accent))
                        .into_any_element(),
                    PlanStepStatus::Done => div()
                        .size_5()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(accent)
                        .child(Icon::new(IconName::Check).xsmall().text_color(accent_ink))
                        .into_any_element(),
                    PlanStepStatus::Failed => div()
                        .size_5()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(danger)
                        .child(Icon::new(IconName::Close).xsmall().text_color(accent_ink))
                        .into_any_element(),
                };
                // The thread between markers fills in as steps finish.
                let thread = div().flex_1().w(px(1.5)).bg(
                    if !review && step.status == PlanStepStatus::Done {
                        accent
                    } else if review {
                        isonomic_ui::transparent_black()
                    } else {
                        line
                    },
                );
                let title_ink = match step.status {
                    PlanStepStatus::Pending if !review => quiet,
                    PlanStepStatus::Failed => danger,
                    _ => ink,
                };
                let time = self.step_time(step).map(clock);
                let retry = (step.status == PlanStepStatus::Failed).then(|| {
                    let id = step.id.clone();
                    Button::new(("retry", ix))
                        .outline()
                        .xsmall()
                        .icon(IconName::RotateCw)
                        .label("Try again from here")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.retry_from(&id, cx);
                            cx.emit(PlanEditorEvent::Retry(id.clone()));
                        }))
                });

                let body = v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .pb_3()
                    .map(|this| {
                        if is_renaming {
                            this.child(Input::new(&self.rename).small())
                        } else {
                            this.child(
                                div()
                                    .font_medium()
                                    .text_color(title_ink)
                                    .child(step.title.clone()),
                            )
                        }
                    })
                    .when(step.detail.is_some() || step.tool.is_some(), |this| {
                        this.child(
                            h_flex()
                                .flex_wrap()
                                .gap_x_1p5()
                                .gap_y_0p5()
                                .text_xs()
                                .text_color(quiet)
                                .children(step.detail.clone())
                                .children(step.tool.clone().map(|tool| {
                                    div()
                                        .px_1()
                                        .rounded(px(4.))
                                        .bg(chip)
                                        .font_family(mono.clone())
                                        .whitespace_nowrap()
                                        .child(tool)
                                })),
                        )
                    })
                    .children(
                        step.error
                            .clone()
                            .map(|error| div().text_xs().text_color(danger).child(error)),
                    )
                    .children(retry.map(|retry| h_flex().pt_1().child(retry)));

                h_flex()
                    .id(("plan-step", ix))
                    .items_stretch()
                    .gap_2()
                    .px_1()
                    .pt_1p5()
                    .rounded(radius)
                    .when(is_selected, |this| this.bg(chip))
                    .when(review, |this| {
                        let dragged = DraggedStep {
                            id: step.id.clone(),
                            title: step.title.clone(),
                        };
                        this.hover(|style| style.bg(chip))
                            .on_click(cx.listener(
                                move |this, event: &isonomic_ui::ClickEvent, window, cx| {
                                    this.selected = Some(ix);
                                    this.focus.focus(window, cx);
                                    if event.click_count() > 1 {
                                        this.begin_rename(ix, window, cx);
                                    }
                                    cx.notify();
                                },
                            ))
                            .drag_over::<DraggedStep>(move |style, _, _, _| {
                                style.border_t_2().border_color(accent)
                            })
                            .on_drop(cx.listener(move |this, dragged: &DraggedStep, _, cx| {
                                this.drop_on(dragged, ix, cx)
                            }))
                            .child(
                                div()
                                    .id(("grip", ix))
                                    .flex_none()
                                    .pt_1p5()
                                    .px_0p5()
                                    .cursor_grab()
                                    .on_drag(dragged, |dragged, _, _, cx| {
                                        cx.new(|_| dragged.clone())
                                    })
                                    .child(grip(quiet.opacity(if is_selected {
                                        0.9
                                    } else {
                                        0.4
                                    }))),
                            )
                    })
                    .child(
                        v_flex()
                            .flex_none()
                            .items_center()
                            .gap_1()
                            .child(marker)
                            .when(!is_last, |this| this.child(thread)),
                    )
                    .child(body)
                    .map(|this| {
                        if review {
                            this.child(
                                Button::new(("remove", ix))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .accessibility_label(format!("Remove {}", step.title))
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| this.remove(ix, cx)),
                                    ),
                            )
                        } else {
                            this.child(
                                div()
                                    .flex_none()
                                    .pt_0p5()
                                    .text_xs()
                                    .text_color(if step.status == PlanStepStatus::Running {
                                        ink
                                    } else {
                                        quiet
                                    })
                                    .children(time),
                            )
                        }
                    })
            })
            .collect();

        let add_row = review.then(|| {
            h_flex()
                .gap_2()
                .px_1()
                .pl(px(30.))
                .child(Icon::new(IconName::Plus).small().text_color(quiet))
                .child(
                    div()
                        .flex_1()
                        .child(Input::new(&self.add).small().appearance(false)),
                )
        });

        let footer = match self.phase {
            PlanPhase::Review => h_flex()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(quiet)
                        .child("Drag to move, Enter to rename"),
                )
                .when(self.cancellable, |this| {
                    this.child(
                        Button::new("plan-cancel")
                            .ghost()
                            .small()
                            .label("Cancel")
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(PlanEditorEvent::Cancel))),
                    )
                })
                .child(
                    Button::new("plan-run")
                        .primary()
                        .small()
                        .icon(IconName::Play)
                        .label("Run plan")
                        .disabled(self.steps.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.renaming = None;
                            cx.emit(PlanEditorEvent::Run(this.steps.clone()))
                        })),
                ),
            PlanPhase::Running => {
                // The clock stops while the run waits on a failed step.
                let elapsed = self.run_time(self.stopped_at.unwrap_or_else(Instant::now));
                let (icon, words, color) = match failed {
                    Some(ix) => (
                        IconName::CircleAlert,
                        format!("Stopped at step {} of {total}", ix + 1),
                        danger,
                    ),
                    None => (
                        IconName::LoaderCircle,
                        format!(
                            "Running, step {} of {total}",
                            current.map(|ix| ix + 1).unwrap_or(done + 1).min(total)
                        ),
                        quiet,
                    ),
                };
                h_flex()
                    .gap_2()
                    .text_xs()
                    .child(Icon::new(icon).small().text_color(color))
                    .child(div().flex_1().text_color(color).child(words))
                    .child(div().text_color(quiet).child(clock(elapsed)))
            }
            PlanPhase::Finished => h_flex()
                .gap_2()
                .text_xs()
                .child(Icon::new(IconName::CircleCheck).small().text_color(success))
                .child(div().flex_1().text_color(quiet).child(match self.run_took {
                    Some(took) => format!("Done in {}", clock(took)),
                    None => "Done".to_string(),
                }))
                .child(
                    Button::new("plan-again")
                        .outline()
                        .small()
                        .icon(IconName::RotateCw)
                        .label("Run again")
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.emit(PlanEditorEvent::Run(this.steps.clone()))
                        })),
                ),
        };

        v_flex()
            .id("plan-editor")
            .key_context("PlanEditor")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .max_w(px(460.))
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .line_height(relative(1.4))
            .shadow(resting_shadow(cx))
            .child(
                v_flex()
                    .gap_1()
                    .px_4()
                    .pt_4()
                    .pb_2()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .text_xs()
                            .text_color(quiet)
                            .child(Icon::new(IconName::Menu).xsmall())
                            .child(format!("Plan, {}", steps_words(total)))
                            .child(div().flex_1())
                            .when(review, |this| this.children(self.estimate.clone())),
                    )
                    .child(div().text_base().font_semibold().child(self.title.clone())),
            )
            .child(
                v_flex()
                    .px_3()
                    .children(rows)
                    .children(add_row)
                    .when(total == 0 && !review, |this| {
                        this.child(div().p_2().text_color(quiet).child("No steps"))
                    }),
            )
            .child(
                div()
                    .mt_2()
                    .px_4()
                    .py_2p5()
                    .border_t_1()
                    .border_color(line)
                    .child(footer),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps() -> Vec<PlanStep> {
        ["a", "b", "c", "d"]
            .into_iter()
            .map(|id| PlanStep::new(id, id))
            .collect()
    }

    fn ids(steps: &[PlanStep]) -> String {
        steps.iter().map(|step| step.id.as_ref()).collect()
    }

    #[test]
    fn steps_move_both_ways() {
        let mut plan = steps();
        assert!(move_within(&mut plan, 0, 2));
        assert_eq!(ids(&plan), "bcad");
        assert!(move_within(&mut plan, 3, 0));
        assert_eq!(ids(&plan), "dbca");
        assert!(!move_within(&mut plan, 1, 1));
        assert!(!move_within(&mut plan, 9, 0));
    }

    #[test]
    fn retrying_resets_the_step_and_the_rest() {
        let mut plan = steps();
        for step in &mut plan[..2] {
            step.status = PlanStepStatus::Done;
        }
        plan[2] = plan[2].clone().error("Quota reached");
        retry_reset(&mut plan, "c");
        let statuses: Vec<_> = plan.iter().map(|step| step.status).collect();
        assert_eq!(
            statuses,
            [
                PlanStepStatus::Done,
                PlanStepStatus::Done,
                PlanStepStatus::Pending,
                PlanStepStatus::Pending
            ]
        );
        assert!(plan[2].error.is_none());
    }

    #[test]
    fn times_read_short() {
        assert_eq!(clock(Duration::from_millis(800)), "0.8s");
        assert_eq!(clock(Duration::from_secs(14)), "14s");
        assert_eq!(clock(Duration::from_secs(125)), "2m 05s");
        assert_eq!(steps_words(1), "1 step");
    }
}
