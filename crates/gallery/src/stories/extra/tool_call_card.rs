// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the tool call card.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Task, Window, div, px,
};
use isonomic_ui_extra::tool_call_card::{ToolCallCard, ToolCallGroup, ToolCallStatus};

/// Shows a group whose last call runs, fails, and succeeds when retried.
pub struct ToolCallCardStory {
    last: ToolCallStatus,
    _run: Task<()>,
}

impl ToolCallCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            last: ToolCallStatus::Running,
            _run: Self::finish_later(ToolCallStatus::Failed, cx),
        })
    }

    /// Let the running call end this way after a few seconds.
    fn finish_later(outcome: ToolCallStatus, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            this.update(cx, |this, cx| {
                this.last = outcome;
                cx.notify();
            })
            .ok();
        })
    }
}

impl Story for ToolCallCardStory {
    fn title() -> &'static str {
        "Tool call card"
    }

    fn description() -> &'static str {
        "One tool call of an assistant on a single line."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ToolCallCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let posting = match self.last {
            ToolCallStatus::Running => "Posting the journal entry",
            ToolCallStatus::Success => "Posted the journal entry",
            ToolCallStatus::Failed => "Could not post the journal entry",
        };

        v_flex()
            .gap_3()
            .max_w(px(520.))
            .child(
                ToolCallGroup::new("turn")
                    .default_open(true)
                    .call(
                        ToolCallCard::new("search", "ledger.search", ToolCallStatus::Success)
                            .icon(IconName::Search)
                            .summary("Searched March for unmatched payments")
                            .arguments(r#"{"period":"2026-03","status":"unmatched","limit":50}"#)
                            .result(
                                r#"{"count":3,"entries":[{"id":"pay_8841","amount":1240.5,"payee":"Harbour Supply"},{"id":"pay_8857","amount":310,"payee":"Northgate Print"},{"id":"pay_8902","amount":87.25,"payee":null}]}"#,
                            )
                            .duration(Duration::from_millis(1420)),
                    )
                    .call(
                        ToolCallCard::new("rates", "fx.rate", ToolCallStatus::Success)
                            .icon(IconName::Globe)
                            .summary("Looked up the rate for 31 March")
                            .arguments(r#"{"from":"USD","to":"EUR","on":"2026-03-31"}"#)
                            .result("0.9214")
                            .duration(Duration::from_millis(380)),
                    )
                    .call(
                        ToolCallCard::new("post", "journal.post", self.last)
                            .icon(IconName::FileText)
                            .summary(posting)
                            .arguments(r#"{"entry":"accrual","amount":1637.75,"period":"2026-03"}"#)
                            .when_result(self.last)
                            .error("The period is locked. Reopen March or post to April.")
                            .on_retry(move |_, cx| {
                                story.update(cx, |this, cx| {
                                    this.last = ToolCallStatus::Running;
                                    this._run = Self::finish_later(ToolCallStatus::Success, cx);
                                    cx.notify();
                                })
                            }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("The last call fails after four seconds. Retry lets it through."),
            )
    }
}

/// Adds the result only once the call has returned one.
trait WhenResult {
    fn when_result(self, status: ToolCallStatus) -> Self;
}

impl WhenResult for ToolCallCard {
    fn when_result(self, status: ToolCallStatus) -> Self {
        match status {
            ToolCallStatus::Success => self.result(r#"{"posted":true,"entry_id":"je_2291"}"#),
            ToolCallStatus::Running | ToolCallStatus::Failed => self,
        }
    }
}
