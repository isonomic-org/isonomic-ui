// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Run steering panel: correct an agent while it works.
//!
//! The panel shows the run's steps and a field for notes. Enter queues a note
//! for the next step; when that step starts, the note moves under it. Ctrl or
//! Cmd with Enter applies a note to the current step right away. Every note
//! stays with the step it changed, so the run is easy to follow later.

use std::time::{Duration, Instant};

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How often the clocks redraw while the run goes.
const TICK: Duration = Duration::from_millis(200);
/// How long a note that was just taken in stays lit.
const LIT_FOR: Duration = Duration::from_millis(2400);

/// Where one step stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteerStepStatus {
    /// Not started.
    Pending,
    /// Going now.
    Active,
    /// Finished.
    Done,
}

/// When a note reached the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteTiming {
    /// Still waiting for the next step.
    Queued,
    /// Taken in when its step started.
    AtStepStart,
    /// Applied to the running step at once.
    Immediately,
}

/// A note for the agent.
#[derive(Debug, Clone)]
pub struct SteerNote {
    id: SharedString,
    text: SharedString,
    timing: NoteTiming,
    taken_at: Option<Instant>,
}

impl SteerNote {
    /// Create a queued note.
    pub fn new(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            timing: NoteTiming::Queued,
            taken_at: None,
        }
    }

    /// The note's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// What the note says.
    pub fn text(&self) -> &SharedString {
        &self.text
    }

    /// When the note reached the agent.
    pub fn timing(&self) -> NoteTiming {
        self.timing
    }
}

/// One step of the run.
#[derive(Debug, Clone)]
pub struct SteerStep {
    id: SharedString,
    title: SharedString,
    status: SteerStepStatus,
    started: Option<Instant>,
    took: Option<Duration>,
    notes: Vec<SteerNote>,
}

impl SteerStep {
    /// Create a pending step.
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: SteerStepStatus::Pending,
            started: None,
            took: None,
            notes: Vec::new(),
        }
    }

    /// Mark the step finished, with how long it took.
    pub fn done(mut self, took: Duration) -> Self {
        self.status = SteerStepStatus::Done;
        self.took = Some(took);
        self
    }

    /// Mark the step as the one going now, started `ago`.
    pub fn active_for(mut self, ago: Duration) -> Self {
        self.status = SteerStepStatus::Active;
        self.started = Instant::now().checked_sub(ago);
        self
    }

    /// The step's id.
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// The notes that changed this step.
    pub fn notes(&self) -> &[SteerNote] {
        &self.notes
    }
}

/// How the run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Going; the clocks tick.
    Running,
    /// Held by a person.
    Paused,
    /// Over; the field closes.
    Done,
}

/// What people asked of the run.
#[derive(Debug, Clone)]
pub enum RunSteeringEvent {
    /// A note was queued for the next step.
    Queued(SteerNote),
    /// A note should reach the agent now, on the current step. It is already
    /// shown under that step. Queued notes sent with Now arrive here too.
    Interrupt(SteerNote),
    /// A queued note was taken back.
    Removed(SharedString),
    /// Notes were taken in because a step started.
    PickedUp {
        /// The step that started.
        step: SharedString,
        /// The notes it took in.
        notes: Vec<SteerNote>,
    },
    /// Pause was pressed.
    Pause,
    /// Resume was pressed.
    Resume,
}

/// A running agent's steps, with a field to steer it.
pub struct RunSteeringPanel {
    title: SharedString,
    agent: Option<SharedString>,
    steps: Vec<SteerStep>,
    queue: Vec<SteerNote>,
    state: RunState,
    started: Instant,
    paused_at: Option<Instant>,
    paused: Duration,
    written: usize,
    pausable: bool,
    input: Entity<InputState>,
    focus: FocusHandle,
    _ticker: Task<()>,
    _input_events: Subscription,
}

impl EventEmitter<RunSteeringEvent> for RunSteeringPanel {}

impl Focusable for RunSteeringPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl RunSteeringPanel {
    /// Create a running panel with no steps, its clock starting now.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Tell the agent what to change"));
        let input_events = cx.subscribe_in(&input, window, |this, input, event, window, cx| {
            if let InputEvent::PressEnter { secondary, shift } = event
                && !shift
            {
                let text = input.read(cx).value().trim().to_string();
                if *secondary {
                    this.interrupt_with(text, window, cx);
                } else {
                    this.queue_text(text, window, cx);
                }
            }
        });
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });

        Self {
            title: "Run".into(),
            agent: None,
            steps: Vec::new(),
            queue: Vec::new(),
            state: RunState::Running,
            started: Instant::now(),
            paused_at: None,
            paused: Duration::ZERO,
            written: 0,
            pausable: true,
            input,
            focus: cx.focus_handle(),
            _ticker: ticker,
            _input_events: input_events,
        }
    }

    /// Set what the agent is working on.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Name the agent or model, shown under the title.
    pub fn agent(mut self, agent: impl Into<SharedString>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    /// Add a step to the plan.
    pub fn step(mut self, step: SteerStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Start with a note already waiting for the next step.
    pub fn queued(mut self, note: SteerNote) -> Self {
        self.queue.push(note);
        self
    }

    /// Say when the run began, so the header clock is right.
    pub fn started_ago(mut self, ago: Duration) -> Self {
        self.started = Instant::now().checked_sub(ago).unwrap_or(self.started);
        self
    }

    /// Hide the Pause and Resume button.
    pub fn pausable(mut self, pausable: bool) -> Self {
        self.pausable = pausable;
        self
    }

    /// The steps, with the notes each one took in.
    pub fn steps(&self) -> &[SteerStep] {
        &self.steps
    }

    /// The notes still waiting for the next step.
    pub fn queue(&self) -> &[SteerNote] {
        &self.queue
    }

    /// Start a step. The step that was going is marked done, and every queued
    /// note moves under the new step.
    pub fn start_step(&mut self, id: &str, cx: &mut Context<Self>) {
        let now = Instant::now();
        for step in &mut self.steps {
            if step.status == SteerStepStatus::Active {
                step.status = SteerStepStatus::Done;
                step.took = step.started.map(|at| now - at);
            }
        }
        let Some(step) = self.steps.iter_mut().find(|step| step.id.as_ref() == id) else {
            return;
        };
        step.status = SteerStepStatus::Active;
        step.started = Some(now);
        step.took = None;
        let taken: Vec<SteerNote> = self
            .queue
            .drain(..)
            .map(|mut note| {
                note.timing = NoteTiming::AtStepStart;
                note.taken_at = Some(now);
                note
            })
            .collect();
        step.notes.extend(taken.iter().cloned());
        if !taken.is_empty() {
            cx.emit(RunSteeringEvent::PickedUp {
                step: step.id.clone(),
                notes: taken,
            });
        }
        cx.notify();
    }

    /// Finish the run. The step that was going is marked done.
    pub fn finish(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        for step in &mut self.steps {
            if step.status == SteerStepStatus::Active {
                step.status = SteerStepStatus::Done;
                step.took = step.started.map(|at| now - at);
            }
        }
        self.set_state(RunState::Done, cx);
    }

    /// Set how the run stands. The clock holds while paused.
    pub fn set_state(&mut self, state: RunState, cx: &mut Context<Self>) {
        let now = Instant::now();
        match (self.state, state) {
            (RunState::Running, RunState::Paused | RunState::Done) => self.paused_at = Some(now),
            (RunState::Paused, RunState::Running) => {
                if let Some(at) = self.paused_at.take() {
                    self.paused += now - at;
                }
            }
            _ => {}
        }
        self.state = state;
        cx.notify();
    }

    fn next_id(&mut self) -> SharedString {
        self.written += 1;
        format!("note-{}", self.written).into()
    }

    fn clear_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
    }

    fn queue_text(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if text.is_empty() || self.state == RunState::Done {
            return;
        }
        let note = SteerNote::new(self.next_id(), text);
        self.queue.push(note.clone());
        self.clear_field(window, cx);
        cx.emit(RunSteeringEvent::Queued(note));
        cx.notify();
    }

    fn interrupt_with(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if text.is_empty() || self.state == RunState::Done {
            return;
        }
        let note = SteerNote::new(self.next_id(), text);
        self.clear_field(window, cx);
        self.apply_now(note, cx);
    }

    fn apply_now(&mut self, mut note: SteerNote, cx: &mut Context<Self>) {
        note.timing = NoteTiming::Immediately;
        note.taken_at = Some(Instant::now());
        if let Some(step) = self
            .steps
            .iter_mut()
            .find(|step| step.status == SteerStepStatus::Active)
        {
            step.notes.push(note.clone());
        }
        cx.emit(RunSteeringEvent::Interrupt(note));
        cx.notify();
    }

    fn send_queued_now(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if let Some(ix) = self.queue.iter().position(|note| &note.id == id) {
            let note = self.queue.remove(ix);
            self.apply_now(note, cx);
        }
    }

    fn remove_queued(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        self.queue.retain(|note| &note.id != id);
        cx.emit(RunSteeringEvent::Removed(id.clone()));
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape"
            && self.input.read(cx).focus_handle(cx).is_focused(window)
        {
            self.clear_field(window, cx);
            cx.stop_propagation();
        }
    }

    fn run_time(&self) -> Duration {
        let until = self.paused_at.unwrap_or_else(Instant::now);
        until
            .saturating_duration_since(self.started)
            .saturating_sub(self.paused)
    }
}

/// A duration for the clocks: "0.4s", "12s", "2m 05s".
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

/// "Step 2 of 5": the running step, else the last finished one.
fn position_words(steps: &[SteerStep]) -> Option<String> {
    let total = steps.len();
    if total == 0 {
        return None;
    }
    let at = steps
        .iter()
        .position(|step| step.status == SteerStepStatus::Active)
        .or_else(|| {
            steps
                .iter()
                .rposition(|step| step.status == SteerStepStatus::Done)
        })
        .map(|ix| ix + 1)
        .unwrap_or(1);
    Some(format!("Step {at} of {total}"))
}

/// A short tag saying how a note reached the agent.
fn timing_words(timing: NoteTiming) -> &'static str {
    match timing {
        NoteTiming::Queued => "Waiting",
        NoteTiming::AtStepStart => "At step start",
        NoteTiming::Immediately => "Right away",
    }
}

/// The key hint chips under the field.
fn key_chip(label: &'static str, line: Hsla) -> impl IntoElement {
    div()
        .px_1()
        .rounded(px(4.))
        .border_1()
        .border_color(line)
        .child(label)
}

impl Render for RunSteeringPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.blue;
        let success = theme.success;
        let warning = theme.warning;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let now = Instant::now();
        let held = self.state != RunState::Running;

        let (dot, state_words) = match self.state {
            RunState::Running => (accent, "Running"),
            RunState::Paused => (warning, "Paused"),
            RunState::Done => (success, "Done"),
        };
        let pause = (self.pausable && self.state != RunState::Done).then(|| {
            let paused = self.state == RunState::Paused;
            Button::new("steer-pause")
                .outline()
                .small()
                .icon(if paused {
                    IconName::Play
                } else {
                    IconName::Pause
                })
                .accessibility_label(if paused { "Resume" } else { "Pause" })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if paused {
                        this.set_state(RunState::Running, cx);
                        cx.emit(RunSteeringEvent::Resume);
                    } else {
                        this.set_state(RunState::Paused, cx);
                        cx.emit(RunSteeringEvent::Pause);
                    }
                }))
        });

        let run_words = format!(
            "{}{}, {state_words}, {}",
            self.title,
            position_words(&self.steps)
                .map(|p| format!(", {p}"))
                .unwrap_or_default(),
            clock(self.run_time())
        );
        let header = h_flex()
            .id("steer-header")
            .role(Role::Status)
            .aria_label(run_words.clone())
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(line)
            .child(Icon::new(IconName::Bot).small().text_color(accent))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().font_medium().child(self.title.clone()))
                    .child(
                        div().text_caption().text_color(quiet).truncate().child(
                            [
                                self.agent.clone().map(String::from),
                                position_words(&self.steps),
                            ]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join(", "),
                        ),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1p5()
                    .text_caption()
                    .child(div().size_2().rounded_full().bg(dot))
                    .child(format!("{state_words}, {}", clock(self.run_time()))),
            )
            .children(pause);

        let steps = self.steps.iter().enumerate().map(|(ix, step)| {
            let last = ix + 1 == self.steps.len();
            let marker = match step.status {
                SteerStepStatus::Done => div()
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(accent)
                    .child(Icon::new(IconName::Check).xsmall().text_color(fill)),
                SteerStepStatus::Active => div()
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::LoaderCircle).small().text_color(accent)),
                SteerStepStatus::Pending => div()
                    .size(px(16.))
                    .rounded_full()
                    .border_1()
                    .border_color(line),
            };
            let time = match step.status {
                SteerStepStatus::Active => step.started.map(|at| {
                    let until = if held {
                        self.paused_at.unwrap_or(now)
                    } else {
                        now
                    };
                    until.saturating_duration_since(at)
                }),
                _ => step.took,
            };
            let notes = step.notes.iter().map(|note| {
                let lit = note
                    .taken_at
                    .is_some_and(|at| now.saturating_duration_since(at) < LIT_FOR);
                h_flex()
                    .gap_2()
                    .mt_1()
                    .pl_2()
                    .py_1()
                    .pr_2()
                    .border_l_2()
                    .border_color(accent)
                    .rounded_r(radius)
                    .bg(if lit {
                        accent.opacity(0.16)
                    } else {
                        accent.opacity(0.06)
                    })
                    .text_caption()
                    .child(div().flex_1().min_w_0().child(note.text.clone()))
                    .child(
                        div()
                            .flex_none()
                            .text_color(quiet)
                            .child(timing_words(note.timing)),
                    )
            });
            let step_words = {
                let mut words = format!(
                    "{}, {}",
                    step.title,
                    match step.status {
                        SteerStepStatus::Done => "done",
                        SteerStepStatus::Active => "in progress",
                        SteerStepStatus::Pending => "to do",
                    }
                );
                if let Some(time) = time {
                    words.push_str(&format!(", {}", clock(time)));
                }
                for note in &step.notes {
                    words.push_str(&format!(
                        ". Note: {}, {}",
                        note.text,
                        timing_words(note.timing)
                    ));
                }
                words
            };
            h_flex()
                .id(("steer-step", ix))
                .role(Role::ListItem)
                .aria_label(step_words)
                .items_stretch()
                .gap_2p5()
                .child(
                    v_flex()
                        .flex_none()
                        .items_center()
                        .pt(px(2.))
                        .child(marker)
                        .when(!last, |this| {
                            this.child(div().flex_1().w(px(1.)).mt_0p5().bg(
                                if step.status == SteerStepStatus::Done {
                                    accent
                                } else {
                                    line
                                },
                            ))
                        }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .pb_2p5()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .when(step.status == SteerStepStatus::Pending, |this| {
                                            this.text_color(quiet)
                                        })
                                        .when(step.status == SteerStepStatus::Active, |this| {
                                            this.font_medium()
                                        })
                                        .child(step.title.clone()),
                                )
                                .children(time.map(|time| {
                                    div()
                                        .flex_none()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(clock(time))
                                })),
                        )
                        .children(notes),
                )
        });

        let queue = (!self.queue.is_empty()).then(|| {
            v_flex()
                .gap_1()
                .px_4()
                .py_2p5()
                .border_t_1()
                .border_color(line)
                .child(
                    h_flex()
                        .gap_1p5()
                        .text_caption()
                        .text_color(quiet)
                        .child("Waiting for the next step")
                        .child(
                            div()
                                .px_1()
                                .rounded(px(4.))
                                .bg(chip)
                                .child(self.queue.len().to_string()),
                        ),
                )
                .children(self.queue.iter().enumerate().map(|(ix, note)| {
                    let now_id = note.id.clone();
                    let remove_id = note.id.clone();
                    h_flex()
                        .id(("steer-queued", ix))
                        .role(Role::ListItem)
                        .aria_label(format!("Waiting for the next step: {}", note.text))
                        .gap_1()
                        .pl_2()
                        .border_l_2()
                        .border_color(line)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_caption()
                                .child(note.text.clone()),
                        )
                        .child(
                            Button::new(("steer-now", ix))
                                .ghost()
                                .xsmall()
                                .label("Now")
                                .disabled(held)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.send_queued_now(&now_id, cx)
                                })),
                        )
                        .child(
                            Button::new(("steer-remove", ix))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .accessibility_label(format!("Remove {}", note.text))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_queued(&remove_id, cx)
                                })),
                        )
                }))
        });

        let composer = (self.state != RunState::Done).then(|| {
            let empty = self.input.read(cx).value().trim().is_empty();
            v_flex()
                .gap_1p5()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(line)
                .child(
                    h_flex()
                        .items_end()
                        .gap_2()
                        .child(
                            div().flex_1().child(
                                Input::new(&self.input)
                                    .small()
                                    .aria_label("Note for the agent"),
                            ),
                        )
                        .child(
                            Button::new("steer-send")
                                .primary()
                                .small()
                                .icon(IconName::ArrowUp)
                                .accessibility_label("Queue for the next step")
                                .disabled(empty)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let text = this.input.read(cx).value().trim().to_string();
                                    this.queue_text(text, window, cx);
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .text_caption()
                        .text_color(quiet)
                        .child(key_chip("Enter", line))
                        .child("next step")
                        .child(div().w_2())
                        .child(key_chip("Ctrl+Enter", line))
                        .child("right away"),
                )
        });

        v_flex()
            .id("run-steering")
            .role(Role::Group)
            .aria_label(run_words)
            .key_context("RunSteeringPanel")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .w_full()
            .max_w(px(460.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.4))
            .shadow(resting_shadow(cx))
            .child(header)
            .child(
                v_flex()
                    .id("steer-steps")
                    .role(Role::List)
                    .aria_label("Steps")
                    .px_4()
                    .pt_3()
                    .pb_1()
                    .children(steps),
            )
            .children(queue)
            .children(composer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_follows_the_running_step() {
        let steps = [
            SteerStep::new("a", "A").done(Duration::from_secs(1)),
            SteerStep::new("b", "B").active_for(Duration::ZERO),
            SteerStep::new("c", "C"),
        ];
        assert_eq!(position_words(&steps).as_deref(), Some("Step 2 of 3"));
    }

    #[test]
    fn position_falls_back_to_the_last_done_step() {
        let steps = [
            SteerStep::new("a", "A").done(Duration::from_secs(1)),
            SteerStep::new("b", "B").done(Duration::from_secs(1)),
        ];
        assert_eq!(position_words(&steps).as_deref(), Some("Step 2 of 2"));
        assert_eq!(position_words(&[]), None);
    }

    #[test]
    fn clocks_read_short() {
        assert_eq!(clock(Duration::from_millis(400)), "0.4s");
        assert_eq!(clock(Duration::from_secs(125)), "2m 05s");
        assert_eq!(timing_words(NoteTiming::Immediately), "Right away");
    }
}
