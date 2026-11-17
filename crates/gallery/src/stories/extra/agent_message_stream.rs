// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the agent message stream.

use std::time::Duration;

use crate::Story;
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window, div,
};
use isonomic_ui_extra::agent_message_stream::{
    AgentMessageStream, MessageStreamEvent, Participant, ParticipantKind, StreamMessage,
};

/// A close team talking while it works. The story plays the session: a
/// note you send is answered by whoever you mention.
pub struct AgentMessageStreamStory {
    stream: Entity<AgentMessageStream>,
    sent: usize,
    _events: Subscription,
}

fn people() -> Vec<Participant> {
    use ParticipantKind::*;
    vec![
        Participant::new("lead", "Coordinator", Agent),
        Participant::new("recon", "Reconciler", Agent),
        Participant::new("cards", "CardMatcher", Agent),
        Participant::new("payables", "Payables", Agent),
        Participant::new("maya", "Maya", Person),
    ]
}

fn history() -> Vec<StreamMessage> {
    vec![
        StreamMessage::status("s1", "Run started at 1:02 pm"),
        StreamMessage::new(
            "m1",
            "lead",
            "@Reconciler please match March, cards first, then the operating account.",
        )
        .to("recon")
        .when("1:02 pm"),
        StreamMessage::new(
            "m2",
            "recon",
            "On it. Handing the card payouts to @CardMatcher.",
        )
        .to("lead")
        .when("1:03 pm"),
        StreamMessage::new(
            "m3",
            "cards",
            "201 of 214 matched. Posting failed: the March period is locked.",
        )
        .to("recon")
        .when("1:55 pm"),
        StreamMessage::new(
            "m4",
            "recon",
            "@Coordinator we need the ledger unlocked, or I post to April.",
        )
        .to("lead")
        .when("1:56 pm"),
        StreamMessage::new(
            "m5",
            "lead",
            "@Maya can you unlock March, or should we post to April?",
        )
        .to("maya")
        .when("1:57 pm"),
        StreamMessage::status("s2", "Payables is waiting for an approval"),
    ]
}

impl AgentMessageStreamStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let stream = cx.new(|cx| {
                let mut stream = AgentMessageStream::new(people(), window, cx);
                stream.set_messages(history(), cx);
                stream
            });
            let events = cx.subscribe(&stream, |this: &mut Self, stream, event, cx| {
                if let MessageStreamEvent::Send { text, mentions } = event {
                    this.sent += 1;
                    let n = this.sent;
                    stream.update(cx, |stream, cx| {
                        stream.push(
                            StreamMessage::new(format!("you-{n}"), "maya", text.clone())
                                .when("now"),
                            cx,
                        );
                    });
                    let reply_from = mentions.first().cloned().unwrap_or_else(|| "lead".into());
                    let stream = stream.downgrade();
                    cx.spawn(async move |_, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(900))
                            .await;
                        stream
                            .update(cx, |stream, cx| {
                                stream.push(
                                    StreamMessage::new(
                                        format!("reply-{n}"),
                                        reply_from,
                                        "Got it, I'll take that into the next step.",
                                    )
                                    .to("maya")
                                    .when("now"),
                                    cx,
                                );
                            })
                            .ok();
                    })
                    .detach();
                }
            });
            Self {
                stream,
                sent: 0,
                _events: events,
            }
        })
    }
}

impl Story for AgentMessageStreamStory {
    fn title() -> &'static str {
        "Agent message stream"
    }

    fn description() -> &'static str {
        "A live log of what assistants say to each other, with a composer that can mention them."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for AgentMessageStreamStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w_full().child(self.stream.clone())
    }
}
