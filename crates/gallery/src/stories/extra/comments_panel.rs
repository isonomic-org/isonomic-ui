// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the comments panel.

use std::time::{Duration, Instant};

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Task, Window, div,
};
use isonomic_ui_extra::comments_panel::{
    CommentAuthor, CommentsPanel, CommentsPanelEvent, PanelComment,
};

/// How long the pretend assistant takes to apply an edit.
const EDIT_FOR: Duration = Duration::from_millis(1800);

/// Comments on a draft board report. Run edit pretends to apply the edit and
/// reports back a moment later.
pub struct CommentsPanelStory {
    panel: Entity<CommentsPanel>,
    last: SharedString,
    _edit: Option<Task<()>>,
    _events: Subscription,
}

impl CommentsPanelStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let now = Instant::now();
            let ago = |mins: u64| {
                now.checked_sub(Duration::from_secs(mins * 60))
                    .unwrap_or(now)
            };
            let panel = cx.new(|cx| {
                CommentsPanel::new(CommentAuthor::new("Robin Ash"), window, cx)
                    .title("Comments on the March report")
                    .closable(true)
                    .comment(PanelComment::new(
                        "c1",
                        CommentAuthor::new("Mara Lind"),
                        ago(3),
                        "/edit Show every amount in thousands, with one decimal.",
                    ))
                    .comment(PanelComment::new(
                        "c2",
                        CommentAuthor::new("Jonah Webb"),
                        ago(7),
                        "Should the one-off grant sit outside operating income?",
                    ))
                    .comment(PanelComment::new(
                        "c3",
                        CommentAuthor::new("Lena Hart"),
                        ago(14),
                        "The cash bridge reads much better now.",
                    ))
                    .comment(PanelComment::new(
                        "c4",
                        CommentAuthor::new("Theo Brandt"),
                        ago(22),
                        "/edit Move the payroll note under staff costs.",
                    ))
                    .comment(
                        PanelComment::new(
                            "c5",
                            CommentAuthor::new("Sofia Marin"),
                            ago(40),
                            "Can this go to the board on Thursday?",
                        )
                        .resolved(true),
                    )
            });
            let events = cx.subscribe(&panel, |this: &mut Self, panel, event, cx| {
                this.last = match event {
                    CommentsPanelEvent::Added(comment) => format!("Posted: {}", comment.text()),
                    CommentsPanelEvent::Resolved(id, true) => format!("Resolved {id}"),
                    CommentsPanelEvent::Resolved(id, false) => format!("Opened {id} again"),
                    CommentsPanelEvent::Close => "Close pressed".to_string(),
                    CommentsPanelEvent::RunEdit(comment) => {
                        let id = comment.id().clone();
                        this._edit = Some(cx.spawn(async move |_, cx| {
                            cx.background_executor().timer(EDIT_FOR).await;
                            panel.update(cx, |panel, cx| panel.finish_edit(&id, cx));
                        }));
                        format!("Running: {}", comment.edit_request().unwrap_or_default())
                    }
                }
                .into();
                cx.notify();
            });
            Self {
                panel,
                last: "Nothing yet".into(),
                _edit: None,
                _events: events,
            }
        })
    }
}

impl Story for CommentsPanelStory {
    fn title() -> &'static str {
        "Comments panel"
    }

    fn description() -> &'static str {
        "A sidebar feed of comments on one piece of work."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CommentsPanelStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.panel.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
