// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the rate limit scheduler.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::rate_limit_scheduler::{QueuedCall, RateLimit, RateLimitScheduler};

/// Shows a bookkeeping assistant's calls queued against three services.
/// The story pauses limits, reorders and drops calls when asked.
pub struct RateLimitSchedulerStory {
    limits: Vec<RateLimit>,
    calls: Vec<QueuedCall>,
}

fn limits() -> Vec<RateLimit> {
    vec![
        RateLimit::new("bank", "Bank feed", 6, 60).spent(4, 18),
        RateLimit::new("ledger", "Ledger", 10, 60).spent(9, 41),
        RateLimit::new("mail", "Mail", 3, 120).spent(0, 120),
    ]
}

fn calls() -> Vec<QueuedCall> {
    vec![
        QueuedCall::new("q1", "Pull today's card payouts", "bank").source("Reconciler"),
        QueuedCall::new("q2", "Post 12 matched lines", "ledger")
            .cost(2)
            .source("Reconciler"),
        QueuedCall::new("q3", "Pull the savings account", "bank").source("Reconciler"),
        QueuedCall::new("q4", "Send three payment reminders", "mail")
            .cost(3)
            .source("Collections"),
        QueuedCall::new("q5", "Pull last month's statement", "bank")
            .cost(2)
            .source("Close checklist"),
        QueuedCall::new("q6", "Post the rent accrual", "ledger").source("Close checklist"),
        QueuedCall::new("q7", "Send the close summary", "mail").source("Close checklist"),
        QueuedCall::new("q8", "Refresh vendor balances", "ledger")
            .cost(4)
            .source("Payables"),
    ]
}

impl RateLimitSchedulerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            limits: limits(),
            calls: calls(),
        })
    }
}

impl Story for RateLimitSchedulerStory {
    fn title() -> &'static str {
        "Rate limit scheduler"
    }

    fn description() -> &'static str {
        "Queued calls laid out against the limits they share."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for RateLimitSchedulerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pause = cx.entity();
        let reorder = cx.entity();
        let drop = cx.entity();
        v_flex()
            .w_full()
            .gap_3()
            .child(
                RateLimitScheduler::new("close-queue", self.limits.clone(), self.calls.clone())
                    .title("Calls waiting for room")
                    .on_pause(move |id, paused, _, cx| {
                        pause.update(cx, |this, cx| {
                            if let Some(limit) = this.limits.iter_mut().find(|l| &l.id == id) {
                                limit.paused = paused;
                            }
                            cx.notify();
                        })
                    })
                    .on_move(move |id, up, _, cx| {
                        reorder.update(cx, |this, cx| {
                            if let Some(at) = this.calls.iter().position(|c| &c.id == id) {
                                let to = if up {
                                    at.checked_sub(1)
                                } else {
                                    Some(at + 1).filter(|to| *to < this.calls.len())
                                };
                                if let Some(to) = to {
                                    this.calls.swap(at, to);
                                }
                            }
                            cx.notify();
                        })
                    })
                    .on_drop(move |id, _, cx| {
                        drop.update(cx, |this, cx| {
                            this.calls.retain(|c| &c.id != id);
                            cx.notify();
                        })
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Tab to a call, then Alt with Up or Down moves it and Delete drops it."),
            )
    }
}
