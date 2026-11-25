// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Trace timeline: a run's spans as a folding tree beside bars on a shared time axis, with time, tokens and cost per step.
//!
//! The left side is the span tree: each span indented under the one that
//! started it, foldable. The right side lines each span up as a bar on one
//! time axis, so overlaps and waits show at a glance; running spans reach
//! the live edge. Each row ends with its duration, tokens and cost. The
//! selected span's figures and share of the whole show underneath.

use std::rc::Rc;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use isonomic_ui::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder as _, px, relative,
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

type SpanHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

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
}

impl Styled for TraceTimeline {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for TraceTimeline {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let fill = theme.background;
        let line = theme.border;
        let chip = theme.muted;
        let accent = theme.primary;
        let radius = theme.radius;
        let radius_lg = theme.radius_lg;
        let danger = Tone::Danger.color(cx);

        let now = self.now;
        let span_of = |s: &TraceSpan| s.end.unwrap_or(now);
        let total_end = self.spans.iter().map(span_of).max().unwrap_or(now).max(now) as f32;
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

        let rows: Vec<_> = trace_rows(&self.spans, &self.open)
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
                h_flex()
                    .id(ElementId::NamedChild(
                        self.id.clone().into(),
                        format!("span-{}", span.id).into(),
                    ))
                    .h(px(28.))
                    .gap_2()
                    .px_2()
                    .rounded(radius)
                    .cursor_pointer()
                    .when(is_selected, |this| this.bg(chip))
                    .hover(|s| s.bg(chip.opacity(0.6)))
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
                                    .id(ElementId::NamedChild(
                                        self.id.clone().into(),
                                        format!("fold-{}", span.id).into(),
                                    ))
                                    .size(px(16.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(has_children, |this| {
                                        this.rounded(px(4.))
                                            .hover(|s| s.bg(line))
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
                    .child(
                        div().relative().flex_1().h(px(12.)).child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(relative(span.start as f32 / total_end))
                                .w(relative(((end - span.start) as f32 / total_end).max(0.004)))
                                .rounded(px(2.))
                                .bg(color.opacity(if is_selected { 0.9 } else { 0.55 }))
                                .when(span.end.is_none(), |this| {
                                    this.border_r_2().border_color(color)
                                }),
                        ),
                    )
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
                            .child(if tokens >= 1000 {
                                format!("{:.1}k", tokens as f64 / 1000.)
                            } else {
                                tokens.to_string()
                            }),
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

        let detail = self
            .selected
            .as_ref()
            .and_then(|id| find(&self.spans, id))
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
                    ),
            )
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
                            .child(div().flex_1().child("0"))
                            .child(span_time(total_end as u32)),
                    )
                    .child(div().w(px(58.)).flex_none().text_right().child("Time"))
                    .child(div().w(px(52.)).flex_none().text_right().child("Tokens"))
                    .child(div().w(px(52.)).flex_none().text_right().child("Cost")),
            )
            .child(v_flex().p_2().gap_px().children(rows))
            .child(
                div()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(line)
                    .text_caption()
                    .text_color(quiet)
                    .child(detail),
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
}
