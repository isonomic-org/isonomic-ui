// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Folder tree: nested folders and files that open and close, with guide lines for depth, counts beside folders, and a full keyboard path.
//!
//! Up and Down move through what's showing. Right opens a folder, or steps
//! into it when it's open; Left closes it, or steps out to its parent.
//! Enter opens a file. Clicking a folder's chevron opens or closes it, and
//! clicking a row picks it. The tree reports what's picked and opened.

use std::collections::HashSet;

use isonomic_ui::component::TextRole as _;
use isonomic_ui::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use isonomic_ui::{
    App, Context, ElementId, EventEmitter, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

/// A folder or a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    /// Unique among all nodes.
    pub id: SharedString,
    /// What shows.
    pub label: SharedString,
    /// What's inside. A node with none is a file.
    pub children: Vec<TreeNode>,
    /// A count shown beside it, such as unread items.
    pub count: Option<usize>,
}

impl TreeNode {
    /// A file.
    pub fn file(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            count: None,
        }
    }

    /// A folder holding `children`.
    pub fn folder(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        children: Vec<TreeNode>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children,
            count: None,
        }
    }

    /// Show a count beside it.
    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    fn is_folder(&self) -> bool {
        !self.children.is_empty()
    }
}

/// One row that's showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    /// How deep it sits, from 0.
    pub depth: usize,
    /// The node's id.
    pub id: SharedString,
    /// The parent's id, if any.
    pub parent: Option<SharedString>,
    /// Whether it's a folder.
    pub folder: bool,
}

/// The rows showing when the folders in `open` are open, top to bottom.
pub fn visible_rows(nodes: &[TreeNode], open: &HashSet<SharedString>) -> Vec<TreeRow> {
    fn walk(
        nodes: &[TreeNode],
        open: &HashSet<SharedString>,
        depth: usize,
        parent: Option<&SharedString>,
        out: &mut Vec<TreeRow>,
    ) {
        for node in nodes {
            out.push(TreeRow {
                depth,
                id: node.id.clone(),
                parent: parent.cloned(),
                folder: node.is_folder(),
            });
            if node.is_folder() && open.contains(&node.id) {
                walk(&node.children, open, depth + 1, Some(&node.id), out);
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, open, 0, None, &mut out);
    out
}

fn find<'a>(nodes: &'a [TreeNode], id: &str) -> Option<&'a TreeNode> {
    nodes.iter().find_map(|node| {
        if node.id == id {
            Some(node)
        } else {
            find(&node.children, id)
        }
    })
}

/// What the tree reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeEvent {
    /// A row was picked.
    Pick(SharedString),
    /// A file was opened.
    Open(SharedString),
}

/// Nested folders and files.
pub struct FolderTree {
    nodes: Vec<TreeNode>,
    open: HashSet<SharedString>,
    picked: Option<SharedString>,
    label: SharedString,
    focus: FocusHandle,
}

impl EventEmitter<TreeEvent> for FolderTree {}

impl Focusable for FolderTree {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl FolderTree {
    /// Create a tree with nothing open.
    pub fn new(
        label: impl Into<SharedString>,
        nodes: Vec<TreeNode>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            nodes,
            open: HashSet::new(),
            picked: None,
            label: label.into(),
            focus: cx.focus_handle(),
        }
    }

    /// Open these folders to start.
    pub fn opened(mut self, ids: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.open.extend(ids.into_iter().map(Into::into));
        self
    }

    /// The picked row's id.
    pub fn picked(&self) -> Option<&SharedString> {
        self.picked.as_ref()
    }

    fn pick(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if self.picked.as_ref() != Some(&id) {
            self.picked = Some(id.clone());
            cx.emit(TreeEvent::Pick(id));
        }
        cx.notify();
    }

    fn toggle(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        if !self.open.remove(id) {
            self.open.insert(id.clone());
        }
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let rows = visible_rows(&self.nodes, &self.open);
        if rows.is_empty() {
            return;
        }
        let at = self
            .picked
            .as_ref()
            .and_then(|id| rows.iter().position(|r| &r.id == id));
        let row = at.map(|ix| rows[ix].clone());
        match event.keystroke.key.as_str() {
            "down" => {
                let next = at.map_or(0, |ix| (ix + 1).min(rows.len() - 1));
                self.pick(rows[next].id.clone(), cx);
            }
            "up" => {
                let next = at.map_or(0, |ix| ix.saturating_sub(1));
                self.pick(rows[next].id.clone(), cx);
            }
            "right" => {
                if let Some(row) = row.filter(|r| r.folder) {
                    if self.open.contains(&row.id) {
                        if let Some(child) = rows.get(at.unwrap_or(0) + 1) {
                            self.pick(child.id.clone(), cx);
                        }
                    } else {
                        self.toggle(&row.id, cx);
                    }
                }
            }
            "left" => match row {
                Some(row) if row.folder && self.open.contains(&row.id) => self.toggle(&row.id, cx),
                Some(TreeRow {
                    parent: Some(parent),
                    ..
                }) => self.pick(parent, cx),
                _ => {}
            },
            "enter" => match row {
                Some(row) if row.folder => self.toggle(&row.id, cx),
                Some(row) => cx.emit(TreeEvent::Open(row.id)),
                None => {}
            },
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for FolderTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let quiet = theme.muted_foreground;
        let line = theme.border;
        let picked_bg = theme.accent;
        let hover = theme.muted.opacity(0.6);
        let ring = theme.ring;
        let focused = self.focus.is_focused(window);
        let rows = visible_rows(&self.nodes, &self.open);

        let items = rows.into_iter().map(|row| {
            let node = find(&self.nodes, &row.id).cloned();
            let open = self.open.contains(&row.id);
            let picked = self.picked.as_ref() == Some(&row.id);
            let id = row.id.clone();
            let toggle_id = row.id.clone();
            let label = node.as_ref().map(|n| n.label.clone()).unwrap_or_default();
            let count = node.as_ref().and_then(|n| n.count);
            h_flex()
                .id(ElementId::Name(format!("tree-row-{}", row.id).into()))
                .h(px(26.))
                .pr_2()
                .rounded(px(4.))
                .text_body()
                .cursor_pointer()
                .when(picked, |this| this.bg(picked_bg))
                .when(picked && focused, |this| this.border_1().border_color(ring))
                .when(!picked, |this| this.hover(move |s| s.bg(hover)))
                .role(Role::TreeItem)
                .aria_level(row.depth + 1)
                .when(row.folder, |this| this.aria_expanded(open))
                .aria_label(match (row.folder, open) {
                    (true, true) => format!("{label}, folder, open"),
                    (true, false) => format!("{label}, folder, closed"),
                    _ => label.to_string(),
                })
                .aria_selected(picked)
                .on_click(
                    cx.listener(move |this, event: &isonomic_ui::ClickEvent, window, cx| {
                        window.focus(&this.focus, cx);
                        this.pick(id.clone(), cx);
                        if event.click_count() >= 2 {
                            let is_folder = find(&this.nodes, &id).is_some_and(TreeNode::is_folder);
                            if is_folder {
                                this.toggle(&id, cx);
                            } else {
                                cx.emit(TreeEvent::Open(id.clone()));
                            }
                        }
                    }),
                )
                .children((0..row.depth).map(|_| {
                    div()
                        .w(px(16.))
                        .h_full()
                        .flex()
                        .justify_center()
                        .child(div().w(px(1.)).h_full().bg(line))
                }))
                .child(
                    div()
                        .id(ElementId::Name(format!("tree-chevron-{}", row.id).into()))
                        .w(px(16.))
                        .flex()
                        .justify_center()
                        .when(row.folder, |this| {
                            this.child(
                                Icon::new(if open {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .xsmall()
                                .text_color(quiet),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.toggle(&toggle_id, cx);
                                },
                            ))
                        }),
                )
                .child(
                    Icon::new(if row.folder {
                        IconName::Folder
                    } else {
                        IconName::File
                    })
                    .small()
                    .text_color(quiet)
                    .mr_1p5(),
                )
                .child(div().flex_1().min_w_0().truncate().child(label))
                .when_some(count, |this, count| {
                    this.child(
                        div()
                            .text_caption()
                            .text_color(quiet)
                            .child(count.to_string()),
                    )
                })
        });

        v_flex()
            .id("folder-tree")
            .track_focus(&self.focus)
            .gap_px()
            .role(Role::Tree)
            .aria_label(self.label.clone())
            .on_key_down(cx.listener(Self::key_down))
            .children(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<TreeNode> {
        vec![
            TreeNode::folder(
                "clients",
                "Clients",
                vec![
                    TreeNode::folder(
                        "harbor",
                        "Harbor Dental",
                        vec![TreeNode::file("harbor-q3", "Q3 report.pdf")],
                    ),
                    TreeNode::file("brightline", "Brightline notes.txt"),
                ],
            ),
            TreeNode::file("readme", "Read me.txt"),
        ]
    }

    #[test]
    fn rows_follow_open_folders() {
        let nodes = sample();
        let closed = visible_rows(&nodes, &HashSet::new());
        assert_eq!(
            closed.iter().map(|r| r.id.as_ref()).collect::<Vec<_>>(),
            ["clients", "readme"]
        );
        let open: HashSet<SharedString> = ["clients".into(), "harbor".into()].into_iter().collect();
        let rows = visible_rows(&nodes, &open);
        assert_eq!(
            rows.iter().map(|r| r.id.as_ref()).collect::<Vec<_>>(),
            ["clients", "harbor", "harbor-q3", "brightline", "readme"]
        );
        assert_eq!(rows[2].depth, 2);
        assert_eq!(rows[2].parent.as_deref(), Some("harbor"));
        assert!(rows[1].folder && !rows[2].folder);
        assert_eq!(
            find(&nodes, "harbor-q3").map(|n| n.label.as_ref()),
            Some("Q3 report.pdf")
        );
        assert_eq!(TreeNode::file("a", "A").count(3).count, Some(3));
    }
}
