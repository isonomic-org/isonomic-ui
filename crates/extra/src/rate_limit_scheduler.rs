// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rate limit scheduler: queued calls laid out against the limits they share.
//!
//! Each limit is a budget of calls per window, with what's used so far and
//! when the window resets. Queued calls spend a budget in priority order;
//! a call that doesn't fit waits for the next window. The scheduler shows
//! when each call will go, on a timeline and in a list, and lets the owner
//! pause a limit, reorder the queue or drop a call. It works the timing out
//! but never runs anything.
//!
//! A limit can instead be a refilling bucket: it holds up to its limit and
//! gains a steady trickle back every second, so a call waits only until
//! enough has dripped in. A call that was turned away carries a retry
//! countdown, drawn as a draining bar under its name, with Retry now.
//!
//! Keyboard: each queued call is a Tab stop. Alt with Up or Down moves it,
//! Delete drops it, and R retries a waiting retry now.

use std::rc::Rc;

use isonomic_ui::base::{Transition, transition};
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
    ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// A budget of calls per window, shared by everything that uses it.
#[derive(Debug, Clone, PartialEq)]
pub struct RateLimit {
    /// Identifies it.
    pub id: SharedString,
    /// What it limits, such as a service name.
    pub name: SharedString,
    /// Calls allowed per window.
    pub limit: u32,
    /// How long a window lasts, in seconds.
    pub window_secs: u32,
    /// Calls already spent in this window.
    pub used: u32,
    /// Seconds until this window resets.
    pub resets_in: u32,
    /// Whether the owner has held its calls back.
    pub paused: bool,
    /// For a refilling bucket, how much comes back each second; `None` for
    /// a fixed window that resets all at once.
    pub refill_per_sec: Option<f32>,
}

impl RateLimit {
    /// Create a fresh limit.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        limit: u32,
        window_secs: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            limit,
            window_secs: window_secs.max(1),
            used: 0,
            resets_in: window_secs.max(1),
            paused: false,
            refill_per_sec: None,
        }
    }

    /// Make it a refilling bucket holding up to `capacity`, gaining
    /// `per_sec` back every second. Pass what's spent with `spent`; the
    /// reset time is not used.
    pub fn bucket(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        capacity: u32,
        per_sec: f32,
    ) -> Self {
        let per_sec = per_sec.max(0.001);
        let mut limit = Self::new(
            id,
            name,
            capacity,
            (capacity as f32 / per_sec).ceil() as u32,
        );
        limit.refill_per_sec = Some(per_sec);
        limit
    }

    /// Say how much of this window is spent and when it resets.
    pub fn spent(mut self, used: u32, resets_in: u32) -> Self {
        self.used = used;
        self.resets_in = resets_in;
        self
    }

    /// Hold its calls back.
    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }
}

/// A call waiting for room under a limit.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuedCall {
    /// Identifies it.
    pub id: SharedString,
    /// What it does.
    pub label: SharedString,
    /// The limit it spends.
    pub limit: SharedString,
    /// How many calls' worth of budget it needs.
    pub cost: u32,
    /// Who asked for it, if worth saying.
    pub source: Option<SharedString>,
    /// Set when it was turned away and waits to try again.
    pub retry: Option<CallRetry>,
}

/// A call's countdown to trying again after being turned away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallRetry {
    /// Seconds until it tries again.
    pub in_secs: u32,
    /// The whole wait, in seconds, so the countdown can be drawn.
    pub of_secs: u32,
    /// Which try comes next, from 2.
    pub attempt: u32,
}

impl QueuedCall {
    /// Create a call that costs one.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        limit: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            limit: limit.into(),
            cost: 1,
            source: None,
            retry: None,
        }
    }

    /// Say it was turned away and tries again (`attempt`, from 2) in
    /// `in_secs` of an `of_secs` wait.
    pub fn retry(mut self, in_secs: u32, of_secs: u32, attempt: u32) -> Self {
        self.retry = Some(CallRetry {
            in_secs,
            of_secs: of_secs.max(in_secs).max(1),
            attempt: attempt.max(2),
        });
        self
    }

    /// Set how much budget it needs.
    pub fn cost(mut self, cost: u32) -> Self {
        self.cost = cost.max(1);
        self
    }

    /// Say who asked for it.
    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = Some(source.into());
        self
    }
}

/// When a queued call will go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallTiming {
    /// In this many seconds; zero is now.
    At(u32),
    /// Its limit is paused.
    Held,
    /// It costs more than its limit ever allows, or its limit is unknown.
    Never,
}

/// Work out when each queued call will go, in queue order.
///
/// Calls spend their limit's budget first come, first served. A call that
/// doesn't fit what's left waits for the next reset, when the full budget
/// comes back.
///
/// A refilling bucket instead lets a call wait only until enough has come
/// back. A call counting down to a retry goes no sooner than its retry.
pub fn schedule(limits: &[RateLimit], calls: &[QueuedCall]) -> Vec<CallTiming> {
    struct Budget {
        left: u32,
        at: u32,
        level: f32,
        clock: f32,
    }
    let mut budgets: Vec<Budget> = limits
        .iter()
        .map(|limit| Budget {
            left: limit.limit.saturating_sub(limit.used),
            at: 0,
            level: limit.limit.saturating_sub(limit.used) as f32,
            clock: 0.,
        })
        .collect();
    calls
        .iter()
        .map(|call| {
            let Some(ix) = limits.iter().position(|l| l.id == call.limit) else {
                return CallTiming::Never;
            };
            let limit = &limits[ix];
            if limit.paused {
                return CallTiming::Held;
            }
            if call.cost > limit.limit {
                return CallTiming::Never;
            }
            let budget = &mut budgets[ix];
            let retry = call.retry.map_or(0, |r| r.in_secs);
            if let Some(rate) = limit.refill_per_sec {
                let cost = call.cost as f32;
                if budget.level + 1e-4 < cost {
                    budget.clock += (cost - budget.level) / rate;
                    budget.level = cost;
                }
                budget.level -= cost;
                return CallTiming::At((budget.clock - 1e-4).ceil().max(0.) as u32).later(retry);
            }
            if call.cost > budget.left {
                budget.at = if budget.at == 0 {
                    limit.resets_in
                } else {
                    budget.at + limit.window_secs
                };
                budget.left = limit.limit;
            }
            budget.left -= call.cost;
            CallTiming::At(budget.at).later(retry)
        })
        .collect()
}

impl CallTiming {
    fn later(self, secs: u32) -> Self {
        match self {
            CallTiming::At(at) => CallTiming::At(at.max(secs)),
            other => other,
        }
    }
}

/// How fast a bucket refills, as people say it: "2 a second", "1 every 4s".
pub fn refill_words(per_sec: f32) -> String {
    if per_sec >= 1. {
        let whole = (per_sec * 10.).round() / 10.;
        if whole.fract() == 0. {
            format!("{whole:.0} a second")
        } else {
            format!("{whole:.1} a second")
        }
    } else {
        format!("1 every {}s", (1. / per_sec.max(0.001)).round() as u32)
    }
}

/// Seconds until a bucket is full again; zero when it is.
pub fn bucket_full_in(limit: &RateLimit) -> u32 {
    match limit.refill_per_sec {
        Some(rate) => (limit.used as f32 / rate.max(0.001)).ceil() as u32,
        None => limit.resets_in,
    }
}

/// A retry countdown in words: "Try 2 in 12s".
pub fn retry_words(retry: &CallRetry) -> String {
    match retry.in_secs {
        0 => format!("Try {} now", retry.attempt),
        secs => format!("Try {} {}", retry.attempt, wait_words(secs)),
    }
}

/// Write a wait as people say it: "now", "in 40s", "in 2m 5s".
pub fn wait_words(secs: u32) -> String {
    match secs {
        0 => "now".into(),
        1..=59 => format!("in {secs}s"),
        _ if secs % 60 == 0 => format!("in {}m", secs / 60),
        _ => format!("in {}m {}s", secs / 60, secs % 60),
    }
}

type PauseHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
type MoveHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
type DropHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type RetryHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Limits, the calls queued against them and when each will go.
#[derive(IntoElement)]
pub struct RateLimitScheduler {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    limits: Vec<RateLimit>,
    calls: Vec<QueuedCall>,
    horizon_secs: u32,
    on_pause: Option<PauseHandler>,
    on_move: Option<MoveHandler>,
    on_drop: Option<DropHandler>,
    on_retry_now: Option<RetryHandler>,
}

impl RateLimitScheduler {
    /// Create a scheduler for these limits and calls, in queue order.
    pub fn new(id: impl Into<ElementId>, limits: Vec<RateLimit>, calls: Vec<QueuedCall>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: "Queued calls".into(),
            limits,
            calls,
            horizon_secs: 180,
            on_pause: None,
            on_move: None,
            on_drop: None,
            on_retry_now: None,
        }
    }

    /// Offer Retry now on calls counting down to a retry, run when pressed.
    pub fn on_retry_now(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_retry_now = Some(Rc::new(handler));
        self
    }

    /// Set the heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// How far ahead the timeline reaches, in seconds. Three minutes by default.
    pub fn horizon(mut self, secs: u32) -> Self {
        self.horizon_secs = secs.max(30);
        self
    }

    /// Run when a limit is paused (`true`) or resumed.
    pub fn on_pause(
        mut self,
        handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pause = Some(Rc::new(handler));
        self
    }

    /// Run when a call is moved up (`true`) or down the queue.
    pub fn on_move(
        mut self,
        handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_move = Some(Rc::new(handler));
        self
    }

    /// Run when a call is dropped from the queue.
    pub fn on_drop(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_drop = Some(Rc::new(handler));
        self
    }
}

impl Styled for RateLimitScheduler {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A steady colour for each limit, so its calls match it on the timeline.
fn hue(ix: usize, cx: &App) -> Hsla {
    let theme = cx.theme();
    let palette = [
        theme.primary,
        theme.success,
        theme.warning,
        theme.info,
        theme.danger,
    ];
    palette[ix % palette.len()]
}

impl RenderOnce for RateLimitScheduler {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let danger = theme.danger;
        let warning = theme.warning;
        let success = theme.success;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;

        let timings = schedule(&self.limits, &self.calls);
        let horizon = self.horizon_secs;
        let now_count = timings.iter().filter(|t| **t == CallTiming::At(0)).count();
        let last = timings
            .iter()
            .filter_map(|t| match t {
                CallTiming::At(secs) => Some(*secs),
                _ => None,
            })
            .max();
        let held = timings.iter().filter(|t| **t == CallTiming::Held).count();
        let colors: Vec<Hsla> = (0..self.limits.len()).map(|ix| hue(ix, cx)).collect();

        let limit_rows: Vec<_> = self
            .limits
            .iter()
            .enumerate()
            .map(|(ix, limit)| {
                let bucket = limit.refill_per_sec;
                let share = if limit.limit == 0 {
                    1.
                } else if bucket.is_some() {
                    // A bucket shows what it holds, so it fills as it refills.
                    (limit.limit.saturating_sub(limit.used) as f32 / limit.limit as f32).min(1.)
                } else {
                    (limit.used as f32 / limit.limit as f32).min(1.)
                };
                let share = match bucket {
                    Some(_) => transition(
                        ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("level-{}", limit.id).into(),
                        ),
                        share,
                        Transition::new(std::time::Duration::from_millis(900)),
                        window,
                        cx,
                    ),
                    None => share,
                };
                let full = limit.used >= limit.limit;
                let queued = self.calls.iter().filter(|c| c.limit == limit.id).count();
                let pause = self.on_pause.clone();
                let id = limit.id.clone();
                let paused = limit.paused;
                h_flex()
                    .gap_3()
                    .py_1p5()
                    .child(div().size(px(8.)).flex_none().rounded_full().bg(colors[ix]))
                    .child(
                        v_flex()
                            .w(px(150.))
                            .flex_none()
                            .child(div().truncate().font_medium().child(limit.name.clone()))
                            .child(div().text_caption().text_color(quiet).child(match bucket {
                                Some(rate) => {
                                    format!("Holds {}, refills {}", limit.limit, refill_words(rate))
                                }
                                None => format!(
                                    "{} per {}",
                                    limit.limit,
                                    if limit.window_secs % 60 == 0 {
                                        match limit.window_secs / 60 {
                                            1 => "minute".to_string(),
                                            n => format!("{n} minutes"),
                                        }
                                    } else {
                                        format!("{}s", limit.window_secs)
                                    }
                                ),
                            })),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(120.))
                            .gap_1()
                            .child(
                                div().h(px(6.)).rounded_full().bg(chip).child(
                                    div()
                                        .h_full()
                                        .rounded_full()
                                        .bg(if full && bucket.is_none() {
                                            danger
                                        } else if bucket.is_some() && full {
                                            danger
                                        } else {
                                            colors[ix]
                                        })
                                        .w(relative(share)),
                                ),
                            )
                            .child(
                                h_flex()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(
                                        div()
                                            .flex_1()
                                            .when(full, |this| this.text_color(danger))
                                            .child(match bucket {
                                                Some(_) => format!(
                                                    "{} of {} left",
                                                    limit.limit.saturating_sub(limit.used),
                                                    limit.limit
                                                ),
                                                None => format!(
                                                    "{} of {} used",
                                                    limit.used, limit.limit
                                                ),
                                            }),
                                    )
                                    .child(match bucket {
                                        Some(_) if limit.used == 0 => "full".to_string(),
                                        Some(_) => {
                                            format!("full {}", wait_words(bucket_full_in(limit)))
                                        }
                                        None => format!("resets {}", wait_words(limit.resets_in)),
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .w(px(64.))
                            .flex_none()
                            .text_caption()
                            .text_color(if paused { warning } else { quiet })
                            .child(if paused {
                                "Paused".to_string()
                            } else {
                                format!("{queued} queued")
                            }),
                    )
                    .child(
                        Button::new(ElementId::NamedChild(
                            self.id.clone().into(),
                            format!("pause-{}", limit.id).into(),
                        ))
                        .xsmall()
                        .ghost()
                        .icon(if paused {
                            IconName::Play
                        } else {
                            IconName::Pause
                        })
                        .accessibility_label(format!(
                            "{} {}",
                            if paused { "Resume" } else { "Pause" },
                            limit.name
                        ))
                        .on_click(move |_, window, cx| {
                            if let Some(pause) = &pause {
                                pause(&id, !paused, window, cx);
                            }
                        }),
                    )
            })
            .collect();

        // The timeline: one track per limit, a mark for each call that goes
        // within the horizon.
        let tick_every = if horizon <= 120 { 30 } else { 60 };
        let ticks: Vec<u32> = (0..=horizon).step_by(tick_every as usize).collect();
        let tracks: Vec<_> = self
            .limits
            .iter()
            .enumerate()
            .map(|(ix, limit)| {
                let mut marks: Vec<(u32, u32)> = Vec::new();
                for (call, timing) in self.calls.iter().zip(&timings) {
                    if call.limit != limit.id {
                        continue;
                    }
                    if let CallTiming::At(secs) = timing {
                        if *secs <= horizon {
                            match marks.iter_mut().find(|(at, _)| at == secs) {
                                Some((_, count)) => *count += 1,
                                None => marks.push((*secs, 1)),
                            }
                        }
                    }
                }
                let resets: Vec<u32> = if limit.refill_per_sec.is_some() {
                    Vec::new()
                } else {
                    (0..)
                        .map(|n| limit.resets_in + n * limit.window_secs)
                        .take_while(|at| *at <= horizon)
                        .collect()
                };
                div()
                    .relative()
                    .h(px(18.))
                    .w_full()
                    .when(limit.paused, |this| this.opacity(0.45))
                    .child(
                        div()
                            .absolute()
                            .top(px(8.))
                            .left_0()
                            .right_0()
                            .h(px(2.))
                            .rounded_full()
                            .bg(chip),
                    )
                    .children(resets.into_iter().map(|at| {
                        div()
                            .absolute()
                            .top(px(3.))
                            .left(relative(at as f32 / horizon as f32))
                            .w(px(1.))
                            .h(px(12.))
                            .bg(line)
                    }))
                    .children(marks.into_iter().map(|(at, count)| {
                        div()
                            .absolute()
                            .top(px(1.))
                            .left(relative(at as f32 / horizon as f32))
                            .ml(px(-8.))
                            .h(px(16.))
                            .min_w(px(16.))
                            .px_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(colors[ix])
                            .text_color(fill)
                            .text_label_small()
                            .child(count.to_string())
                    }))
            })
            .collect();

        let call_rows: Vec<_> = self
            .calls
            .iter()
            .zip(&timings)
            .enumerate()
            .map(|(row, (call, timing))| {
                let limit_ix = self.limits.iter().position(|l| l.id == call.limit);
                let color = limit_ix.map(|ix| colors[ix]).unwrap_or(quiet);
                let limit_name = limit_ix
                    .map(|ix| self.limits[ix].name.clone())
                    .unwrap_or_else(|| "Unknown limit".into());
                let (when, when_color) = match timing {
                    CallTiming::At(0) if call.retry.is_some() => ("Retrying".to_string(), warning),
                    CallTiming::At(0) => ("Now".to_string(), success),
                    CallTiming::At(secs) if call.retry.is_some_and(|r| r.in_secs >= *secs) => {
                        (wait_words(*secs), warning)
                    }
                    CallTiming::At(secs) => (wait_words(*secs), ink),
                    CallTiming::Held => ("Held".to_string(), warning),
                    CallTiming::Never => ("Won't fit".to_string(), danger),
                };
                let focus = window
                    .use_keyed_state(
                        (self.id.clone(), format!("call-{}", call.id)),
                        cx,
                        |_, cx| cx.focus_handle().tab_stop(true),
                    )
                    .read(cx)
                    .clone();
                let is_focused = focus.is_focused(window);
                let first = row == 0;
                let last_row = row + 1 == self.calls.len();
                let keys = {
                    let id = call.id.clone();
                    let on_move = self.on_move.clone();
                    let on_drop = self.on_drop.clone();
                    let on_retry = self.on_retry_now.clone().filter(|_| call.retry.is_some());
                    move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                        let key = event.keystroke.key.as_str();
                        if key == "r" && !event.keystroke.modifiers.modified() {
                            if let Some(on_retry) = &on_retry {
                                on_retry(&id, window, cx);
                                cx.stop_propagation();
                            }
                            return;
                        }
                        if event.keystroke.modifiers.alt && matches!(key, "up" | "down") {
                            if let Some(on_move) = &on_move {
                                let up = key == "up";
                                if (up && !first) || (!up && !last_row) {
                                    on_move(&id, up, window, cx);
                                }
                            }
                            cx.stop_propagation();
                        } else if matches!(key, "delete" | "backspace") {
                            if let Some(on_drop) = &on_drop {
                                on_drop(&id, window, cx);
                            }
                            cx.stop_propagation();
                        }
                    }
                };
                let button = |name: &str, icon: IconName, label: String, disabled: bool| {
                    Button::new(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("{name}-{}", call.id).into(),
                    ))
                    .xsmall()
                    .ghost()
                    .icon(icon)
                    .disabled(disabled)
                    .accessibility_label(label)
                };
                let up = {
                    let on_move = self.on_move.clone();
                    let id = call.id.clone();
                    button(
                        "up",
                        IconName::ArrowUp,
                        format!("Move {} up", call.label),
                        first,
                    )
                    .on_click(move |_, window, cx| {
                        if let Some(on_move) = &on_move {
                            on_move(&id, true, window, cx);
                        }
                    })
                };
                let down = {
                    let on_move = self.on_move.clone();
                    let id = call.id.clone();
                    button(
                        "down",
                        IconName::ArrowDown,
                        format!("Move {} down", call.label),
                        last_row,
                    )
                    .on_click(move |_, window, cx| {
                        if let Some(on_move) = &on_move {
                            on_move(&id, false, window, cx);
                        }
                    })
                };
                let remove = {
                    let on_drop = self.on_drop.clone();
                    let id = call.id.clone();
                    button(
                        "drop",
                        IconName::Close,
                        format!("Drop {}", call.label),
                        false,
                    )
                    .on_click(move |_, window, cx| {
                        if let Some(on_drop) = &on_drop {
                            on_drop(&id, window, cx);
                        }
                    })
                };
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("row-{}", call.id).into(),
                    ))
                    .track_focus(&focus)
                    .on_key_down(keys)
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded(radius)
                    .when(row > 0, |this| this.border_t_1().border_color(line))
                    .when(is_focused, |this| this.focus_ring_style(window, cx))
                    .role(Role::Row)
                    .aria_label(format!(
                        "{}, {}, {}{}{}",
                        call.label,
                        limit_name,
                        when.to_lowercase(),
                        if call.cost > 1 {
                            format!(", costs {}", call.cost)
                        } else {
                            String::new()
                        },
                        call.retry
                            .map(|r| format!(", turned away, {}", retry_words(&r).to_lowercase()))
                            .unwrap_or_default()
                    ))
                    .child(
                        div()
                            .w(px(18.))
                            .flex_none()
                            .text_caption()
                            .text_color(quiet)
                            .child((row + 1).to_string()),
                    )
                    .child(div().size(px(8.)).flex_none().rounded_full().bg(color))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().child(call.label.clone()))
                            .child(
                                div().truncate().text_caption().text_color(quiet).child(
                                    match (&call.source, call.cost) {
                                        (Some(source), 1) => {
                                            format!("{limit_name} \u{b7} {source}")
                                        }
                                        (Some(source), cost) => {
                                            format!(
                                                "{limit_name} \u{b7} {source} \u{b7} costs {cost}"
                                            )
                                        }
                                        (None, 1) => limit_name.to_string(),
                                        (None, cost) => format!("{limit_name} \u{b7} costs {cost}"),
                                    },
                                ),
                            )
                            .when_some(call.retry, |this, retry| {
                                let left = transition(
                                    ElementId::NamedChild(
                                        self.id.clone().into(),
                                        format!("retry-{}", call.id).into(),
                                    ),
                                    retry.in_secs as f32 / retry.of_secs as f32,
                                    Transition::new(std::time::Duration::from_millis(900)),
                                    window,
                                    cx,
                                );
                                this.child(
                                    h_flex()
                                        .gap_2()
                                        .mt_0p5()
                                        .child(
                                            div()
                                                .flex_none()
                                                .text_caption()
                                                .text_color(warning)
                                                .child(retry_words(&retry)),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .max_w(px(160.))
                                                .h(px(3.))
                                                .rounded_full()
                                                .bg(chip)
                                                .child(
                                                    div()
                                                        .h_full()
                                                        .rounded_full()
                                                        .bg(warning)
                                                        .w(relative(left.clamp(0., 1.))),
                                                ),
                                        ),
                                )
                            }),
                    )
                    .child(
                        div()
                            .w(px(72.))
                            .flex_none()
                            .text_right()
                            .text_label_small()
                            .text_color(when_color)
                            .child(when),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .when_some(
                                self.on_retry_now.clone().filter(|_| call.retry.is_some()),
                                |this, on_retry| {
                                    let id = call.id.clone();
                                    this.child(
                                        button(
                                            "retry",
                                            IconName::Redo,
                                            format!("Retry {} now", call.label),
                                            false,
                                        )
                                        .on_click(move |_, window, cx| on_retry(&id, window, cx)),
                                    )
                                },
                            )
                            .child(up)
                            .child(down)
                            .child(remove),
                    )
            })
            .collect();

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(760.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(line)
                    .child(Icon::new(IconName::Time).small().text_color(quiet))
                    .child(div().flex_1().font_medium().child(self.title))
                    .child(div().text_caption().text_color(quiet).child(
                        match (self.calls.len(), last) {
                            (0, _) => "Nothing queued".to_string(),
                            (n, Some(last)) if last > 0 => format!(
                                "{n} queued, {now_count} now, all done {}{}",
                                wait_words(last),
                                if held > 0 {
                                    format!(", {held} held")
                                } else {
                                    String::new()
                                }
                            ),
                            (n, _) => format!(
                                "{n} queued, {now_count} now{}",
                                if held > 0 {
                                    format!(", {held} held")
                                } else {
                                    String::new()
                                }
                            ),
                        },
                    )),
            )
            .child(v_flex().px_4().py_1().children(limit_rows))
            .child(
                v_flex()
                    .gap_1p5()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(line)
                    .children(tracks)
                    .child(
                        div()
                            .relative()
                            .h(px(14.))
                            .w_full()
                            .text_caption()
                            .text_color(quiet)
                            .children(ticks.into_iter().map(|at| {
                                let words = match at {
                                    0 => "Now".to_string(),
                                    _ if at % 60 == 0 => format!("{}m", at / 60),
                                    _ => format!("{at}s"),
                                };
                                div()
                                    .absolute()
                                    .top_0()
                                    .when(at < horizon, |this| {
                                        this.left(relative(at as f32 / horizon as f32))
                                    })
                                    .when(at == horizon, |this| this.right_0())
                                    .child(words)
                            })),
                    ),
            )
            .child(
                v_flex()
                    .px_2()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .children(call_rows)
                    .when(self.calls.is_empty(), |this| {
                        this.child(
                            div()
                                .px_2()
                                .py_3()
                                .text_color(quiet)
                                .child("Nothing waiting for room."),
                        )
                    }),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_fill_this_window_then_wait_for_resets() {
        let limits = vec![RateLimit::new("bank", "Bank feed", 3, 60).spent(1, 20)];
        let calls: Vec<_> = (0..5)
            .map(|n| QueuedCall::new(format!("c{n}"), "Fetch", "bank"))
            .collect();
        assert_eq!(
            schedule(&limits, &calls),
            [
                CallTiming::At(0),
                CallTiming::At(0),
                CallTiming::At(20),
                CallTiming::At(20),
                CallTiming::At(20),
            ]
        );
    }

    #[test]
    fn a_costly_call_waits_for_a_whole_window() {
        let limits = vec![RateLimit::new("mail", "Mail", 4, 60).spent(2, 10)];
        let calls = vec![
            QueuedCall::new("a", "Send batch", "mail").cost(3),
            QueuedCall::new("b", "Send batch", "mail").cost(3),
            QueuedCall::new("c", "Huge", "mail").cost(5),
        ];
        assert_eq!(
            schedule(&limits, &calls),
            [CallTiming::At(10), CallTiming::At(70), CallTiming::Never]
        );
    }

    #[test]
    fn paused_and_unknown_limits_hold_calls() {
        let limits = vec![RateLimit::new("a", "A", 5, 60).paused(true)];
        let calls = vec![
            QueuedCall::new("x", "X", "a"),
            QueuedCall::new("y", "Y", "b"),
        ];
        assert_eq!(
            schedule(&limits, &calls),
            [CallTiming::Held, CallTiming::Never]
        );
    }

    #[test]
    fn a_bucket_lets_calls_go_as_it_refills() {
        // Holds 4, 2 used, one back every 2 seconds.
        let limits = vec![RateLimit::bucket("api", "API", 4, 0.5).spent(2, 0)];
        let calls: Vec<_> = (0..5)
            .map(|n| QueuedCall::new(format!("c{n}"), "Call", "api"))
            .collect();
        assert_eq!(
            schedule(&limits, &calls),
            [
                CallTiming::At(0),
                CallTiming::At(0),
                CallTiming::At(2),
                CallTiming::At(4),
                CallTiming::At(6),
            ]
        );
        let costly = vec![QueuedCall::new("big", "Batch", "api").cost(5)];
        assert_eq!(schedule(&limits, &costly), [CallTiming::Never]);
        assert_eq!(bucket_full_in(&limits[0]), 4);
    }

    #[test]
    fn a_retry_goes_no_sooner_than_its_countdown() {
        let limits = vec![RateLimit::new("bank", "Bank", 5, 60).spent(0, 60)];
        let calls = vec![
            QueuedCall::new("a", "A", "bank").retry(12, 30, 2),
            QueuedCall::new("b", "B", "bank"),
        ];
        assert_eq!(
            schedule(&limits, &calls),
            [CallTiming::At(12), CallTiming::At(0)]
        );
        assert_eq!(retry_words(&calls[0].retry.unwrap()), "Try 2 in 12s");
        let now = QueuedCall::new("c", "C", "bank")
            .retry(0, 5, 1)
            .retry
            .unwrap();
        assert_eq!((now.attempt, now.of_secs), (2, 5));
        assert_eq!(retry_words(&now), "Try 2 now");
    }

    #[test]
    fn refills_read_naturally() {
        assert_eq!(refill_words(2.), "2 a second");
        assert_eq!(refill_words(1.5), "1.5 a second");
        assert_eq!(refill_words(0.25), "1 every 4s");
    }

    #[test]
    fn waits_read_naturally() {
        assert_eq!(wait_words(0), "now");
        assert_eq!(wait_words(42), "in 42s");
        assert_eq!(wait_words(120), "in 2m");
        assert_eq!(wait_words(125), "in 2m 5s");
    }
}
