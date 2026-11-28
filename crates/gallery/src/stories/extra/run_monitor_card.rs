// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the run monitor card.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::v_flex;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Task, Window,
};
use isonomic_ui_extra::run_monitor_card::{FinishedStep, MonitorState, RunMonitorCard};

const PLAN: [&str; 7] = [
    "Pull the card payouts",
    "Pull the bank statement",
    "Match payouts to ledger lines",
    "Match bank lines by reference",
    "Post 201 matches",
    "Flag what's left",
    "Write the close note",
];

/// Plays a reconciliation run that advances every two seconds, beside a
/// finished one. The story keeps the clock, the steps and the fold.
pub struct RunMonitorCardStory {
    done: Vec<FinishedStep>,
    elapsed: u32,
    paused: bool,
    stopped: bool,
    folded: bool,
    _tick: Task<()>,
}

impl RunMonitorCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let tick = cx.spawn(
                async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                    loop {
                        cx.background_executor().timer(Duration::from_secs(1)).await;
                        let alive = this.update(cx, |this, cx| {
                            if this.paused || this.stopped {
                                return;
                            }
                            this.elapsed += 1;
                            if this.elapsed % 2 == 0 {
                                if this.done.len() == PLAN.len() {
                                    this.done.clear();
                                    this.elapsed = 0;
                                } else {
                                    let label = PLAN[this.done.len()];
                                    this.done.push(FinishedStep::new(label, 2));
                                }
                            }
                            cx.notify();
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                },
            );
            Self {
                done: Vec::new(),
                elapsed: 0,
                paused: false,
                stopped: false,
                folded: false,
                _tick: tick,
            }
        })
    }
}

impl Story for RunMonitorCardStory {
    fn title() -> &'static str {
        "Run monitor card"
    }

    fn description() -> &'static str {
        "One run's current step, time, spend and finished steps, foldable to a bar."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RunMonitorCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fold = cx.entity();
        let pause = cx.entity();
        let stop = cx.entity();
        let finished = self.done.len() == PLAN.len();
        let state = match (self.stopped, self.paused, finished) {
            (true, _, _) => MonitorState::Failed,
            (_, _, true) => MonitorState::Done,
            (_, true, _) => MonitorState::Paused,
            _ => MonitorState::Running,
        };
        let current = if self.stopped {
            "Stopped by you".to_string()
        } else {
            PLAN.get(self.done.len())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Wrote the close note".into())
        };
        v_flex()
            .w_full()
            .gap_4()
            .child(
                RunMonitorCard::new("live-run", "Reconcile March")
                    .state(state, current)
                    .usage(self.elapsed, 4 + self.done.len() as u32 * 3)
                    .budget(25)
                    .steps(self.done.clone(), Some(PLAN.len()))
                    .folded(self.folded)
                    .on_fold(move |_, cx| {
                        fold.update(cx, |this, cx| {
                            this.folded = !this.folded;
                            cx.notify();
                        })
                    })
                    .on_pause(move |_, cx| {
                        pause.update(cx, |this, cx| {
                            this.paused = !this.paused;
                            cx.notify();
                        })
                    })
                    .on_stop(move |_, cx| {
                        stop.update(cx, |this, cx| {
                            this.stopped = true;
                            cx.notify();
                        })
                    }),
            )
            .child(
                RunMonitorCard::new("done-run", "Chase overdue invoices")
                    .state(
                        MonitorState::Failed,
                        "Sending the third reminder: the mail service refused it",
                    )
                    .usage(4_380, 61)
                    .budget(50)
                    .steps(
                        vec![
                            FinishedStep::new("Find invoices over 30 days", 41),
                            FinishedStep::new("Draft seven reminders", 312),
                            FinishedStep::new("Send the first two", 18),
                            FinishedStep::new("Send the third", 6).failed(),
                        ],
                        Some(5),
                    ),
            )
    }
}
