// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Gallery story for the entity preview card.

use crate::Story;
use isonomic_ui::component::{ActiveTheme as _, h_flex, v_flex};
use isonomic_ui::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    Window, div,
};
use isonomic_ui_extra::common::Tone;
use isonomic_ui_extra::entity_preview_card::{EntityKind, EntityPreview, EntityPreviewCard};

/// An assistant's summary with names that preview on hover. The story says
/// which was opened.
pub struct EntityPreviewCardStory {
    opened: Option<&'static str>,
}

impl EntityPreviewCardStory {
    /// Create the story view.
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self { opened: None })
    }
}

impl Story for EntityPreviewCardStory {
    fn title() -> &'static str {
        "Entity preview card"
    }

    fn description() -> &'static str {
        "A name in running text that previews what it stands for on hover: a customer, an invoice, an assistant."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl Render for EntityPreviewCardStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open = |what: &'static str, cx: &mut Context<Self>| {
            let story = cx.entity();
            move |_: &mut Window, cx: &mut App| {
                story.update(cx, |this, cx| {
                    this.opened = Some(what);
                    cx.notify();
                })
            }
        };
        let customer = EntityPreviewCard::new(
            "customer",
            "Northwind Dental",
            EntityPreview::new(
                EntityKind::Company,
                "Northwind Dental",
                "Customer since 2019",
            )
            .status("Overdue", Tone::Warning)
            .fact("Owes", "$3,120.00 over 2 invoices")
            .fact("Usually pays", "12 days late")
            .fact("Contact", "Dana Whitlock"),
        )
        .on_open(open("Northwind Dental", cx));
        let invoice = EntityPreviewCard::new(
            "invoice",
            "invoice 2291",
            EntityPreview::new(EntityKind::Document, "Invoice 2291", "Sent 2 March")
                .status("41 days late", Tone::Danger)
                .fact("Amount", "$3,120.00")
                .fact("Reminders", "2 sent, last on 4 April"),
        )
        .on_open(open("Invoice 2291", cx));
        let agent = EntityPreviewCard::new(
            "agent",
            "Collections",
            EntityPreview::new(EntityKind::Agent, "Collections", "Chases overdue invoices")
                .status("Working", Tone::Info)
                .fact("Today", "5 reminders sent")
                .fact("Spent", "$0.12"),
        );
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .max_w(isonomic_ui::px(520.))
                    .child(customer)
                    .child("still owes on")
                    .child(invoice)
                    .child(". I've asked")
                    .child(agent)
                    .child("to send a firmer reminder tomorrow."),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(match self.opened {
                        Some(what) => format!("Opened {what}"),
                        None => "Hover the underlined names.".into(),
                    }),
            )
    }
}
