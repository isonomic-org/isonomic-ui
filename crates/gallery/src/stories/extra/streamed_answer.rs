// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the streamed answer.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, px,
};
use isonomic_ui_extra::streamed_answer::{AnswerSource, StreamedAnswer, StreamedAnswerEvent};

const ANSWER: &str =
    "**Software subscriptions** rose 18% in September, mostly from two changes [1,6].

- **New seats:** the design team added four seats to its drawing tool on 3 September [2].
- **Annual renewal:** the file storage plan renewed for the year and was booked in one month [3,5].
- **Card fees:** foreign card fees on two invoices added a little more [4].

Spread the renewal over twelve months to keep the monthly figures comparable [5].";

/// Plays what a session would do: no words while the sources are read, then
/// the whole answer at once, streamed in at the answer's own pace.
fn ask(answer: &Entity<StreamedAnswer>, cx: &mut App) {
    answer.update(cx, |answer, cx| {
        answer.set_streaming(true, cx);
        answer.set_answer("", cx);
        answer.start(cx);
    });
    let answer = answer.downgrade();
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(700))
            .await;
        answer
            .update(cx, |answer, cx| {
                answer.set_answer(ANSWER, cx);
                answer.set_streaming(false, cx);
            })
            .ok();
    })
    .detach();
}

/// A bookkeeping question answered from six made-up sources.
pub struct StreamedAnswerStory {
    answer: Entity<StreamedAnswer>,
    last: SharedString,
    _events: Subscription,
}

impl StreamedAnswerStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let answer = cx.new(|cx| {
                StreamedAnswer::new("Why did software costs jump in September?", cx)
                    .framed(true)
                    .source(
                        AnswerSource::new("September ledger, software accounts", "Ledger")
                            .domain("ledgerline.example")
                            .icon(IconName::FileText)
                            .snippet("6120 Software subscriptions: 4,310.00, up from 3,650.00."),
                    )
                    .source(
                        AnswerSource::new("#finance: new seats for the design team", "Teamchat")
                            .domain("teamchat.example")
                            .snippet("Approved four more seats from the 3rd, billed monthly."),
                    )
                    .source(
                        AnswerSource::new("Invoice 0912 from the storage provider", "Invoice")
                            .domain("docuvault.example")
                            .icon(IconName::File)
                            .snippet("Annual plan, 12 months, paid in full."),
                    )
                    .source(
                        AnswerSource::new("Card statement for September", "Bankfeed")
                            .domain("bankfeed.example")
                            .icon(IconName::Building2),
                    )
                    .source(
                        AnswerSource::new("Renewal notice for the storage plan", "Notice")
                            .domain("docuvault.example"),
                    )
                    .source(
                        AnswerSource::new("August ledger, software accounts", "Ledger")
                            .domain("ledgerline.example")
                            .icon(IconName::FileText),
                    )
                    .follow_up("Spread the renewal over twelve months")
                    .follow_up("Compare software costs with last year")
            });
            ask(&answer, cx);
            let events = cx.subscribe(&answer, |this: &mut Self, answer, event, cx| {
                if let StreamedAnswerEvent::Regenerate = event {
                    ask(&answer, cx);
                }
                this.last = match event {
                    StreamedAnswerEvent::FollowUp(question) => format!("Asked: {question}"),
                    StreamedAnswerEvent::Regenerate => "Regenerating".to_string(),
                    StreamedAnswerEvent::Stop => "Stopped".to_string(),
                    StreamedAnswerEvent::Feedback { up: true, .. } => "Thumbs up".to_string(),
                    StreamedAnswerEvent::Feedback { reason, .. } => {
                        format!("Thumbs down: {}", reason.clone().unwrap_or_default())
                    }
                    StreamedAnswerEvent::Copied(text) => {
                        format!("Copied {} characters", text.chars().count())
                    }
                    StreamedAnswerEvent::OpenSource(ix) => format!("Open source {}", ix + 1),
                }
                .into();
                cx.notify();
            });
            Self {
                answer,
                last: "Nothing yet".into(),
                _events: events,
            }
        })
    }
}

impl Story for StreamedAnswerStory {
    fn title() -> &'static str {
        "Streamed answer"
    }

    fn description() -> &'static str {
        "An assistant's reply with the sources it used."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for StreamedAnswerStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .max_w(px(640.))
            .child(self.answer.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
