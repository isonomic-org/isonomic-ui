// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the draft review card.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div,
};
use isonomic_ui_extra::draft_review_card::{
    DraftContact, DraftReviewCard, DraftReviewEvent, DraftTone,
};

/// Shows a payment reminder waiting for review, and reports what was decided.
pub struct DraftReviewCardStory {
    card: Entity<DraftReviewCard>,
    last: SharedString,
    _events: Subscription,
}

impl DraftReviewCardStory {
    /// Create the story view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let card = cx.new(|cx| {
                DraftReviewCard::new(window, cx)
                    .reference("INV-2291")
                    .status("14 days overdue")
                    .account("Harbour Supply")
                    .amount("1,240.50")
                    .author("Collections assistant")
                    .contact(DraftContact::new("Dana Whitlock", "dana@harbour.example"))
                    .contact(DraftContact::new(
                        "Accounts team",
                        "accounts@harbour.example",
                    ))
                    .attachment("Invoice-2291.pdf")
                    .link("Pay online", "https://pay.books.example/inv/2291")
                    .tone(DraftTone::new("Warm", |to| {
                        format!(
                            "Hi {}, a quick note that invoice 2291 for 1,240.50 fell due two \
                             weeks ago. If it is already on its way, thank you and please ignore \
                             this. Otherwise you can settle it here:",
                            to.first_name()
                        )
                    }))
                    .tone(DraftTone::new("Brief", |to| {
                        format!(
                            "Hi {}, invoice 2291 (1,240.50) is 14 days overdue. Payment link:",
                            to.first_name()
                        )
                    }))
                    .tone(DraftTone::new("Formal", |to| {
                        format!(
                            "Dear {}, our records show invoice 2291 for 1,240.50 as unpaid \
                             14 days after its due date. Please arrange payment through the \
                             link below:",
                            to.name
                        )
                    }))
            });
            let events = cx.subscribe(&card, |this: &mut Self, _, event, cx| {
                this.last = match event {
                    DraftReviewEvent::Send(draft) => {
                        format!(
                            "Sent to {} ({} characters)",
                            draft.to.address,
                            draft.body.len()
                        )
                    }
                    DraftReviewEvent::Discard => "Discarded".to_string(),
                    DraftReviewEvent::Undo => "Undone, the draft is open again".to_string(),
                }
                .into();
                cx.notify();
            });

            Self {
                card,
                last: "Nothing decided yet".into(),
                _events: events,
            }
        })
    }
}

impl Story for DraftReviewCardStory {
    fn title() -> &'static str {
        "Draft review card"
    }

    fn description() -> &'static str {
        "A message an assistant wrote, waiting for a person."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for DraftReviewCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.card.clone()).child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.last.clone()),
        )
    }
}
