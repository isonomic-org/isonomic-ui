//! Gallery story for the run control composer.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::run_control_composer::{RunControlComposer, RunControlEvent, RunPhase};

/// A reconciliation run that can be paused, stopped, retried and steered.
/// The story plays the session: queued notes are taken in two seconds
/// after each other, and a stopped run can be started again with a note.
pub struct RunControlComposerStory {
    composer: Entity<RunControlComposer>,
    queued: Vec<(SharedString, SharedString)>,
    log: Vec<String>,
    next: usize,
    _events: Subscription,
}

impl RunControlComposerStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let composer = cx.new(|cx| {
                let mut composer = RunControlComposer::new(window, cx);
                composer.set_phase(RunPhase::Running, "Matching card payouts, step 3 of 7", cx);
                composer
            });
            let events = cx.subscribe(&composer, |this: &mut Self, composer, event, cx| {
                let (phase, status) = match event {
                    RunControlEvent::Queue(text) => {
                        this.next += 1;
                        this.queued
                            .push((format!("q{}", this.next).into(), text.clone()));
                        this.log.push(format!("Queued: {text}"));
                        let story = cx.entity().downgrade();
                        cx.spawn(async move |_, cx| {
                            cx.background_executor().timer(Duration::from_secs(2)).await;
                            story
                                .update(cx, |this, cx| {
                                    if !this.queued.is_empty() {
                                        let (_, text) = this.queued.remove(0);
                                        this.log.push(format!("Taken in at the next step: {text}"));
                                        let queued = this.queued.clone();
                                        this.composer.update(cx, |c, cx| c.set_queued(queued, cx));
                                        cx.notify();
                                    }
                                })
                                .ok();
                        })
                        .detach();
                        (None, None)
                    }
                    RunControlEvent::Steer(text) => {
                        this.log.push(format!("Steered now: {text}"));
                        (Some(RunPhase::Running), Some("Changing course".to_string()))
                    }
                    RunControlEvent::Unqueue(id) => {
                        this.queued.retain(|(q, _)| q != id);
                        (None, None)
                    }
                    RunControlEvent::Pause => {
                        (Some(RunPhase::Paused), Some("Held after step 3".into()))
                    }
                    RunControlEvent::Resume => (
                        Some(RunPhase::Running),
                        Some("Matching card payouts, step 3 of 7".into()),
                    ),
                    RunControlEvent::Stop => (
                        Some(RunPhase::Failed),
                        Some("Stopped by you at step 3".into()),
                    ),
                    RunControlEvent::Retry => {
                        (Some(RunPhase::Running), Some("Retrying step 3".into()))
                    }
                };
                let queued = this.queued.clone();
                composer.update(cx, |c, cx| {
                    if let (Some(phase), Some(status)) = (phase, status) {
                        c.set_phase(phase, status, cx);
                    }
                    c.set_queued(queued, cx);
                });
                cx.notify();
            });
            Self {
                composer,
                queued: Vec::new(),
                log: Vec::new(),
                next: 0,
                _events: events,
            }
        })
    }
}

impl Story for RunControlComposerStory {
    fn title() -> &'static str {
        "Run control composer"
    }

    fn description() -> &'static str {
        "The message field for a running assistant, with stop, pause, resume, retry, steer and a queue."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RunControlComposerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex()
            .w_full()
            .gap_3()
            .child(self.composer.clone())
            .child(
                v_flex().gap_0p5().text_xs().text_color(quiet).children(
                    self.log
                        .iter()
                        .rev()
                        .take(5)
                        .map(|line| div().child(line.clone())),
                ),
            )
    }
}
