// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Trace timeline: a run's spans as a folding tree beside bars on a shared time axis, with time, tokens and cost per step.
//!
//! The left side is the span tree: each span indented under the one that
//! started it, foldable. The right side lines each span up as a bar on one
//! time axis, so overlaps and waits show at a glance; running spans reach
//! the live edge. Each row ends with its duration, tokens and cost. The
//! selected span's figures and share of the whole show underneath.
//!
//! Opt-in extras: a flame graph view (every span a block, stacked by
//! depth, width by time), zooming into one span so the axis spans only its
//! time, with a trail back out, and a breakdown strip that splits the time,
//! tokens or cost of the zoomed span across the steps it started.

use std::rc::Rc;

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex, look,
    tab::{Tab, TabBar},
    tooltip::Tooltip,
    v_flex,
};
use isonomic_ui::{
    App, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::common::{Tone, resting_shadow};

/// One span of a trace.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceSpan {
    /// Identifies it.
    pub id: SharedString,
    /// What it was.
    pub label: SharedString,
    /// When it started, in milliseconds from the start of the run.
    pub start: u32,
    /// When it ended, or `None` while running.
    pub end: Option<u32>,
    /// Tokens it used on its own.
    pub tokens: u32,
    /// What it cost on its own, in hundredths of a cent.
    pub cost: u32,
    /// Whether it failed.
    pub failed: bool,
    /// Spans it started.
    pub children: Vec<TraceSpan>,
}

impl TraceSpan {
    /// Create a finished span.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        start: u32,
        end: u32,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            start,
            end: Some(end.max(start)),
            tokens: 0,
            cost: 0,
            failed: false,
            children: Vec::new(),
        }
    }

    /// Mark it as still running.
    pub fn running(mut self) -> Self {
        self.end = None;
        self
    }

    /// Mark it as failed.
    pub fn failed(mut self) -> Self {
        self.failed = true;
        self
    }

    /// Set its own tokens and cost in hundredths of a cent.
    pub fn usage(mut self, tokens: u32, cost: u32) -> Self {
        self.tokens = tokens;
        self.cost = cost;
        self
    }

    /// Add a span it started.
    pub fn child(mut self, span: TraceSpan) -> Self {
        self.children.push(span);
        self
    }

    /// Its tokens and cost including everything it started.
    pub fn totals(&self) -> (u64, u64) {
        self.children
            .iter()
            .fold((self.tokens as u64, self.cost as u64), |(t, c), child| {
                let (ct, cc) = child.totals();
                (t + ct, c + cc)
            })
    }
}

/// A visible row: the span's id and depth.
pub fn trace_rows<'a>(
    spans: &'a [TraceSpan],
    open: &[SharedString],
) -> Vec<(&'a TraceSpan, usize)> {
    fn walk<'a>(
        spans: &'a [TraceSpan],
        open: &[SharedString],
        depth: usize,
        out: &mut Vec<(&'a TraceSpan, usize)>,
    ) {
        for span in spans {
            out.push((span, depth));
            if open.contains(&span.id) {
                walk(&span.children, open, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(spans, open, 0, &mut out);
    out
}

/// Write milliseconds short: "640 ms", "2.4 s", "1 m 5 s".
pub fn span_time(ms: u32) -> String {
    match ms {
        0..=999 => format!("{ms} ms"),
        1_000..=59_999 => format!("{:.1} s", ms as f32 / 1000.),
        _ => format!("{} m {} s", ms / 60_000, (ms % 60_000) / 1000),
    }
}

/// How the spans are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TraceView {
    /// The span tree beside bars on a time axis.
    #[default]
    Timeline,
    /// Every span a block, stacked by depth, width by time.
    Flame,
}

impl TraceView {
    /// Both views, in switch order.
    pub const EVERY: [TraceView; 2] = [TraceView::Timeline, TraceView::Flame];

    /// Its name.
    pub fn words(self) -> &'static str {
        match self {
            TraceView::Timeline => "Timeline",
            TraceView::Flame => "Flame graph",
        }
    }
}

/// What the breakdown splits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TraceMetric {
    /// Wall time.
    #[default]
    Time,
    /// Tokens used.
    Tokens,
    /// Money spent.
    Cost,
}

impl TraceMetric {
    /// All three, in switch order.
    pub const EVERY: [TraceMetric; 3] = [TraceMetric::Time, TraceMetric::Tokens, TraceMetric::Cost];

    /// Its name.
    pub fn words(self) -> &'static str {
        match self {
            TraceMetric::Time => "Time",
            TraceMetric::Tokens => "Tokens",
            TraceMetric::Cost => "Cost",
        }
    }

    /// Write an amount of it.
    pub fn amount(self, value: u64) -> String {
        match self {
            TraceMetric::Time => span_time(value.min(u32::MAX as u64) as u32),
            TraceMetric::Tokens => token_words(value),
            TraceMetric::Cost => cost_words(value),
        }
    }
}

fn token_words(tokens: u64) -> String {
    if tokens >= 1000 {
        format!("{:.1}k", tokens as f64 / 1000.)
    } else {
        tokens.to_string()
    }
}

fn cost_words(cost: u64) -> String {
    let cents = cost as f64 / 100.;
    if cents < 100. {
        format!("{cents:.1}\u{a2}")
    } else {
        format!("${:.2}", cents / 100.)
    }
}

fn find<'a>(spans: &'a [TraceSpan], id: &str) -> Option<&'a TraceSpan> {
    spans.iter().find_map(|s| {
        if s.id.as_ref() == id {
            Some(s)
        } else {
            find(&s.children, id)
        }
    })
}

/// The spans from the top of the run down to `id`, outermost first; empty
/// when it is not in the trace.
pub fn span_path<'a>(spans: &'a [TraceSpan], id: &str) -> Vec<&'a TraceSpan> {
    for span in spans {
        if span.id.as_ref() == id {
            return vec![span];
        }
        let mut below = span_path(&span.children, id);
        if !below.is_empty() {
            below.insert(0, span);
            return below;
        }
    }
    Vec::new()
}

/// How much of a stretch of time a set of spans covers, counting overlaps once.
pub fn covered(mut intervals: Vec<(u32, u32)>) -> u32 {
    intervals.sort_unstable();
    let mut total = 0;
    let mut reach: Option<(u32, u32)> = None;
    for (from, to) in intervals {
        match reach {
            Some((start, end)) if from <= end => reach = Some((start, end.max(to))),
            Some((start, end)) => {
                total += end - start;
                reach = Some((from, to));
            }
            None => reach = Some((from, to)),
        }
    }
    total + reach.map_or(0, |(start, end)| end - start)
}

/// One block of the flame graph.
#[derive(Debug, Clone, PartialEq)]
pub struct FlameBlock {
    /// The span it draws.
    pub id: SharedString,
    /// Its row, 0 at the top.
    pub depth: usize,
    /// Where it starts across the axis, 0 to 1.
    pub from: f32,
    /// Where it ends across the axis, 0 to 1.
    pub to: f32,
}

/// Lay out the flame graph of `spans` over the time from `window.0` to
/// `window.1`: blocks clipped to it, those wholly outside left out.
pub fn flame_blocks(spans: &[TraceSpan], window: (u32, u32), now: u32) -> Vec<FlameBlock> {
    fn walk(
        spans: &[TraceSpan],
        window: (u32, u32),
        now: u32,
        depth: usize,
        out: &mut Vec<FlameBlock>,
    ) {
        let (w0, w1) = window;
        let wide = (w1.saturating_sub(w0)).max(1) as f32;
        for span in spans {
            let end = span.end.unwrap_or(now).max(span.start);
            let (from, to) = (span.start.max(w0), end.min(w1));
            if to < from || (to == from && end > span.start) {
                continue;
            }
            out.push(FlameBlock {
                id: span.id.clone(),
                depth,
                from: (from - w0) as f32 / wide,
                to: (to - w0) as f32 / wide,
            });
            walk(&span.children, window, now, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(spans, window, now, 0, &mut out);
    out
}

/// The time a span's view covers: its own start to end, or to `now` while
/// running.
pub fn zoom_window(span: &TraceSpan, now: u32) -> (u32, u32) {
    let end = span.end.unwrap_or(now).max(span.start + 1);
    (span.start, end)
}

/// Where an arrow key moves the flame graph's cursor from `at`, among the
/// spans under `roots`: left and right to the neighbouring sibling, up to
/// the parent, down to the first child. `None` when the key does not move.
pub fn flame_step(roots: &[TraceSpan], at: Option<&str>, key: &str) -> Option<SharedString> {
    let Some(at) = at.filter(|at| find(roots, at).is_some()) else {
        return match key {
            "left" | "right" | "up" | "down" | "home" => roots.first().map(|s| s.id.clone()),
            _ => None,
        };
    };
    let path = span_path(roots, at);
    let here = path.last()?;
    let siblings: &[TraceSpan] = match path.len() {
        0 | 1 => roots,
        n => &path[n - 2].children,
    };
    let ix = siblings.iter().position(|s| s.id == here.id)?;
    match key {
        "left" => ix.checked_sub(1).map(|ix| siblings[ix].id.clone()),
        "right" => siblings.get(ix + 1).map(|s| s.id.clone()),
        "up" => (path.len() > 1).then(|| path[path.len() - 2].id.clone()),
        "down" => here.children.first().map(|s| s.id.clone()),
        "home" => roots.first().map(|s| s.id.clone()),
        _ => None,
    }
}

/// One part of the breakdown.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakdownPart {
    /// The span it stands for, or `None` for work the parent did itself.
    pub id: Option<SharedString>,
    /// Its name.
    pub label: SharedString,
    /// How much of the metric it holds.
    pub value: u64,
}

/// Split a metric over what `root` started, plus the part it did itself;
/// with no root, over the run's top-level spans. Parts holding nothing are
/// left out.
pub fn breakdown_parts(
    spans: &[TraceSpan],
    root: Option<&TraceSpan>,
    metric: TraceMetric,
    now: u32,
) -> Vec<BreakdownPart> {
    let value = |span: &TraceSpan| -> u64 {
        match metric {
            TraceMetric::Time => (span.end.unwrap_or(now).saturating_sub(span.start)) as u64,
            TraceMetric::Tokens => span.totals().0,
            TraceMetric::Cost => span.totals().1,
        }
    };
    let items = root.map_or(spans, |root| &root.children);
    let mut parts: Vec<BreakdownPart> = items
        .iter()
        .map(|span| BreakdownPart {
            id: Some(span.id.clone()),
            label: span.label.clone(),
            value: value(span),
        })
        .collect();
    if let Some(root) = root {
        let own = match metric {
            TraceMetric::Time => {
                let (from, to) = zoom_window(root, now);
                let inner = root
                    .children
                    .iter()
                    .map(|c| {
                        let end = c.end.unwrap_or(now).clamp(from, to);
                        (c.start.clamp(from, to), end)
                    })
                    .collect();
                (to - from).saturating_sub(covered(inner)) as u64
            }
            TraceMetric::Tokens => root.tokens as u64,
            TraceMetric::Cost => root.cost as u64,
        };
        parts.push(BreakdownPart {
            id: None,
            label: "Own work".into(),
            value: own,
        });
    }
    parts.retain(|part| part.value > 0);
    parts
}

/// Each part's share of the whole, in percent, rounded so they add up to 100.
pub fn breakdown_shares(parts: &[BreakdownPart]) -> Vec<u32> {
    let total: u64 = parts.iter().map(|p| p.value).sum();
    if total == 0 {
        return vec![0; parts.len()];
    }
    let mut shares: Vec<u32> = parts
        .iter()
        .map(|p| (p.value * 100 / total) as u32)
        .collect();
    let mut rest = 100 - shares.iter().sum::<u32>();
    // Hand the rounding leftovers to the largest remainders.
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by_key(|&ix| std::cmp::Reverse((parts[ix].value * 100) % total));
    for ix in order {
        if rest == 0 {
            break;
        }
        shares[ix] += 1;
        rest -= 1;
    }
    shares
}

/// The spoken name of the flame graph: where it is zoomed and which block
/// the cursor is on.
pub fn flame_words(zoom: Option<&TraceSpan>, at: Option<&TraceSpan>, now: u32) -> String {
    let duration = |span: &TraceSpan| {
        span.end
            .map(|e| span_time(e - span.start))
            .unwrap_or_else(|| "running".into())
    };
    let mut words = String::from("Flame graph");
    match zoom {
        Some(zoom) => words.push_str(&format!(", zoomed to {}, {}", zoom.label, duration(zoom))),
        None => words.push_str(&format!(", whole run, {}", span_time(now))),
    }
    if let Some(at) = at {
        words.push_str(&format!(
            ". On {}, {}{}",
            at.label,
            duration(at),
            if at.failed { ", failed" } else { "" }
        ));
    }
    words.push_str(". Arrows move, Enter zooms in, Escape zooms out");
    words
}

type SpanHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ZoomHandler = Rc<dyn Fn(Option<&SharedString>, &mut Window, &mut App)>;
type ViewHandler = Rc<dyn Fn(TraceView, &mut Window, &mut App)>;
type MetricHandler = Rc<dyn Fn(TraceMetric, &mut Window, &mut App)>;

/// A run's spans on a shared time axis.
#[derive(IntoElement)]
pub struct TraceTimeline {
    id: ElementId,
    style: StyleRefinement,
    spans: Vec<TraceSpan>,
    now: u32,
    open: Vec<SharedString>,
    selected: Option<SharedString>,
    on_select: Option<SpanHandler>,
    on_toggle: Option<SpanHandler>,
    view: TraceView,
    on_view: Option<ViewHandler>,
    zoom: Option<SharedString>,
    on_zoom: Option<ZoomHandler>,
    breakdown: Option<TraceMetric>,
    on_breakdown: Option<MetricHandler>,
}

impl TraceTimeline {
    /// Create a timeline at a moment `now`, in milliseconds from the start.
    pub fn new(id: impl Into<ElementId>, spans: Vec<TraceSpan>, now: u32) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            spans,
            now: now.max(1),
            open: Vec::new(),
            selected: None,
            on_select: None,
            on_toggle: None,
            view: TraceView::default(),
            on_view: None,
            zoom: None,
            on_zoom: None,
            breakdown: None,
            on_breakdown: None,
        }
    }

    /// The spans unfolded.
    pub fn open(mut self, ids: Vec<SharedString>) -> Self {
        self.open = ids;
        self
    }

    /// The selected span.
    pub fn selected(mut self, id: Option<SharedString>) -> Self {
        self.selected = id;
        self
    }

    /// Run when a row is clicked.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Run when a span is folded or unfolded.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Draw it as a timeline (the default) or a flame graph.
    pub fn view(mut self, view: TraceView) -> Self {
        self.view = view;
        self
    }

    /// Show a switch between the views, run when it is used.
    pub fn on_view(mut self, handler: impl Fn(TraceView, &mut Window, &mut App) + 'static) -> Self {
        self.on_view = Some(Rc::new(handler));
        self
    }

    /// Zoom into one span: the axis covers only its time and only it and
    /// what it started are drawn.
    pub fn zoom(mut self, id: Option<SharedString>) -> Self {
        self.zoom = id;
        self
    }

    /// Let people zoom (click a flame block, Enter, the zoom buttons) and
    /// back out (the trail, Escape); run with the span to zoom to, `None`
    /// for the whole run.
    pub fn on_zoom(
        mut self,
        handler: impl Fn(Option<&SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_zoom = Some(Rc::new(handler));
        self
    }

    /// Show a strip splitting this metric over the zoomed span's steps.
    pub fn breakdown(mut self, metric: TraceMetric) -> Self {
        self.breakdown = Some(metric);
        self
    }

    /// Show a switch between the breakdown's metrics, run when it is used.
    pub fn on_breakdown(
        mut self,
        handler: impl Fn(TraceMetric, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_breakdown = Some(Rc::new(handler));
        self
    }
}

impl Styled for TraceTimeline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

const FLAME_ROW: f32 = 24.;

fn depth_color(depth: usize, cx: &App) -> Hsla {
    let theme = cx.theme();
    match depth % 5 {
        0 => theme.chart_1,
        1 => theme.chart_2,
        2 => theme.chart_3,
        3 => theme.chart_4,
        _ => theme.chart_5,
    }
}

impl RenderOnce for TraceTimeline {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let ring = theme.ring;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let mono = theme.mono_font_family.clone();
        let danger = Tone::Danger.color(cx);
        let wash = look::ink::wash(cx);

        let now = self.now;
        let span_of = |s: &TraceSpan| s.end.unwrap_or(now);
        let zoomed = self.zoom.as_ref().and_then(|id| find(&self.spans, id));
        let trail: Vec<&TraceSpan> = zoomed
            .map(|z| span_path(&self.spans, &z.id))
            .unwrap_or_default();
        let roots: &[TraceSpan] = match zoomed {
            Some(z) => std::slice::from_ref(z),
            None => &self.spans,
        };
        let (w0, w1) = match zoomed {
            Some(z) => zoom_window(z, now),
            None => (
                0,
                self.spans.iter().map(span_of).max().unwrap_or(now).max(now),
            ),
        };
        let wide = (w1 - w0).max(1) as f32;
        let (all_tokens, all_cost) = self.spans.iter().fold((0u64, 0u64), |(t, c), s| {
            let (st, sc) = s.totals();
            (t + st, c + sc)
        });
        let running = {
            fn count(spans: &[TraceSpan]) -> usize {
                spans
                    .iter()
                    .map(|s| usize::from(s.end.is_none()) + count(&s.children))
                    .sum()
            }
            count(&self.spans)
        };
        let zoom_to = self.on_zoom.clone();

        // The header's view switch.
        let view_switch = self.on_view.clone().map(|on_view| {
            TabBar::new(part("view".into()))
                .segmented()
                .small()
                .selected_index(
                    TraceView::EVERY
                        .iter()
                        .position(|v| *v == self.view)
                        .unwrap_or(0),
                )
                .on_click(move |ix: &usize, window, cx| {
                    on_view(TraceView::EVERY[(*ix).min(1)], window, cx)
                })
                .children(TraceView::EVERY.iter().map(|v| Tab::new().label(v.words())))
        });

        // The trail back out of a zoom.
        let crumbs = (!trail.is_empty()).then(|| {
            let mut items = vec![("whole".to_string(), "Whole run".into(), None)];
            items.extend(
                trail
                    .iter()
                    .map(|s| (format!("to-{}", s.id), s.label.clone(), Some(s.id.clone()))),
            );
            let last = items.len() - 1;
            h_flex()
                .gap_1()
                .px_4()
                .pt_2()
                .text_caption()
                .child(Icon::new(CatalogIcon::ZoomIn).xsmall().text_color(quiet))
                .children(items.into_iter().enumerate().flat_map(
                    |(ix, (key, label, target)): (
                        usize,
                        (String, SharedString, Option<SharedString>),
                    )| {
                        let here = ix == last;
                        let zoom_to = zoom_to.clone();
                        let crumb = div()
                            .id(part(format!("crumb-{key}")))
                            .px_1()
                            .rounded(px(4.))
                            .text_color(if here { ink } else { quiet })
                            .role(if here { Role::ListItem } else { Role::Link })
                            .aria_label(if here {
                                format!("{label}, zoomed here")
                            } else {
                                format!("Zoom out to {label}")
                            })
                            .when(!here && zoom_to.is_some(), |this| {
                                this.cursor_pointer().on_click(move |_, window, cx| {
                                    if let Some(zoom_to) = &zoom_to {
                                        zoom_to(target.as_ref(), window, cx);
                                    }
                                })
                            })
                            .child(label);
                        let sep = (ix > 0).then(|| div().text_color(quiet).child("/"));
                        sep.into_iter()
                            .map(IntoElement::into_any_element)
                            .chain(std::iter::once(crumb.into_any_element()))
                            .collect::<Vec<_>>()
                    },
                ))
        });

        let body = match self.view {
            TraceView::Timeline => {
                let rows: Vec<_> = trace_rows(roots, &self.open)
                    .into_iter()
                    .map(|(span, depth)| {
                        let end = span_of(span);
                        let (tokens, cost) = span.totals();
                        let is_open = self.open.contains(&span.id);
                        let is_selected = self.selected.as_ref() == Some(&span.id);
                        let has_children = !span.children.is_empty();
                        let color = if span.failed { danger } else { accent };
                        let select = self.on_select.clone();
                        let toggle = self.on_toggle.clone();
                        let (id, toggle_id) = (span.id.clone(), span.id.clone());
                        let hovered = window.use_keyed_state(
                            part(format!("hover-{}", span.id)),
                            cx,
                            |_, _| false,
                        );
                        let fade = look::hover_fade(
                            part(format!("fade-{}", span.id)),
                            *hovered.read(cx),
                            window,
                            cx,
                        );
                        let fold_hovered = window.use_keyed_state(
                            part(format!("fold-hover-{}", span.id)),
                            cx,
                            |_, _| false,
                        );
                        let fold_fade = look::hover_fade(
                            part(format!("fold-fade-{}", span.id)),
                            *fold_hovered.read(cx),
                            window,
                            cx,
                        );
                        let rest = if is_selected { chip } else { chip.opacity(0.) };
                        let (bar_from, bar_to) = (span.start.max(w0), end.min(w1));
                        let bar = (bar_to > bar_from || (end == span.start && span.start >= w0))
                            .then(|| {
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left(relative((bar_from - w0) as f32 / wide))
                                    .w(relative(
                                        (bar_to.saturating_sub(bar_from) as f32 / wide).max(0.004),
                                    ))
                                    .rounded(px(2.))
                                    .bg(color.opacity(if is_selected { 0.9 } else { 0.55 }))
                                    .when(span.end.is_none(), |this| {
                                        this.border_r_2().border_color(color)
                                    })
                            });
                        h_flex()
                            .id(part(format!("span-{}", span.id)))
                            .h(px(28.))
                            .gap_2()
                            .px_2()
                            .rounded(radius)
                            .cursor_pointer()
                            .tab_index(0)
                            .focus_visible(move |s| s.border_2().border_color(ring))
                            .bg(look::mix(
                                rest,
                                chip.opacity(0.6),
                                if is_selected { 0. } else { fade },
                            ))
                            .on_hover(move |over, _, cx| {
                                hovered.update(cx, |state, cx| {
                                    *state = *over;
                                    cx.notify();
                                })
                            })
                            .role(Role::ListItem)
                            .aria_label(format!(
                                "{}, {}{}",
                                span.label,
                                span.end
                                    .map(|e| span_time(e - span.start))
                                    .unwrap_or_else(|| "running".into()),
                                if span.failed { ", failed" } else { "" }
                            ))
                            .aria_selected(is_selected)
                            .when(has_children, |this| this.aria_expanded(is_open))
                            .on_click(move |_, window, cx| {
                                if let Some(select) = &select {
                                    select(&id, window, cx);
                                }
                            })
                            .child(
                                h_flex()
                                    .w(px(200.))
                                    .flex_none()
                                    .gap_1()
                                    .pl(px(depth as f32 * 14.))
                                    .child(
                                        div()
                                            .id(part(format!("fold-{}", span.id)))
                                            .size(px(16.))
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .when(has_children, |this| {
                                                this.rounded(px(4.))
                                                    .bg(look::mix(
                                                        line.opacity(0.),
                                                        line,
                                                        fold_fade,
                                                    ))
                                                    .on_hover(move |over, _, cx| {
                                                        fold_hovered.update(cx, |state, cx| {
                                                            *state = *over;
                                                            cx.notify();
                                                        })
                                                    })
                                                    .on_click(move |_, window, cx| {
                                                        cx.stop_propagation();
                                                        if let Some(toggle) = &toggle {
                                                            toggle(&toggle_id, window, cx);
                                                        }
                                                    })
                                                    .child(
                                                        Icon::new(if is_open {
                                                            IconName::ChevronDown
                                                        } else {
                                                            IconName::ChevronRight
                                                        })
                                                        .xsmall()
                                                        .text_color(quiet),
                                                    )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_caption()
                                            .when(span.failed, |this| this.text_color(danger))
                                            .child(span.label.clone()),
                                    ),
                            )
                            .child(div().relative().flex_1().h(px(12.)).children(bar))
                            .child(
                                div()
                                    .w(px(58.))
                                    .flex_none()
                                    .text_right()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(
                                        span.end
                                            .map(|e| span_time(e - span.start))
                                            .unwrap_or_else(|| "running".into()),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(52.))
                                    .flex_none()
                                    .text_right()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(token_words(tokens)),
                            )
                            .child(
                                div()
                                    .w(px(52.))
                                    .flex_none()
                                    .text_right()
                                    .text_caption()
                                    .text_color(quiet)
                                    .child(cost_words(cost)),
                            )
                    })
                    .collect();
                v_flex()
                    .child(
                        h_flex()
                            .gap_2()
                            .px_4()
                            .pt_2()
                            .text_caption()
                            .text_color(quiet)
                            .child(div().w(px(200.)).flex_none().child("Span"))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .child(div().flex_1().child(if w0 == 0 {
                                        "0".to_string()
                                    } else {
                                        span_time(w0)
                                    }))
                                    .child(span_time(w1)),
                            )
                            .child(div().w(px(58.)).flex_none().text_right().child("Time"))
                            .child(div().w(px(52.)).flex_none().text_right().child("Tokens"))
                            .child(div().w(px(52.)).flex_none().text_right().child("Cost")),
                    )
                    .child(v_flex().p_2().gap_px().children(rows))
                    .into_any_element()
            }
            TraceView::Flame => {
                let blocks = flame_blocks(roots, (w0, w1), now);
                let depth = blocks.iter().map(|b| b.depth + 1).max().unwrap_or(1);
                let at = self.selected.as_ref().and_then(|id| find(roots, id));
                let spoken = flame_words(zoomed, at, now);
                let tiles = blocks.into_iter().filter_map(|block| {
                    let span = find(roots, &block.id)?;
                    let is_selected = self.selected.as_ref() == Some(&span.id);
                    let base = if span.failed {
                        danger
                    } else {
                        depth_color(block.depth, cx)
                    };
                    let hovered = window.use_keyed_state(
                        part(format!("flame-hover-{}", span.id)),
                        cx,
                        |_, _| false,
                    );
                    let fade = look::hover_fade(
                        part(format!("flame-fade-{}", span.id)),
                        *hovered.read(cx),
                        window,
                        cx,
                    );
                    let strength = if is_selected { 0.5 } else { 0.22 + 0.14 * fade };
                    let select = self.on_select.clone();
                    let zoom_to = self.on_zoom.clone();
                    let id = span.id.clone();
                    let (tokens, cost) = span.totals();
                    Some(
                        div()
                            .id(part(format!("flame-{}", span.id)))
                            .absolute()
                            .top(px(block.depth as f32 * FLAME_ROW))
                            .left(relative(block.from))
                            .w(relative((block.to - block.from).max(0.003)))
                            .h(px(FLAME_ROW - 2.))
                            .pr(px(1.))
                            .child(
                                h_flex()
                                    .size_full()
                                    .px(px(6.))
                                    .gap_1()
                                    .rounded(px(4.))
                                    .overflow_hidden()
                                    .border_l_2()
                                    .border_color(base)
                                    .bg(base.opacity(strength))
                                    .when(is_selected, |this| {
                                        this.shadow(vec![isonomic_ui::BoxShadow {
                                            color: base,
                                            offset: isonomic_ui::point(px(0.), px(0.)),
                                            blur_radius: px(0.),
                                            spread_radius: px(1.),
                                            inset: true,
                                        }])
                                    })
                                    .when(span.end.is_none(), |this| {
                                        this.border_r_2().border_color(base)
                                    })
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_caption()
                                            .text_color(if span.failed { danger } else { ink })
                                            .child(span.label.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .ml_auto()
                                            .text_size(px(10.))
                                            .line_height(px(16.))
                                            .font_family(mono.clone())
                                            .text_color(quiet)
                                            .child(
                                                span.end
                                                    .map(|e| span_time(e - span.start))
                                                    .unwrap_or_else(|| "running".into()),
                                            ),
                                    ),
                            )
                            .cursor_pointer()
                            .tooltip({
                                let words = format!(
                                    "{} · {} · {} tokens · {}",
                                    span.label,
                                    span.end
                                        .map(|e| span_time(e - span.start))
                                        .unwrap_or_else(|| "running".into()),
                                    token_words(tokens),
                                    cost_words(cost)
                                );
                                move |window, cx| Tooltip::new(words.clone()).build(window, cx)
                            })
                            .on_hover(move |over, _, cx| {
                                hovered.update(cx, |state, cx| {
                                    *state = *over;
                                    cx.notify();
                                })
                            })
                            .on_click(move |_, window, cx| {
                                if let Some(select) = &select {
                                    select(&id, window, cx);
                                }
                                if let Some(zoom_to) = &zoom_to {
                                    zoom_to(Some(&id), window, cx);
                                }
                            }),
                    )
                });
                let key_roots = roots.to_vec();
                let key_trail: Vec<SharedString> = trail.iter().map(|s| s.id.clone()).collect();
                let key_at = self.selected.clone();
                let key_select = self.on_select.clone();
                let key_zoom = self.on_zoom.clone();
                v_flex()
                    .px_4()
                    .pt_3()
                    .pb_2()
                    .gap_1()
                    .child(
                        h_flex()
                            .text_size(px(10.))
                            .line_height(px(16.))
                            .font_family(mono.clone())
                            .text_color(quiet)
                            .child(div().flex_1().child(if w0 == 0 {
                                "0".to_string()
                            } else {
                                span_time(w0)
                            }))
                            .child(
                                div()
                                    .flex_1()
                                    .text_center()
                                    .child(span_time(w0 + (w1 - w0) / 2)),
                            )
                            .child(div().flex_1().text_right().child(span_time(w1))),
                    )
                    .child(
                        div()
                            .id(part("flame".into()))
                            .test_support()
                            .relative()
                            .w_full()
                            .h(px(depth as f32 * FLAME_ROW))
                            .rounded(px(4.))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .focus_visible(move |s| s.border_2().border_color(ring))
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                let key = event.keystroke.key.as_str();
                                match key {
                                    "enter" => {
                                        if let (Some(zoom), Some(at)) = (&key_zoom, &key_at) {
                                            zoom(Some(at), window, cx);
                                        }
                                    }
                                    "escape" => {
                                        let Some(zoom) = &key_zoom else { return };
                                        if key_trail.is_empty() {
                                            return;
                                        }
                                        let out =
                                            key_trail.len().checked_sub(2).map(|ix| &key_trail[ix]);
                                        zoom(out, window, cx);
                                    }
                                    _ => {
                                        let Some(next) =
                                            flame_step(&key_roots, key_at.as_deref(), key)
                                        else {
                                            return;
                                        };
                                        if let Some(select) = &key_select {
                                            select(&next, window, cx);
                                        }
                                    }
                                }
                                cx.stop_propagation();
                            })
                            .children(tiles),
                    )
                    .into_any_element()
            }
        };

        // The breakdown strip.
        let breakdown = self.breakdown.map(|metric| {
            let parts = breakdown_parts(&self.spans, zoomed, metric, now);
            let shares = breakdown_shares(&parts);
            let total: u64 = parts.iter().map(|p| p.value).sum();
            let color_of = |ix: usize, part: &BreakdownPart| {
                if part.id.is_none() {
                    quiet
                } else {
                    depth_color(ix, cx)
                }
            };
            let on_metric = self.on_breakdown.clone();
            let segments = parts.iter().enumerate().map(|(ix, p)| {
                div()
                    .h_full()
                    .flex_basis(relative(p.value as f32 / total.max(1) as f32))
                    .flex_shrink_0()
                    .bg(color_of(ix, p))
                    .when(ix > 0, |this| this.border_l_1().border_color(fill))
            });
            let legend = parts
                .iter()
                .zip(&shares)
                .enumerate()
                .map(|(ix, (p, share))| {
                    h_flex()
                        .id(part(format!("share-{ix}")))
                        .gap(px(6.))
                        .role(Role::ListItem)
                        .aria_label(format!(
                            "{}, {}, {share} percent",
                            p.label,
                            metric.amount(p.value)
                        ))
                        .child(div().size(px(8.)).rounded(px(2.)).bg(color_of(ix, p)))
                        .child(div().text_color(quiet).child(p.label.clone()))
                        .child(
                            div()
                                .font_family(mono.clone())
                                .text_color(ink)
                                .child(format!("{} · {share}%", metric.amount(p.value))),
                        )
                });
            v_flex()
                .id(part("breakdown".into()))
                .gap_2()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(line)
                .role(Role::List)
                .aria_label(format!(
                    "{} breakdown of {}",
                    metric.words(),
                    zoomed.map_or("the whole run".into(), |z| z.label.clone())
                ))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .text_eyebrow()
                                .text_color(quiet)
                                .child(format!(
                                    "{} by step · {}",
                                    metric.words().to_uppercase(),
                                    metric.amount(total)
                                )),
                        )
                        .when_some(on_metric, |this, on_metric| {
                            this.child(
                                TabBar::new(part("metric".into()))
                                    .segmented()
                                    .small()
                                    .selected_index(
                                        TraceMetric::EVERY
                                            .iter()
                                            .position(|m| *m == metric)
                                            .unwrap_or(0),
                                    )
                                    .on_click(move |ix: &usize, window, cx| {
                                        on_metric(TraceMetric::EVERY[(*ix).min(2)], window, cx)
                                    })
                                    .children(
                                        TraceMetric::EVERY
                                            .iter()
                                            .map(|m| Tab::new().label(m.words())),
                                    ),
                            )
                        }),
                )
                .child(
                    h_flex()
                        .h(px(8.))
                        .w_full()
                        .rounded(px(4.))
                        .overflow_hidden()
                        .bg(wash)
                        .children(segments),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_x_4()
                        .gap_y_1()
                        .text_caption()
                        .children(legend),
                )
        });

        let picked = self.selected.as_ref().and_then(|id| find(&self.spans, id));
        let detail = picked
            .map(|span| {
                let (tokens, cost) = span.totals();
                format!(
                    "{}: {} from {}, {} tokens, {} ({:.0}% of the cost)",
                    span.label,
                    span.end
                        .map(|e| span_time(e - span.start))
                        .unwrap_or_else(|| "still running".into()),
                    span_time(span.start),
                    tokens,
                    cost_words(cost),
                    100. * cost as f64 / all_cost.max(1) as f64
                )
            })
            .unwrap_or_else(|| "Select a span for its figures.".into());
        let zoom_buttons = self.on_zoom.clone().map(|zoom| {
            let can_in = picked.is_some_and(|p| zoomed.is_none_or(|z| z.id != p.id));
            let target = picked.map(|p| p.id.clone());
            let out = trail.len().checked_sub(2).map(|ix| trail[ix].id.clone());
            let zoom_out = zoom.clone();
            h_flex()
                .gap_1()
                .flex_none()
                .when(can_in, |this| {
                    this.child(
                        Button::new(part("zoom-in".into()))
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(CatalogIcon::ZoomIn))
                            .label("Zoom in")
                            .on_click(move |_, window, cx| zoom(target.as_ref(), window, cx)),
                    )
                })
                .when(zoomed.is_some(), |this| {
                    this.child(
                        Button::new(part("zoom-out".into()))
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(CatalogIcon::ZoomOut))
                            .label("Zoom out")
                            .on_click(move |_, window, cx| zoom_out(out.as_ref(), window, cx)),
                    )
                })
        });

        v_flex()
            .id(self.id.clone())
            .w_full()
            .max_w(px(900.))
            .rounded(radius_lg)
            .bg(fill)
            .text_color(ink)
            .text_body()
            .shadow(resting_shadow(cx))
            .child(
                h_flex()
                    .gap_3()
                    .px_4()
                    .py_2p5()
                    .border_b_1()
                    .border_color(line)
                    .child(Icon::new(IconName::Network).small().text_color(quiet))
                    .child(
                        div()
                            .flex_1()
                            .font_medium()
                            .child(format!("{} so far", span_time(now))),
                    )
                    .when(running > 0, |this| {
                        this.child(
                            div()
                                .text_caption()
                                .text_color(accent)
                                .child(format!("{running} running")),
                        )
                    })
                    .child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child(format!("{all_tokens} tokens")),
                    )
                    .child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child(cost_words(all_cost)),
                    )
                    .children(view_switch),
            )
            .children(crumbs)
            .child(body)
            .children(breakdown)
            .child(
                h_flex()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .min_h(px(36.))
                    .border_t_1()
                    .border_color(line)
                    .text_caption()
                    .text_color(quiet)
                    .child(div().flex_1().min_w_0().child(detail))
                    .children(zoom_buttons),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_totals_follow_the_tree() {
        let spans = vec![
            TraceSpan::new("a", "A", 0, 100)
                .usage(10, 5)
                .child(TraceSpan::new("b", "B", 10, 50).usage(20, 7))
                .child(TraceSpan::new("c", "C", 60, 0).running().usage(5, 1)),
        ];
        assert_eq!(spans[0].totals(), (35, 13));
        assert_eq!(trace_rows(&spans, &[]).len(), 1);
        let open: Vec<SharedString> = vec!["a".into()];
        let rows: Vec<_> = trace_rows(&spans, &open)
            .iter()
            .map(|(s, d)| (s.id.to_string(), *d))
            .collect();
        assert_eq!(rows, [("a".into(), 0), ("b".into(), 1), ("c".into(), 1)]);
        assert_eq!(span_time(2_400), "2.4 s");
    }

    fn tree() -> Vec<TraceSpan> {
        vec![
            TraceSpan::new("a", "A", 0, 100)
                .usage(10, 4)
                .child(
                    TraceSpan::new("b", "B", 10, 50)
                        .usage(20, 6)
                        .child(TraceSpan::new("d", "D", 20, 30).usage(5, 1)),
                )
                .child(TraceSpan::new("c", "C", 40, 80).usage(30, 10)),
            TraceSpan::new("e", "E", 100, 0).running().usage(2, 1),
        ]
    }

    #[test]
    fn paths_lead_down_to_a_span() {
        let spans = tree();
        let ids: Vec<_> = span_path(&spans, "d")
            .iter()
            .map(|s| s.id.to_string())
            .collect();
        assert_eq!(ids, ["a", "b", "d"]);
        assert!(span_path(&spans, "zz").is_empty());
    }

    #[test]
    fn coverage_counts_overlaps_once() {
        assert_eq!(covered(vec![]), 0);
        assert_eq!(covered(vec![(10, 50), (40, 80)]), 70);
        assert_eq!(covered(vec![(40, 80), (0, 5), (10, 20)]), 55);
    }

    #[test]
    fn flame_blocks_clip_to_the_window() {
        let spans = tree();
        let all = flame_blocks(&spans, (0, 200), 200);
        assert_eq!(all.len(), 5);
        assert_eq!(all[2].id.as_ref(), "d");
        assert_eq!(all[2].depth, 2);
        assert!((all[4].to - 1.).abs() < 1e-6, "running span reaches now");
        let b = find(&spans, "b").unwrap();
        let zoomed = flame_blocks(std::slice::from_ref(b), zoom_window(b, 200), 200);
        assert_eq!(zoomed.len(), 2);
        assert_eq!((zoomed[0].from, zoomed[0].to), (0., 1.));
        assert!((zoomed[1].from - 0.25).abs() < 1e-6);
        // C starts after B's window ends: it is left out entirely.
        let a = find(&spans, "a").unwrap();
        let narrow = flame_blocks(std::slice::from_ref(a), (0, 30), 200);
        assert!(narrow.iter().all(|b| b.id.as_ref() != "c"));
    }

    #[test]
    fn arrows_walk_the_tree() {
        let spans = tree();
        assert_eq!(flame_step(&spans, None, "right").as_deref(), Some("a"));
        assert_eq!(flame_step(&spans, Some("a"), "right").as_deref(), Some("e"));
        assert_eq!(flame_step(&spans, Some("a"), "left"), None);
        assert_eq!(flame_step(&spans, Some("a"), "down").as_deref(), Some("b"));
        assert_eq!(flame_step(&spans, Some("b"), "right").as_deref(), Some("c"));
        assert_eq!(flame_step(&spans, Some("d"), "up").as_deref(), Some("b"));
        assert_eq!(flame_step(&spans, Some("c"), "down"), None);
        assert_eq!(flame_step(&spans, Some("d"), "home").as_deref(), Some("a"));
        assert_eq!(flame_step(&spans, Some("d"), "x"), None);
        // Zoomed into B, the cursor stays inside it.
        let b = find(&spans, "b").unwrap().clone();
        let roots = [b];
        assert_eq!(flame_step(&roots, Some("b"), "right"), None);
        assert_eq!(flame_step(&roots, Some("a"), "down").as_deref(), Some("b"));
    }

    #[test]
    fn breakdown_splits_the_zoomed_span() {
        let spans = tree();
        let a = find(&spans, "a").unwrap();
        let time = breakdown_parts(&spans, Some(a), TraceMetric::Time, 200);
        let values: Vec<_> = time
            .iter()
            .map(|p| (p.label.to_string(), p.value))
            .collect();
        assert_eq!(
            values,
            [("B".into(), 40), ("C".into(), 40), ("Own work".into(), 30)]
        );
        let tokens = breakdown_parts(&spans, Some(a), TraceMetric::Tokens, 200);
        assert_eq!(
            tokens.iter().map(|p| p.value).collect::<Vec<_>>(),
            [25, 30, 10]
        );
        let top = breakdown_parts(&spans, None, TraceMetric::Cost, 200);
        assert_eq!(top.iter().map(|p| p.value).collect::<Vec<_>>(), [21, 1]);
        assert!(top.iter().all(|p| p.id.is_some()));
    }

    #[test]
    fn shares_add_up_to_a_hundred() {
        let part = |value| BreakdownPart {
            id: None,
            label: "x".into(),
            value,
        };
        let shares = breakdown_shares(&[part(1), part(1), part(1)]);
        assert_eq!(shares.iter().sum::<u32>(), 100);
        assert_eq!(breakdown_shares(&[part(3), part(1)]), [75, 25]);
        assert_eq!(breakdown_shares(&[part(0)]), [0]);
    }

    #[test]
    fn metric_amounts_and_spoken_words() {
        assert_eq!(TraceMetric::Time.amount(1_500), "1.5 s");
        assert_eq!(TraceMetric::Tokens.amount(12_300), "12.3k");
        assert_eq!(TraceMetric::Cost.amount(250), "2.5\u{a2}");
        let spans = tree();
        let words = flame_words(find(&spans, "a"), find(&spans, "c"), 200);
        assert!(words.starts_with("Flame graph, zoomed to A, 100 ms. On C, 40 ms"));
        assert!(flame_words(None, None, 1_000).contains("whole run, 1.0 s"));
    }
}
