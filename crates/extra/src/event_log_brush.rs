// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Event log with a timeline brush: every event of a run as a dot on a time
//! track, a brush that narrows the log to a window of time, and the log
//! itself underneath.
//!
//! The track has one lane per level, errors on top. Dragging across it draws
//! a window; dragging inside the window moves it, and dragging an edge
//! resizes it. A click on a dot picks that event, and a click on empty track
//! clears the window. From the keyboard the track is a Tab stop: Left and
//! Right move the window, Shift with them resizes it, Home and End send it to
//! either end and Escape clears it.
//!
//! Below the track, toggles filter the log by level and by kind, and a field
//! filters it by text. The log is a list: Up and Down move the pick, Home and
//! End jump to the ends. The picked event shows each of its attempts with its
//! status, how long it took and the error it ended with.

use std::{cell::Cell, f32::consts::TAU, rc::Rc};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _,
    button::{Button, ButtonVariants as _, Toggle, ToggleGroup},
    h_flex,
    input::{Input, InputEvent, InputState},
    look,
    tag::Tag,
    v_flex,
};
use isonomic_ui::{
    App, AppContext as _, Bounds, Context, ElementId, Entity, EventEmitter, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement as _, PathBuilder, Pixels, Render, Role,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    canvas, div, point, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{inset, panel, shell, shell_header};

/// One lane of the track, and the room above the first.
const LANE: f32 = 15.;
const LANE_TOP: f32 = 10.;
const TRACK_H: f32 = LANE_TOP * 2. + LANE * 3.;
/// How close the pointer must be to a brush edge or a dot to take it.
const GRAB: f32 = 6.;
/// How far the pointer must travel before a press becomes a drag.
const DRAG_START: f32 = 3.;
/// How much of the run one arrow key moves or resizes the window by.
const KEY_STEP: f64 = 0.05;
/// The narrowest window, as a share of the run.
const MIN_WINDOW: f64 = 0.02;

/// How serious an event is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogLevel {
    /// Detail for whoever is debugging.
    Debug,
    /// Ordinary progress.
    Info,
    /// Something went wrong and was handled.
    Warning,
    /// Something failed.
    Error,
}

impl LogLevel {
    /// Every level, most serious first: the order of the lanes.
    pub const ALL: [LogLevel; 4] = [
        LogLevel::Error,
        LogLevel::Warning,
        LogLevel::Info,
        LogLevel::Debug,
    ];

    /// Its name.
    pub fn name(self) -> &'static str {
        match self {
            LogLevel::Debug => "Debug",
            LogLevel::Info => "Info",
            LogLevel::Warning => "Warning",
            LogLevel::Error => "Error",
        }
    }

    /// Its lane on the track, from the top.
    pub fn lane(self) -> usize {
        Self::ALL
            .iter()
            .position(|level| *level == self)
            .unwrap_or(0)
    }
}

/// How one attempt at an event's work ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptStatus {
    /// It worked.
    Succeeded,
    /// It failed.
    Failed,
    /// It ran out of time.
    TimedOut,
    /// It is still going.
    Running,
}

impl AttemptStatus {
    fn name(self) -> &'static str {
        match self {
            AttemptStatus::Succeeded => "Succeeded",
            AttemptStatus::Failed => "Failed",
            AttemptStatus::TimedOut => "Timed out",
            AttemptStatus::Running => "Running",
        }
    }
}

/// One attempt at an event's work.
#[derive(Debug, Clone, PartialEq)]
pub struct LogAttempt {
    /// How it ended.
    pub status: AttemptStatus,
    /// How long it took, in milliseconds.
    pub duration_ms: f64,
    /// The error it ended with.
    pub error: Option<SharedString>,
}

impl LogAttempt {
    /// An attempt that ended `status` after `duration_ms`.
    pub fn new(status: AttemptStatus, duration_ms: f64) -> Self {
        Self {
            status,
            duration_ms: duration_ms.max(0.),
            error: None,
        }
    }

    /// The error it ended with.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }
}

/// One event of a run.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEvent {
    /// A name unique within the log.
    pub id: SharedString,
    /// When it happened, in seconds from the start of the run.
    pub at: f64,
    /// How serious it is.
    pub level: LogLevel,
    /// What part of the run it came from, such as "fetch".
    pub kind: SharedString,
    /// What happened.
    pub message: SharedString,
    /// Each try at its work, first to last.
    pub attempts: Vec<LogAttempt>,
}

impl LogEvent {
    /// An event `at` seconds into the run.
    pub fn new(
        id: impl Into<SharedString>,
        at: f64,
        level: LogLevel,
        kind: impl Into<SharedString>,
        message: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            at: at.max(0.),
            level,
            kind: kind.into(),
            message: message.into(),
            attempts: Vec::new(),
        }
    }

    /// Add an attempt.
    pub fn attempt(mut self, attempt: LogAttempt) -> Self {
        self.attempts.push(attempt);
        self
    }
}

/// What the log reports.
#[derive(Debug, Clone, PartialEq)]
pub enum EventLogEvent {
    /// An event was picked, or the pick cleared.
    Picked(Option<SharedString>),
    /// The brush settled on a window of time, in seconds, or was cleared.
    WindowChanged(Option<(f64, f64)>),
}

/// A round step near `raw`: 1, 2 or 5 times a power of ten.
pub fn nice_step(raw: f64) -> f64 {
    if !raw.is_finite() || raw <= 0. {
        return 1.;
    }
    let power = 10f64.powf(raw.log10().floor());
    let scaled = raw / power;
    let nice = [1., 2., 5., 10.]
        .into_iter()
        .find(|step| scaled <= *step)
        .unwrap_or(10.);
    nice * power
}

/// Where the track ends: past the last event, on a round step.
pub fn span_end(events: &[LogEvent]) -> f64 {
    let last = events.iter().map(|event| event.at).fold(0., f64::max);
    if last <= 0. {
        return 1.;
    }
    let step = nice_step(last / 5.);
    ((last / step).floor() + 1.) * step
}

/// The labelled times along a track that ends at `span`.
pub fn axis_ticks(span: f64) -> Vec<f64> {
    let step = nice_step(span / 5.);
    let count = (span / step).floor() as usize;
    (0..=count).map(|ix| ix as f64 * step).collect()
}

/// A time as a clock: "0:07", "12:30", "1:02:03".
pub fn format_clock(seconds: f64) -> String {
    let whole = seconds.max(0.).floor() as u64;
    let (hours, minutes, secs) = (whole / 3600, whole / 60 % 60, whole % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

/// A time with tenths, for the log: "0:07.4".
pub fn format_stamp(seconds: f64) -> String {
    let tenths = ((seconds.max(0.) * 10.).round() as u64) % 10;
    format!("{}.{tenths}", format_clock((seconds * 10.).round() / 10.))
}

/// A duration, short: "840 ms", "4.2 s", "1 m 4 s".
pub fn format_duration(ms: f64) -> String {
    if ms < 1000. {
        format!("{} ms", ms.round() as i64)
    } else if ms < 60_000. {
        format!("{:.1} s", ms / 1000.)
    } else {
        let secs = (ms / 1000.).round() as i64;
        format!("{} m {} s", secs / 60, secs % 60)
    }
}

/// What a press on the track takes hold of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushGrab {
    /// The window's start edge.
    Start,
    /// The window's end edge.
    End,
    /// The inside of the window, to move it.
    Inside,
    /// Empty track, to draw a new window.
    Outside,
}

/// What a press at `x` takes hold of, given the window's edges in pixels.
pub fn grab_at(window: Option<(f32, f32)>, x: f32) -> BrushGrab {
    match window {
        Some((start, _)) if (x - start).abs() <= GRAB => BrushGrab::Start,
        Some((_, end)) if (x - end).abs() <= GRAB => BrushGrab::End,
        Some((start, end)) if x > start && x < end => BrushGrab::Inside,
        _ => BrushGrab::Outside,
    }
}

fn settle(from: f64, to: f64, span: f64) -> Option<(f64, f64)> {
    let (from, to) = (from.min(to).max(0.), from.max(to).min(span));
    (to - from >= span * MIN_WINDOW).then_some((from, to))
}

/// The window while a drag that took `grab` at time `anchor` is at `now`.
pub fn drag_window(
    grab: BrushGrab,
    anchor: f64,
    now: f64,
    before: Option<(f64, f64)>,
    span: f64,
) -> Option<(f64, f64)> {
    let least = span * MIN_WINDOW;
    match (grab, before) {
        (BrushGrab::Inside, Some(window)) => Some(shift_window(window, now - anchor, span)),
        (BrushGrab::Start, Some((_, end))) => Some((now.clamp(0., end - least), end)),
        (BrushGrab::End, Some((start, _))) => Some((start, now.clamp(start + least, span))),
        _ => settle(anchor, now, span),
    }
}

/// The window moved by `delta` seconds, kept whole inside the run.
pub fn shift_window((start, end): (f64, f64), delta: f64, span: f64) -> (f64, f64) {
    let width = end - start;
    let start = (start + delta).clamp(0., (span - width).max(0.));
    (start, start + width)
}

/// The window with its end moved by `delta` seconds; no window is the whole
/// run. Growing back to the whole run clears it.
pub fn resize_window(window: Option<(f64, f64)>, delta: f64, span: f64) -> Option<(f64, f64)> {
    let (start, end) = window.unwrap_or((0., span));
    let end = (end + delta).clamp(start + span * MIN_WINDOW, span);
    (start > 0. || end < span).then_some((start, end))
}

/// What the log is narrowed to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LogFilter {
    /// Levels left out.
    pub hidden_levels: Vec<LogLevel>,
    /// Kinds left out.
    pub hidden_kinds: Vec<SharedString>,
    /// Text the message, kind or id must hold, in any case.
    pub query: String,
    /// The window of time, in seconds.
    pub window: Option<(f64, f64)>,
}

impl LogFilter {
    /// Whether an event passes the level, kind and text filters.
    pub fn keeps(&self, event: &LogEvent) -> bool {
        let query = self.query.trim().to_lowercase();
        !self.hidden_levels.contains(&event.level)
            && !self.hidden_kinds.contains(&event.kind)
            && (query.is_empty()
                || event.message.to_lowercase().contains(&query)
                || event.kind.to_lowercase().contains(&query)
                || event.id.to_lowercase().contains(&query))
    }

    /// Whether an event falls in the window.
    pub fn in_window(&self, event: &LogEvent) -> bool {
        self.window
            .is_none_or(|(from, to)| event.at >= from && event.at <= to)
    }
}

/// The events the log shows, in time order.
pub fn visible_rows(events: &[LogEvent], filter: &LogFilter) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..events.len())
        .filter(|&ix| filter.keeps(&events[ix]) && filter.in_window(&events[ix]))
        .collect();
    rows.sort_by(|a, b| events[*a].at.total_cmp(&events[*b].at));
    rows
}

/// The event whose dot is under a point of a track `width` wide.
pub fn dot_at(events: &[LogEvent], span: f64, width: f32, x: f32, y: f32) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .map(|(ix, event)| {
            let dx = (event.at / span) as f32 * width - x;
            let dy = LANE_TOP + event.level.lane() as f32 * LANE - y;
            (ix, (dx * dx + dy * dy).sqrt())
        })
        .filter(|(_, distance)| *distance <= GRAB)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(ix, _)| ix)
}

/// Where a key moves the pick among the log's rows. `None`: not a key here.
pub fn step_row(rows: &[usize], now: Option<usize>, key: &str) -> Option<Option<usize>> {
    if rows.is_empty() {
        return None;
    }
    let at = now.and_then(|ix| rows.iter().position(|row| *row == ix));
    let last = rows.len() - 1;
    let next = match key {
        "down" => at.map_or(0, |at| (at + 1).min(last)),
        "up" => at.map_or(last, |at| at.saturating_sub(1)),
        "home" => 0,
        "end" => last,
        "escape" => return Some(None),
        _ => return None,
    };
    Some(Some(rows[next]))
}

/// How an event's attempts went, in a line.
pub fn attempt_outcome(attempts: &[LogAttempt]) -> String {
    let total: f64 = attempts.iter().map(|attempt| attempt.duration_ms).sum();
    let count = attempts.len();
    let plural = if count == 1 { "attempt" } else { "attempts" };
    match attempts.last().map(|attempt| attempt.status) {
        None => "No attempts".into(),
        Some(AttemptStatus::Succeeded) if count == 1 => {
            format!("Succeeded first time in {}", format_duration(total))
        }
        Some(AttemptStatus::Succeeded) => format!(
            "Succeeded on attempt {count} after {}",
            format_duration(total)
        ),
        Some(AttemptStatus::Running) => format!("Running, attempt {count}"),
        Some(_) => format!("Gave up after {count} {plural}, {}", format_duration(total)),
    }
}

/// What a screen reader hears for an event.
pub fn spoken_event(event: &LogEvent) -> String {
    let mut words = format!(
        "{} at {}, {}, {}: {}",
        event.level.name(),
        format_stamp(event.at),
        event.kind,
        event.id,
        event.message
    );
    if !event.attempts.is_empty() {
        words.push_str(&format!(". {}", attempt_outcome(&event.attempts)));
    }
    words
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    grab: BrushGrab,
    anchor: f64,
    anchor_x: f32,
    before: Option<(f64, f64)>,
    dot: Option<usize>,
    moved: bool,
}

/// A run's events over time, with a brush and a filterable log.
pub struct EventLogBrush {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    events: Vec<LogEvent>,
    span: f64,
    kinds: Vec<SharedString>,
    hidden_levels: Vec<LogLevel>,
    hidden_kinds: Vec<SharedString>,
    window: Option<(f64, f64)>,
    picked: Option<usize>,
    hovered_row: Option<usize>,
    drag: Option<Drag>,
    track: Rc<Cell<Bounds<Pixels>>>,
    track_focus: FocusHandle,
    list_focus: FocusHandle,
    scroll: ScrollHandle,
    search: Entity<InputState>,
    _events: Subscription,
}

impl EventEmitter<EventLogEvent> for EventLogBrush {}

impl EventLogBrush {
    /// Show `events`, in any order.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        events: Vec<LogEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Filter messages"));
        let subscription = cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let mut kinds: Vec<SharedString> = Vec::new();
        for event in &events {
            if !kinds.contains(&event.kind) {
                kinds.push(event.kind.clone());
            }
        }
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            span: span_end(&events),
            events,
            kinds,
            hidden_levels: Vec::new(),
            hidden_kinds: Vec::new(),
            window: None,
            picked: None,
            hovered_row: None,
            drag: None,
            track: Rc::new(Cell::new(Bounds::default())),
            track_focus: cx.focus_handle().tab_stop(true),
            list_focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            search,
            _events: subscription,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Narrow the log to a window of time, in seconds.
    pub fn set_window(&mut self, window: Option<(f64, f64)>, cx: &mut Context<Self>) {
        self.window = window.and_then(|(from, to)| settle(from, to, self.span));
        cx.notify();
    }

    /// Pick the event with this id.
    pub fn pick(&mut self, id: &str, cx: &mut Context<Self>) {
        self.picked = self.events.iter().position(|event| event.id == id);
        cx.notify();
    }

    fn filter(&self, cx: &App) -> LogFilter {
        LogFilter {
            hidden_levels: self.hidden_levels.clone(),
            hidden_kinds: self.hidden_kinds.clone(),
            query: self.search.read(cx).value().to_string(),
            window: self.window,
        }
    }

    fn set_pick(&mut self, pick: Option<usize>, cx: &mut Context<Self>) {
        self.picked = pick;
        let id = pick.map(|ix| self.events[ix].id.clone());
        cx.emit(EventLogEvent::Picked(id));
        cx.notify();
    }

    fn change_window(&mut self, window: Option<(f64, f64)>, cx: &mut Context<Self>) {
        if self.window != window {
            self.window = window;
            cx.emit(EventLogEvent::WindowChanged(window));
            cx.notify();
        }
    }

    fn track_point(&self, position: isonomic_ui::Point<Pixels>) -> (f32, f32, f32) {
        let bounds = self.track.get();
        (
            f32::from(position.x - bounds.left()),
            f32::from(position.y - bounds.top()),
            f32::from(bounds.size.width).max(1.),
        )
    }

    fn press(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (x, y, width) = self.track_point(event.position);
        let span = self.span;
        let edges = self
            .window
            .map(|(from, to)| ((from / span) as f32 * width, (to / span) as f32 * width));
        self.drag = Some(Drag {
            grab: grab_at(edges, x),
            anchor: f64::from(x / width) * span,
            anchor_x: x,
            before: self.window,
            dot: dot_at(&self.events, span, width, x, y),
            moved: false,
        });
        self.track_focus.focus(window, cx);
    }

    fn drag_to(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(mut drag) = self.drag else { return };
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let (x, _, width) = self.track_point(event.position);
        if (x - drag.anchor_x).abs() > DRAG_START {
            drag.moved = true;
        }
        self.drag = Some(drag);
        if drag.moved {
            let now = (f64::from(x / width) * self.span).clamp(0., self.span);
            self.window = drag_window(drag.grab, drag.anchor, now, drag.before, self.span);
            cx.notify();
        }
    }

    fn release(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else { return };
        if drag.moved {
            cx.emit(EventLogEvent::WindowChanged(self.window));
            cx.notify();
        } else if let Some(dot) = drag.dot {
            self.set_pick(Some(dot), cx);
        } else if drag.grab == BrushGrab::Outside {
            self.change_window(None, cx);
        }
    }

    fn track_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let step = self.span * KEY_STEP;
        let shift = event.keystroke.modifiers.shift;
        let next = match (event.keystroke.key.as_str(), shift) {
            ("left", true) => resize_window(self.window, -step, self.span),
            ("right", true) => resize_window(self.window, step, self.span),
            ("left", false) => self.window.map(|w| shift_window(w, -step, self.span)),
            ("right", false) => self.window.map(|w| shift_window(w, step, self.span)),
            ("home", _) => self.window.map(|w| shift_window(w, -self.span, self.span)),
            ("end", _) => self.window.map(|w| shift_window(w, self.span, self.span)),
            ("escape", _) => None,
            _ => return false,
        };
        self.change_window(next, cx);
        true
    }

    fn list_key(&mut self, event: &KeyDownEvent, rows: &[usize], cx: &mut Context<Self>) -> bool {
        let Some(next) = step_row(rows, self.picked, event.keystroke.key.as_str()) else {
            return false;
        };
        if let Some(at) = next.and_then(|ix| rows.iter().position(|row| *row == ix)) {
            self.scroll.scroll_to_item(at);
        }
        self.set_pick(next, cx);
        true
    }
}

fn level_color(level: LogLevel, cx: &App) -> Hsla {
    let theme = cx.theme();
    match level {
        LogLevel::Error => theme.danger,
        LogLevel::Warning => theme.warning,
        LogLevel::Info => theme.chart_1,
        LogLevel::Debug => theme.muted_foreground,
    }
}

fn level_tag(level: LogLevel) -> Tag {
    match level {
        LogLevel::Error => Tag::danger(),
        LogLevel::Warning => Tag::warning(),
        LogLevel::Info => Tag::info(),
        LogLevel::Debug => Tag::secondary(),
    }
}

fn paint_dot(window: &mut Window, x: f32, y: f32, radius: f32, color: Hsla) {
    let ring: Vec<_> = (0..14)
        .map(|step| {
            let angle = step as f32 / 14. * TAU;
            point(px(x + radius * angle.cos()), px(y + radius * angle.sin()))
        })
        .collect();
    let mut dot = PathBuilder::fill();
    dot.add_polygon(&ring, true);
    if let Ok(dot) = dot.build() {
        window.paint_path(dot, color);
    }
}

impl Render for EventLogBrush {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.id.clone();
        let part = |name: String| ElementId::NamedChild(id.clone().into(), name.into());
        let filter = self.filter(cx);
        let rows = visible_rows(&self.events, &filter);
        let span = self.span;
        let total = self.events.len();
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let accent = theme.chart_1;
        let surface = theme.background;
        let ring = theme.ring;
        let picked_fill = theme.list_active;
        let picked_border = theme.list_active_border;
        let mono = theme.mono_font_family.clone();
        let wash = look::ink::wash(cx);
        let hairline = look::ink::hairline(cx);

        // Dots: dim when filtered out or outside the window.
        let dots: Vec<(f32, f32, Hsla, f32)> = self
            .events
            .iter()
            .map(|event| {
                let strength = if !filter.keeps(event) {
                    0.12
                } else if !filter.in_window(event) {
                    0.3
                } else {
                    1.
                };
                (
                    (event.at / span) as f32,
                    LANE_TOP + event.level.lane() as f32 * LANE,
                    level_color(event.level, cx),
                    strength,
                )
            })
            .collect();
        let picked_dot = self.picked.map(|ix| dots[ix]);
        let brush = self
            .window
            .map(|(from, to)| ((from / span) as f32, (to / span) as f32));
        let seen = self.track.clone();
        let track_canvas = canvas(
            move |bounds, _, _| seen.set(bounds),
            move |bounds, _, window, _| {
                let width = f32::from(bounds.size.width);
                let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
                for lane in 0..4 {
                    let y = top + LANE_TOP + lane as f32 * LANE;
                    let mut path = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(4.)]);
                    path.move_to(point(px(left), px(y)));
                    path.line_to(point(px(left + width), px(y)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, hairline);
                    }
                }
                if let Some((from, to)) = brush {
                    let (x0, x1) = (left + from * width, left + to * width);
                    window.paint_quad(isonomic_ui::fill(
                        Bounds::from_corners(
                            point(px(x0), px(top)),
                            point(px(x1), px(top + TRACK_H)),
                        ),
                        accent.opacity(0.1),
                    ));
                    for x in [x0, x1] {
                        window.paint_quad(isonomic_ui::fill(
                            Bounds::from_corners(
                                point(px(x - 0.75), px(top)),
                                point(px(x + 0.75), px(top + TRACK_H)),
                            ),
                            accent,
                        ));
                        let mid = top + TRACK_H / 2.;
                        window.paint_quad(
                            isonomic_ui::fill(
                                Bounds::from_corners(
                                    point(px(x - 3.), px(mid - 9.)),
                                    point(px(x + 3.), px(mid + 9.)),
                                ),
                                accent,
                            )
                            .corner_radii(px(3.)),
                        );
                    }
                }
                for &(at, y, color, strength) in &dots {
                    paint_dot(
                        window,
                        left + at * width,
                        top + y,
                        3.5,
                        color.opacity(strength),
                    );
                }
                if let Some((at, y, _, _)) = picked_dot {
                    let (x, y) = (left + at * width, top + y);
                    let mut path = PathBuilder::stroke(px(1.5));
                    for step in 0..=24 {
                        let angle = step as f32 / 24. * TAU;
                        let at = point(px(x + 6.5 * angle.cos()), px(y + 6.5 * angle.sin()));
                        if step == 0 {
                            path.move_to(at);
                        } else {
                            path.line_to(at);
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, ink);
                    }
                }
            },
        )
        .absolute()
        .inset_0();

        let ticks = axis_ticks(span);
        let last_tick = ticks.len().saturating_sub(1);
        let tick_labels = ticks.iter().enumerate().map(|(ix, tick)| {
            div()
                .absolute()
                .top_0()
                .left(relative((tick / span) as f32))
                .w(px(0.))
                .flex()
                .map(|this| {
                    if ix == 0 {
                        this.justify_start()
                    } else if ix == last_tick && *tick >= span {
                        this.justify_end()
                    } else {
                        this.justify_center()
                    }
                })
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .font_family(mono.clone())
                        .text_size(px(10.))
                        .line_height(px(16.))
                        .text_color(quiet)
                        .child(format_clock(*tick)),
                )
        });

        let in_window = self
            .events
            .iter()
            .filter(|event| filter.in_window(event))
            .count();
        let window_line = match self.window {
            Some((from, to)) => format!(
                "Window {} – {} · {in_window} events",
                format_stamp(from),
                format_stamp(to)
            ),
            None => format!(
                "Whole run, {} · drag across the track to narrow it",
                format_clock(span)
            ),
        };
        let track_label = match self.window {
            Some((from, to)) => format!(
                "Time window from {} to {}, {in_window} of {total} events. Arrow keys move it, Shift with arrows resizes it, Escape clears it.",
                format_stamp(from),
                format_stamp(to)
            ),
            None => format!(
                "Timeline of {total} events over {}. No window set: Shift with Left narrows one from the end.",
                format_clock(span)
            ),
        };

        let level_counts: Vec<usize> = LogLevel::ALL
            .iter()
            .map(|level| {
                self.events
                    .iter()
                    .filter(|event| event.level == *level && filter.in_window(event))
                    .count()
            })
            .collect();
        let level_toggles = ToggleGroup::new(part("levels".into()))
            .small()
            .accessibility_label("Levels shown")
            .children(
                LogLevel::ALL
                    .iter()
                    .zip(&level_counts)
                    .map(|(level, count)| {
                        Toggle::new(part(format!("level-{}", level.name())))
                            .label(format!("{} {count}", level.name()))
                            .checked(!self.hidden_levels.contains(level))
                    }),
            )
            .on_click(cx.listener(|this, checks: &Vec<bool>, _, cx| {
                this.hidden_levels = LogLevel::ALL
                    .iter()
                    .zip(checks)
                    .filter(|(_, on)| !**on)
                    .map(|(level, _)| *level)
                    .collect();
                cx.notify();
            }));
        let kind_toggles = ToggleGroup::new(part("kinds".into()))
            .small()
            .accessibility_label("Kinds shown")
            .children(self.kinds.iter().map(|kind| {
                Toggle::new(part(format!("kind-{kind}")))
                    .label(kind.clone())
                    .checked(!self.hidden_kinds.contains(kind))
            }))
            .on_click(cx.listener(|this, checks: &Vec<bool>, _, cx| {
                this.hidden_kinds = this
                    .kinds
                    .iter()
                    .zip(checks)
                    .filter(|(_, on)| !**on)
                    .map(|(kind, _)| kind.clone())
                    .collect();
                cx.notify();
            }));

        let list_focused = self.list_focus.is_focused(window);
        let list_rows: Vec<_> = rows
            .iter()
            .map(|&ix| {
                let event = &self.events[ix];
                let is_picked = self.picked == Some(ix);
                let lit = look::hover_fade(
                    part(format!("row-{}", event.id)),
                    self.hovered_row == Some(ix),
                    window,
                    cx,
                );
                let failures = event
                    .attempts
                    .iter()
                    .filter(|attempt| attempt.status != AttemptStatus::Succeeded)
                    .count();
                h_flex()
                    .id(part(format!("row-{}", event.id)))
                    .test_support()
                    .h(px(30.))
                    .flex_none()
                    .gap_3()
                    .px_3()
                    .rounded(px(8.))
                    .bg(look::mix(wash.opacity(0.), wash, lit))
                    .when(is_picked, |this| {
                        this.bg(picked_fill)
                            .border_1()
                            .border_color(if list_focused {
                                picked_border
                            } else {
                                picked_fill
                            })
                    })
                    .cursor_pointer()
                    .role(Role::ListBoxOption)
                    .aria_selected(is_picked)
                    .aria_label(spoken_event(event))
                    .on_hover(cx.listener(move |this, over: &bool, _, cx| {
                        if *over {
                            this.hovered_row = Some(ix);
                        } else if this.hovered_row == Some(ix) {
                            this.hovered_row = None;
                        }
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.list_focus.focus(window, cx);
                        this.set_pick(Some(ix), cx);
                    }))
                    .child(
                        div()
                            .w(px(52.))
                            .flex_none()
                            .font_family(mono.clone())
                            .text_size(px(12.))
                            .text_color(quiet)
                            .child(format_stamp(event.at)),
                    )
                    .child(
                        h_flex()
                            .w(px(68.))
                            .flex_none()
                            .gap(px(6.))
                            .text_caption()
                            .child(
                                div()
                                    .size(px(6.))
                                    .rounded_full()
                                    .bg(level_color(event.level, cx)),
                            )
                            .child(event.level.name()),
                    )
                    .child(
                        div()
                            .w(px(64.))
                            .flex_none()
                            .truncate()
                            .font_family(mono.clone())
                            .text_size(px(12.))
                            .text_color(quiet)
                            .child(event.kind.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_body()
                            .child(event.message.clone()),
                    )
                    .when(event.attempts.len() > 1, |this| {
                        this.child(
                            div()
                                .flex_none()
                                .font_family(mono.clone())
                                .text_size(px(11.))
                                .text_color(if failures > 0 {
                                    level_color(LogLevel::Warning, cx)
                                } else {
                                    quiet
                                })
                                .child(format!("×{}", event.attempts.len())),
                        )
                    })
            })
            .collect();

        let detail = match self.picked.map(|ix| &self.events[ix]) {
            None => div()
                .mt_3()
                .px_1()
                .text_caption()
                .text_color(quiet)
                .child("Pick an event, in the log or on the track, to read its attempts.")
                .into_any_element(),
            Some(event) => {
                let longest = event
                    .attempts
                    .iter()
                    .map(|attempt| attempt.duration_ms)
                    .fold(1., f64::max);
                let attempts = event.attempts.iter().enumerate().map(|(n, attempt)| {
                    let (tag, tone) = match attempt.status {
                        AttemptStatus::Succeeded => (Tag::success(), cx.theme().success),
                        AttemptStatus::Failed => (Tag::danger(), cx.theme().danger),
                        AttemptStatus::TimedOut => (Tag::warning(), cx.theme().warning),
                        AttemptStatus::Running => (Tag::info(), cx.theme().info),
                    };
                    v_flex()
                        .id(part(format!("attempt-{n}")))
                        .gap_1()
                        .py(px(6.))
                        .role(Role::ListItem)
                        .aria_label(format!(
                            "Attempt {}: {}, {}{}",
                            n + 1,
                            attempt.status.name().to_lowercase(),
                            format_duration(attempt.duration_ms),
                            attempt
                                .error
                                .as_ref()
                                .map(|error| format!(", {error}"))
                                .unwrap_or_default()
                        ))
                        .child(
                            h_flex()
                                .gap_3()
                                .child(
                                    div()
                                        .w(px(72.))
                                        .flex_none()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child(format!("Attempt {}", n + 1)),
                                )
                                .child(
                                    div()
                                        .w(px(76.))
                                        .flex_none()
                                        .child(tag.small().child(attempt.status.name())),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(40.))
                                        .h(px(4.))
                                        .rounded_full()
                                        .bg(hairline)
                                        .child(
                                            div()
                                                .h_full()
                                                .rounded_full()
                                                .w(relative((attempt.duration_ms / longest) as f32))
                                                .bg(tone),
                                        ),
                                )
                                .child(
                                    div()
                                        .w(px(64.))
                                        .flex_none()
                                        .text_right()
                                        .font_family(mono.clone())
                                        .text_size(px(12.))
                                        .child(format_duration(attempt.duration_ms)),
                                ),
                        )
                        .when_some(attempt.error.clone(), |this, error| {
                            this.child(
                                div()
                                    .pl(px(84.))
                                    .font_family(mono.clone())
                                    .text_size(px(12.))
                                    .line_height(px(16.))
                                    .text_color(tone)
                                    .child(error),
                            )
                        })
                });
                inset(cx)
                    .id(part("detail".into()))
                    .mt_3()
                    .p_3()
                    .v_flex()
                    .gap_2()
                    .role(Role::Region)
                    .aria_label(spoken_event(event))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(level_tag(event.level).small().child(event.level.name()))
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_size(px(12.))
                                    .text_color(quiet)
                                    .child(format!(
                                        "{} · {} · {}",
                                        format_stamp(event.at),
                                        event.kind,
                                        event.id
                                    )),
                            ),
                    )
                    .child(div().text_label().child(event.message.clone()))
                    .child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child(attempt_outcome(&event.attempts)),
                    )
                    .when(!event.attempts.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .id(part("attempts".into()))
                                .role(Role::List)
                                .aria_label("Attempts")
                                .children(attempts),
                        )
                    })
                    .into_any_element()
            }
        };

        let shown = rows.len();
        let key_rows = rows;
        let track_focused = self.track_focus.is_focused(window);
        shell(cx)
            .id(self.id.clone())
            .max_w(px(720.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::Activity)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                Some(
                    div()
                        .font_family(mono.clone())
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{shown} of {total} events"))
                        .into_any_element(),
                ),
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(
                        div()
                            .id(part("track".into()))
                            .test_support()
                            .track_focus(&self.track_focus)
                            .relative()
                            .w_full()
                            .h(px(TRACK_H))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(if track_focused {
                                ring
                            } else {
                                surface.opacity(0.)
                            })
                            .role(Role::Slider)
                            .aria_label(track_label)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                    this.press(event, window, cx)
                                }),
                            )
                            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                                this.drag_to(event, cx)
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseUpEvent, _, cx| this.release(cx)),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseUpEvent, _, cx| this.release(cx)),
                            )
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if this.track_key(event, cx) {
                                    cx.stop_propagation();
                                }
                            }))
                            .child(track_canvas),
                    )
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(px(18.))
                            .mt_1()
                            .children(tick_labels),
                    )
                    .child(
                        h_flex()
                            .mt_1()
                            .gap_2()
                            .justify_between()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(window_line),
                            )
                            .when(self.window.is_some(), |this| {
                                this.child(
                                    Button::new(part("clear-window".into()))
                                        .ghost()
                                        .xsmall()
                                        .icon(Icon::new(CatalogIcon::Close))
                                        .label("Clear window")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.change_window(None, cx)
                                        })),
                                )
                            }),
                    ),
            )
            .child(
                panel(cx)
                    .mt(px(6.))
                    .p_3()
                    .v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(level_toggles)
                            .child(div().w(px(1.)).h(px(16.)).bg(hairline))
                            .child(kind_toggles),
                    )
                    .child(
                        Input::new(&self.search)
                            .small()
                            .cleanable(true)
                            .prefix(
                                Icon::new(CatalogIcon::Search)
                                    .size(px(14.))
                                    .text_color(quiet),
                            )
                            .aria_label("Filter the log by text"),
                    )
                    .child(
                        div()
                            .id(part("list".into()))
                            .test_support()
                            .track_focus(&self.list_focus)
                            .max_h(px(248.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .v_flex()
                            .gap(px(2.))
                            .role(Role::ListBox)
                            .aria_label(format!(
                                "Log, {shown} of {total} events. Up and Down move the pick."
                            ))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if this.list_key(event, &key_rows, cx) {
                                    cx.stop_propagation();
                                }
                            }))
                            .when(shown == 0, |this| {
                                this.child(
                                    div()
                                        .py_4()
                                        .text_center()
                                        .text_caption()
                                        .text_color(quiet)
                                        .child("No events match. Widen the window or the filters."),
                                )
                            })
                            .children(list_rows),
                    )
                    .child(detail),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> Vec<LogEvent> {
        vec![
            LogEvent::new("a", 2., LogLevel::Info, "fetch", "Fetched page 1"),
            LogEvent::new(
                "b",
                9.5,
                LogLevel::Warning,
                "fetch",
                "Slow reply from mirror",
            )
            .attempt(LogAttempt::new(AttemptStatus::TimedOut, 5000.).error("no reply"))
            .attempt(LogAttempt::new(AttemptStatus::Succeeded, 1200.)),
            LogEvent::new("c", 4., LogLevel::Error, "store", "Write refused")
                .attempt(LogAttempt::new(AttemptStatus::Failed, 80.).error("disk full")),
            LogEvent::new("d", 31., LogLevel::Debug, "parse", "Parsed 40 rows"),
        ]
    }

    #[test]
    fn the_track_ends_on_a_round_step_past_the_last_event() {
        assert_eq!(nice_step(6.2), 10.);
        assert_eq!(nice_step(0.3), 0.5);
        assert_eq!(span_end(&run()), 40.);
        assert_eq!(span_end(&[]), 1.);
        assert_eq!(axis_ticks(40.), [0., 10., 20., 30., 40.]);
    }

    #[test]
    fn times_and_durations_read_short() {
        assert_eq!(format_clock(7.9), "0:07");
        assert_eq!(format_clock(750.), "12:30");
        assert_eq!(format_clock(3723.), "1:02:03");
        assert_eq!(format_stamp(7.44), "0:07.4");
        assert_eq!(format_stamp(59.96), "1:00.0");
        assert_eq!(format_duration(840.4), "840 ms");
        assert_eq!(format_duration(4210.), "4.2 s");
        assert_eq!(format_duration(64_000.), "1 m 4 s");
    }

    #[test]
    fn a_press_takes_an_edge_the_inside_or_empty_track() {
        let window = Some((100., 200.));
        assert_eq!(grab_at(window, 104.), BrushGrab::Start);
        assert_eq!(grab_at(window, 197.), BrushGrab::End);
        assert_eq!(grab_at(window, 150.), BrushGrab::Inside);
        assert_eq!(grab_at(window, 20.), BrushGrab::Outside);
        assert_eq!(grab_at(None, 150.), BrushGrab::Outside);
    }

    #[test]
    fn drags_draw_move_and_resize_the_window() {
        // Drawn either way across empty track.
        assert_eq!(
            drag_window(BrushGrab::Outside, 30., 10., None, 40.),
            Some((10., 30.))
        );
        // Too narrow to count.
        assert_eq!(drag_window(BrushGrab::Outside, 10., 10.5, None, 40.), None);
        // Moved whole, and stopped at the end of the run.
        let window = Some((10., 20.));
        assert_eq!(
            drag_window(BrushGrab::Inside, 15., 18., window, 40.),
            Some((13., 23.))
        );
        assert_eq!(
            drag_window(BrushGrab::Inside, 15., 60., window, 40.),
            Some((30., 40.))
        );
        // An edge stops short of the other edge.
        assert_eq!(
            drag_window(BrushGrab::Start, 10., 30., window, 40.),
            Some((19.2, 20.))
        );
        assert_eq!(
            drag_window(BrushGrab::End, 20., 35., window, 40.),
            Some((10., 35.))
        );
        assert_eq!(shift_window((10., 20.), -15., 40.), (0., 10.));
        // Resizing from no window narrows the whole run from its end; growing
        // back to the whole run clears it.
        assert_eq!(resize_window(None, -2., 40.), Some((0., 38.)));
        assert_eq!(resize_window(Some((0., 38.)), 2., 40.), None);
        assert_eq!(resize_window(Some((5., 6.)), -5., 40.), Some((5., 5.8)));
    }

    #[test]
    fn filters_narrow_the_log_in_time_order() {
        let events = run();
        let mut filter = LogFilter::default();
        assert_eq!(visible_rows(&events, &filter), [0, 2, 1, 3]);
        filter.hidden_levels = vec![LogLevel::Debug];
        filter.query = "SLOW".into();
        assert_eq!(visible_rows(&events, &filter), [1]);
        filter.query = "store".into();
        assert_eq!(visible_rows(&events, &filter), [2]);
        filter.query.clear();
        filter.hidden_kinds = vec!["fetch".into()];
        assert_eq!(visible_rows(&events, &filter), [2]);
        filter.hidden_kinds.clear();
        filter.window = Some((3., 10.));
        assert_eq!(visible_rows(&events, &filter), [2, 1]);
    }

    #[test]
    fn a_dot_is_found_in_its_lane() {
        let events = run();
        // 400 wide over 40 s: 10 px a second. "c" is an error at 4 s, top lane.
        assert_eq!(dot_at(&events, 40., 400., 41., LANE_TOP + 1.), Some(2));
        // "b" is a warning at 9.5 s, second lane.
        assert_eq!(dot_at(&events, 40., 400., 95., LANE_TOP + LANE), Some(1));
        assert_eq!(dot_at(&events, 40., 400., 95., LANE_TOP), None);
        assert_eq!(LogLevel::Debug.lane(), 3);
    }

    #[test]
    fn keys_step_through_the_rows() {
        let rows = [0, 2, 1];
        assert_eq!(step_row(&rows, None, "down"), Some(Some(0)));
        assert_eq!(step_row(&rows, Some(0), "down"), Some(Some(2)));
        assert_eq!(step_row(&rows, Some(1), "down"), Some(Some(1)));
        assert_eq!(step_row(&rows, None, "up"), Some(Some(1)));
        assert_eq!(step_row(&rows, Some(2), "home"), Some(Some(0)));
        assert_eq!(step_row(&rows, Some(2), "escape"), Some(None));
        assert_eq!(step_row(&rows, Some(2), "x"), None);
        assert_eq!(step_row(&[], None, "down"), None);
    }

    #[test]
    fn attempts_and_events_read_in_words() {
        let events = run();
        assert_eq!(
            attempt_outcome(&events[1].attempts),
            "Succeeded on attempt 2 after 6.2 s"
        );
        assert_eq!(
            attempt_outcome(&events[2].attempts),
            "Gave up after 1 attempt, 80 ms"
        );
        assert_eq!(attempt_outcome(&[]), "No attempts");
        assert_eq!(
            attempt_outcome(&[LogAttempt::new(AttemptStatus::Succeeded, 300.)]),
            "Succeeded first time in 300 ms"
        );
        assert_eq!(
            spoken_event(&events[2]),
            "Error at 0:04.0, store, c: Write refused. Gave up after 1 attempt, 80 ms"
        );
    }
}
