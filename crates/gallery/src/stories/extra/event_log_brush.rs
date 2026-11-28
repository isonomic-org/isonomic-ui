// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the event log with a timeline brush.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::event_log_brush::{
    AttemptStatus, EventLogBrush, EventLogEvent, LogAttempt, LogEvent, LogLevel,
};

/// A made-up nightly import of a seed library's catalogue: about seventy
/// events over three minutes, with a mirror that times out and a store that
/// fills up near the end.
fn nightly_import() -> Vec<LogEvent> {
    let mut seed: u32 = 11;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(seed >> 8) / f64::from(1u32 << 24)
    };
    let mut events = Vec::new();
    let mut at = 1.2;
    for page in 1..=24 {
        let id = format!("fetch-{page:02}");
        let slow = page % 7 == 0;
        let mut event = LogEvent::new(
            id,
            at,
            if slow {
                LogLevel::Warning
            } else {
                LogLevel::Info
            },
            "fetch",
            if slow {
                format!("Page {page} came from the backup mirror")
            } else {
                format!("Fetched catalogue page {page}")
            },
        );
        if slow {
            event = event
                .attempt(
                    LogAttempt::new(AttemptStatus::TimedOut, 8000.)
                        .error("main mirror: no reply in 8 s"),
                )
                .attempt(LogAttempt::new(AttemptStatus::Failed, 420.).error("main mirror: 503"))
                .attempt(LogAttempt::new(
                    AttemptStatus::Succeeded,
                    1900. + next() * 600.,
                ));
        } else {
            event = event.attempt(LogAttempt::new(
                AttemptStatus::Succeeded,
                300. + next() * 500.,
            ));
        }
        events.push(event);
        events.push(LogEvent::new(
            format!("parse-{page:02}"),
            at + 0.8 + next() * 1.5,
            LogLevel::Debug,
            "parse",
            format!(
                "Page {page}: {} seed varieties read",
                30 + (next() * 40.) as u32
            ),
        ));
        if page % 3 == 0 {
            events.push(
                LogEvent::new(
                    format!("store-{page:02}"),
                    at + 3. + next(),
                    if page >= 21 {
                        LogLevel::Error
                    } else {
                        LogLevel::Info
                    },
                    "store",
                    if page >= 21 {
                        format!("Could not save pages {} to {page}", page - 2)
                    } else {
                        format!("Saved pages {} to {page}", page - 2)
                    },
                )
                .attempt(if page >= 21 {
                    LogAttempt::new(AttemptStatus::Failed, 140.).error("volume full: 0 bytes free")
                } else {
                    LogAttempt::new(AttemptStatus::Succeeded, 60. + next() * 90.)
                })
                .when_full(page >= 21),
            );
        }
        at += 5. + next() * 3.;
    }
    events.push(LogEvent::new(
        "notify",
        at + 2.,
        LogLevel::Warning,
        "notify",
        "Import finished with 2 stores failed; the curator was told",
    ));
    events
}

trait Retries {
    fn when_full(self, full: bool) -> Self;
}

impl Retries for LogEvent {
    /// A full volume is tried twice more before the store gives up.
    fn when_full(self, full: bool) -> Self {
        if !full {
            return self;
        }
        self.attempt(
            LogAttempt::new(AttemptStatus::Failed, 150.).error("volume full: 0 bytes free"),
        )
        .attempt(LogAttempt::new(AttemptStatus::Failed, 145.).error("volume full: 0 bytes free"))
    }
}

/// A nightly import, picked and windowed from outside to show the detail.
pub struct EventLogBrushStory {
    log: Entity<EventLogBrush>,
    last: SharedString,
    _events: Subscription,
}

impl EventLogBrushStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let log = cx.new(|cx| {
                let mut log =
                    EventLogBrush::new("import", "Nightly import", nightly_import(), window, cx)
                        .subtitle("Seed catalogue, run of 3 June");
                log.set_window(Some((40., 120.)), cx);
                log.pick("fetch-14", cx);
                log
            });
            let events = cx.subscribe(&log, |this: &mut Self, _, event, cx| {
                this.last = match event {
                    EventLogEvent::Picked(Some(id)) => format!("Picked {id}").into(),
                    EventLogEvent::Picked(None) => "Pick cleared".into(),
                    EventLogEvent::WindowChanged(Some((from, to))) => {
                        format!("Window {from:.1} s to {to:.1} s").into()
                    }
                    EventLogEvent::WindowChanged(None) => "Window cleared".into(),
                };
                cx.notify();
            });
            Self {
                log,
                last: "Drag across the track, or pick a row".into(),
                _events: events,
            }
        })
    }
}

impl Story for EventLogBrushStory {
    fn title() -> &'static str {
        "Event log brush"
    }

    fn description() -> &'static str {
        "A run's events as dots over time, a brush that narrows the log to a window, filters by level, kind and text, and each event's attempts."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for EventLogBrushStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let quiet = cx.theme().muted_foreground;
        v_flex()
            .gap_6()
            .child(self.log.clone())
            .child(div().text_sm().text_color(quiet).child(self.last.clone()))
    }
}
