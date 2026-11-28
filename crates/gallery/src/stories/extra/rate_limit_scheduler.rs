// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the rate limit scheduler.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use std::time::Duration;

use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Task, Window, div,
};
use isonomic_ui_extra::rate_limit_scheduler::{
    CallTiming, QueuedCall, RateLimit, RateLimitScheduler, schedule,
};

/// Shows a bookkeeping assistant's calls queued against three services.
/// The story pauses limits, reorders and drops calls when asked.
///
/// Under it, an assistant's calls to a model service whose limits refill
/// as buckets; two calls were turned away and count down to their retry.
/// A clock ticks every second: buckets refill, countdowns run down, and a
/// retry that reaches zero goes.
pub struct RateLimitSchedulerStory {
    limits: Vec<RateLimit>,
    calls: Vec<QueuedCall>,
    buckets: Vec<RateLimit>,
    bucket_calls: Vec<QueuedCall>,
    second: u32,
    _tick: Task<()>,
}

fn buckets() -> Vec<RateLimit> {
    vec![
        RateLimit::bucket("requests", "Model requests", 8, 0.5).spent(7, 0),
        RateLimit::bucket("tokens", "Tokens (thousands)", 40, 2.).spent(22, 0),
    ]
}

fn bucket_calls() -> Vec<QueuedCall> {
    vec![
        QueuedCall::new("b1", "Summarise the March ledger", "requests")
            .source("Close checklist")
            .retry(9, 16, 3),
        QueuedCall::new("b2", "Draft supplier email", "requests").source("Payables"),
        QueuedCall::new("b3", "Classify 40 receipts", "tokens")
            .cost(18)
            .source("Receipt filing"),
        QueuedCall::new("b4", "Explain a variance", "requests")
            .source("Reconciler")
            .retry(4, 8, 2),
        QueuedCall::new("b5", "Write the close summary", "tokens")
            .cost(12)
            .source("Close checklist"),
        QueuedCall::new("b6", "Answer a client question", "requests").source("Inbox"),
    ]
}

/// Advance the buckets and countdowns by one second, the `second`th.
fn tick(second: u32, limits: &mut [RateLimit], calls: &mut Vec<QueuedCall>) {
    for limit in limits.iter_mut() {
        if let Some(rate) = limit.refill_per_sec {
            // Whole units come back on the seconds they are due.
            let due = |at: u32| (rate * at as f32).floor() as u32;
            limit.used = limit.used.saturating_sub(due(second + 1) - due(second));
        }
    }
    for call in calls.iter_mut() {
        if let Some(retry) = &mut call.retry {
            retry.in_secs = retry.in_secs.saturating_sub(1);
        }
    }
    // A retry that reaches zero goes, if its bucket has room.
    if let Some(at) = calls
        .iter()
        .position(|c| c.retry.is_some_and(|r| r.in_secs == 0))
        && let Some(limit) = limits.iter_mut().find(|l| l.id == calls[at].limit)
        && limit.used + calls[at].cost <= limit.limit
    {
        limit.used += calls[at].cost;
        calls.remove(at);
    }
    // Every other second, the first call with room goes.
    if second % 2 == 1
        && let Some(at) = schedule(limits, calls)
            .iter()
            .zip(calls.iter())
            .position(|(t, c)| *t == CallTiming::At(0) && c.retry.is_none())
        && let Some(limit) = limits.iter_mut().find(|l| l.id == calls[at].limit)
    {
        limit.used += calls[at].cost;
        calls.remove(at);
    }
    // Once every call has gone, start the scene again.
    if calls.is_empty() {
        *calls = bucket_calls();
        limits.clone_from_slice(&buckets());
    }
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
        cx.new(|cx| {
            let ticker = cx.spawn(
                async move |this: isonomic_ui::WeakEntity<Self>, cx: &mut isonomic_ui::AsyncApp| {
                    loop {
                        cx.background_executor().timer(Duration::from_secs(1)).await;
                        let alive = this.update(cx, |this, cx| {
                            tick(this.second, &mut this.buckets, &mut this.bucket_calls);
                            this.second += 1;
                            cx.notify();
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                },
            );
            Self {
                limits: limits(),
                calls: calls(),
                buckets: buckets(),
                bucket_calls: bucket_calls(),
                second: 0,
                _tick: ticker,
            }
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
            .child({
                let retry = cx.entity();
                let drop = cx.entity();
                RateLimitScheduler::new(
                    "model-queue",
                    self.buckets.clone(),
                    self.bucket_calls.clone(),
                )
                .title("Model calls")
                .horizon(60)
                .on_retry_now(move |id, _, cx| {
                    retry.update(cx, |this, cx| {
                        if let Some(call) = this.bucket_calls.iter_mut().find(|c| &c.id == id)
                            && let Some(retry) = &mut call.retry
                        {
                            retry.in_secs = 0;
                        }
                        cx.notify();
                    })
                })
                .on_drop(move |id, _, cx| {
                    drop.update(cx, |this, cx| {
                        this.bucket_calls.retain(|c| &c.id != id);
                        cx.notify();
                    })
                })
            })
    }
}
