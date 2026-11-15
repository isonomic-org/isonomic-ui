//! Run control composer: the message field for a running assistant, with stop, pause, resume, retry, steer and a queue.
//!
//! While a run is going, a message can either wait for the next step
//! (Enter queues it) or cut in now ("Steer now"). Queued messages sit above
//! the field as chips that can be taken back. A status line says what the
//! run is doing, and its buttons change with it: pause and stop while it
//! runs, resume while paused, retry after a failure. The composer holds
//! only the text being written; everything else comes from its owner.

use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputEvent, Textarea, TextareaState},
    v_flex,
};
use isonomic_ui::{
    Animation, AnimationExt as _, AnyElement, App, AppContext as _, Context, Entity, EventEmitter,
    FocusHandle, Focusable, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Transformation, Window, div, percentage, prelude::FluentBuilder as _, px,
};

use crate::common::{Tone, resting_shadow};

/// Where the run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunPhase {
    /// Nothing running.
    Idle,
    /// Working.
    Running,
    /// Held.
    Paused,
    /// Stopped with a problem.
    Failed,
    /// Finished.
    Done,
}

/// What the composer reports.
#[derive(Debug, Clone, PartialEq)]
pub enum RunControlEvent {
    /// Send this now, interrupting the current step.
    Steer(SharedString),
    /// Send this after the current step.
    Queue(SharedString),
    /// Take a queued message back, by its id.
    Unqueue(SharedString),
    /// Stop the run.
    Stop,
    /// Hold the run.
    Pause,
    /// Carry on.
    Resume,
    /// Run the failed step again.
    Retry,
}

/// Which buttons a phase offers, in order.
pub fn controls_for(phase: RunPhase) -> &'static [RunControlEvent] {
    match phase {
        RunPhase::Running => &[RunControlEvent::Pause, RunControlEvent::Stop],
        RunPhase::Paused => &[RunControlEvent::Resume, RunControlEvent::Stop],
        RunPhase::Failed => &[RunControlEvent::Retry],
        RunPhase::Idle | RunPhase::Done => &[],
    }
}

/// A composer for steering a run.
pub struct RunControlComposer {
    field: Entity<TextareaState>,
    phase: RunPhase,
    status: SharedString,
    queued: Vec<(SharedString, SharedString)>,
    _field_events: Subscription,
}

impl EventEmitter<RunControlEvent> for RunControlComposer {}

impl Focusable for RunControlComposer {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.focus_handle(cx)
    }
}

impl RunControlComposer {
    /// Create a composer.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| {
            let mut field = TextareaState::new(window, cx)
                .auto_grow(1, 6)
                .placeholder("Add a note for the run");
            field.set_submit_on_enter(true, cx);
            field
        });
        let field_events =
            cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { shift: false, .. } => this.send(false, window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            });
        Self {
            field,
            phase: RunPhase::Idle,
            status: SharedString::default(),
            queued: Vec::new(),
            _field_events: field_events,
        }
    }

    /// Set where the run is and a line about what it's doing.
    pub fn set_phase(
        &mut self,
        phase: RunPhase,
        status: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.phase = phase;
        self.status = status.into();
        cx.notify();
    }

    /// Show these queued messages, each as id and text.
    pub fn set_queued(
        &mut self,
        queued: Vec<(SharedString, SharedString)>,
        cx: &mut Context<Self>,
    ) {
        self.queued = queued;
        cx.notify();
    }

    fn send(&mut self, now: bool, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.field.read(cx).value();
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let live = matches!(self.phase, RunPhase::Running | RunPhase::Paused);
        cx.emit(if now || !live {
            RunControlEvent::Steer(text.into())
        } else {
            RunControlEvent::Queue(text.into())
        });
        self.field
            .update(cx, |field, cx| field.set_value("", window, cx));
        cx.notify();
    }
}

impl Render for RunControlComposer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let radius_lg = theme.radius_lg;
        let phase = self.phase;
        let live = matches!(phase, RunPhase::Running | RunPhase::Paused);
        let has_text = !self.field.read(cx).value().trim().is_empty();
        let (tone, word) = match phase {
            RunPhase::Idle => (Tone::Neutral, "Ready"),
            RunPhase::Running => (Tone::Info, "Running"),
            RunPhase::Paused => (Tone::Warning, "Paused"),
            RunPhase::Failed => (Tone::Danger, "Failed"),
            RunPhase::Done => (Tone::Success, "Done"),
        };
        let color = tone.color(cx);

        let mark: AnyElement = if phase == RunPhase::Running {
            Icon::new(IconName::LoaderCircle)
                .xsmall()
                .text_color(color)
                .with_animation(
                    "run-spin",
                    Animation::new(std::time::Duration::from_secs(1)).repeat(),
                    |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
                )
                .into_any_element()
        } else {
            div()
                .size(px(7.))
                .rounded_full()
                .bg(color)
                .into_any_element()
        };

        let controls: Vec<_> = controls_for(phase)
            .iter()
            .map(|event| {
                let (icon, label) = match event {
                    RunControlEvent::Pause => (IconName::Pause, "Pause"),
                    RunControlEvent::Resume => (IconName::Play, "Resume"),
                    RunControlEvent::Stop => (IconName::Square, "Stop"),
                    RunControlEvent::Retry => (IconName::RefreshCw, "Retry"),
                    _ => (IconName::Info, ""),
                };
                let event = event.clone();
                Button::new(SharedString::from(format!("run-{label}")))
                    .xsmall()
                    .ghost()
                    .when(label == "Stop", |b| b.danger())
                    .icon(icon)
                    .label(label)
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(event.clone())))
            })
            .collect();

        let chips: Vec<_> = self
            .queued
            .iter()
            .enumerate()
            .map(|(ix, (id, text))| {
                let id = id.clone();
                h_flex()
                    .gap_1()
                    .max_w_full()
                    .pl_2()
                    .rounded(px(6.))
                    .bg(chip)
                    .text_xs()
                    .child(div().text_color(quiet).child(format!("{}.", ix + 1)))
                    .child(div().min_w_0().truncate().child(text.clone()))
                    .child(
                        Button::new(SharedString::from(format!("unqueue-{id}")))
                            .xsmall()
                            .ghost()
                            .icon(IconName::Close)
                            .accessibility_label(format!("Take back: {text}"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(RunControlEvent::Unqueue(id.clone()))
                            })),
                    )
            })
            .collect();

        v_flex()
            .w_full()
            .max_w(px(640.))
            .gap_2()
            .p_2()
            .rounded(radius_lg)
            .border_1()
            .border_color(line)
            .bg(fill)
            .text_color(ink)
            .text_sm()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_1()
                    .text_xs()
                    .child(mark)
                    .child(div().font_medium().text_color(color).child(word))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(quiet)
                            .child(self.status.clone()),
                    )
                    .children(controls),
            )
            .when(!chips.is_empty(), |this| {
                this.child(
                    v_flex()
                        .gap_1()
                        .px_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(quiet)
                                .child("Queued for the next step"),
                        )
                        .child(h_flex().flex_wrap().gap_1().children(chips)),
                )
            })
            .child(
                div().px_1().child(
                    Textarea::new(&self.field)
                        .appearance(false)
                        .aria_label("Note for the run"),
                ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        div()
                            .flex_1()
                            .px_1()
                            .text_xs()
                            .text_color(quiet)
                            .child(if live {
                                "Enter queues it for the next step"
                            } else {
                                "Enter sends it"
                            }),
                    )
                    .when(live, |this| {
                        this.child(
                            Button::new("run-queue")
                                .small()
                                .outline()
                                .label("Queue")
                                .disabled(!has_text)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.send(false, window, cx)),
                                ),
                        )
                    })
                    .child(
                        Button::new("run-steer")
                            .small()
                            .primary()
                            .icon(IconName::ArrowUp)
                            .label(if live { "Steer now" } else { "Send" })
                            .disabled(!has_text)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.send(true, window, cx)),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_phase_offers_its_controls() {
        assert_eq!(
            controls_for(RunPhase::Running),
            &[RunControlEvent::Pause, RunControlEvent::Stop]
        );
        assert_eq!(controls_for(RunPhase::Failed), &[RunControlEvent::Retry]);
        assert!(controls_for(RunPhase::Done).is_empty());
    }
}
