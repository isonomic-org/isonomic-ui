// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the run status bar.

use std::time::{Duration, Instant};

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Task, Window, div, px,
};
use isonomic_ui_extra::prompt_composer::PromptComposer;
use isonomic_ui_extra::run_status_bar::{RunStatus, RunStatusBar, RunStatusButton};

/// A chat with a local model: the bar in each of its states with the buttons
/// each one carries, the bar where it lives above the composer, and the bar
/// in a narrow column with a reason.
pub struct RunStatusBarStory {
    composer: Entity<PromptComposer>,
    /// When the working run began, for its elapsed time.
    started: Instant,
    /// The state whose shorthand action is working, for a moment after it is pressed.
    busy: Option<RunStatus>,
    last: SharedString,
    _work: Option<Task<()>>,
    _clock: Task<()>,
}

impl RunStatusBarStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let composer = cx.new(|cx| PromptComposer::new("Message\u{2026}", window, cx));
            composer.update(cx, |composer, cx| {
                composer.set_attach(false, cx);
                composer.set_dictation(false, cx);
            });
            // The owner keeps the clock: redraw once a second.
            let clock = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            });
            Self {
                composer,
                started: Instant::now(),
                busy: None,
                last: "Nothing pressed yet".into(),
                _work: None,
                _clock: clock,
            }
        })
    }

    /// The shorthand action shows as working for a moment, then says what it did.
    fn act(&mut self, status: RunStatus, cx: &mut Context<Self>) {
        self.busy = Some(status);
        self.last = match status {
            RunStatus::CutShort => "Continuing\u{2026}".into(),
            _ => "Trying again\u{2026}".into(),
        };
        cx.notify();
        self._work = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            this.update(cx, |this, cx| {
                this.busy = None;
                this.last = match status {
                    RunStatus::CutShort => "Continued".into(),
                    _ => "Tried again".into(),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    /// A button that says it was pressed.
    fn button(&self, label: &'static str, cx: &mut Context<Self>) -> RunStatusButton {
        RunStatusButton::new(label).on_click(cx.listener(move |this, _, _, cx| {
            this.last = format!("{label} pressed").into();
            cx.notify();
        }))
    }

    /// A bar with the one-button shorthand.
    fn shorthand(
        &self,
        name: &'static str,
        status: RunStatus,
        cx: &mut Context<Self>,
    ) -> RunStatusBar {
        RunStatusBar::new(name, status)
            .action_busy(self.busy == Some(status))
            .on_action(cx.listener(|this, status: &RunStatus, _, cx| this.act(*status, cx)))
    }
}

impl Story for RunStatusBarStory {
    fn title() -> &'static str {
        "Run status bar"
    }

    fn description() -> &'static str {
        "The bar above the composer that says what the run is doing or how it ended, with the things to do about it."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RunStatusBarStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        let heading = |text: &'static str| div().text_xs().text_color(quiet).child(text);
        let elapsed = self.started.elapsed();

        v_flex()
            .w_full()
            .max_w(px(720.))
            .gap_6()
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("Each state, with its buttons"))
                    .child(
                        RunStatusBar::new("running", RunStatus::Running)
                            .elapsed(elapsed)
                            .button(self.button("Steer", cx))
                            .button(self.button("Stop", cx)),
                    )
                    .child(
                        RunStatusBar::new("stopped", RunStatus::Stopped)
                            .button(self.button("Dismiss", cx))
                            .button(self.button("Retry", cx))
                            .button(self.button("Resume", cx).primary()),
                    )
                    .child(
                        RunStatusBar::new("failed", RunStatus::Failed)
                            .button(self.button("Dismiss", cx))
                            .button(self.button("Retry", cx)),
                    )
                    .child(RunStatusBar::new("done", RunStatus::Done))
                    .child(self.shorthand("cut-short", RunStatus::CutShort, cx))
                    .child(RunStatusBar::new("reconnecting", RunStatus::Reconnecting))
                    .child(RunStatusBar::new("closed", RunStatus::Closed))
                    .child(div().text_xs().text_color(quiet).child(self.last.clone())),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(heading(
                        "An agent's status line: the step, details that turn over, its own clock and Stop",
                    ))
                    .child(
                        RunStatusBar::new("agent-step", RunStatus::Running)
                            .step(3, Some(7))
                            .message("Matching the bank feed")
                            .rotating_details([
                                "Reading 214 card transactions",
                                "Pairing payments with invoices",
                                "Checking three that do not match",
                            ])
                            .since(self.started)
                            .on_stop(cx.listener(|this, _, _, cx| {
                                this.last = "Stop pressed".into();
                                cx.notify();
                            })),
                    )
                    .child(
                        RunStatusBar::new("agent-open-step", RunStatus::Running)
                            .step(2, None)
                            .message("Drafting the reminder emails")
                            .detail("Four customers, oldest invoice first")
                            .since(self.started),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("The one-button shorthand"))
                    .child(self.shorthand("failed-shorthand", RunStatus::Failed, cx)),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(heading("Above the composer"))
                    .child(
                        RunStatusBar::new("above-composer", RunStatus::Stopped)
                            .button(self.button("Retry", cx)),
                    )
                    .child(self.composer.clone()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .w_full()
                    .max_w(px(320.))
                    .child(heading("In a narrow column, with a reason"))
                    .child(
                        RunStatusBar::new("narrow", RunStatus::Failed)
                            .message("The model could not be loaded.")
                            .detail(
                                "There is not enough free memory for this model. Close other \
                                 apps or pick a smaller model, then try again.",
                            )
                            .button(self.button("Retry", cx)),
                    ),
            )
    }
}
