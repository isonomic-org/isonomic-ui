// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tool fan-out graph: the calls an agent makes, fanning out to tools and
//! from those tools to the tools they call in turn.
//!
//! The agent sits on the left; each column to the right is one more hop.
//! Every line is one caller-to-tool edge: it thickens with the number of
//! calls, takes the warning or danger colour when its calls fail often, and
//! carries its call count, average latency and error rate. The hottest path,
//! following the busiest edge at every hop, is drawn in the accent colour.
//!
//! Pointing at a node or a line picks it, as does clicking. From the
//! keyboard the graph is one Tab stop: Right walks from a node to its
//! busiest outgoing line and on to the tool at its end, Left walks back,
//! Up and Down move between siblings, Home returns to the agent and Escape
//! clears the pick. The readout underneath reads the pick in full.

use std::{cell::Cell, rc::Rc};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::{Presence, TestSupportExt as _, Transition, transition};
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TextRole as _, h_flex, look, tag::Tag,
    v_flex,
};
use isonomic_ui::{
    App, Bounds, ElementId, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseDownEvent, MouseMoveEvent, ParentElement as _, PathBuilder, Pixels, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, canvas, div,
    point, prelude::FluentBuilder as _, px,
};

use crate::common::{panel, shell, shell_header};

/// A node box, and the distance between rows.
const NODE_W: f32 = 132.;
const NODE_H: f32 = 40.;
const ROW: f32 = 58.;
/// The straight run into a node, where the edge's figures sit.
const RUN: f32 = 118.;
/// How close the pointer must come to a line to pick it.
const REACH: f32 = 6.;

/// Calls from one caller to one tool, and the calls that tool made in turn.
#[derive(Debug, Clone, PartialEq)]
pub struct FanoutCall {
    /// The tool called.
    pub tool: SharedString,
    /// How many times it was called.
    pub calls: u32,
    /// How many of those calls failed.
    pub errors: u32,
    /// Average time a call took, in milliseconds.
    pub latency_ms: f64,
    /// The tools this one called.
    pub children: Vec<FanoutCall>,
}

impl FanoutCall {
    /// `calls` calls to `tool`, taking `latency_ms` on average.
    pub fn new(tool: impl Into<SharedString>, calls: u32, latency_ms: f64) -> Self {
        Self {
            tool: tool.into(),
            calls,
            errors: 0,
            latency_ms: latency_ms.max(0.),
            children: Vec::new(),
        }
    }

    /// How many of the calls failed.
    pub fn errors(mut self, errors: u32) -> Self {
        self.errors = errors.min(self.calls);
        self
    }

    /// A tool this one called.
    pub fn child(mut self, call: FanoutCall) -> Self {
        self.children.push(call);
        self
    }
}

/// One node of the graph, laid flat. Node 0 is the agent; every other node
/// is a tool, and the edge into it is named by the same index.
#[derive(Debug, Clone, PartialEq)]
pub struct FanNode {
    /// Its name.
    pub label: SharedString,
    /// Its hop from the agent: 0 for the agent.
    pub depth: usize,
    /// The node that called it.
    pub parent: Option<usize>,
    /// Calls along the edge into it (for the agent, all its calls out).
    pub calls: u32,
    /// Failed calls along the edge into it.
    pub errors: u32,
    /// Average latency along the edge into it.
    pub latency_ms: f64,
    /// The nodes it called, in order.
    pub children: Vec<usize>,
    /// Its row, in rows from the top; parents sit between their children.
    pub row: f32,
}

/// Flatten the agent and its calls, depth first, and give every node a row:
/// tools that call nothing take a row each, callers centre on theirs.
pub fn flatten(agent: &SharedString, calls: &[FanoutCall]) -> Vec<FanNode> {
    fn walk(call: &FanoutCall, parent: usize, depth: usize, nodes: &mut Vec<FanNode>) -> usize {
        let ix = nodes.len();
        nodes.push(FanNode {
            label: call.tool.clone(),
            depth,
            parent: Some(parent),
            calls: call.calls,
            errors: call.errors,
            latency_ms: call.latency_ms,
            children: Vec::new(),
            row: 0.,
        });
        nodes[parent].children.push(ix);
        for child in &call.children {
            walk(child, ix, depth + 1, nodes);
        }
        ix
    }
    let mut nodes = vec![FanNode {
        label: agent.clone(),
        depth: 0,
        parent: None,
        calls: calls.iter().map(|call| call.calls).sum(),
        errors: calls.iter().map(|call| call.errors).sum(),
        latency_ms: 0.,
        children: Vec::new(),
        row: 0.,
    }];
    for call in calls {
        walk(call, 0, 1, &mut nodes);
    }
    let mut next = 0.;
    place_rows(&mut nodes, 0, &mut next);
    nodes
}

fn place_rows(nodes: &mut [FanNode], ix: usize, next: &mut f32) {
    let children = nodes[ix].children.clone();
    if children.is_empty() {
        nodes[ix].row = *next;
        *next += 1.;
        return;
    }
    for &child in &children {
        place_rows(nodes, child, next);
    }
    let first = nodes[children[0]].row;
    let last = nodes[children[children.len() - 1]].row;
    nodes[ix].row = (first + last) / 2.;
}

/// How many rows the graph needs.
pub fn row_count(nodes: &[FanNode]) -> usize {
    nodes
        .iter()
        .filter(|node| node.children.is_empty())
        .count()
        .max(1)
}

/// How many columns: the deepest hop, plus the agent's.
pub fn column_count(nodes: &[FanNode]) -> usize {
    nodes.iter().map(|node| node.depth).max().unwrap_or(0) + 1
}

/// The hottest path from the agent: at every hop, the busiest edge. Ties go
/// to the first.
pub fn hottest_path(nodes: &[FanNode]) -> Vec<usize> {
    let mut path = Vec::new();
    let mut at = 0;
    while let Some(next) = nodes.get(at).and_then(|node| busiest_child(nodes, node)) {
        path.push(next);
        at = next;
    }
    path
}

fn busiest_child(nodes: &[FanNode], node: &FanNode) -> Option<usize> {
    node.children.iter().copied().reduce(|best, ix| {
        if nodes[ix].calls > nodes[best].calls {
            ix
        } else {
            best
        }
    })
}

/// The share of calls that failed, 0 to 1.
pub fn error_rate(calls: u32, errors: u32) -> f64 {
    if calls == 0 {
        0.
    } else {
        f64::from(errors.min(calls)) / f64::from(calls)
    }
}

/// How worrying an error rate is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorLevel {
    /// Under 1%.
    Clean,
    /// 1% to under 10%.
    Elevated,
    /// 10% or more.
    High,
}

/// The level of an error rate.
pub fn error_level(rate: f64) -> ErrorLevel {
    if rate >= 0.1 {
        ErrorLevel::High
    } else if rate >= 0.01 {
        ErrorLevel::Elevated
    } else {
        ErrorLevel::Clean
    }
}

/// A latency, short: "180ms", "1.4s", "12s".
pub fn latency_text(ms: f64) -> String {
    if ms < 1000. {
        format!("{}ms", ms.round() as i64)
    } else if ms < 10_000. {
        format!("{:.1}s", ms / 1000.)
    } else {
        format!("{}s", (ms / 1000.).round() as i64)
    }
}

/// An error rate, short: "0%", "2.4%", "12%".
pub fn rate_text(rate: f64) -> String {
    let percent = rate * 100.;
    if percent == 0. {
        "0%".into()
    } else if percent < 10. {
        format!("{percent:.1}%")
    } else {
        format!("{}%", percent.round() as i64)
    }
}

/// What is picked: a node, or the edge into a node (named by that node).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanPick {
    /// A node.
    Node(usize),
    /// The edge into this node from its caller.
    Edge(usize),
}

fn siblings(nodes: &[FanNode], ix: usize) -> &[usize] {
    match nodes[ix].parent {
        Some(parent) => &nodes[parent].children,
        None => &[],
    }
}

fn sibling(nodes: &[FanNode], ix: usize, forward: bool) -> usize {
    let all = siblings(nodes, ix);
    let Some(at) = all.iter().position(|&one| one == ix) else {
        return ix;
    };
    if forward {
        all.get(at + 1).copied().unwrap_or(ix)
    } else {
        at.checked_sub(1).map_or(ix, |before| all[before])
    }
}

/// Where a key moves the pick. `None` means the key does nothing here.
pub fn step(nodes: &[FanNode], now: Option<FanPick>, key: &str) -> Option<Option<FanPick>> {
    if nodes.is_empty() {
        return None;
    }
    let next = match (now, key) {
        (_, "escape") => None,
        (_, "home") => Some(FanPick::Node(0)),
        (None, "left" | "right" | "up" | "down") => Some(FanPick::Node(0)),
        (Some(FanPick::Node(ix)), "right") => {
            Some(busiest_child(nodes, &nodes[ix]).map_or(FanPick::Node(ix), FanPick::Edge))
        }
        (Some(FanPick::Node(ix)), "left") => Some(if ix == 0 {
            FanPick::Node(0)
        } else {
            FanPick::Edge(ix)
        }),
        (Some(FanPick::Node(ix)), "up") => Some(FanPick::Node(sibling(nodes, ix, false))),
        (Some(FanPick::Node(ix)), "down") => Some(FanPick::Node(sibling(nodes, ix, true))),
        (Some(FanPick::Edge(ix)), "right") => Some(FanPick::Node(ix)),
        (Some(FanPick::Edge(ix)), "left") => Some(FanPick::Node(nodes[ix].parent.unwrap_or(0))),
        (Some(FanPick::Edge(ix)), "up") => Some(FanPick::Edge(sibling(nodes, ix, false))),
        (Some(FanPick::Edge(ix)), "down") => Some(FanPick::Edge(sibling(nodes, ix, true))),
        _ => return None,
    };
    Some(next)
}

/// Where things sit in a graph `width` wide with `columns` columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FanGeometry {
    /// The graph's width.
    pub width: f32,
    /// How many columns.
    pub columns: usize,
}

impl FanGeometry {
    /// The left edge of a column: columns spread from edge to edge.
    pub fn column_x(&self, depth: usize) -> f32 {
        if self.columns < 2 {
            return 0.;
        }
        let free = (self.width - NODE_W).max(0.);
        free * depth as f32 / (self.columns - 1) as f32
    }

    /// A node's box: left, top, width, height.
    pub fn node_box(&self, node: &FanNode) -> (f32, f32, f32, f32) {
        (self.column_x(node.depth), node.row * ROW, NODE_W, NODE_H)
    }

    /// The line into `node` from `parent`, as points: a curve from the
    /// caller's right side, then a straight run into the node.
    pub fn edge_points(&self, parent: &FanNode, node: &FanNode) -> Vec<(f32, f32)> {
        let (x0, y0) = (
            self.column_x(parent.depth) + NODE_W,
            parent.row * ROW + NODE_H / 2.,
        );
        let (x1, y1) = (self.column_x(node.depth), node.row * ROW + NODE_H / 2.);
        let run_from = (x1 - RUN).max(x0 + (x1 - x0) * 0.3);
        let mid = (x0 + run_from) / 2.;
        let mut points: Vec<(f32, f32)> = (0..=16)
            .map(|step| {
                let t = step as f32 / 16.;
                let u = 1. - t;
                // A cubic with both handles level: leaves flat, lands flat.
                let x = u * u * u * x0
                    + 3. * u * u * t * mid
                    + 3. * u * t * t * mid
                    + t * t * t * run_from;
                let y = u * u * u * y0 + 3. * u * u * t * y0 + 3. * u * t * t * y1 + t * t * t * y1;
                (x, y)
            })
            .collect();
        points.push((x1, y1));
        points
    }
}

/// The shortest distance from a point to a line through `points`.
pub fn distance_to_line(points: &[(f32, f32)], x: f32, y: f32) -> f32 {
    points
        .windows(2)
        .map(|pair| {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            let (dx, dy) = (bx - ax, by - ay);
            let length = dx * dx + dy * dy;
            let t = if length > 0. {
                (((x - ax) * dx + (y - ay) * dy) / length).clamp(0., 1.)
            } else {
                0.
            };
            let (px, py) = (ax + t * dx, ay + t * dy);
            ((x - px).powi(2) + (y - py).powi(2)).sqrt()
        })
        .fold(f32::INFINITY, f32::min)
}

/// What sits under the pointer: a node box first, then the nearest line
/// within reach.
pub fn pick_at(nodes: &[FanNode], geometry: FanGeometry, x: f32, y: f32) -> Option<FanPick> {
    if let Some(ix) = nodes.iter().position(|node| {
        let (left, top, w, h) = geometry.node_box(node);
        (left..=left + w).contains(&x) && (top..=top + h).contains(&y)
    }) {
        return Some(FanPick::Node(ix));
    }
    nodes
        .iter()
        .enumerate()
        .filter_map(|(ix, node)| {
            let parent = &nodes[node.parent?];
            let distance = distance_to_line(&geometry.edge_points(parent, node), x, y);
            (distance <= REACH).then_some((ix, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(ix, _)| FanPick::Edge(ix))
}

/// What a screen reader hears for a pick.
pub fn spoken_pick(nodes: &[FanNode], pick: FanPick, hot: &[usize]) -> String {
    let edge_words = |node: &FanNode| {
        let rate = error_rate(node.calls, node.errors);
        format!(
            "{} calls, {} average, {} failed, {} error rate",
            node.calls,
            latency_text(node.latency_ms),
            node.errors,
            rate_text(rate)
        )
    };
    match pick {
        FanPick::Node(0) => format!(
            "{}, agent, {} tools called directly, {} calls in all",
            nodes[0].label,
            nodes[0].children.len(),
            nodes[0].calls
        ),
        FanPick::Node(ix) => {
            let node = &nodes[ix];
            format!(
                "{}, tool, called by {}: {}{}{}",
                node.label,
                nodes[node.parent.unwrap_or(0)].label,
                edge_words(node),
                match node.children.len() {
                    0 => String::new(),
                    1 => ", calls 1 tool".into(),
                    more => format!(", calls {more} tools"),
                },
                if hot.contains(&ix) {
                    ", on the hottest path"
                } else {
                    ""
                }
            )
        }
        FanPick::Edge(ix) => {
            let node = &nodes[ix];
            format!(
                "Line from {} to {}: {}{}",
                nodes[node.parent.unwrap_or(0)].label,
                node.label,
                edge_words(node),
                if hot.contains(&ix) {
                    ", on the hottest path"
                } else {
                    ""
                }
            )
        }
    }
}

type SelectHandler = Rc<dyn Fn(Option<FanPick>, &mut Window, &mut App)>;

/// An agent's calls fanning out to tools.
#[derive(IntoElement)]
pub struct ToolFanoutGraph {
    id: ElementId,
    style: StyleRefinement,
    title: SharedString,
    subtitle: Option<SharedString>,
    agent: SharedString,
    calls: Vec<FanoutCall>,
    on_select: Option<SelectHandler>,
}

impl ToolFanoutGraph {
    /// Create a graph of the calls `agent` made.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        agent: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            title: title.into(),
            subtitle: None,
            agent: agent.into(),
            calls: Vec::new(),
            on_select: None,
        }
    }

    /// A quieter line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Add the calls to one tool.
    pub fn call(mut self, call: FanoutCall) -> Self {
        self.calls.push(call);
        self
    }

    /// Run with the node or line the reader picks, or none when cleared.
    pub fn on_select(
        mut self,
        handler: impl Fn(Option<FanPick>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for ToolFanoutGraph {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

fn stat(
    label: &'static str,
    value: String,
    color: Hsla,
    quiet: Hsla,
    mono: SharedString,
) -> impl IntoElement {
    v_flex()
        .min_w(px(72.))
        .child(div().text_caption().text_color(quiet).child(label))
        .child(
            div()
                .font_family(mono)
                .text_size(px(15.))
                .line_height(px(22.))
                .font_medium()
                .text_color(color)
                .child(value),
        )
}

impl RenderOnce for ToolFanoutGraph {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let part = |name: String| ElementId::NamedChild(self.id.clone().into(), name.into());
        let nodes = flatten(&self.agent, &self.calls);
        let hot = hottest_path(&nodes);
        let columns = column_count(&nodes);
        let height = row_count(&nodes) as f32 * ROW - (ROW - NODE_H);
        let peak = nodes
            .iter()
            .skip(1)
            .map(|node| node.calls)
            .max()
            .unwrap_or(1)
            .max(1);

        let theme = cx.theme();
        let ink = theme.foreground;
        let quiet = theme.muted_foreground;
        let accent = theme.chart_1;
        let warning = theme.warning;
        let danger = theme.danger;
        let surface = theme.background;
        let picked_fill = theme.list_active;
        let picked_border = theme.list_active_border;
        let ring = theme.ring;
        let hairline = theme.border;
        let mono = theme.mono_font_family.clone();
        let (fast, slow) = (theme.motion.duration_fast, theme.motion.duration_slow);
        let level_color = move |level: ErrorLevel| match level {
            ErrorLevel::Clean => quiet,
            ErrorLevel::Elevated => warning,
            ErrorLevel::High => danger,
        };

        let active = window.use_keyed_state(part("active".into()), cx, |_, _| None::<FanPick>);
        let hover = window.use_keyed_state(part("hover".into()), cx, |_, _| None::<FanPick>);
        let picked = (*active.read(cx)).filter(|pick| match pick {
            FanPick::Node(ix) => *ix < nodes.len(),
            FanPick::Edge(ix) => *ix > 0 && *ix < nodes.len(),
        });
        let hovered = *hover.read(cx);
        let graph_bounds = window
            .use_keyed_state(part("bounds".into()), cx, |_, _| {
                Rc::new(Cell::new(Bounds::<Pixels>::default()))
            })
            .read(cx)
            .clone();
        let drawn = Presence::new(part("appear".into()), true)
            .transition(Transition::new(slow * 2))
            .sample(window, cx)
            .progress;
        let focus = transition(
            part("focus".into()),
            if picked.is_some() { 1f32 } else { 0. },
            Transition::new(fast),
            window,
            cx,
        );

        // Which lines and nodes belong to the pick, so the rest can fade.
        let related_edge = |ix: usize| match picked {
            None => true,
            Some(FanPick::Edge(at)) => at == ix,
            Some(FanPick::Node(at)) => at == ix || nodes[ix].parent == Some(at),
        };
        let related_node = |ix: usize| match picked {
            None => true,
            Some(FanPick::Node(at)) => {
                at == ix || nodes[at].parent == Some(ix) || nodes[ix].parent == Some(at)
            }
            Some(FanPick::Edge(at)) => at == ix || nodes[at].parent == Some(ix),
        };

        // Hover and pick ease in rather than snap.
        let edge_lit: Vec<f32> = (0..nodes.len())
            .map(|ix| {
                let on = ix > 0
                    && (hovered == Some(FanPick::Edge(ix)) || picked == Some(FanPick::Edge(ix)));
                look::hover_fade(part(format!("edge-{ix}")), on, window, cx)
            })
            .collect();
        let node_lit: Vec<f32> = (0..nodes.len())
            .map(|ix| {
                let on = hovered == Some(FanPick::Node(ix));
                look::hover_fade(part(format!("node-{ix}")), on, window, cx)
            })
            .collect();

        struct Line {
            ix: usize,
            parent: usize,
            wide: f32,
            color: Hsla,
            strength: f32,
        }
        let lines: Vec<Line> = (1..nodes.len())
            .map(|ix| {
                let node = &nodes[ix];
                let level = error_level(error_rate(node.calls, node.errors));
                let color = if hot.contains(&ix) {
                    accent
                } else {
                    match level {
                        ErrorLevel::Clean => ink.opacity(0.3),
                        other => level_color(other),
                    }
                };
                let share = (node.calls as f32 / peak as f32).sqrt();
                Line {
                    ix,
                    parent: node.parent.unwrap_or(0),
                    wide: 1.25 + 3.5 * share + 1.5 * edge_lit[ix],
                    color,
                    strength: if related_edge(ix) {
                        1.
                    } else {
                        1. - 0.7 * focus
                    },
                }
            })
            .collect();

        let seen = graph_bounds.clone();
        let paint_nodes = nodes.clone();
        let graph = canvas(
            move |bounds, _, _| seen.set(bounds),
            move |bounds, _, window, _| {
                let geometry = FanGeometry {
                    width: f32::from(bounds.size.width),
                    columns,
                };
                for line in &lines {
                    let points =
                        geometry.edge_points(&paint_nodes[line.parent], &paint_nodes[line.ix]);
                    // Lines draw out from the agent, one hop after another.
                    let hop = paint_nodes[line.ix].depth as f32 - 1.;
                    let share = ((drawn * columns as f32 - hop) / 1.).clamp(0., 1.);
                    let shown = ((points.len() - 1) as f32 * share).ceil() as usize + 1;
                    if share <= 0. {
                        continue;
                    }
                    let mut path = PathBuilder::stroke(px(line.wide));
                    for (step, &(x, y)) in points.iter().take(shown).enumerate() {
                        let at = point(bounds.left() + px(x), bounds.top() + px(y));
                        if step == 0 {
                            path.move_to(at);
                        } else {
                            path.line_to(at);
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, line.color.opacity(line.color.a * line.strength));
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        // Node boxes and each edge's figures sit in columns spread edge to edge.
        let column_divs = (0..columns).map(|depth| {
            let boxes =
                nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| node.depth == depth)
                    .map(|(ix, node)| {
                        let is_picked = picked == Some(FanPick::Node(ix));
                        let on_path = ix == 0 || hot.contains(&ix);
                        let rate = error_rate(node.calls, node.errors);
                        let shown =
                            ((drawn * columns as f32 - depth as f32 + 0.6) / 0.6).clamp(0., 1.);
                        let strength = if related_node(ix) {
                            1.
                        } else {
                            1. - 0.55 * focus
                        };
                        let sub = format!("{} calls", node.calls);
                        let figures = (ix > 0).then(|| {
                            let level = error_level(rate);
                            let edge_picked = picked == Some(FanPick::Edge(ix));
                            h_flex()
                                .absolute()
                                .top(px(node.row * ROW + NODE_H / 2. - 19.))
                                .right(px(NODE_W + 8.))
                                .gap(px(6.))
                                .px(px(4.))
                                .rounded(px(4.))
                                .bg(surface)
                                .when(edge_picked, |this| this.bg(picked_fill))
                                .font_family(mono.clone())
                                .text_size(px(10.))
                                .line_height(px(14.))
                                .whitespace_nowrap()
                                .opacity(
                                    shown
                                        * if related_edge(ix) {
                                            1.
                                        } else {
                                            1. - 0.7 * focus
                                        },
                                )
                                .child(div().text_color(ink).child(format!("{}×", node.calls)))
                                .child(div().text_color(quiet).child(latency_text(node.latency_ms)))
                                .child(div().text_color(level_color(level)).child(rate_text(rate)))
                        });
                        let box_ =
                            h_flex()
                                .absolute()
                                .top(px(node.row * ROW))
                                .left_0()
                                .w(px(NODE_W))
                                .h(px(NODE_H))
                                .gap_2()
                                .px_2()
                                .rounded(px(10.))
                                .bg(surface)
                                .border_1()
                                .border_color(look::mix(
                                    if on_path {
                                        accent.opacity(0.6)
                                    } else {
                                        hairline
                                    },
                                    ink.opacity(0.4),
                                    node_lit[ix],
                                ))
                                .when(is_picked, |this| {
                                    this.bg(picked_fill).border_color(picked_border)
                                })
                                .opacity(shown * strength)
                                .cursor_pointer()
                                .child(
                                    Icon::new(if ix == 0 {
                                        CatalogIcon::AiAgent
                                    } else {
                                        CatalogIcon::ToolKit
                                    })
                                    .size(px(14.))
                                    .text_color(if on_path { accent } else { quiet }),
                                )
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .flex_1()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(12.))
                                                .line_height(px(16.))
                                                .font_medium()
                                                .child(node.label.clone()),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .font_family(mono.clone())
                                                .text_size(px(10.))
                                                .line_height(px(14.))
                                                .text_color(quiet)
                                                .child(sub),
                                        ),
                                );
                        div().children(figures).child(box_)
                    });
            div()
                .relative()
                .flex_none()
                .w(px(NODE_W))
                .h(px(height))
                .children(boxes)
        });

        let readout = {
            let row = |pick: FanPick| -> (String, String, Vec<(&'static str, String, Hsla)>) {
                match pick {
                    FanPick::Node(0) => (
                        nodes[0].label.to_string(),
                        "Agent".into(),
                        vec![
                            ("Calls", nodes[0].calls.to_string(), ink),
                            ("Tools", (nodes.len() - 1).to_string(), ink),
                            (
                                "Errors",
                                rate_text(error_rate(nodes[0].calls, nodes[0].errors)),
                                level_color(error_level(error_rate(
                                    nodes[0].calls,
                                    nodes[0].errors,
                                ))),
                            ),
                        ],
                    ),
                    FanPick::Node(ix) | FanPick::Edge(ix) => {
                        let node = &nodes[ix];
                        let caller = &nodes[node.parent.unwrap_or(0)];
                        let rate = error_rate(node.calls, node.errors);
                        let (title, kind) = match pick {
                            FanPick::Edge(_) => {
                                (format!("{} → {}", caller.label, node.label), "Line".into())
                            }
                            _ => (
                                node.label.to_string(),
                                format!(
                                    "Tool, called by {}{}",
                                    caller.label,
                                    match node.children.len() {
                                        0 => String::new(),
                                        more => format!(", calls {more}"),
                                    }
                                ),
                            ),
                        };
                        let share = if caller.calls > 0 {
                            format!(
                                "{}%",
                                (node.calls as f64 / caller.calls as f64 * 100.).round()
                            )
                        } else {
                            "—".into()
                        };
                        (
                            title,
                            kind,
                            vec![
                                ("Calls", node.calls.to_string(), ink),
                                ("Latency", latency_text(node.latency_ms), ink),
                                (
                                    "Errors",
                                    format!("{} · {}", node.errors, rate_text(rate)),
                                    level_color(error_level(rate)),
                                ),
                                ("Of caller", share, ink),
                            ],
                        )
                    }
                }
            };
            match picked {
                Some(pick) => {
                    let (title, kind, stats) = row(pick);
                    let on_hot = match pick {
                        FanPick::Node(ix) | FanPick::Edge(ix) => ix > 0 && hot.contains(&ix),
                    };
                    v_flex()
                        .id(part("readout".into()))
                        .mt_3()
                        .px_1()
                        .gap_2()
                        .role(Role::Status)
                        .aria_label(spoken_pick(&nodes, pick, &hot))
                        .child(
                            h_flex()
                                .flex_wrap()
                                .gap_2()
                                .child(div().text_label().child(title))
                                .child(div().text_caption().text_color(quiet).child(kind))
                                .when(on_hot, |this| {
                                    this.child(
                                        Tag::info()
                                            .small()
                                            .gap_1()
                                            .child(Icon::new(CatalogIcon::Fire).size(px(12.)))
                                            .child("Hottest path"),
                                    )
                                }),
                        )
                        .child(h_flex().flex_wrap().gap_x_6().gap_y_2().children(
                            stats.into_iter().map(|(label, value, color)| {
                                stat(label, value, color, quiet, mono.clone())
                            }),
                        ))
                        .into_any_element()
                }
                None => {
                    let path: Vec<String> = std::iter::once(0)
                        .chain(hot.iter().copied())
                        .map(|ix| nodes[ix].label.to_string())
                        .collect();
                    h_flex()
                        .mt_3()
                        .px_1()
                        .gap_2()
                        .text_caption()
                        .text_color(quiet)
                        .child(
                            Icon::new(CatalogIcon::Fire)
                                .size(px(14.))
                                .text_color(accent),
                        )
                        .child("Hottest path")
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(ink)
                                .child(path.join(" → ")),
                        )
                        .into_any_element()
                }
            }
        };

        let spoken = match picked {
            Some(pick) => format!("{}: {}", self.title, spoken_pick(&nodes, pick, &hot)),
            None => format!(
                "{}: {} calls from {} to {} tools; hottest path {}. Arrow keys walk the graph.",
                self.title,
                nodes[0].calls,
                nodes[0].label,
                nodes.len() - 1,
                std::iter::once(0)
                    .chain(hot.iter().copied())
                    .map(|ix| nodes[ix].label.to_string())
                    .collect::<Vec<_>>()
                    .join(" to ")
            ),
        };

        let legend = h_flex()
            .mt_3()
            .px_1()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .text_caption()
            .text_color(quiet)
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().w(px(14.)).h(px(3.)).rounded_full().bg(accent))
                    .child("Hottest path"),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().w(px(14.)).h(px(3.)).rounded_full().bg(warning))
                    .child("Errors 1%+"),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().w(px(14.)).h(px(3.)).rounded_full().bg(danger))
                    .child("Errors 10%+"),
            )
            .child(div().child("Thicker lines carry more calls"));

        let move_state = hover.clone();
        let move_nodes = nodes.clone();
        let move_bounds = graph_bounds.clone();
        let move_select = self.on_select.clone();
        let move_active = active.clone();
        let leave_state = hover;
        let leave_active = active.clone();
        let leave_select = self.on_select.clone();
        let down_state = active.clone();
        let down_nodes = nodes.clone();
        let down_bounds = graph_bounds;
        let down_select = self.on_select.clone();
        let key_state = active;
        let key_nodes = nodes.clone();
        let key_select = self.on_select.clone();
        let under = move |bounds: Bounds<Pixels>,
                          nodes: &[FanNode],
                          position: isonomic_ui::Point<Pixels>| {
            let geometry = FanGeometry {
                width: f32::from(bounds.size.width),
                columns,
            };
            pick_at(
                nodes,
                geometry,
                f32::from(position.x - bounds.left()),
                f32::from(position.y - bounds.top()),
            )
        };

        shell(cx)
            .id(self.id.clone())
            .max_w(px(720.))
            .child(shell_header(
                Some(Icon::new(CatalogIcon::ChartNetwork)),
                self.title.clone(),
                self.subtitle.clone().map(|line| line.into_any_element()),
                Some(
                    div()
                        .font_family(mono.clone())
                        .text_caption()
                        .text_color(quiet)
                        .child(format!("{} calls", nodes[0].calls))
                        .into_any_element(),
                ),
                cx,
            ))
            .child(
                panel(cx)
                    .p_4()
                    .child(
                        div()
                            .id(part("graph".into()))
                            .test_support()
                            .relative()
                            .w_full()
                            .p(px(6.))
                            .rounded(px(12.))
                            .role(Role::Group)
                            .aria_label(spoken)
                            .tab_index(0)
                            .border_2()
                            .border_color(ring.opacity(0.))
                            .focus_visible(move |style| style.border_color(ring))
                            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                                let next = under(move_bounds.get(), &move_nodes, event.position);
                                if *move_state.read(cx) != next {
                                    move_state.update(cx, |state, cx| {
                                        *state = next;
                                        cx.notify();
                                    });
                                    // Pointing reads a part; it does not move a pick made by click or key.
                                    if move_active.read(cx).is_none()
                                        && let Some(select) = &move_select
                                    {
                                        select(next, window, cx);
                                    }
                                }
                            })
                            .on_hover(move |over, window, cx| {
                                if !*over {
                                    leave_state.update(cx, |state, cx| {
                                        *state = None;
                                        cx.notify();
                                    });
                                    if leave_active.read(cx).is_none()
                                        && let Some(select) = &leave_select
                                    {
                                        select(None, window, cx);
                                    }
                                }
                            })
                            .on_mouse_down(
                                isonomic_ui::MouseButton::Left,
                                move |event: &MouseDownEvent, window, cx| {
                                    let hit = under(down_bounds.get(), &down_nodes, event.position);
                                    // Clicking the picked part again, or empty space, clears it.
                                    let next = if hit == *down_state.read(cx) {
                                        None
                                    } else {
                                        hit
                                    };
                                    down_state.update(cx, |state, cx| {
                                        *state = next;
                                        cx.notify();
                                    });
                                    if let Some(select) = &down_select {
                                        select(next, window, cx);
                                    }
                                },
                            )
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                let now = *key_state.read(cx);
                                let Some(next) =
                                    step(&key_nodes, now, event.keystroke.key.as_str())
                                else {
                                    return;
                                };
                                key_state.update(cx, |state, cx| {
                                    *state = next;
                                    cx.notify();
                                });
                                if let Some(select) = &key_select {
                                    select(next, window, cx);
                                }
                                cx.stop_propagation();
                            })
                            .child(
                                div().relative().w_full().h(px(height)).child(graph).child(
                                    h_flex()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .size_full()
                                        .items_start()
                                        .when(columns > 1, |this| this.justify_between())
                                        .children(column_divs),
                                ),
                            ),
                    )
                    .child(readout)
                    .child(legend),
            )
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calls() -> Vec<FanoutCall> {
        vec![
            FanoutCall::new("search", 40, 220.)
                .errors(1)
                .child(FanoutCall::new("fetch page", 60, 900.).errors(9))
                .child(FanoutCall::new("cache", 30, 4.)),
            FanoutCall::new("calendar", 12, 140.),
            FanoutCall::new("mail", 12, 310.).errors(3),
        ]
    }

    fn graph() -> Vec<FanNode> {
        flatten(&"planner".into(), &calls())
    }

    #[test]
    fn calls_flatten_depth_first_and_callers_centre_on_their_tools() {
        let nodes = graph();
        let labels: Vec<&str> = nodes.iter().map(|node| node.label.as_ref()).collect();
        assert_eq!(
            labels,
            [
                "planner",
                "search",
                "fetch page",
                "cache",
                "calendar",
                "mail"
            ]
        );
        assert_eq!(nodes[0].calls, 64);
        assert_eq!(nodes[0].errors, 4);
        assert_eq!(nodes[2].parent, Some(1));
        assert_eq!(nodes[1].children, [2, 3]);
        let rows: Vec<f32> = nodes.iter().map(|node| node.row).collect();
        assert_eq!(rows, [1.75, 0.5, 0., 1., 2., 3.]);
        assert_eq!(row_count(&nodes), 4);
        assert_eq!(column_count(&nodes), 3);
        let lone = flatten(&"solo".into(), &[]);
        assert_eq!((row_count(&lone), column_count(&lone)), (1, 1));
    }

    #[test]
    fn the_hottest_path_follows_the_busiest_edge() {
        let nodes = graph();
        assert_eq!(hottest_path(&nodes), [1, 2]);
        // Ties go to the first.
        let tie = flatten(
            &"a".into(),
            &[FanoutCall::new("x", 5, 1.), FanoutCall::new("y", 5, 1.)],
        );
        assert_eq!(hottest_path(&tie), [1]);
        assert!(hottest_path(&flatten(&"a".into(), &[])).is_empty());
    }

    #[test]
    fn error_rates_and_latencies_read_short() {
        assert_eq!(error_rate(60, 9), 0.15);
        assert_eq!(error_rate(0, 0), 0.);
        assert_eq!(error_level(0.005), ErrorLevel::Clean);
        assert_eq!(error_level(0.025), ErrorLevel::Elevated);
        assert_eq!(error_level(0.15), ErrorLevel::High);
        assert_eq!(rate_text(0.), "0%");
        assert_eq!(rate_text(0.025), "2.5%");
        assert_eq!(rate_text(0.15), "15%");
        assert_eq!(latency_text(4.2), "4ms");
        assert_eq!(latency_text(1440.), "1.4s");
        assert_eq!(latency_text(12_600.), "13s");
        // Errors never exceed calls.
        assert_eq!(FanoutCall::new("x", 2, 1.).errors(5).errors, 2);
    }

    #[test]
    fn keys_walk_from_node_to_line_to_node() {
        let nodes = graph();
        let walk = |now, key| step(&nodes, now, key).unwrap();
        assert_eq!(walk(None, "right"), Some(FanPick::Node(0)));
        // Right from a node takes its busiest line, then the tool.
        assert_eq!(
            walk(Some(FanPick::Node(0)), "right"),
            Some(FanPick::Edge(1))
        );
        assert_eq!(
            walk(Some(FanPick::Edge(1)), "right"),
            Some(FanPick::Node(1))
        );
        assert_eq!(
            walk(Some(FanPick::Node(1)), "right"),
            Some(FanPick::Edge(2))
        );
        // Left walks back the same way.
        assert_eq!(walk(Some(FanPick::Node(2)), "left"), Some(FanPick::Edge(2)));
        assert_eq!(walk(Some(FanPick::Edge(2)), "left"), Some(FanPick::Node(1)));
        // Up and down stay among siblings, stopping at the ends.
        assert_eq!(walk(Some(FanPick::Node(1)), "down"), Some(FanPick::Node(4)));
        assert_eq!(walk(Some(FanPick::Node(5)), "down"), Some(FanPick::Node(5)));
        assert_eq!(walk(Some(FanPick::Edge(4)), "up"), Some(FanPick::Edge(1)));
        assert_eq!(walk(Some(FanPick::Node(0)), "up"), Some(FanPick::Node(0)));
        // A tool with nothing below stays put on Right.
        assert_eq!(
            walk(Some(FanPick::Node(5)), "right"),
            Some(FanPick::Node(5))
        );
        assert_eq!(walk(Some(FanPick::Node(3)), "home"), Some(FanPick::Node(0)));
        assert_eq!(walk(Some(FanPick::Node(3)), "escape"), None);
        assert_eq!(step(&nodes, None, "a"), None);
    }

    #[test]
    fn the_pointer_picks_boxes_first_then_the_nearest_line() {
        let nodes = graph();
        let geometry = FanGeometry {
            width: 532.,
            columns: 3,
        };
        // Columns at 0, 200 and 400.
        assert_eq!(geometry.column_x(2), 400.);
        let (left, top, ..) = geometry.node_box(&nodes[2]);
        assert_eq!((left, top), (400., 0.));
        assert_eq!(pick_at(&nodes, geometry, 410., 10.), Some(FanPick::Node(2)));
        // On the straight run into "mail", just left of its box.
        let mail_y = nodes[5].row * ROW + NODE_H / 2.;
        assert_eq!(
            pick_at(&nodes, geometry, 190., mail_y + 2.),
            Some(FanPick::Edge(5))
        );
        assert_eq!(pick_at(&nodes, geometry, 300., 500.), None);
        let line = geometry.edge_points(&nodes[0], &nodes[5]);
        assert_eq!(
            line.first(),
            Some(&(NODE_W, nodes[0].row * ROW + NODE_H / 2.))
        );
        assert_eq!(line.last(), Some(&(200., mail_y)));
        assert_eq!(distance_to_line(&[(0., 0.), (10., 0.)], 5., 3.), 3.);
        assert_eq!(distance_to_line(&[(0., 0.), (10., 0.)], 13., 4.), 5.);
    }

    #[test]
    fn picks_read_in_words() {
        let nodes = graph();
        let hot = hottest_path(&nodes);
        assert_eq!(
            spoken_pick(&nodes, FanPick::Edge(2), &hot),
            "Line from search to fetch page: 60 calls, 900ms average, 9 failed, 15% error rate, on the hottest path"
        );
        assert_eq!(
            spoken_pick(&nodes, FanPick::Node(1), &hot),
            "search, tool, called by planner: 40 calls, 220ms average, 1 failed, 2.5% error rate, calls 2 tools, on the hottest path"
        );
        assert_eq!(
            spoken_pick(&nodes, FanPick::Node(0), &hot),
            "planner, agent, 3 tools called directly, 64 calls in all"
        );
    }
}
