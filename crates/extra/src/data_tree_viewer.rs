// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data tree viewer: structured output as a tree to open, search and copy from.
//!
//! Every row shows a key and its value, coloured by type; objects and lists
//! show how many entries they hold and open on click. Clicking a key copies
//! its path (`$.order.items[0].sku`), Enter on a value copies the value.
//! Search matches keys and values, opens the branches that lead to a match
//! and marks it. The tree is one tab stop: arrows move, right and left open
//! and close.

use std::{collections::HashSet, time::Duration};

use isonomic_ui::assets::IconName as CatalogIcon;
use isonomic_ui::base::TestSupportExt as _;
use isonomic_ui::component::{
    ActiveTheme as _, Icon, Sizable as _, TextRole as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    look,
};
use isonomic_ui::{
    AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, Role,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, prelude::FluentBuilder as _, px,
};

use crate::common::{panel, shell, shell_header};

/// How long "copied" stays beside a row.
const COPIED_FOR: Duration = Duration::from_millis(1400);
/// Indent per level, and before the first.
const INDENT: f32 = 18.;
const INDENT_START: f32 = 12.;

/// A structured value.
#[derive(Debug, Clone, PartialEq)]
pub enum DataValue {
    /// Nothing.
    Null,
    /// True or false.
    Bool(bool),
    /// A number.
    Number(f64),
    /// Text.
    Text(String),
    /// Values in order.
    List(Vec<DataValue>),
    /// Named values, in the order given.
    Map(Vec<(String, DataValue)>),
}

impl DataValue {
    /// A map from pairs.
    pub fn map<K: Into<String>>(entries: impl IntoIterator<Item = (K, DataValue)>) -> Self {
        Self::Map(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }

    /// A list.
    pub fn list(items: impl IntoIterator<Item = DataValue>) -> Self {
        Self::List(items.into_iter().collect())
    }

    /// Text.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    fn is_branch(&self) -> bool {
        matches!(self, Self::List(_) | Self::Map(_))
    }

    /// One value as it is written in the tree and copied: quoted text, plain
    /// numbers, `[3]` or `{2}` for a branch.
    pub fn summary(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => number_text(*value),
            Self::Text(text) => quoted(text),
            Self::List(items) => format!("[{}]", items.len()),
            Self::Map(entries) => format!("{{{}}}", entries.len()),
        }
    }

    /// The whole value written out with two-space indents.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        let pad = "  ".repeat(depth + 1);
        let close = "  ".repeat(depth);
        match self {
            Self::List(items) if !items.is_empty() => {
                out.push_str("[\n");
                for (ix, item) in items.iter().enumerate() {
                    out.push_str(&pad);
                    item.write(out, depth + 1);
                    out.push_str(if ix + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&close);
                out.push(']');
            }
            Self::Map(entries) if !entries.is_empty() => {
                out.push_str("{\n");
                for (ix, (key, value)) in entries.iter().enumerate() {
                    out.push_str(&pad);
                    out.push_str(&quoted(key));
                    out.push_str(": ");
                    value.write(out, depth + 1);
                    out.push_str(if ix + 1 < entries.len() { ",\n" } else { "\n" });
                }
                out.push_str(&close);
                out.push('}');
            }
            Self::List(_) => out.push_str("[]"),
            Self::Map(_) => out.push_str("{}"),
            leaf => out.push_str(&leaf.summary()),
        }
    }
}

fn number_text(value: f64) -> String {
    if value.fract() == 0. && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// The path of a named child: `$.name`, or `$["two words"]` when the name is
/// not a plain identifier.
pub fn key_path(parent: &str, key: &str) -> String {
    let mut chars = key.chars();
    let plain = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$');
    if plain {
        format!("{parent}.{key}")
    } else {
        format!("{parent}[{}]", quoted(key))
    }
}

/// One row of the flattened tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Where it sits: `$.order.items[0]`.
    pub path: String,
    /// Its key or index; none for the root.
    pub label: Option<String>,
    /// Its value as written in the row.
    pub summary: String,
    /// How deep it sits; the root is 0.
    pub depth: usize,
    /// The row of its parent.
    pub parent: Option<usize>,
    /// It holds other values.
    pub branch: bool,
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Number,
    Bool,
    Other,
}

/// Flatten a value into rows, parents before children.
pub fn flatten(root: &DataValue) -> Vec<Node> {
    fn walk(
        value: &DataValue,
        path: String,
        label: Option<String>,
        depth: usize,
        parent: Option<usize>,
        out: &mut Vec<Node>,
    ) {
        let at = out.len();
        out.push(Node {
            summary: value.summary(),
            label,
            depth,
            parent,
            branch: value.is_branch(),
            kind: match value {
                DataValue::Text(_) => Kind::Text,
                DataValue::Number(_) => Kind::Number,
                DataValue::Bool(_) => Kind::Bool,
                _ => Kind::Other,
            },
            path: path.clone(),
        });
        match value {
            DataValue::List(items) => {
                for (ix, item) in items.iter().enumerate() {
                    walk(
                        item,
                        format!("{path}[{ix}]"),
                        Some(ix.to_string()),
                        depth + 1,
                        Some(at),
                        out,
                    );
                }
            }
            DataValue::Map(entries) => {
                for (key, item) in entries {
                    walk(
                        item,
                        key_path(&path, key),
                        Some(key.clone()),
                        depth + 1,
                        Some(at),
                        out,
                    );
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(root, "$".into(), None, 0, None, &mut out);
    out
}

/// The rows whose key, or whose value when it is not a branch, contains the
/// query, ignoring case.
pub fn matching(nodes: &[Node], query: &str) -> HashSet<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return HashSet::new();
    }
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            node.label
                .as_ref()
                .is_some_and(|label| label.to_lowercase().contains(&query))
                || (!node.branch && node.summary.to_lowercase().contains(&query))
        })
        .map(|(ix, _)| ix)
        .collect()
}

/// The rows on show: every ancestor is open, or leads to a match.
pub fn visible(nodes: &[Node], open: &HashSet<usize>, matches: &HashSet<usize>) -> Vec<usize> {
    let mut forced = HashSet::new();
    for &ix in matches {
        let mut up = nodes[ix].parent;
        while let Some(parent) = up {
            forced.insert(parent);
            up = nodes[parent].parent;
        }
    }
    (0..nodes.len())
        .filter(|&ix| {
            let mut up = nodes[ix].parent;
            while let Some(parent) = up {
                if !open.contains(&parent) && !forced.contains(&parent) {
                    return false;
                }
                up = nodes[parent].parent;
            }
            true
        })
        .collect()
}

/// Split `text` around the first place it contains `query`, ignoring case.
pub fn split_match<'a>(text: &'a str, query: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    let lower = text.to_lowercase();
    let at = lower.find(&query.to_lowercase())?;
    let end = at + query.len();
    // Lower-casing can change byte lengths; only split on real boundaries.
    (lower.len() == text.len() && text.is_char_boundary(at) && text.is_char_boundary(end))
        .then(|| (&text[..at], &text[at..end], &text[end..]))
}

/// What was copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataTreeEvent {
    /// A path, such as `$.order.total`.
    CopiedPath(String),
    /// A value, or the whole document.
    CopiedValue(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Copied {
    Path(usize),
    Value(usize),
    All,
}

/// A structured value as a tree.
pub struct DataTreeViewer {
    title: SharedString,
    root: DataValue,
    nodes: Vec<Node>,
    open: HashSet<usize>,
    cursor: usize,
    copied: Option<(Copied, u64)>,
    copies: u64,
    search: Entity<InputState>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    _events: Subscription,
}

impl EventEmitter<DataTreeEvent> for DataTreeViewer {}

impl DataTreeViewer {
    /// Show `root`, with branches open down to `depth` levels.
    pub fn new(
        title: impl Into<SharedString>,
        root: DataValue,
        depth: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search keys and values"));
        let events = cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let nodes = flatten(&root);
        let open = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.branch && node.depth < depth)
            .map(|(ix, _)| ix)
            .collect();
        Self {
            title: title.into(),
            root,
            nodes,
            open,
            cursor: 0,
            copied: None,
            copies: 0,
            search,
            focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            _events: events,
        }
    }

    /// Open every branch.
    pub fn expand_all(&mut self, cx: &mut Context<Self>) {
        self.open = (0..self.nodes.len())
            .filter(|&ix| self.nodes[ix].branch)
            .collect();
        cx.notify();
    }

    /// Close everything below the root.
    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.open = HashSet::from([0]);
        self.cursor = 0;
        cx.notify();
    }

    fn toggle(&mut self, ix: usize, open: Option<bool>, cx: &mut Context<Self>) {
        if open.unwrap_or(!self.open.contains(&ix)) {
            self.open.insert(ix);
        } else {
            self.open.remove(&ix);
        }
        cx.notify();
    }

    fn copy(&mut self, what: Copied, cx: &mut Context<Self>) {
        let (text, event) = match what {
            Copied::Path(ix) => {
                let path = self.nodes[ix].path.clone();
                (path.clone(), DataTreeEvent::CopiedPath(path))
            }
            Copied::Value(ix) => {
                let text = self.nodes[ix].summary.clone();
                (text.clone(), DataTreeEvent::CopiedValue(text))
            }
            Copied::All => {
                let text = self.root.pretty();
                (text.clone(), DataTreeEvent::CopiedValue(text))
            }
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        cx.emit(event);
        self.copies += 1;
        let token = self.copies;
        self.copied = Some((what, token));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            let _ = this.update(cx, |this, cx| {
                if this.copied.is_some_and(|(_, at)| at == token) {
                    this.copied = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn on_key(&mut self, key: &str, rows: &[usize], cx: &mut Context<Self>) -> bool {
        let Some(at) = rows.iter().position(|&ix| ix == self.cursor) else {
            self.cursor = rows.first().copied().unwrap_or(0);
            return false;
        };
        let ix = self.cursor;
        let node = &self.nodes[ix];
        let (branch, is_open, parent) = (node.branch, self.open.contains(&ix), node.parent);
        let next = rows.get(at + 1).copied();
        match key {
            "down" => self.cursor = next.unwrap_or(ix),
            "up" => self.cursor = rows[at.saturating_sub(1)],
            "home" => self.cursor = rows[0],
            "end" => self.cursor = rows[rows.len() - 1],
            "right" if branch && is_open => self.cursor = next.unwrap_or(ix),
            "right" if branch => self.toggle(ix, Some(true), cx),
            "left" if branch && is_open => self.toggle(ix, Some(false), cx),
            "left" => self.cursor = parent.unwrap_or(ix),
            "enter" if branch => self.toggle(ix, None, cx),
            "enter" => self.copy(Copied::Value(ix), cx),
            _ => return false,
        }
        if let Some(row) = rows.iter().position(|&ix| ix == self.cursor) {
            self.scroll.scroll_to_item(row);
        }
        cx.notify();
        true
    }
}

fn marked(text: &str, query: &str, mark: Hsla) -> impl IntoElement {
    match split_match(text, query) {
        Some((before, hit, after)) => h_flex()
            .min_w_0()
            .child(before.to_string())
            .child(div().rounded(px(2.)).bg(mark).child(hit.to_string()))
            .child(div().min_w_0().truncate().child(after.to_string())),
        None => h_flex()
            .min_w_0()
            .child(div().min_w_0().truncate().child(text.to_string())),
    }
}

impl Render for DataTreeViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search.read(cx).value().to_string();
        let matches = matching(&self.nodes, &query);
        let rows = visible(&self.nodes, &self.open, &matches);
        let leaves = self.nodes.iter().filter(|node| !node.branch).count();
        let branches = self.nodes.len() - leaves;
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let (key_ink, text_ink, number_ink, bool_ink) =
            (theme.info, theme.success, theme.warning, theme.chart_4);
        let good = theme.success;
        let mark = theme.warning.opacity(0.3);
        let match_wash = theme.warning.opacity(0.08);
        let hover = look::ink::wash(cx);
        let cursor_wash = look::ink::wash_strong(cx);
        let mono = theme.mono_font_family.clone();
        let focused = self.focus.is_focused(window);
        let keys = rows.clone();

        let lines = rows.iter().map(|&ix| {
            let node = &self.nodes[ix];
            let is_open = self.open.contains(&ix)
                || (!query.trim().is_empty() && {
                    // Opened by the search when it leads to a match.
                    rows.iter().any(|&row| self.nodes[row].parent == Some(ix))
                });
            let label: SharedString = node
                .label
                .clone()
                .map(SharedString::from)
                .unwrap_or_else(|| self.title.clone());
            let note = self.copied.and_then(|(what, _)| match what {
                Copied::Path(at) if at == ix => Some("Path copied"),
                Copied::Value(at) if at == ix => Some("Copied"),
                _ => None,
            });
            let value_ink = if node.branch {
                quiet
            } else {
                match node.kind {
                    Kind::Text => text_ink,
                    Kind::Number => number_ink,
                    Kind::Bool => bool_ink,
                    Kind::Other => quiet,
                }
            };
            let branch = node.branch;
            h_flex()
                .id(("row", ix))
                .test_support()
                .h(px(24.))
                .gap_1()
                .pr_2()
                .pl(px(INDENT_START + INDENT * node.depth as f32))
                .when(matches.contains(&ix), |this| this.bg(match_wash))
                .when(focused && self.cursor == ix, |this| this.bg(cursor_wash))
                .hover(move |style| style.bg(hover))
                .role(Role::TreeItem)
                .aria_label(format!(
                    "{label}: {}{}",
                    node.summary,
                    match (branch, is_open) {
                        (true, true) => ", open",
                        (true, false) => ", closed",
                        _ => "",
                    }
                ))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.cursor = ix;
                    this.focus.focus(window, cx);
                    if branch {
                        this.toggle(ix, None, cx);
                    } else {
                        cx.notify();
                    }
                }))
                .child(
                    div()
                        .flex_none()
                        .size(px(12.))
                        .text_color(quiet)
                        .when(branch, |this| {
                            this.child(
                                Icon::new(if is_open {
                                    CatalogIcon::ChevronDown
                                } else {
                                    CatalogIcon::ChevronRight
                                })
                                .size(px(12.)),
                            )
                        }),
                )
                .child(
                    div()
                        .id(("key", ix))
                        .test_support()
                        .flex_none()
                        .max_w(px(220.))
                        .px(px(2.))
                        .rounded(px(4.))
                        .text_color(key_ink)
                        .cursor_pointer()
                        .hover(move |style| style.bg(cursor_wash))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.cursor = ix;
                            this.copy(Copied::Path(ix), cx);
                        }))
                        .child(marked(&label, &query, mark)),
                )
                .child(div().text_color(quiet).child(":"))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_color(value_ink)
                        .child(if branch {
                            marked(&node.summary, "", mark).into_any_element()
                        } else {
                            marked(&node.summary, &query, mark).into_any_element()
                        }),
                )
                .when_some(note, |this, note| {
                    this.child(
                        h_flex()
                            .flex_none()
                            .gap_1()
                            .font_family(cx.theme().font_family.clone())
                            .text_caption()
                            .text_color(good)
                            .child(Icon::new(CatalogIcon::Checkmark).size(px(12.)))
                            .child(note),
                    )
                })
        });

        let copied_all = matches!(self.copied, Some((Copied::All, _)));
        let aside = h_flex()
            .gap_1()
            .child(
                Button::new("expand-all")
                    .ghost()
                    .small()
                    .icon(Icon::new(CatalogIcon::ExpandAll))
                    .tooltip("Expand all")
                    .accessibility_label("Expand all")
                    .on_click(cx.listener(|this, _, _, cx| this.expand_all(cx))),
            )
            .child(
                Button::new("collapse-all")
                    .ghost()
                    .small()
                    .icon(Icon::new(CatalogIcon::CollapseAll))
                    .tooltip("Collapse all")
                    .accessibility_label("Collapse all")
                    .on_click(cx.listener(|this, _, _, cx| this.collapse_all(cx))),
            )
            .child(
                Button::new("copy-all")
                    .ghost()
                    .small()
                    .icon(Icon::new(if copied_all {
                        CatalogIcon::Checkmark
                    } else {
                        CatalogIcon::Copy
                    }))
                    .tooltip(if copied_all { "Copied" } else { "Copy all" })
                    .accessibility_label(if copied_all { "Copied" } else { "Copy all" })
                    .on_click(cx.listener(|this, _, _, cx| this.copy(Copied::All, cx))),
            );

        let searching = !query.trim().is_empty();
        let footer: SharedString = if searching && matches.is_empty() {
            format!("Nothing matches “{}”.", query.trim()).into()
        } else if searching {
            format!(
                "{} match{}",
                matches.len(),
                if matches.len() == 1 { "" } else { "es" }
            )
            .into()
        } else {
            "Click a key to copy its path · ↑↓ to move, → ← to open and close".into()
        };

        shell(cx)
            .id("data-tree")
            .max_w(px(576.))
            .role(Role::Region)
            .aria_label(self.title.clone())
            .child(shell_header(
                Some(Icon::new(CatalogIcon::DataStructured)),
                self.title.clone(),
                Some(format!("{leaves} values · {branches} objects and lists").into_any_element()),
                Some(aside.into_any_element()),
                cx,
            ))
            .child(
                div().px(px(6.)).pb_2().child(
                    Input::new(&self.search).cleanable(true).prefix(
                        Icon::new(CatalogIcon::Search)
                            .size(px(14.))
                            .text_color(quiet),
                    ),
                ),
            )
            .child(
                panel(cx)
                    .id("tree")
                    .test_support()
                    .max_h(px(384.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .py_2()
                    .font_family(mono)
                    .text_size(px(13.))
                    .line_height(px(24.))
                    .role(Role::Tree)
                    .aria_label(format!("{}, {leaves} values", self.title))
                    .track_focus(&self.focus)
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if this.on_key(event.keystroke.key.as_str(), &keys, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .children(lines),
            )
            .child(
                div()
                    .px(px(14.))
                    .pt_2()
                    .pb(px(6.))
                    .text_caption()
                    .text_color(quiet)
                    .child(footer),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> DataValue {
        DataValue::map([
            ("id", DataValue::text("A-104")),
            ("paid", DataValue::Bool(true)),
            (
                "items",
                DataValue::list([
                    DataValue::map([
                        ("sku", DataValue::text("mug-blue")),
                        ("qty", DataValue::Number(2.)),
                    ]),
                    DataValue::map([
                        ("sku", DataValue::text("plate")),
                        ("qty", DataValue::Number(6.)),
                    ]),
                ]),
            ),
            ("gift note", DataValue::Null),
        ])
    }

    #[test]
    fn paths_use_dots_indexes_and_quotes() {
        assert_eq!(key_path("$", "id"), "$.id");
        assert_eq!(key_path("$", "gift note"), "$[\"gift note\"]");
        assert_eq!(key_path("$", "2nd"), "$[\"2nd\"]");
        let nodes = flatten(&order());
        let paths: Vec<_> = nodes.iter().map(|node| node.path.as_str()).collect();
        assert_eq!(paths[0], "$");
        assert!(paths.contains(&"$.items[1].qty"));
        assert!(paths.contains(&"$[\"gift note\"]"));
        assert_eq!(nodes.len(), 11);
    }

    #[test]
    fn values_are_written_by_type() {
        assert_eq!(DataValue::text("a \"b\"").summary(), "\"a \\\"b\\\"\"");
        assert_eq!(DataValue::Number(2.).summary(), "2");
        assert_eq!(DataValue::Number(2.5).summary(), "2.5");
        assert_eq!(DataValue::Null.summary(), "null");
        assert_eq!(order().summary(), "{4}");
        assert_eq!(DataValue::list([]).pretty(), "[]");
        assert_eq!(
            DataValue::map([("a", DataValue::list([DataValue::Number(1.)]))]).pretty(),
            "{\n  \"a\": [\n    1\n  ]\n}"
        );
    }

    #[test]
    fn search_opens_the_way_to_a_match() {
        let nodes = flatten(&order());
        let closed = HashSet::from([0]);
        // With only the root open, the nested rows are hidden.
        assert_eq!(visible(&nodes, &closed, &HashSet::new()).len(), 5);
        let matches = matching(&nodes, "PLATE");
        assert_eq!(matches.len(), 1);
        let rows = visible(&nodes, &closed, &matches);
        let hit = *matches.iter().next().unwrap();
        assert!(rows.contains(&hit));
        // The sibling item stays closed.
        assert!(!rows.iter().any(|&ix| nodes[ix].path == "$.items[0].sku"));
        // Keys match too; branch summaries do not.
        assert_eq!(matching(&nodes, "qty").len(), 2);
        assert!(matching(&nodes, "{4}").is_empty());
        assert!(matching(&nodes, "  ").is_empty());
    }

    #[test]
    fn a_match_is_split_out_of_its_text() {
        assert_eq!(split_match("mug-blue", "BLU"), Some(("mug-", "blu", "e")));
        assert_eq!(split_match("mug", "x"), None);
        assert_eq!(split_match("mug", ""), None);
    }
}
