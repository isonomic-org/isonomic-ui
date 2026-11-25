// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the sent message.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::sent_message::{
    Delivery, SentAttachment, SentMessage, SentMessageData, SentMessageEvent,
};

/// How long the made-up network takes to deliver.
const DELIVERY: Duration = Duration::from_millis(900);

/// Messages to an allotment group: sent with files, edited, on its way, and failed.
pub struct SentMessageStory {
    messages: Vec<Entity<SentMessage>>,
    last: SharedString,
    _events: Vec<Subscription>,
}

impl SentMessageStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let data = vec![
                SentMessageData::new(
                    "Plot 14 is cleared and the beds are marked out.\nPhotos and the planting \
                     plan are attached.",
                )
                .attachments(vec![
                    SentAttachment::image("beds", "raised-beds.jpg").size(1_843_200),
                    SentAttachment::file("plan", "planting-plan-spring.pdf").size(245_760),
                    SentAttachment::file("notes", "soil-notes.txt").size(812),
                ])
                .sent_at("14:02"),
                SentMessageData::new("Water is back on at the east tap from Friday.")
                    .sent_at("14:10")
                    .edited(true),
                SentMessageData::new("Can someone lend me a wheelbarrow on Saturday morning?")
                    .status(Delivery::Sending),
                SentMessageData::new("The shed key is back on its hook.")
                    .status(Delivery::Failed),
            ];
            let mut messages = Vec::new();
            let mut events = Vec::new();
            for (ix, data) in data.into_iter().enumerate() {
                let message = cx.new(|cx| SentMessage::new(("sent", ix), data, window, cx));
                events.push(cx.subscribe(&message, Self::on_event));
                messages.push(message);
            }
            Self {
                messages,
                last: "Nothing happened yet".into(),
                _events: events,
            }
        })
    }

    fn on_event(
        &mut self,
        message: Entity<SentMessage>,
        event: &SentMessageEvent,
        cx: &mut Context<Self>,
    ) {
        let (note, resend) = match event {
            SentMessageEvent::Edited(text) => (format!("Edited: {text}"), Some(text.clone())),
            SentMessageEvent::Retry => ("Trying again".to_string(), None),
            SentMessageEvent::Copied(text) => {
                self.last = format!("Copied {} characters", text.chars().count()).into();
                cx.notify();
                return;
            }
        };
        self.last = note.into();
        message.update(cx, |message, cx| {
            let mut data = message.data().clone();
            if let Some(text) = resend {
                data.content = text;
                data.edited = true;
            }
            data.status = Delivery::Sending;
            message.set_data(data, cx);
        });
        let message = message.downgrade();
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(DELIVERY).await;
            let _ = message.update(cx, |message, cx| {
                let mut data = message.data().clone();
                data.status = Delivery::Sent;
                data.sent_at = Some("14:31".into());
                message.set_data(data, cx);
            });
        })
        .detach();
        cx.notify();
    }
}

impl Story for SentMessageStory {
    fn title() -> &'static str {
        "Sent message"
    }

    fn description() -> &'static str {
        "The person's own message: attachments, edit in place, copy, and whether it is sending, sent or failed."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for SentMessageStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .max_w(px(576.))
            .gap_6()
            .children(self.messages.clone())
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
