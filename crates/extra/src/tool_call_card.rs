// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Tool call card: one tool call of an assistant on a single line.
//!
//! The line shows the tool's name, a short summary and how the call stands: a
//! live timer while it runs, the time it took, or that it failed. It folds open
//! to the arguments and the result, each with a copy button, and to the error
//! with a retry. [`ToolCallGroup`] folds the calls of one turn into one quiet
//! line.

use std::{
    hash::{DefaultHasher, Hash as _, Hasher as _},
    ops::Range,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use isonomic_ui::base::Button as Pressable;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Rope, Sizable as _, StyledExt as _, ThemeStyled as _,
    button::{Button, ButtonVariants as _},
    clipboard::Clipboard,
    h_flex,
    highlighter::SyntaxHighlighter,
    shimmer::ShimmerText,
    spinner::Spinner,
    v_flex,
};
use isonomic_ui::{
    App, ClickEvent, Context, ElementId, HighlightStyle, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement,
    Styled, StyledText, Task, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::resting_shadow;

/// How often a running call redraws its timer.
const TICK: Duration = Duration::from_millis(100);
/// Code longer than this many lines is capped until "Show all" is pressed.
const CAPPED_LINES: usize = 12;

/// How a tool call stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    /// Still running.
    Running,
    /// Returned a result.
    Success,
    /// Failed.
    Failed,
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type OpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type Styles = Vec<(Range<usize>, HighlightStyle)>;

/// One tool call, folded to a line.
#[derive(IntoElement)]
pub struct ToolCallCard {
    id: ElementId,
    style: StyleRefinement,
    name: SharedString,
    status: ToolCallStatus,
    summary: Option<SharedString>,
    icon: IconName,
    arguments: Option<SharedString>,
    result: Option<SharedString>,
    error: Option<SharedString>,
    started_at: Option<Instant>,
    duration: Option<Duration>,
    default_open: bool,
    open: Option<bool>,
    open_on_error: bool,
    on_open_change: Option<OpenHandler>,
    on_retry: Option<Handler>,
}

impl ToolCallCard {
    /// Create a card for a call, with the tool's name as the assistant calls it.
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        status: ToolCallStatus,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            name: name.into(),
            status,
            summary: None,
            icon: IconName::SquareTerminal,
            arguments: None,
            result: None,
            error: None,
            started_at: None,
            duration: None,
            default_open: false,
            open: None,
            open_on_error: true,
            on_open_change: None,
            on_retry: None,
        }
    }

    /// A short line in plain words: "Searching the ledger" while running,
    /// "Searched the ledger" when done.
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Set the icon on the left.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }

    /// The arguments the tool was called with. JSON is laid out and coloured;
    /// other text is shown as it is.
    pub fn arguments(mut self, arguments: impl Into<SharedString>) -> Self {
        self.arguments = Some(arguments.into());
        self
    }

    /// What the tool returned, treated like the arguments.
    pub fn result(mut self, result: impl Into<SharedString>) -> Self {
        self.result = Some(result.into());
        self
    }

    /// The error message, shown when the call failed.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// When the call started. Without it the timer counts from when the card
    /// first showed the call running.
    pub fn started_at(mut self, started_at: Instant) -> Self {
        self.started_at = Some(started_at);
        self
    }

    /// How long the call took. Without it the card shows the time it measured.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Start folded open.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Control whether the card is open. Pair it with `on_open_change`.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Whether a failed call opens by itself and closes again on the next run,
    /// unless someone toggled it by hand. On by default.
    pub fn open_on_error(mut self, open: bool) -> Self {
        self.open_on_error = open;
        self
    }

    /// Run when the line is pressed, with the state it asks for.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Show a Retry button next to the error.
    pub fn on_retry(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }
}

impl Styled for ToolCallCard {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A duration the way a status shows it. A live timer always shows tenths of a
/// second below a minute; a finished call under a second shows milliseconds.
fn format_duration(elapsed: Duration, live: bool) -> String {
    let millis = elapsed.as_millis();
    if !live && millis < 1000 {
        return format!("{}ms", millis.max(1));
    }
    if millis < 60_000 {
        return format!("{}.{}s", millis / 1000, millis % 1000 / 100);
    }
    let seconds = elapsed.as_secs();
    let (minutes, hours) = (seconds / 60, seconds / 3600);
    if hours == 0 {
        format!("{minutes}m {:02}s", seconds % 60)
    } else if hours < 24 {
        format!("{hours}h {:02}m", minutes % 60)
    } else {
        format!("{}d {}h", hours / 24, hours % 24)
    }
}

/// Lay JSON out with one value per line. Returns `None` for text that does not
/// start like JSON or whose brackets do not close; that text is shown as it is.
fn pretty_json(text: &str) -> Option<String> {
    let text = text.trim();
    if !text.starts_with(['{', '[']) {
        return None;
    }
    let new_line = |out: &mut String, depth: usize| {
        out.push('\n');
        out.extend(std::iter::repeat_n("  ", depth));
    };
    let mut out = String::with_capacity(text.len() * 2);
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            '{' | '[' => {
                out.push(ch);
                while chars.peek().is_some_and(|next| next.is_whitespace()) {
                    chars.next();
                }
                // An empty object or list stays on one line.
                let close = if ch == '{' { '}' } else { ']' };
                if chars.peek() == Some(&close) {
                    out.push(close);
                    chars.next();
                } else {
                    depth += 1;
                    new_line(&mut out, depth);
                }
            }
            '}' | ']' => {
                depth = depth.checked_sub(1)?;
                new_line(&mut out, depth);
                out.push(ch);
            }
            ',' => {
                out.push(ch);
                new_line(&mut out, depth);
            }
            ':' => out.push_str(": "),
            ch if ch.is_whitespace() => {}
            ch => out.push(ch),
        }
    }
    (depth == 0 && !in_string).then_some(out)
}

/// What opening and closing by itself does when the status changes: a failed
/// call opens, and closes again on the next run if nobody touched it since.
/// Returns whether the card is open and whether it opened by itself.
fn after_status_change(
    previous: ToolCallStatus,
    next: ToolCallStatus,
    open: bool,
    auto_opened: bool,
) -> (bool, bool) {
    if next == ToolCallStatus::Failed && !open {
        (true, true)
    } else if previous == ToolCallStatus::Failed && auto_opened {
        (false, false)
    } else {
        (open, auto_opened)
    }
}

/// The time all calls took together, if every one of them says how long it took.
fn total_duration(durations: &[Option<Duration>]) -> Option<Duration> {
    if durations.is_empty() {
        return None;
    }
    durations.iter().copied().sum()
}

fn count_label(count: usize) -> String {
    match count {
        1 => "Used 1 tool".into(),
        n => format!("Used {n} tools"),
    }
}

/// What a screen reader hears for a call's header: the tool, what it does and
/// where it stands, as in "ledger.search, Search March, done in 1.4s".
fn call_name(
    name: &str,
    summary: Option<&str>,
    status: ToolCallStatus,
    time: Option<&str>,
) -> String {
    let state = match (status, time) {
        (ToolCallStatus::Running, Some(time)) => format!("running, {time}"),
        (ToolCallStatus::Running, None) => "running".into(),
        (ToolCallStatus::Success, Some(time)) => format!("done in {time}"),
        (ToolCallStatus::Success, None) => "done".into(),
        (ToolCallStatus::Failed, _) => "failed".into(),
    };
    [Some(name), summary, Some(state.as_str())]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ")
}

/// Redraw on every tick for as long as the task is kept.
fn ticker<T: 'static>(cx: &mut Context<T>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(TICK).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    })
}

/// Arguments or a result, laid out and coloured once per text and theme.
#[derive(Default)]
struct Code {
    key: u64,
    text: SharedString,
    styles: Styles,
}

struct CardState {
    open: bool,
    /// The card opened by itself because the call failed.
    auto_opened: bool,
    last_status: ToolCallStatus,
    started: Instant,
    measured: Option<Duration>,
    /// Whether the arguments and the result show in full.
    show_all: [bool; 2],
    code: [Code; 2],
    ticker: Option<Task<()>>,
}

/// One labelled block of code with a copy button, capped when it is long.
fn code_section(
    id: ElementId,
    label: &'static str,
    text: SharedString,
    styles: Styles,
    show_all: bool,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let quiet = theme.muted_foreground;
    let panel = theme.foreground.opacity(0.04);
    let radius = theme.radius;
    let mono = theme.mono_font_family.clone();
    let is_long = text.lines().count() > CAPPED_LINES;

    v_flex()
        .gap_1()
        .child(
            h_flex()
                .justify_between()
                .child(
                    div()
                        .text_caption()
                        .font_medium()
                        .text_color(quiet)
                        .child(label),
                )
                .child(Clipboard::new((id.clone(), "copy")).value(text.clone())),
        )
        .child(
            div()
                .p_2p5()
                .rounded(radius)
                .bg(panel)
                .font_family(mono)
                .text_caption()
                .line_height(relative(1.5))
                .when(is_long && !show_all, |this| {
                    this.max_h(px(CAPPED_LINES as f32 * 18. + 20.))
                        .overflow_hidden()
                })
                .child(StyledText::new(text).with_highlights(styles)),
        )
        .when(is_long, |this| {
            this.child(
                div().child(
                    Button::new((id, "more"))
                        .ghost()
                        .xsmall()
                        .label(if show_all { "Show less" } else { "Show all" })
                        .on_click(on_toggle),
                ),
            )
        })
}

impl RenderOnce for ToolCallCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let accent = theme.info;
        let danger = theme.danger;
        let radius = theme.radius;
        let mono = theme.mono_font_family.clone();
        let highlight_theme = theme.highlight_theme.clone();

        let status = self.status;
        let opens_itself = self.open_on_error && status == ToolCallStatus::Failed;
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| CardState {
            open: self.default_open || opens_itself,
            auto_opened: !self.default_open && opens_itself,
            last_status: status,
            started: Instant::now(),
            measured: None,
            show_all: [false; 2],
            code: Default::default(),
            ticker: None,
        });
        let (open, elapsed, show_all) = state.update(cx, |state, cx| {
            if state.last_status != status {
                if status == ToolCallStatus::Running {
                    state.started = Instant::now();
                    state.measured = None;
                } else if state.last_status == ToolCallStatus::Running {
                    state.measured = Some(state.started.elapsed());
                }
                if self.open_on_error {
                    (state.open, state.auto_opened) = after_status_change(
                        state.last_status,
                        status,
                        state.open,
                        state.auto_opened,
                    );
                }
                state.last_status = status;
            }
            if status == ToolCallStatus::Running {
                if let Some(started_at) = self.started_at {
                    state.started = started_at;
                }
                if state.ticker.is_none() {
                    state.ticker = Some(ticker(cx));
                }
                (state.open, Some(state.started.elapsed()), state.show_all)
            } else {
                state.ticker = None;
                (state.open, state.measured, state.show_all)
            }
        });
        let open = self.open.unwrap_or(open);

        // Lay out and colour the code only while it shows.
        let mut code: [Option<(SharedString, Styles)>; 2] = [None, None];
        if open {
            for (ix, raw) in [&self.arguments, &self.result].into_iter().enumerate() {
                let Some(raw) = raw else {
                    continue;
                };
                let key = {
                    let mut hasher = DefaultHasher::new();
                    raw.hash(&mut hasher);
                    (Arc::as_ptr(&highlight_theme) as usize).hash(&mut hasher);
                    hasher.finish()
                };
                code[ix] = Some(state.update(cx, |state, _| {
                    let cached = &mut state.code[ix];
                    if cached.key != key {
                        let (text, styles) = match pretty_json(raw) {
                            Some(text) => {
                                let mut highlighter = SyntaxHighlighter::new("json");
                                highlighter.update(None, &Rope::from_str(&text), None);
                                let styles =
                                    highlighter.styles(&(0..text.len()), highlight_theme.as_ref());
                                (text, styles)
                            }
                            None => (raw.to_string(), Vec::new()),
                        };
                        *cached = Code {
                            key,
                            text: text.into(),
                            styles,
                        };
                    }
                    (cached.text.clone(), cached.styles.clone())
                }));
            }
        }

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);
        let time = match status {
            ToolCallStatus::Running => elapsed.map(|elapsed| format_duration(elapsed, true)),
            ToolCallStatus::Success => self
                .duration
                .or(elapsed)
                .map(|elapsed| format_duration(elapsed, false)),
            ToolCallStatus::Failed => Some("Failed".into()),
        };

        let head = Pressable::new((self.id.clone(), "head"))
            .accessibility_label(call_name(
                &self.name,
                self.summary.as_deref(),
                status,
                time.as_deref(),
            ))
            .aria_expanded(open)
            .track_focus(&focus)
            .on_click({
                let state = state.clone();
                let on_open_change = self.on_open_change.clone();
                let controlled = self.open.is_some();
                move |_, window, cx| {
                    if !controlled {
                        state.update(cx, |state, cx| {
                            state.open = !open;
                            state.auto_opened = false;
                            cx.notify();
                        });
                    }
                    if let Some(handler) = &on_open_change {
                        handler(!open, window, cx);
                    }
                }
            })
            .justify_start()
            .gap_2p5()
            .w_full()
            .h(px(44.))
            .px_3()
            .rounded(radius)
            .cursor_pointer()
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .child(Icon::new(self.icon).small().flex_none().text_color(quiet))
            .child(
                div()
                    .flex_none()
                    .max_w(relative(0.5))
                    .truncate()
                    .font_family(mono)
                    .text_size(px(12.))
                    .font_medium()
                    .child(self.name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(quiet)
                    .children(self.summary),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1p5()
                    .map(|this| match status {
                        ToolCallStatus::Running => this.child(Spinner::new().small().color(accent)),
                        ToolCallStatus::Success => {
                            this.child(Icon::new(IconName::CircleCheck).small().text_color(accent))
                        }
                        ToolCallStatus::Failed => {
                            this.child(Icon::new(IconName::CircleAlert).small().text_color(danger))
                        }
                    })
                    .children(time.map(|time| {
                        div()
                            .text_caption()
                            .text_color(if status == ToolCallStatus::Failed {
                                danger
                            } else {
                                quiet
                            })
                            .child(time)
                    })),
            )
            .child(
                Icon::new(if open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .xsmall()
                .flex_none()
                .text_color(quiet),
            );

        let body = open.then(|| {
            let [arguments, result] = code;
            let has_arguments = arguments.is_some();
            let has_result = result.is_some();
            let toggle = |ix: usize| {
                let state = state.clone();
                move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    state.update(cx, |state, cx| {
                        state.show_all[ix] = !state.show_all[ix];
                        cx.notify();
                    })
                }
            };
            let note = |text: &'static str| div().text_color(quiet).child(text);

            v_flex()
                .gap_3()
                .px_3()
                .pb_3()
                .when(status == ToolCallStatus::Failed, |this| {
                    let message = self.error.unwrap_or_else(|| "The tool call failed.".into());
                    this.child(
                        h_flex()
                            .items_start()
                            .gap_2()
                            .child(
                                Icon::new(IconName::CircleAlert)
                                    .small()
                                    .flex_none()
                                    .mt_0p5()
                                    .text_color(danger),
                            )
                            .child(div().flex_1().min_w_0().child(message))
                            .when_some(self.on_retry, |this, on_retry| {
                                this.child(
                                    Button::new((self.id.clone(), "retry"))
                                        .outline()
                                        .xsmall()
                                        .icon(IconName::RefreshCw)
                                        .label("Retry")
                                        .on_click(move |_, window, cx| on_retry(window, cx)),
                                )
                            }),
                    )
                })
                .when_some(arguments, |this, (text, styles)| {
                    let id = ElementId::from((self.id.clone(), "arguments"));
                    this.child(code_section(
                        id,
                        "Arguments",
                        text,
                        styles,
                        show_all[0],
                        toggle(0),
                        cx,
                    ))
                })
                .when_some(result, |this, (text, styles)| {
                    let id = ElementId::from((self.id.clone(), "result"));
                    this.child(code_section(
                        id,
                        "Result",
                        text,
                        styles,
                        show_all[1],
                        toggle(1),
                        cx,
                    ))
                })
                .when(!has_result && status == ToolCallStatus::Running, |this| {
                    this.child(note("Waiting for the result…"))
                })
                .when(
                    !has_arguments && !has_result && status == ToolCallStatus::Success,
                    |this| this.child(note("No arguments or result.")),
                )
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .rounded(radius)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .shadow(resting_shadow(cx))
            .child(head)
            .children(body)
            .refine_style(&self.style)
    }
}

/// The tool calls of one turn, folded into one line that sums them up.
#[derive(IntoElement)]
pub struct ToolCallGroup {
    id: ElementId,
    style: StyleRefinement,
    calls: Vec<ToolCallCard>,
    label: Option<SharedString>,
    running_label: SharedString,
    duration: Option<Duration>,
    default_open: bool,
}

impl ToolCallGroup {
    /// Create an empty group, folded shut.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            calls: Vec::new(),
            label: None,
            running_label: "Using tools…".into(),
            duration: None,
            default_open: false,
        }
    }

    /// Add a call after the ones already added.
    pub fn call(mut self, call: ToolCallCard) -> Self {
        self.calls.push(call);
        self
    }

    /// Set the label shown once nothing is running. Defaults to "Used 3 tools".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the label shown while any call is running.
    pub fn running_label(mut self, label: impl Into<SharedString>) -> Self {
        self.running_label = label.into();
        self
    }

    /// Set the total time. Without it the group adds up the calls' durations,
    /// or shows the time it measured while calls were running.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Start folded open.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }
}

impl Styled for ToolCallGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct GroupState {
    open: bool,
    /// When the calls that are running now started.
    started: Option<Instant>,
    /// Time spent with calls running in earlier stretches.
    accumulated: Duration,
    ticker: Option<Task<()>>,
}

impl RenderOnce for ToolCallGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let subtle = isonomic_ui::component::look::ink::subtle(cx);
        let faint = isonomic_ui::component::look::ink::faint(cx);
        let raised = theme.popover;
        let danger = theme.danger;
        let radius = theme.radius;

        let is_running = |call: &&ToolCallCard| call.status == ToolCallStatus::Running;
        let running = self.calls.iter().any(|call| is_running(&call));
        let failed = self
            .calls
            .iter()
            .filter(|call| call.status == ToolCallStatus::Failed)
            .count();
        let earliest = self
            .calls
            .iter()
            .filter(is_running)
            .filter_map(|call| call.started_at)
            .min();
        let durations: Vec<_> = self.calls.iter().map(|call| call.duration).collect();
        let icons: Vec<_> = self
            .calls
            .iter()
            .rev()
            .take(3)
            .rev()
            .map(|call| call.icon.clone())
            .collect();

        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| GroupState {
            open: self.default_open,
            started: None,
            accumulated: Duration::ZERO,
            ticker: None,
        });
        let (open, measured) = state.update(cx, |state, cx| {
            if running {
                // Count from the earliest running call that knows when it
                // started, so the group never shows less than its calls.
                if state.started.is_none() {
                    state.started = Some(earliest.unwrap_or_else(Instant::now));
                }
                if state.ticker.is_none() {
                    state.ticker = Some(ticker(cx));
                }
            } else {
                if let Some(started) = state.started.take() {
                    state.accumulated += started.elapsed();
                }
                state.ticker = None;
            }
            let live = state
                .started
                .map_or(Duration::ZERO, |started| started.elapsed());
            (state.open, state.accumulated + live)
        });
        let total = if running {
            Some(measured)
        } else {
            self.duration
                .or_else(|| total_duration(&durations))
                .or_else(|| (!measured.is_zero()).then_some(measured))
        };

        let focus = window
            .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let is_focused = focus.is_focused(window);
        let title = if running {
            self.running_label.clone()
        } else {
            self.label
                .clone()
                .unwrap_or_else(|| count_label(self.calls.len()).into())
        };

        // Read as one line: "Used 4 tools, 1 failed, 3.1s".
        let name = [
            Some(title.to_string()),
            (!running && failed > 0).then(|| format!("{failed} failed")),
            total.map(|total| format_duration(total, running)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
        let head = Pressable::new((self.id.clone(), "head"))
            .accessibility_label(name)
            .aria_expanded(open)
            .track_focus(&focus)
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |state, cx| {
                        state.open = !state.open;
                        cx.notify();
                    })
                }
            })
            .justify_start()
            .gap_2()
            .h(px(32.))
            .pr_1()
            .rounded(radius)
            .cursor_pointer()
            .when(is_focused, |this| this.focus_ring_style(window, cx))
            .child(
                // The tools overlap like a hand of cards: raised tiles, each
                // tucked 4px under the next.
                h_flex()
                    .flex_none()
                    .children(icons.into_iter().enumerate().map(|(ix, icon)| {
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(22.))
                            .when(ix > 0, |this| this.ml(px(-4.)))
                            .rounded(px(6.))
                            .bg(raised)
                            .shadow(resting_shadow(cx))
                            .text_color(quiet)
                            .child(Icon::new(icon).xsmall())
                    })),
            )
            .map(|this| {
                if running {
                    this.child(ShimmerText::new(title).id((self.id.clone(), "title")))
                } else {
                    this.child(div().font_medium().text_color(subtle).child(title))
                }
            })
            .when(!running && failed > 0, |this| {
                this.child(div().text_color(danger).child(format!("· {failed} failed")))
            })
            .children(total.map(|total| {
                div()
                    .text_color(faint)
                    .child(format!("· {}", format_duration(total, running)))
            }))
            .child(
                Icon::new(if open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .xsmall()
                .flex_none()
                .text_color(quiet),
            );

        v_flex()
            .id(self.id.clone())
            .w_full()
            .gap_1p5()
            .text_color(ink)
            .text_body()
            .line_height(relative(1.45))
            .child(h_flex().child(head))
            .when(open, |this| {
                this.child(v_flex().gap_2().children(self.calls))
            })
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_header_reads_tool_summary_and_state() {
        assert_eq!(
            call_name(
                "ledger.search",
                Some("Search March"),
                ToolCallStatus::Success,
                Some("1.4s")
            ),
            "ledger.search, Search March, done in 1.4s"
        );
        assert_eq!(
            call_name("rates.fetch", None, ToolCallStatus::Failed, Some("Failed")),
            "rates.fetch, failed"
        );
        assert_eq!(
            call_name("post", None, ToolCallStatus::Running, None),
            "post, running"
        );
    }

    #[test]
    fn durations_read_in_the_right_unit() {
        assert_eq!(format_duration(Duration::from_millis(380), false), "380ms");
        assert_eq!(format_duration(Duration::from_millis(380), true), "0.3s");
        assert_eq!(format_duration(Duration::ZERO, false), "1ms");
        assert_eq!(
            format_duration(Duration::from_millis(12_340), false),
            "12.3s"
        );
        assert_eq!(format_duration(Duration::from_secs(125), false), "2m 05s");
        assert_eq!(
            format_duration(Duration::from_secs(3 * 3600 + 12 * 60), false),
            "3h 12m"
        );
        assert_eq!(
            format_duration(Duration::from_secs(52 * 3600), false),
            "2d 4h"
        );
    }

    #[test]
    fn json_is_laid_out_one_value_per_line() {
        let pretty = pretty_json(r#"{"query":"vat, 2026","limit":5,"tags":[],"deep":{"a":[1,2]}}"#);
        let expected = [
            "{",
            "  \"query\": \"vat, 2026\",",
            "  \"limit\": 5,",
            "  \"tags\": [],",
            "  \"deep\": {",
            "    \"a\": [",
            "      1,",
            "      2",
            "    ]",
            "  }",
            "}",
        ]
        .join("\n");
        assert_eq!(pretty.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn text_that_is_not_json_is_left_alone() {
        assert_eq!(pretty_json("3 rows updated"), None);
        assert_eq!(pretty_json("{\"open\": [1, 2"), None);
        assert_eq!(pretty_json("]"), None);
        // Brackets and quotes inside strings do not count.
        assert_eq!(
            pretty_json(r#"["a \"}\" b"]"#).as_deref(),
            Some("[\n  \"a \\\"}\\\" b\"\n]")
        );
    }

    #[test]
    fn a_failed_call_opens_and_closes_by_itself() {
        use ToolCallStatus::{Failed, Running, Success};
        // Failing opens a closed card and remembers that it did.
        assert_eq!(
            after_status_change(Running, Failed, false, false),
            (true, true)
        );
        // The next run closes it again.
        assert_eq!(
            after_status_change(Failed, Running, true, true),
            (false, false)
        );
        // A card someone opened by hand stays open.
        assert_eq!(
            after_status_change(Failed, Running, true, false),
            (true, false)
        );
        assert_eq!(
            after_status_change(Running, Success, false, false),
            (false, false)
        );
    }

    #[test]
    fn a_group_adds_up_only_complete_durations() {
        let second = Some(Duration::from_secs(1));
        assert_eq!(
            total_duration(&[second, second]),
            Some(Duration::from_secs(2))
        );
        assert_eq!(total_duration(&[second, None]), None);
        assert_eq!(total_duration(&[]), None);
        assert_eq!(count_label(1), "Used 1 tool");
        assert_eq!(count_label(3), "Used 3 tools");
    }
}
