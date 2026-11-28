// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gallery story for the approval card.

use std::time::Duration;

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, IconName, button::Button, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div,
};
use isonomic_ui_extra::approval_card::{ApprovalCard, ApprovalRisk, DenyReason};

/// Shows a request that expires on its own, and one that cannot be undone.
pub struct ApprovalCardStory {
    /// Changing this gives the cards new ids, which asks both again.
    round: usize,
    last: SharedString,
}

impl ApprovalCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            round: 0,
            last: "The first request expires after 45 seconds".into(),
        })
    }

    fn report(&mut self, line: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.last = line.into();
        cx.notify();
    }
}

impl Story for ApprovalCardStory {
    fn title() -> &'static str {
        "Approval card"
    }

    fn description() -> &'static str {
        "An assistant asks before it does something hard to undo."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for ApprovalCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let story = cx.entity();
        let approved = story.clone();
        let denied = story.clone();
        let voided = story.clone();

        v_flex()
            .gap_4()
            .child(
                ApprovalCard::new(("reminders", self.round), "Send payment reminders")
                    .description("Overdue invoices, second reminder")
                    .icon(IconName::Inbox)
                    .risk(ApprovalRisk::Medium)
                    .detail("To", "14 customers")
                    .detail("Total", "18,420.00 outstanding")
                    .detail("Send", "Today, 09:00")
                    .expires_in(Duration::from_secs(45))
                    .on_approve(move |always, _, cx| {
                        let line = if always {
                            "Reminders approved, and allowed from now on"
                        } else {
                            "Reminders approved"
                        };
                        approved.update(cx, |this, cx| this.report(line, cx))
                    })
                    .on_deny(move |reason, _, cx| {
                        let line = match reason {
                            DenyReason::Declined => "Reminders denied",
                            DenyReason::Expired => "Reminders expired without an answer",
                        };
                        denied.update(cx, |this, cx| this.report(line, cx))
                    }),
            )
            .child(
                ApprovalCard::new(("void", self.round), "Void invoice 2291")
                    .description("Harbour Supply, 1,240.50")
                    .icon(IconName::Delete)
                    .risk(ApprovalRisk::High)
                    .allow_always(false)
                    .approve_label("Void it")
                    .on_approve(move |_, _, cx| {
                        voided.update(cx, |this, cx| this.report("Invoice voided", cx))
                    }),
            )
            .child(
                div().child(
                    Button::new("again")
                        .outline()
                        .label("Ask both again")
                        .on_click(move |_, _, cx| {
                            story.update(cx, |this, cx| {
                                this.round += 1;
                                this.report("Asked again", cx);
                            })
                        }),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.last.clone()),
            )
    }
}
