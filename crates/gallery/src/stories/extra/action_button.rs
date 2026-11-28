// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the action button.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::action_button::{ActionButton, ActionState};

/// Three actions the story plays: sending reminders works then finishes,
/// posting fails the first time, and voiding asks twice.
pub struct ActionButtonStory {
    send: ActionState,
    post: ActionState,
    void: ActionState,
    post_tries: u32,
}

impl ActionButtonStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            send: ActionState::Idle,
            post: ActionState::Idle,
            void: ActionState::Idle,
            post_tries: 0,
        })
    }

    fn settle(&mut self, which: &'static str, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            this.update(cx, |this, cx| {
                match which {
                    "send" => this.send = ActionState::Done,
                    "post" => {
                        this.post_tries += 1;
                        this.post = if this.post_tries == 1 {
                            ActionState::Failed
                        } else {
                            ActionState::Done
                        };
                    }
                    _ => this.void = ActionState::Done,
                }
                cx.notify();
            })
            .ok();
            cx.background_executor()
                .timer(Duration::from_millis(1800))
                .await;
            this.update(cx, |this, cx| {
                for state in [&mut this.send, &mut this.post, &mut this.void] {
                    if *state == ActionState::Done {
                        *state = ActionState::Idle;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Story for ActionButtonStory {
    fn title() -> &'static str {
        "Action button"
    }

    fn description() -> &'static str {
        "The kit's button with the states an action goes through: working, done, failed, and a second click to confirm."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ActionButtonStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (a, b, c) = (cx.entity(), cx.entity(), cx.entity());
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        ActionButton::new("send", "Send 7 reminders")
                            .icon(isonomic_ui::component::IconName::ArrowUp)
                            .words("Sending\u{2026}", "Sent", "Couldn't send, try again")
                            .state(self.send)
                            .on_click(move |next, _, cx| {
                                a.update(cx, |this, cx| {
                                    this.send = next;
                                    this.settle("send", cx);
                                    cx.notify();
                                })
                            }),
                    )
                    .child(
                        ActionButton::new("post", "Post 201 matches")
                            .secondary()
                            .words("Posting\u{2026}", "Posted", "March is locked, try again")
                            .state(self.post)
                            .on_click(move |next, _, cx| {
                                b.update(cx, |this, cx| {
                                    this.post = next;
                                    this.settle("post", cx);
                                    cx.notify();
                                })
                            }),
                    )
                    .child(
                        ActionButton::new("void", "Void invoice 2240")
                            .destructive("Click again to void it")
                            .words("Voiding\u{2026}", "Voided", "Couldn't void, try again")
                            .state(self.void)
                            .on_click(move |next, _, cx| {
                                c.update(cx, |this, cx| {
                                    this.void = next;
                                    if next == ActionState::Working {
                                        this.settle("void", cx);
                                    }
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Posting fails the first time, then works."),
            )
    }
}
