// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the run steering panel.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Task, Window, div,
};
use isonomic_ui_extra::run_steering_panel::{
    RunSteeringEvent, RunSteeringPanel, SteerNote, SteerStep,
};

/// How long each pretend step runs.
const STEP_FOR: Duration = Duration::from_secs(6);

/// A reconciliation run in its second step, with one note waiting. A pretend
/// agent moves on every few seconds; pause holds it.
pub struct RunSteeringPanelStory {
    panel: Entity<RunSteeringPanel>,
    last: SharedString,
    paused: bool,
    _agent: Task<()>,
    _events: Subscription,
}

impl RunSteeringPanelStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let panel = cx.new(|cx| {
                RunSteeringPanel::new(window, cx)
                    .title("Reconcile the March card spend")
                    .agent("Ledger assistant")
                    .started_ago(Duration::from_secs(74))
                    .step(
                        SteerStep::new("pull", "Pull 412 card transactions")
                            .done(Duration::from_secs(9)),
                    )
                    .step(
                        SteerStep::new("match", "Match receipts by amount and date")
                            .active_for(Duration::from_secs(3)),
                    )
                    .step(SteerStep::new(
                        "code",
                        "Code each line to an expense account",
                    ))
                    .step(SteerStep::new(
                        "flag",
                        "Flag anything over the policy limit",
                    ))
                    .step(SteerStep::new(
                        "post",
                        "Post the journal and send a summary",
                    ))
                    .queued(SteerNote::new(
                        "seed",
                        "Treat the airport parking as travel, not vehicle costs",
                    ))
            });
            let events = cx.subscribe(&panel, |this: &mut Self, _, event, cx| {
                this.last = match event {
                    RunSteeringEvent::Queued(note) => format!("Queued: {}", note.text()),
                    RunSteeringEvent::Interrupt(note) => format!("Sent now: {}", note.text()),
                    RunSteeringEvent::Removed(id) => format!("Took back {id}"),
                    RunSteeringEvent::PickedUp { step, notes } => {
                        format!("{step} took in {} note(s)", notes.len())
                    }
                    RunSteeringEvent::Pause => {
                        this.paused = true;
                        "Paused".to_string()
                    }
                    RunSteeringEvent::Resume => {
                        this.paused = false;
                        "Resumed".to_string()
                    }
                }
                .into();
                cx.notify();
            });
            let agent = cx.spawn(async move |this, cx| {
                let order = ["code", "flag", "post"];
                let mut next = 0;
                loop {
                    cx.background_executor().timer(STEP_FOR).await;
                    let Ok((paused, panel)) =
                        this.update(cx, |this, _| (this.paused, this.panel.clone()))
                    else {
                        break;
                    };
                    if paused {
                        continue;
                    }
                    panel.update(cx, |panel, cx| match order.get(next) {
                        Some(id) => panel.start_step(id, cx),
                        None => panel.finish(cx),
                    });
                    next += 1;
                    if next > order.len() {
                        break;
                    }
                }
            });
            Self {
                panel,
                last: "Nothing yet".into(),
                paused: false,
                _agent: agent,
                _events: events,
            }
        })
    }
}

impl Story for RunSteeringPanelStory {
    fn title() -> &'static str {
        "Run steering panel"
    }

    fn description() -> &'static str {
        "Correct an agent while it works."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RunSteeringPanelStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.panel.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
