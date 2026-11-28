// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the comment thread.

use std::time::{Duration, SystemTime};

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window, div,
};
use isonomic_ui_extra::comment_thread::{
    CommentThread, CommentThreadEvent, ThreadAuthor, ThreadMessage,
};

/// Shows a thread where a reply that mentions the assistant gets an answer after a moment.
pub struct CommentThreadStory {
    thread: Entity<CommentThread>,
    closed: bool,
    _events: Subscription,
}

impl CommentThreadStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let me = ThreadAuthor::new("Imani Okafor");
            let helper = ThreadAuthor::assistant("Ledger");
            let earlier = |minutes: u64| SystemTime::now() - Duration::from_secs(minutes * 60);

            let thread = cx.new(|cx| {
                let mut thread = CommentThread::new(me.clone(), window, cx)
                    .title("Row 14")
                    .closable(true)
                    .messages([
                        ThreadMessage::new(
                            "ask",
                            me,
                            "/fix the VAT rate on the March invoices is 19, not 7.",
                        )
                        .created_at(earlier(4)),
                        ThreadMessage::new("done", helper.clone(), "")
                            .created_at(earlier(2))
                            .status("Rates corrected")
                            .details([
                                "Changed 12 invoices from 7% to 19%",
                                "Recalculated the March totals",
                                "Left 2 paid invoices untouched",
                            ]),
                    ]);
                thread.set_assistant(helper, window, cx);
                thread
            });

            let events = cx.subscribe(&thread, |this: &mut Self, thread, event, cx| match event {
                CommentThreadEvent::Close => {
                    this.closed = true;
                    cx.notify();
                }
                CommentThreadEvent::Reply(text) if text.contains("@Ledger") => {
                    thread.update(cx, |thread, cx| thread.set_working(true, cx));
                    cx.spawn(async move |_, cx| {
                        cx.background_executor().timer(Duration::from_secs(2)).await;
                        thread.update(cx, |thread, cx| {
                            thread.set_working(false, cx);
                            let answer = ThreadMessage::new(
                                "answer",
                                ThreadAuthor::assistant("Ledger"),
                                "Done. Nothing else in March uses the old rate.",
                            );
                            thread.push(answer, cx);
                        });
                    })
                    .detach();
                }
                CommentThreadEvent::Reply(_) => {}
            });

            Self {
                thread,
                closed: false,
                _events: events,
            }
        })
    }
}

impl Story for CommentThreadStory {
    fn title() -> &'static str {
        "Comment thread"
    }

    fn description() -> &'static str {
        "One conversation attached to a selection, a cell or a line."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for CommentThreadStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.thread.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(if self.closed {
                    "Close was pressed"
                } else {
                    "Mention @Ledger in a reply to get an answer"
                }),
        )
    }
}
