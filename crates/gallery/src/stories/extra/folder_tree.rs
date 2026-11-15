//! Gallery story for the folder tree.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::folder_tree::{FolderTree, TreeEvent, TreeNode};

/// A client document store. Click a row, then use the arrow keys; Enter or
/// a double-click opens a file.
pub struct FolderTreeStory {
    tree: Entity<FolderTree>,
    opened: Option<SharedString>,
    _events: Subscription,
}

fn documents() -> Vec<TreeNode> {
    vec![
        TreeNode::folder(
            "clients",
            "Clients",
            vec![
                TreeNode::folder(
                    "harbor",
                    "Harbor Dental",
                    vec![
                        TreeNode::file("harbor-engagement", "Engagement letter.pdf"),
                        TreeNode::folder(
                            "harbor-2026",
                            "2026",
                            vec![
                                TreeNode::file("harbor-q1", "Q1 close.xlsx"),
                                TreeNode::file("harbor-q2", "Q2 close.xlsx"),
                            ],
                        )
                        .count(2),
                    ],
                ),
                TreeNode::folder(
                    "brightline",
                    "Brightline Cafe",
                    vec![TreeNode::file("brightline-w9", "W-9.pdf")],
                )
                .count(1),
            ],
        ),
        TreeNode::folder(
            "templates",
            "Templates",
            vec![
                TreeNode::file("tpl-invoice", "Invoice.docx"),
                TreeNode::file("tpl-reminder", "Payment reminder.docx"),
            ],
        ),
        TreeNode::file("handbook", "Firm handbook.pdf"),
    ]
}

impl FolderTreeStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let tree = cx.new(|cx| {
                FolderTree::new("Client documents", documents(), cx).opened(["clients", "harbor"])
            });
            let events = cx.subscribe(&tree, |this: &mut Self, _, event: &TreeEvent, cx| {
                if let TreeEvent::Open(id) = event {
                    this.opened = Some(id.clone());
                    cx.notify();
                }
            });
            Self {
                tree,
                opened: None,
                _events: events,
            }
        })
    }
}

impl Story for FolderTreeStory {
    fn title() -> &'static str {
        "Folder tree"
    }

    fn description() -> &'static str {
        "Nested folders and files that open and close, with guide lines for depth, counts, and a full keyboard path."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for FolderTreeStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (line, quiet) = (theme.border, theme.muted_foreground);
        let picked = self.tree.read(cx).picked().cloned();
        h_flex()
            .items_start()
            .gap_6()
            .child(
                div()
                    .w(px(300.))
                    .p_1()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(line)
                    .child(self.tree.clone()),
            )
            .child(
                v_flex()
                    .gap_1()
                    .text_sm()
                    .child(format!(
                        "Picked: {}",
                        picked.as_deref().unwrap_or("nothing")
                    ))
                    .child(div().text_color(quiet).child(format!(
                        "Opened: {}",
                        self.opened.as_deref().unwrap_or("nothing yet")
                    ))),
            )
    }
}
